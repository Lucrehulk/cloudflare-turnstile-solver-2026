# cloudflare-turnstile-solver-2026

A proof-of-concept Cloudflare Turnstile bypass system built with Rust and JavaScript. An effective, multi-component token-harvesting system. No API service required.

---

Still may be subject to change. Do note I am now in college so I have a lot of things I am doing, I have much of the progress done for the top priority feature done, but it's not all done yet. Not sure when it'll be out exactly but it'll be somewhat soon since much of it is done.

---

### Pros

| Major |
| :--- |
| No API service is required. This is completely free of charge to use. |
| Solver can quickly generate tokens. |
| The method is relatively firm and not as easy to patch as other bypasses, as it relies on overriding pages to avoid any policies like CORs or any fingerprinting, and the checkbox identifier will work as long as Cloudflare does not drastically change the UI of the widget itself. |
| This method has a far higher success rate than many other methods. |
| Because this method uses standard web browsers, the entire solving process comes off as legitimate to Cloudflare. |
| Great for building headless applications. Even though the solver itself needs GUI, once the token is solved for you can do everything headlessly. |

| Minor |
| :--- |
| Data is handled and already managed by a server that makes managing your haverested tokens easy. |
| Method is generally effective when you know the website you want to apply it to beforehand. |
| System is effectively modularized into multiple components, and makes for an effective pipeline. This also makes making changes and improvements simpler and component based. |

### Caveats

| Major |
| :--- |
| The solver is **not headless** — a GUI is required. With dockerization this could be fixed. |
| Ineffective for general, random web-scraping. Knowing the websites it will be used on is most effective. |
| No custom fingerprint spoofing for TLS/JA4, canvas, and other metrics like navigator values. But, given the legitimacy of the browsers, this isn't as severe as usual. |

| Minor |
| :--- |
| The method relies on a browser with overrides enabled. |
| Designed for smaller-scale token harvesting, though the token server architecture does support larger-scale operations. |
| Tunneling multiple proxies through each iframe is not supported. Do note this may potentially be added in the future if a feasible solution (some form of advanced tunneling) is found. Note that per-window proxying, however, is supported. |

---

## Why use this Method?

This method is designed as a free alternative to more top-level, or "enterprise" grade bypasses. I wanted to avoid solver APIs, and webdriver methods, for a completely real and legit browser instance. 

This also avoids the use of stealth browsers, which work, but constantly require updating all fingerprinting metrics to match current browsers. This is exceptionally high maintenance. I wanted to create a method that also requires much lower maintenance, and will generally be easy to re-implement if patched.

Also, as previously mentioned, this method is particularly most effective when targeting specific websites, as even with an automatic page loader, custom turnstile render field calls, for example, need to be managed per-website. This method is NOT designed as page load -> solve for any website. So it is not viable for webscraping.

What this method IS viable for, though, is solving repeated instances of Cloudflare Turnstiles on a singular site. Though, I have not compared it to standard methods like undetected Selenium loading, so I can't say if it's particularly better or worse, I can say this can safely generate many tokens, while being an extremely simple and free alternative to API services. 

## Supported Browsers (as of now).

To help create fingerprint variation, the goal of this system is to support multiple browsers (i.e. they have a proxy connector extension).

- Any CDP browser (encompasses the vast majority of the web browser market--including Chrome, Edge, Brave, Opera, and more).

---

## Latency, Throughput, & Overall Effectiveness

This method's primary goal is to token harvest on a specific site. Hence, it's objective is not to just open a site as previously stated. The goal is to generate as many tokens as possible.

This requires the maximization of two metrics:

- Latency (time for a single solver to fully complete the captcha, return token back to requester)
- Throughput (latency scaled by the actual amount of workers that are solving tokens)

### Latency

