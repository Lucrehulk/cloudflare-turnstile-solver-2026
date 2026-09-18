// Paths config for this background script to read files from, for proxies and our override script.

const OVERRIDE_FILE_PATH = String.raw``;

const INJECT_CONFIG_FILE_PATH = String.raw``;

// Object map for proxy ID info.
let active_proxy = null;

// Object map for proxy authentication credentials.
let active_credentials = null;

// Cached parsed config from the inject config file.
let inject_config_content = null;

// Tracks which tab IDs currently have the debugger attached for override interception.
let debugger_attached_tabs = new Set();

// Maps tab ID -> the exact top-level URL the tab is navigating to.
// Only requests matching this URL will be overridden; all others pass through untouched.
let tab_pending_urls = new Map();

// Load our state from local storage on script wakeup to prevent data loss after idling.
const state_loaded = chrome.storage.local.get(['active_proxy', 'active_credentials']).then((res) => {
    active_proxy = res.active_proxy || null;
    active_credentials = res.active_credentials || null;
    update_chrome_proxy_config();
});

// Chromium service workers suspend when idle. This saves state before the script sleeps.
function save_state() {
    chrome.storage.local.set({ active_proxy, active_credentials });
}

// Prevent WebRTC from leaking the host's local IP address (peeking), without disabling WebRTC itself.
if (chrome.privacy && chrome.privacy.network && chrome.privacy.network.webRTCIPHandlingPolicy) {
    chrome.privacy.network.webRTCIPHandlingPolicy.set({ value: "default_public_interface_only" });
}

// Answer proxy authentication challenges natively with stored credentials.
chrome.webRequest.onAuthRequired.addListener(
    (details, callback) => {
        if (!details.isProxy) {
            callback({});
            return;
        }

        // Wait for the storage promise to resolve before processing the credentials.
        state_loaded.then(() => {
            if (active_credentials) {
                callback({
                    authCredentials: {
                        username: active_credentials.username,
                        password: active_credentials.password
                    }
                });
            } else {
                callback({});
            }
        }).catch((err) => {
            console.error("[Proxy Bridge] Error loading credentials:", err);
            callback({});
        });
    },
    { urls: ["<all_urls>"] },
    ["asyncBlocking"]
);

// Update Chromium global proxy configuration using fixed_servers for strict routing.
function update_chrome_proxy_config() {
    if (!active_proxy) {
        chrome.proxy.settings.set({ value: { mode: "system" }, scope: "regular" });
        return;
    }

    const config = {
        mode: "fixed_servers",
        rules: {
            singleProxy: {
                scheme: active_proxy.type,
                host: active_proxy.host,
                port: active_proxy.port
            },
            bypassList: ["localhost", "127.0.0.1"]
        }
    };

    chrome.proxy.settings.set({
        value: config,
        scope: "regular"
    });
}

// Listen to messages posted by the client if we want to set up a proxy.
chrome.runtime.onMessage.addListener((message, sender, send_response) => {
    if (message.action == "setup_proxy" && sender.tab) {
        // Ensure state is loaded before modifying.
        state_loaded.then(() => {
            let proxy_string = message.proxy_details;

            try {
                // "protocol://host:port", "protocol://user:pass@host:port"
                let url = new URL(proxy_string);
                let proxy_type = url.protocol.replace(":", "").toLowerCase();
                let host_name = url.hostname;
                let port_num = parseInt(url.port, 10);

                active_proxy = {
                    type: proxy_type,
                    host: host_name,
                    port: port_num
                };

                // Optional proxy auth, parsed as a "protocol://user:pass@host:port" proxy URL.
                if (url.username) {
                    active_credentials = {
                        username: decodeURIComponent(url.username),
                        password: decodeURIComponent(url.password)
                    };
                } else {
                    active_credentials = null;
                }

                save_state();
                update_chrome_proxy_config();
                send_response({ success: true });
            } catch (err) {
                console.error("[Proxy Bridge] Error parsing proxy URL:", err);
                send_response({ success: false });
            }
        });
        return true;
    }

    // Cache the parsed inject config.
    if (message.action == "inject_config_file_content") {
        if (message.content) {
            inject_config_content = message.content;
        } else {
            console.error("[Inject Config] Could not read inject config file:", message.error);
        }
        return false;
    }

    // Send the inject config to whichever tab requests it (content_main writes it to localStorage).
    if (message.action == "get_inject_payload") {
        send_response({ inject_config: inject_config_content });
        return true;
    }

    // Get the override file content response relayed back from the offscreen document.
    if (message.action == "override_file_content") {
        let { request_id, tab_id, content, error } = message;
        if (error || !content) {
            console.error("[Override] Could not read override file:", error);
            chrome.debugger.sendCommand({ tabId: tab_id }, "Fetch.continueRequest", { requestId: request_id });
            return false;
        }
        chrome.debugger.sendCommand({ tabId: tab_id }, "Fetch.fulfillRequest", {
            requestId: request_id,
            responseCode: 200,
            responseHeaders: [{ name: "Content-Type", value: "text/html; charset=utf-8" }],
            body: btoa(unescape(encodeURIComponent(content)))
        }).catch((err) => {
            console.error("[Override] Fetch.fulfillRequest failed:", err);
        });
        return false;
    }

    return false;
});

