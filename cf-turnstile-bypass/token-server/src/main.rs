// Token server to receive solve requests and route tokens.

use futures_util::{SinkExt, StreamExt};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{accept_async, tungstenite::Message};

const PORT: u16 = 8080;

type Tx = mpsc::UnboundedSender<Message>;

#[derive(Debug, Default)]
struct State {
    // Active connections.
    connections: HashMap<u32, Tx>,
    // Available solvers (not currently solving). 
    // Note this isn't actually a generic queue structure, 
    // there is no specific ordered pick from the HashSet.
    // This doesn't matter for our case as we just want any available solver.
    available_solvers_queue: HashMap<String, HashSet<u32>>,
    // Solver socket ids to user-agent HashMap. 
    // This allows us to not have to send user-agent data in
    // Solver result packets, as we can just lookup its user-agent in the map
    // to re-add it to the available_solvers queue in the right user-agent bucket.
    solver_to_ua: HashMap<u32, String>,
}

static NEXT_ID: AtomicU32 = AtomicU32::new(0);

#[tokio::main]
async fn main() {
    let addr = format!("0.0.0.0:{}", PORT);
    let listener = TcpListener::bind(&addr).await.expect("Failed to bind");
    println!("WebSocket server listening on ws://localhost:{}", PORT);

    let state: Arc<Mutex<State>> = Arc::new(Mutex::new(State::default()));

    while let Ok((stream, _)) = listener.accept().await {
        let state = Arc::clone(&state);
        tokio::spawn(handle_connection(stream, state));
    }
}