This project effectively minimizes the latency, or time for a single solver to solve the token. A few things contribute to this fact:
- Because the method is single site harvesting, no page redirect, or entirely new page loading is required.
- The override token solver file strips away unnecessary html elements. You are left with a black background and the widget in in iframe.
- Real browser fingerprints result in short challenges that take only seconds to go through. The time for a Cloudflare Turnstile challenge to go through takes only a few seconds at most. In general, you'll see results of even under two seconds.
- Pipeline of token transfer through solver -> token server -> receiver/backend is extremely fast.
- Vice versa, pipeline of request to solve through receiver -> token server -> solver is also extremely fast.
- Effectively, the approximate latency for a single solver to get a token to a receiver is:
T_solver_receive_challenge + T_solver_load_widget + T_solver_solve_widget + T_to_bounce_back_to_receiver ~ T_solver_solve_widget (time to solve widget takes a good few seconds, time to do everything else is only a fraction of a second). Now, this value can vary quite a bit. It usually takes at most five seconds but it honestly depends, after a while it may start to slow down too if Cloudflare begins to recognize an attack pattern. No hard specifics on this. It is simply bottlenecked by the cloudflare challenge itself, which not much can be done about. 

This is one of the most effective latencies possible, as it is effectively limited by the time it takes the browser to actually complete the Cloudflare challenge. The only way to even improve the speed on such would be a truly headless, full interaction scheme that could interaction with the turnstile challenge API fully, which is obviously not a feasible method as it would quickly break without much maintenance.

### Throughput

Throughput on this project is also great. In particular, you can easily spawn multiple browsers, including different browsers (as long as they are supported), and because the solve time for a single token usually is under two seconds, even if you spawn, say, even only five browsers (a setup I have used is Chrome + Edge + Brave + Opera + Opera GX for example, though do note of course more can be used for even faster throughput), this can easily rack up to a hundreds of tokens in only a short timeframe. 

Note that this is also on a singular device. If expanded to multiple devices (since the system relies on the token server, this can easily be done), this throughput only increases.

The only issue regarding throughput right now, and also mass automation, is this system's requirement of manually loading tabs to solve this. The system is not headless. Perhaps a solution like dockering with a virtual framebuffer could do this, or some sort of standard equivalent. However, this would usually not make a major difference as the Cloudflare challenges result in mainly a CPU bottleneck, and CPU performance would only receive a minor boost from this. Plus, this would also take a lot of work to do, and eliminate OS level gui clicking. You'd have to click at the browser level. 

Automatic page loader may also work but also brings a lot of depth. These are problems that can be tackled if I have time and want to do this--or someone sees promise in the theory proposed by this repository and pursues perfection of it.

Still, as explained, throughput is very high, particularly due to the extremely low latency combined with the fact you can still easily get multiple solvers up. 

### Overall

This method is extremely effective when it comes to token harvesting. Even without its maximum dockerized potential, it can still effectively generate hundreds of tokens in only minutes. 

---

# Proxy Formats

`protocol://host:port`
`protocol://user:pass@host:port`

The http protocol is recommended. Some browsers have iffy implementation for socks proxies.

---

## Components

The bypass is comprised of five main components:

1. **Token Harvester / Turnstile Widget Loader**
2. **Turnstile Widget Identifier & Clicker**
3. **Token Server**
4. **Proxy Extensions**
5. **Z-index Orderer**

---

### 1. Token Harvester / Turnstile Widget Loader

The Token Harvester loads the Turnstile widget by spawning iframe-based solvers, each pointing at a different Cloudflare site widget. Every solver iframe connects to the token server and forwards any solved tokens to it, which is done once it receives an on demand request from your backend/receivers.

**Setup:**

None. All of the config has been moved to the extension. You will just need the path for this file later (to use in the extension). The setup for such will be detailed there.

**How it works:**

Each solver tab connects to the token server's socket and registers itself as a solver to the token server. These solvers will then receive forwarded requests from the token server to solve turnstile widgets. When this is received, these solvers load the turnstile widget, and passively let it be solved (if a checkbox challenge occurs, the next component in this section, the checkbox clicker will handle that). Once a result is received the turnstile callback function is called, the result of the token is sent back to the server so it can be forwarded it to the correct requester. 