// There can be errors if a message is sent to an uncreated offscreen document,
// so this simply allows us to ensure it exists.
let _offscreen_promise = null;
async function ensure_offscreen_document() {
    if (_offscreen_promise) return _offscreen_promise;
    _offscreen_promise = chrome.offscreen.hasDocument().then((existing) => {
        if (!existing) {
            return chrome.offscreen.createDocument({
                url: "offscreen.html",
                reasons: ["BLOBS"],
                justification: "Read local override file via file:// fetch which is unavailable in service workers"
            });
        }
    });
    return _offscreen_promise;
}

// Read the inject config file and cache it.
async function load_inject_config_file() {
    if (!INJECT_CONFIG_FILE_PATH) return;
    await ensure_offscreen_document();
    chrome.runtime.sendMessage({
        action: "read_inject_config_file",
        file_path: INJECT_CONFIG_FILE_PATH
    });
}

load_inject_config_file();

// Attach the debugger that listens for requests and overrides the target page file with our override to a tab.
async function attach_debugger_to_tab(tab_id) {
    try {
        if (debugger_attached_tabs.has(tab_id)) {
            // Detach the existing session before re-attaching. Without this, Chrome still considers
            // the debugger attached even after we delete the tab from our local set, so the next
            // chrome.debugger.attach call throws "Another debugger is already attached" and the
            // catch block swallows it — meaning Fetch.enable never runs and the override never fires.
            await chrome.debugger.detach({ tabId: tab_id }).catch(() => {});
            debugger_attached_tabs.delete(tab_id);
        }
        await chrome.debugger.attach({ tabId: tab_id }, "1.3");
        debugger_attached_tabs.add(tab_id);
        await chrome.debugger.sendCommand({ tabId: tab_id }, "Fetch.enable", {
            patterns: [{ urlPattern: "*", resourceType: "Document", requestStage: "Response" }]
        });
    } catch (err) {
        console.error("[Override] Failed to attach debugger to tab", tab_id, err);
    }
}

// Detaches the debugger cleanly when a tab is closed.
chrome.tabs.onRemoved.addListener((tab_id) => {
    if (debugger_attached_tabs.has(tab_id)) {
        chrome.debugger.detach({ tabId: tab_id }).catch(() => {});
        debugger_attached_tabs.delete(tab_id);
    }
    tab_pending_urls.delete(tab_id);
});

// Attach the debugger to every tab when this extension loads.
chrome.tabs.query({}, (tabs) => {
    for (let tab of tabs) {
        if (tab.id != null) attach_debugger_to_tab(tab.id);
    }
});

// Attach the debugger to a new tab the moment the tab is created, before it navigates anywhere.
chrome.tabs.onCreated.addListener((tab) => {
    if (tab.id != null) attach_debugger_to_tab(tab.id);
});

// Whenever we are about to navigate we need to get the url so we can know what request to override (the main index.html for that page),
// and we need to attach the debugger again. The issue is that chrome:// tabs are privileged and debuggers cannot be run on them, 
// so there are some issues with loading the debugger from the base tab (empty base tab) to our target tab, so instead we attach it to
// this tab here. Consequently, we cannot immediately inject our override script since it only just loaded as we loaded our target page,
// but reloading the page immediately solves this issue so it's not really a big deal.
chrome.webNavigation.onBeforeNavigate.addListener((details) => {
    if (!OVERRIDE_FILE_PATH) return;
    if (details.frameId !== 0) return;
    tab_pending_urls.set(details.tabId, details.url);
    debugger_attached_tabs.delete(details.tabId);
    attach_debugger_to_tab(details.tabId);
}, { url: [{ schemes: ["http", "https"] }] });

// Freeze the request and intercept the response with our own override file.
// Only do this for the page itself that we navigate to, so no other sources are overridden.
// This way the turnstile interface still works as expected.
chrome.debugger.onEvent.addListener(async (source, method, params) => {
    if (method !== "Fetch.requestPaused") return;
    let tab_id = source.tabId;
    let pending_url = tab_pending_urls.get(tab_id);

    if (!OVERRIDE_FILE_PATH || !pending_url || params.request.url !== pending_url) {
        chrome.debugger.sendCommand(source, "Fetch.continueRequest", { requestId: params.requestId });
        return;
    }

    // Delete the pending URL so only this one request gets overridden.
    // We just want to override the main index.html of the page.
    tab_pending_urls.delete(tab_id);

    await ensure_offscreen_document();
    chrome.runtime.sendMessage({
        action: "read_override_file",
        file_path: OVERRIDE_FILE_PATH,
        request_id: params.requestId,
        tab_id: tab_id
    });
});