async fn handle_connection(stream: TcpStream, state: Arc<Mutex<State>>) {
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("[-] WebSocket handshake failed: {}", e);
            return;
        }
    };

    // Assign socket id and push socket data to the sockets HashMap.
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    
    let (mut ws_tx, mut ws_rx) = ws_stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    {
        let mut s = state.lock().await;
        s.connections.insert(id, tx.clone());
    }

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_tx.send(msg).await.is_err() { break };
        }
    });

    while let Some(result) = ws_rx.next().await {
        let raw = match result {
            Ok(Message::Binary(data)) => data,
            Ok(Message::Close(_)) | Err(_) => break,
            _ => continue,
        };

        let header = raw[0];

        match header {
            // Token result from solver. This token is received, 
            // and forwarded to the requester (receiver) with the associated requester_id.
            // [0, ...requester_id_bytes (u32), proxy_url_len (u8), ...proxy_url_bytes, ...token_bytes]
            // If the solve failed, there will be no token bytes in this packet.
            0 => {
                let mut requester_id_bytes = [0u8; 4];
                requester_id_bytes.copy_from_slice(&raw[1..5]);
                let requester_id = u32::from_le_bytes(requester_id_bytes);

                let mut s = state.lock().await;

                // Route the token back to the specific requester who asked for it by looking up its requester id.
                if let Some(requester_tx) = s.connections.get(&requester_id) {
                    // [0, proxy_url_len (u8), ...proxy_url_bytes, ...token_bytes]
                    // If the solve failed, there will be no token bytes.
                    let mut token_packet = vec![0u8];
                    token_packet.extend_from_slice(&raw[5..]);
                    let _ = requester_tx.send(Message::Binary(token_packet));
                    println!("[+] Routed token back to requester ID: {}.", requester_id);
                } else {
                    println!("[-] Requester ID {} is no longer connected.", requester_id);
                }

                // The solver is now finished. Re-add it to the queue since it is available now.
                // Get the solver's respective user-agent bucket/HashSet to add it back to from the solver_to_ua map.
                if let Some(ua) = s.solver_to_ua.get(&id).cloned() {
                    s.available_solvers_queue.entry(ua.clone()).or_default().insert(id);
                    let available_count = s.available_solvers_queue.get(&ua).map(|q| q.len()).unwrap_or(0);
                    println!("[+] Solver {} re-added to queue. Total available for UA '{}': {}.", id, ua, available_count);
                }
            }

            // On demand solve request from a requester.
            // This will forward our request for a solve to the next available solver in queue.
            // [1, proxy_url_len (u8), ...proxy_url_bytes, user_agent_len (u8), ...user_agent_bytes, ...(field_name_len, ...field_name_bytes, field_value_len, ...field_value_bytes)]
            1 => {
                let proxy_url_len = raw[1] as usize;
                let proxy_url_bytes = &raw[2..2 + proxy_url_len];
                let ua_len = raw[2 + proxy_url_len] as usize;
                let ua_bytes = &raw[3 + proxy_url_len..3 + proxy_url_len + ua_len];
                let requested_ua = String::from_utf8_lossy(ua_bytes).to_string();

                let mut s = state.lock().await;
                
                // Select a target solver id.
                // If the ua_len is 0, no ua was specified. It'll select a solver from a random ua bucket (next in the iter).
                // If the ua was specified, it'll pick a solver from that bucket.
                let solver_opt = if ua_len == 0 {
                    s.available_solvers_queue.iter()
                        .filter_map(|(ua, queue)| queue.iter().next().map(|&id| (id, ua.clone())))
                        .next()
                } else {
                    s.available_solvers_queue
                        .get(&requested_ua)
                        .and_then(|queue| queue.iter().next().map(|&id| (id, requested_ua.clone())))
                };

                if let Some((solver_id, target_ua)) = solver_opt {
                    // Remove this solver from its available_solvers_queue bucket now as it is occupied.
                    if let Some(queue) = s.available_solvers_queue.get_mut(&target_ua) {
                        queue.remove(&solver_id);
                    }

                    if let Some(solver_tx) = s.connections.get(&solver_id) {
                        // [1, proxy_url_len (u8), ...proxy_url_bytes, ...requester_id_bytes (u32), ...(field_name_len, ...field_name_bytes, field_value_len, ...field_value_bytes)]
                        let mut forward_packet = vec![1u8];
                        forward_packet.push(proxy_url_len as u8);
                        forward_packet.extend_from_slice(proxy_url_bytes);
                        forward_packet.extend_from_slice(&id.to_le_bytes());
                        let fields_start_idx = 3 + proxy_url_len + ua_len;
                        if raw.len() > fields_start_idx {
                            forward_packet.extend_from_slice(&raw[fields_start_idx..]);
                        }
                        let _ = solver_tx.send(Message::Binary(forward_packet));
                        println!("[+] Forwarded on-demand request from {} to solver {} (Requested UA: '{}').", id, solver_id, requested_ua);
                    }
                } else {
                    // Indicate that this solver request couldn't go through due to unavailable solvers.
                    // [2]
                    let _ = tx.send(Message::Binary(vec![2]));
                    println!("[-] No solvers available in the queue to handle request from {} for UA '{}'.", id, requested_ua);
                }
            }

            // Register this socket as a solver, and append its id to the available_solvers_queue.
            // [2, ...user_agent_bytes]
            2 => {
                let ua_bytes = &raw[1..];
                let ua = String::from_utf8_lossy(ua_bytes).to_string();

                // Insert the solver into its solver id to ua map, 
                // and add it to the available solvers queue.
                let mut s = state.lock().await;
                s.solver_to_ua.insert(id, ua.clone());
                s.available_solvers_queue.entry(ua.clone()).or_default().insert(id);
                
                let available_count = s.available_solvers_queue.get(&ua).map(|q| q.len()).unwrap_or(0);
                println!("[+] Solver {} added to queue. Total available for UA '{}': {}.", id, ua, available_count);
            }

            // Request to receive all currently available solvers. Useful for checking how many solver instances you can spawn.
            // Planning to restructure this so it logs all solvers per ua. 
            // [3]
            3 => {
                let s = state.lock().await;
                // Sum all available solvers across all ua HashSets.
                let available_solvers_count = s.available_solvers_queue.values().map(|q| q.len()).sum::<usize>() as u32;
                
                let mut response_packet = vec![3u8];
                response_packet.extend_from_slice(&available_solvers_count.to_le_bytes());
                // [3, ...available_solvers_count_bytes (u32)]
                let _ = tx.send(Message::Binary(response_packet));
            }

            // Ping to keep connection alive.
            // [255].
            255 => {
                // Do nothing. It appears browsers just may close inactive socket connections, so I added this ping
                // as an attempt to keep it alive.
            }

            _ => {
                eprintln!("Unknown header byte: {} from socket {}.", header, id);
            }
        }
    }

    // Disconnect socket and clear data.
    let mut s = state.lock().await;
    s.connections.remove(&id);
    
    // If socket was a solver/found in solver_to_ua map remove it from there,
    // and remove it from the solvers_queue for that respective ua's HashSet.
    if let Some(ua) = s.solver_to_ua.remove(&id) {
        if let Some(queue) = s.available_solvers_queue.get_mut(&ua) {
            queue.remove(&id);
            if queue.is_empty() {
                s.available_solvers_queue.remove(&ua);
            }
        }
    }
    
    println!("[-] Socket {} disconnected.", id);
}