**Why overrides?**

Using overrides does require loading the actual page, but it sidesteps issues with CORS policies, TLS fingerprinting, and other browser/address analysis the target site may employ. Because the page loads normally and passes all standard security checks, our modified scripts can generate tokens cleanly without triggering those protections. These override scripts also allow us to save resources, as they allow minimal pages that are designed just to load the turnstile widgets.

---

### 2. Turnstile Clicker

The Turnstile Clicker automatically solves checkbox click challenges. Run the relevant `main.rs` file to start it. The clicker is **disabled by default** — press **F8** to toggle it on or off.

**Setup:**

Set the config values described in `main.rs`. That's all.

**How it works:**

The clicker identifies Cloudflare Turnstile checkboxes by analyzing pixel RGB values. It searches for pixels matching the characteristic grey ring border of the Turnstile checkbox. Once a candidate pixel is found, it performs a depth-first search (DFS) to verify the pixel forms a closed ring/loop. It then searches inward from all four sides to isolate the whitespace within the border — the actual clickable area. Finally, it dispatches OS-level input events to move the mouse to a point within that region and click.

> **Note:** The F8 toggle exists just to prevent any potential false positives. Toggle it on when you're on the pages just to avoid false positives (though it is pretty thorough, but just in case).

---

### 3. Token Server

The Token Server doesn't participate in solving—it routes solver requests to available solvers, and forwards completed tokens back to their respective requesters. Solver iframes forward their tokens here as they're solved.

**Setup:**

Set the `PORT` value in config. That's all.

**Packet & Protocol Structure:**

*All values are little-endian.*

#### Serverbound (client -> server):

| Sent From | Header | Description |
|-----------|--------|-------------|
| Solver | `0` | Incoming token result from a solver. The server routes it back to the specific requester who asked for it by extracting the requester ID, then re-adds the solver to the available queue.<br><br>**Structure:** `<0, ...requester_id_bytes (u32), ...solver_idx_bytes (u32), ...token_bytes>`<br>*Note: If the solver failed to get a token, then there are no token bytes.* |
| Receiver | `1` | On-demand solve request from a requester. The server pulls the next available solver from the queue and forwards this assignment to them.<br><br>**User-Agent Routing:** You can specify a specific user-agent in this packet, which will then make the token server force a solver with that user-agent. This is particularly useful for mimicking real web traffic, and distributing solves across an amount that mimics the real web traffic distribution of user-agents. You can also just leave user-agent as `""` for a random selection.<br><br>**Field Spoofing:** The `fields` data allows you to implement JS field spoofs for a few things:<br>• **JS APIs:** You can spoof JS APIs like navigator properties and window dimensions by specifying `navigator.property`, `window.property`, etc. You can spoof with whatever JS properties you'd like basically. Window/viewport dimensions, navigator properties, etc. are all great properties you can spoof. However, so as to not confuse it with another field type (the next we will talk about), your JS field spoofs should refer to names in the structure of `API.key`. Nested references, like `API.key.key`, are also fine. For your field values, though obviously for the protocol they must be passed in as string data, if the values are directly castable to other primitive types (number, boolean), they will be automatically converted to such by the solvers for their logic. Otherwise, if not directly convertable to said types, they will be kept as strings.<br>• **Render Calls:** The `turnstile.render` function call, which initializes the widget, can take in special fields and extra data, such as `action`, or `cData`. To counter this, you may also specify field data for these in this packet. To specify field data for this, simply make the field name data you pass in the form of `key`. This contrasts from the `API.key` structure of the first case, and the system will know you are referring to a custom render call field. These fields will then be passed into the render call the solver makes.<br><br>**Structure:** `<1, ...solver_idx_bytes (u32), user_agent_len (u8), ...user_agent_bytes ...(field_name_len (u8), ...field_name_bytes, field_value_len (u8), ...field_value_bytes)>` |
| Solver | `2` | Register the sending socket as a solver. The server appends its socket ID to the available solvers queue.<br><br>**Queue Buckets:** It appends the socket ID to the solver queue bucket that matches the specified user-agent provided by the solver. If a bucket/HashSet for such does not exist yet, then it is created and the solver's socket ID is added to it. A user-agent can be referred to by the receiver when making requests, which will force only a solver with the matching user-agent to solve the request.<br><br>**Structure:** `<2, ...user_agent_bytes>` |
| Receiver | `3` | Request the total available solvers count. Good for analyzing how many active solving instances you can spawn.<br><br>**Structure:** `<3>` |

