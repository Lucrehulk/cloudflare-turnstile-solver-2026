// Execute browser binaries and set window sizings and placings based on CLargs.

use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

mod win32 {
    use windows::core::BOOL;
    use windows::Win32::{
        Foundation::{CloseHandle, HWND, LPARAM, RECT},
        System::{
            Console::{SetConsoleCtrlHandler, PHANDLER_ROUTINE},
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW,
                PROCESSENTRY32W, TH32CS_SNAPPROCESS,
            },
        },
        UI::WindowsAndMessaging::{
            EnumWindows, GetClassNameW, GetSystemMetrics, GetWindowRect,
            GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible,
            SetWindowPos, ShowWindow, SM_CXSCREEN, SM_CYSCREEN,
            SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOZORDER, SW_RESTORE,
        },
    };

    pub use windows::core::BOOL as WinBOOL;
    pub use windows::Win32::Foundation::HWND as WinHWND;

    pub fn screen_size() -> (i64, i64) {
        let w = unsafe { GetSystemMetrics(SM_CXSCREEN) } as i64;
        let h = unsafe { GetSystemMetrics(SM_CYSCREEN) } as i64;
        (w, h)
    }

    // Get every (pid, exe_name) tuple over our processes.
    fn snapshot_processes() -> Vec<(u32, String)> {
        let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        let Ok(snap) = snap else { return Vec::new() };

        let mut rows = Vec::new();
        let mut e: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
        e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        unsafe {
            if Process32FirstW(snap, &mut e).is_ok() {
                loop {
                    let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
                    let name = String::from_utf16_lossy(&e.szExeFile[..len]).to_lowercase();
                    rows.push((e.th32ProcessID, name));
                    if Process32NextW(snap, &mut e).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snap);
        }
        rows
    }

    // Return all PIDs with matching exe names.
    pub fn pids_by_exe_name(exe_name: &str) -> Vec<u32> {
        let target = exe_name.to_lowercase();
        snapshot_processes()
            .into_iter()
            .filter(|(_, name)| *name == target)
            .map(|(pid, _)| pid)
            .collect()
    }

    // Find all visible, titled, properly-sized Chrome-family windows belonging to the given exe.
    // Chromium-based browsers all use the Chrome_WidgetWin_1 window class, so we filter using that.
    pub fn find_all_browser_windows(exe_name: &str) -> Vec<(HWND, u32)> {
        let matching_pids = pids_by_exe_name(exe_name);
        if matching_pids.is_empty() {
            return Vec::new();
        }

        struct SearchState {
            pids: Vec<u32>,
            found: Vec<(HWND, u32)>,
        }

        unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
            unsafe {
                let state = &mut *(lparam.0 as *mut SearchState);
                if !IsWindowVisible(hwnd).as_bool() {
                    return BOOL(1);
                }

                let mut pid: u32 = 0;
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
                if !state.pids.contains(&pid) {
                    return BOOL(1);
                }
                if GetWindowTextLengthW(hwnd) == 0 {
                    return BOOL(1);
                }

                let mut class = [0u16; 256];
                let n = GetClassNameW(hwnd, &mut class);
                if n <= 0 {
                    return BOOL(1);
                }
                let class_str = String::from_utf16_lossy(&class[..n as usize]);
                if !class_str.starts_with("Chrome_WidgetWin_1") {
                    return BOOL(1);
                }

                // Skip small windows.
                let mut r = RECT::default();
                let _ = GetWindowRect(hwnd, &mut r);
                if (r.right - r.left) < 200 || (r.bottom - r.top) < 200 {
                    return BOOL(1);
                }

                state.found.push((hwnd, pid));
                BOOL(1)
            }
        }

        let mut state = SearchState { pids: matching_pids, found: Vec::new() };
        let lparam = LPARAM(&mut state as *mut SearchState as isize);
        unsafe {
            let _ = EnumWindows(Some(cb), lparam);
        }
        state.found
    }

    pub fn window_area(hwnd: HWND) -> i64 {
        let mut r = RECT::default();
        unsafe {
            let _ = GetWindowRect(hwnd, &mut r);
        }
        ((r.right - r.left) as i64) * ((r.bottom - r.top) as i64)
    }

    // Restore the window, then set its geometry.
    // We call SetWindowPos twice because some browsers ignore the first call while
    // they're still initializing.
    pub fn set_window_geometry(hwnd: HWND, x: i32, y: i32, w: i32, h: i32) {
        let flags = SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED;
        unsafe {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetWindowPos(hwnd, None, x, y, w, h, flags);
            let _ = SetWindowPos(hwnd, None, x, y, w, h, flags);
        }
    }

    pub fn set_ctrl_handler(h: unsafe extern "system" fn(u32) -> BOOL) {
        unsafe {
            let routine: PHANDLER_ROUTINE = Some(h);
            let _ = SetConsoleCtrlHandler(routine, true);
        }
    }
}

// Strip directory components from a full binary path to get just the exe filename.
fn extract_exe_name(binary_path: &str) -> String {
    binary_path
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(binary_path)
        .to_string()
}