#### Clientbound (server -> client):

| Endpoint | Name | Description |
|----------|------|-------------|
| Receiver | Token | Incoming token delivered to a requester.<br><br>**Structure:** `<...solver_idx_bytes (u32), ...token_bytes>`<br>*Note: If the solver failed to get a token, then there are no token bytes.* |
| Receiver | Solvers Unavailable | A request made by a solver could not be completed because no solvers were available to accept it.<br><br>**Structure:** `<0>` |
| Solver | Solve Request | Solve a turnstile widget request that is delivered to a solver. Field data is parsed and does whatever is necessary (`API.key` -> JavaScript API is spoofed with the given field value, `key` -> turnstile render call adds this field).<br><br>**Structure:** `<...solver_idx_bytes (u32), ...requester_id_bytes (u32), ...(field_name_len (u8), ...field_name_bytes, field_value_len (u8), ...field_value_bytes)>` |
| Receiver | Available Solvers Result | The result to the available solvers count request you made.<br><br>**Length Collision Fix:** Note, the zero at the end of this packet is dummy data. It is actually added because I made the accepted parsing system for these packets length-based to check packet type, but the token packet will deliver 4 bytes if it fails to receive a token. I added the extra byte to this packet to solve the length collision because no branching logic is required for this one, and it's a much smaller and simpler case so I just preferred it.<br><br>**Structure:** `<...available_solvers_bytes (u32), 0>` |

> **Note:** The clientbound packets do not have headers since each endpoint receives few, easily discernible packets. Receivers receive a packet of only length 1 (Solvers Unavailable), the token packet itself (can be length 4 if there is no token and the request failed), or a packet of length 5 (total available solvers). This makes discerning packets by length easy. The solver can only receive a solve request.

**How it works:**

The architecture for the specific protocol of the server is above. The server assigns an ID to every socket, allows solvers to register themselves, for which it stores into available solver buckets (HashSets accessed by an outer HashMap that uses the respective user-agents as keys, meaning you can refer to solvers with specific user-agents only). Receivers can then simply send packets to the server to request solves from solvers, which if the solvers are available the server will forward. The solvers will send the solve results to the server, which will then forward it back to the original requester, which it does by bouncing around the original `requester_id` within these packets.

---

### 4. Proxy Extensions

The extensions allow us to utilize browser proxy API capabilities to connect to proxies, per tab. They also have JS API anti-fingerprint/spoof metrics, along with a WebRTC host peeking block.

For each browser you'll be using, you'll need to add the respective extension for that browser from `proxy-extensions` to whatever browser you are using (ex. cdp for cdp browsers, truly a shocker), and run it. These extensions provide the API necessary for asynchronous proxy connections, allowing you to await and connect to a proxy before continuing execution.

**Setup:**

1. **Set your file paths**
   Set PROXIES_LIST_PATH, OVERRIDE_FILE_PATH, and INJECT_CONFIG_FILE_PATH in `background.js`. Names are self explanatory. Note the proxy list should be a linesplit list of proxies following the expected format discussed earlier in this readme.

2. **Set inject config**
   Set SITEKEY, PROXY_CONNECT_TIMEOUT, USE_PROXY_SOLVING, and TOKEN_SERVER_HOST in your text config. A file with example values is provided in the `proxy-extensions` directory. These names should also be self explanatory. These values are injected as `localStorage` values into your page, and your harvester `index.html` reads and uses them. 

Then just load the extension of course.

## How It Works

Each extension acts as a bridge for proxy routing and fingerprint spoofing, driven by `window.postMessage` events. The execution flow follows something like this:

0. **Page Injections and Debugger Injections**
   localStorage config edits are immediately injected upon page load. File paths for proxies and the override are now read by the extension too and th e file contents can be parsed. Additionally, the extension can attach cdp debuggers to any site (except for privileged chrome:// pages of course), and these debuggers can listen for outgoing web requests, and check if the info for the webrequest that went out matches the site we are currently on, and if it does it returns the override script back instead of the actual site page. 

1. **Initialization**
   The extension listens for a `SET_TAB_PROXY` message sent by the client (which our solvers use). This payload contains the target proxy details and the specific JavaScript field data you want to spoof. *(Note: See the token server section for details on structuring this field data).*

2. **Proxy Routing**
   The extension applies the requested proxy. Because protocols vary by browser, this is handled in one of two ways: it either establishes a direct proxy connection for the tab, or it actively listens for outgoing requests and applies the proxy details to them on the fly.

3. **JS API Spoofing**
   The requested JavaScript APIs are spoofed by overriding native prototypes. This is done using a hidden Symbol key reference pointing to a modifiable entry. This architecture is critical: it allows our script to dynamically overwrite fields with new values without breaking the page, while completely hiding the spoofing metrics from anti-bot systems like Cloudflare.

4. **WebRTC Leak Prevention**
   To maintain operational security, WebRTC host peeking and STUN search features are disabled. This strictly blocks WebRTC host IP leaks while keeping standard WebRTC functionality enabled.

5. **matchMedia Protection**
   `matchMedia`, a method that runs on the CSS engine, can get your real window dimensions if you spoof standard JS window dimension values. It can check and compare values like `width`/`min-width`/`max-width`, or `height`/`min-height`/`max-height`. `matchMedia` returns data in a `matches` property of the response structure, which is a boolean determining if the given query matches or aligns with the actual session's data. 

   If you provide `window.innerWidth` and `window.innerHeight` fields (though to fully spoof well, you'll need to spoof other fields—these just trigger the feature, as mediaQuery compares all pixel data in relation to those dimensions), `matchMedia` will be spoofed. For width and height checks, it will force the `matches` field to output the exact result it would give if your window were actually the dimensions of your spoofed `window.innerWidth` and `window.innerHeight`.

7. **Execution Readiness**
   Once the proxy is fully connected and the environment is secured, the extension sends a `PROXY_READY` message back to the client. This allows solvers to securely await a confirmed proxy connection before continuing their execution.

---

### 5. Z-index Orderer

**Note this is currently only designed for Windows. Not going to add implementation for other operating systems myself, but pull requests are welcome.**

This component serves as a direct solution to a major issue posed by the OS-level gui clicking: the browser overlap can cause tabs to become unclickable. Since our clicker relies on actual rendered data on our screen, if a browser is covered by another browser, it can become unclickable.

So, to deal with this, this component orders and locks Z-indexes for all browsers, which almost entirely negates the overlap issue. Because Cloudflare Turnstile widgets will only spawn in the top left corner, for each browser, you need only to leave that top left corner non-overlapped per browser window. This means effectively, the spacing for your browser windows only have to be the size of the turnstile checkbox--which is very small. 

This is obviously an insanely large reduction from without z-index ordering and locking, as without such you'd need to individually space out browsers as they could not overlap, as if an interaction was made to a browser with a region previously below another browser that has a checkbox over that region, that checkbox would then become covered as the interacted browser would get the top z-index and then cover the other browser. 

With this, though, newer tabs are always locked above older tabs. Because of this, you simply only need to ensure the checkbox area isn't covered by an newer browser--but the checkbox area can now go over older browsers as those older browsers cannot go above the newer browser (well for a short moment they do, our script polls and corrects this at a very quick rate though so it is negligible)--effectively shrinking the required overlap area to just a checkbox. 

Because of this, the issue regarding gui overlap is effectively not an issue at all. On pure screen area alone, with this change you could certainly spawn at least a hundred checkboxes, maybe more but I'm not doing the math for that. Point is, this allows you to spawn as many browsers as you'll need without facing overlap issues. The only issue becomes standard resource bottlenecks. 

**Setup:**

You can a config value in `main.rs` for a for a z-order enforcement loop/thread rate. Otherwise just dependencies as per usual.

**When you run this, press F7 to turn off the new page checking loop this component runs in order to check for new pages. You can turn it back on by pressing F7 again too. It is toggleable, but on by default so you can add new pages. Once you are done loading pages, you can toggle this off to save performance.**

**How it Works:**

Windows OS uses the "handle to window" (HWND) mechanism to identify different windows. This script first initially gets all pre-existing windows when it first runs and stores their HWNDs, this is used for comparison when checking for new windows so we can ignore windows that were already pre-existing. Then, there are two "worker" threads that we use as loops. One checks for new windows. When a new window is created, it is tracked by its HWND and assigned a hard z-index. The other actually enforces the z-order of all tabs. It simply goes over all tracked windows and actually enforces the z-index by using the `setWindowPos` method. Note newer tabs are placed on top, and older tabs go below. 

---

### 6. Browser Launcher

**Note this is currently only designed for Windows. Not going to add implementation for other operating systems myself, but pull requests are welcome.**

**Also note, though this component does automatically launch and position the browsers, for some certain browsers (like Google Chrome) the custom profile login currently does not work, and you have to log in with your own profile. It works better on browsers like Edge where this does work.**

This component introduces automatic browser detection and launching. It has two key binaries: main.rs, and find_browsers.rs. An explanation for each--and the arguments to pass--is provided below. 

**find_browsers.rs:** This binary does as the name suggests, it simply has a list of browser definitions (like paths to look in, arguments to run them, etc.), and searches for those browsers on your device. However, it also can create custom browser profiles (since Cloudflare does not analyze profile integrity) that already have the proxy extension preloaded onto them (given as an argument). You can specify the amount of profiles you want for each browser, plus the path to your unpacked proxy extension, in the CLargs. Profile data is written to the browser_profiles directory, and ./browser_binaries.txt gets the browser shell execution data. Both are wiped and regenerated on each run. Browser keys follow a name + number schema. so `edge 3` generates `edge1`, `edge2`, `edge3`, which are the keys you pass to main.rs.

Structure: `cargo run --bin find_browsers -- [browser count]... path\to\extension`

Example: `cargo run --bin find_browsers -- edge 5 chrome 2 "C:\path\to\extension"`

**main.rs:** This binary makes use of the Win32 crate again (as the Z-index Orderer also does). It uses the definitions provided by find_browsers to run browsers of your choice, and using the Win32 capabilities it can set the positions and dimensions (or geometry) of these browsers. Windows are arranged left-to-right (the Turnstile Widget loads in the top left corner). The positions wrap around and then move down in the Y direction once they go past the visible window dimension, this way the checkboxes remain visible. Also note all browsers that were spawned will close once you exit the process.

Structure: `cargo run --bin main -- url width height spacing_x spacing_y [key...]`

Example: `cargo run --bin main -- "https://example.com" 280 490 300 510 edge1 edge2 edge3 edge4 edge5 chrome1 chrome2`

---

## Starting It Up

1. Generate your browser profile and binary definitions (**browser_launcher, find_browsers.rs**).
2. Start the **token server**.
3. Start the **auto-clicker**.
4. Start the **z-index-orderer**.
5. Either automatically load your browsers (**browser_launcher, main.rs**), or manually open them (or a combination of both).
6. Press **F8** to enable the auto-clicker.
7. Start your backend, token managing and requesting system. 
8. Watch it go.

---

## Some Helpers for your Backend

Your backend that actually gets and requests solves for tokens will need to interact with the token server. 

You will need a reference to a proxies txt list. This list should match the one you set at localStorage.proxies on the solver page.

For any turnstile render call custom fields, such as "cData" or "action" as previously mentioned, you'll need to figure out how they are generated for your target, and recreate the logic to how these fields are generated so that you can pass them into your solve request packet. "action" is usually a hardcoded string, but "cData" is often used as an individual ID/verification field. In short, ensure all fields of the turnstile render call match.

For any JavaScript API fields you'd like to spoof, you'll also need to send that data into the fields arguments of the construct_solver_request_packet. Details on how to structure the fields data is provided in previous sections (see token server section).

**Construct solve request packet:**

```javascript
// proxy_idx = literally just the index of your proxy in the proxy list.
// user_agent = user-agent string of the target you want to run (matches to navigator.userAgent). 
// fields = object, { name: value, name2: value2, ... namen: valuen }. Names and values are strings.
function construct_solver_request_packet(proxy_idx, user_agent = "", fields = {}) {
   let encoder = new TextEncoder();
   let packet = Array(5);
   packet[0] = 1;
   packet[1] = proxy_idx & 255;
   packet[2] = (proxy_idx >> 8) & 255;
   packet[3] = (proxy_idx >> 16) & 255;
   packet[4] = (proxy_idx >> 24) & 255;
   let user_agent_bytes = encoder.encode(user_agent);
   packet[5] = user_agent_bytes.length;
   packet.push(...user_agent_bytes);
   for (let field_name in fields) {
         let field_value = fields[field_name];
         let field_name_bytes = encoder.encode(field_name);
         let field_value_bytes = encoder.encode(field_value);
         let field_name_len = field_name_bytes.length;
         let field_value_len = field_value_bytes.length;
         packet.push(field_name_len);
         packet.push(...field_name_bytes);
         packet.push(field_value_len);
         packet.push(...field_value_bytes);
   }
   return new Uint8Array(packet);
};
```

**Parse token response packet:**

```javascript
// packet = packet buffer.
function parse_token_response_packet(packet) {
    let view = new DataView(packet);
    let solver_idx = view.getUint32(0, true);
    let token = undefined;
    if (packet.length > 4) {
      let u8 = new Uint8Array(packet);
      token = new TextDecoder().decode(u8.subarray(4));
    }
    return [solver_idx, token];
};
```

**Parse available solvers packet:**

```javascript
// packet = packet buffer.
function parse_available_solvers_count_packet(packet) {
    let view = new DataView(packet);
    return [view.getUint32(0, true)];
};
```

**Match packets:**

```javascript
// packet = packet buffer
if (packet.byteLength > 5) {
   // Token Packet
} else if (packet.byteLength == 5) {
   // Available Solvers Result
} else if (packet.byteLength == 4) {
   // Failed Token Result (only solver idx is sent back)
} else {
   // Solvers Unavailable
}
```

---

## Future Plans/What this Needs (may not be done, but if major updates do occur to this project it will likely be these).

Automatic, headless page loading with a docker. Plus Devtools protocol level overriding so standard overrides aren't required. Dockerizing and making this project headless could be huge for maximizing throughput. Plus it'd just make it feel less like a PoC and more like a fully fleshed out project. Do note, however, that due to the extreme CPU usage caused by browsers running Turnstile either way, that the benefits of the headless browser won't be as extreme, since CPU and not RAM is the main bottleneck due to all the tough JS challenges executed by Cloudflare especially. This would still serve as an optimization assuming it didn't result in Cloudflare flagging these sessions. Also you'd haevto lose the OS level click in favor of a browser protocol level click which isn't a huge issue but it is a difference. Point is it'd take a lot of work.

WebGL debug info spoofing (currently items like vendor are not spoofed, however without proper canvas fingerprinting to associate with these modifying such fields could make you get flagged, especially on browsers like Chrome where by default there is no anti-canvas fingerprinting).

Canvas fingerprint spoofing to match given hardware specs.

If a feasible solution is found, a way to tunnel individual iframes (hence enhancing multi-proxy solving outside of just different tabs) may be implemented.

---

## Contributing

All contributions are very welcome. If you have a way to improve this project, please share with issues, pull requests, etc.

---