// Read browser_binaries.txt into a key -> command map.
fn parse_browser_binaries(path: &str) -> HashMap<String, String> {
    let content = fs::read_to_string(path).unwrap();
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once(": ") {
            map.insert(key.to_string(), value.to_string());
        }
    }
    map
}

// Split a shell-style command string into tokens, respecting double-quoted spans.
fn tokenise(cmd: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for ch in cmd.chars() {
        match ch {
            '"' => quoted = !quoted,
            ' ' | '\t' if !quoted => {
                if !cur.is_empty() {
                    tokens.push(cur.clone());
                    cur.clear();
                }
            }
            _ => cur.push(ch),
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    tokens
}

// Compute (x, y) placement for each browser window, wrapping to a new row
// when adding another column would go off the right edge of the screen.
fn compute_positions(
    count: usize,
    browser_width: i64,
    spacing_x: i64,
    spacing_y: i64,
    screen_width: i64,
) -> Vec<(i64, i64)> {
    let mut out = Vec::with_capacity(count);
    let mut x: i64 = 0;
    let mut y: i64 = 0;
    for _ in 0..count {
        if x + browser_width > screen_width {
            x = 0;
            y += spacing_y;
        }
        out.push((x, y));
        x += spacing_x;
    }
    out
}

// Launch a single browser instance, wait for its window to appear, then position it.
// We snapshot existing windows before spawning so we can identify the new one reliably,
// even when other instances of the same browser are already running.
fn launch_and_position(
    key: &str,
    tokens: &[String],
    url: &str,
    width: i64,
    height: i64,
    x: i64,
    y: i64,
) -> u32 {
    use win32::WinHWND as HWND;

    let binary = &tokens[0];
    let exe_name = extract_exe_name(binary);

    let before: HashSet<isize> = win32::find_all_browser_windows(&exe_name)
        .into_iter()
        .map(|(h, _)| h.0 as isize)
        .collect();

    let mut args: Vec<String> = tokens[1..].to_vec();
    args.push(url.to_string());

    let child = Command::new(binary)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let spawn_pid = child.id();
    drop(child);

    // Poll until the browser's window appears or we hit a 10-second deadline.
    // We pick the largest new window to avoid latching onto a small background helper 
    // that surfaces first.
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut result_pid = spawn_pid;

    loop {
        let mut new_ones: Vec<_> = win32::find_all_browser_windows(&exe_name)
            .into_iter()
            .filter(|(h, _)| !before.contains(&(h.0 as isize)))
            .collect();
        if !new_ones.is_empty() {
            new_ones.sort_by_key(|(h, _)| std::cmp::Reverse(win32::window_area(*h)));
            let (hwnd, pid) = new_ones[0];
            // Apply geometry three times with short sleeps between — browsers often
            // resize themselves once after initial paint, which would undo a single call.
            win32::set_window_geometry(hwnd, x as i32, y as i32, width as i32, height as i32);
            println!("{} is up (pid {}) at ({}, {})", key, pid, x, y);
            result_pid = pid;
            thread::sleep(Duration::from_millis(800));
            win32::set_window_geometry(hwnd, x as i32, y as i32, width as i32, height as i32);
            thread::sleep(Duration::from_millis(1500));
            win32::set_window_geometry(hwnd, x as i32, y as i32, width as i32, height as i32);
            break;
        }
        if Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(150));
    }

    result_pid
}

static PIDS: Mutex<Vec<u32>> = Mutex::new(Vec::new());

fn kill_all() {
    let pids = PIDS.lock().unwrap();
    println!("Closing {} browser process(es)...", pids.len());
    for &pid in pids.iter() {
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .output();
    }
}

unsafe extern "system" fn ctrl_handler(_: u32) -> win32::WinBOOL {
    kill_all();
    std::process::exit(0);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let url = &args[1];
    let width: i64 = args[2].parse().unwrap();
    let height: i64 = args[3].parse().unwrap();
    let spacing_x: i64 = args[4].parse().unwrap();
    let spacing_y: i64 = args[5].parse().unwrap();
    let browser_keys: Vec<&str> = args[6..].iter().map(|s| s.as_str()).collect();

    let browsers = parse_browser_binaries("./browser_binaries.txt");

    // Resolve keys to their tokenised commands.
    let resolved: Vec<(&str, Vec<String>)> = browser_keys
        .iter()
        .filter_map(|key| browsers.get(*key).map(|cmd| (*key, tokenise(cmd))))
        .collect();

    if resolved.is_empty() {
        println!("None of the given browser keys were found in browser_binaries.txt.");
        return;
    }

    let (sw, sh) = win32::screen_size();
    println!("Launching {} browser(s) on a {}x{} screen.", resolved.len(), sw, sh);

    win32::set_ctrl_handler(ctrl_handler);

    let positions = compute_positions(resolved.len(), width, spacing_x, spacing_y, sw);

    for ((key, tokens), (x, y)) in resolved.iter().zip(positions.iter()) {
        let pid = launch_and_position(key, tokens, url, width, height, *x, *y);
        // Register each PID immediately so a Ctrl+C mid-launch still kills
        // every browser that's already been started.
        let mut pids = PIDS.lock().unwrap();
        pids.push(pid);
        pids.sort_unstable();
        pids.dedup();
    }

    println!("All browsers launched. Press Ctrl+C to close them and exit.");
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}