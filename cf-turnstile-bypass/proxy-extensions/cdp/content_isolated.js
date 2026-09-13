// Request our payload from the background and forward it to the main context.
(function request_inject_payload(attempt) {
    const MAX_RETRIES = 10;
    const RETRY_DELAY_MS = 150;

    chrome.runtime.sendMessage({ action: "get_inject_payload" }, (payload) => {
        if (chrome.runtime.lastError) {
            if (attempt < MAX_RETRIES) {
                setTimeout(() => request_inject_payload(attempt + 1), RETRY_DELAY_MS);
            }
            return;
        }

        if (payload && (payload.inject_config || payload.proxies)) {
            window.postMessage({ type: "SET_LOCALSTORAGE_INJECT", payload }, "*");
            return;
        }

        if (attempt < MAX_RETRIES) {
            setTimeout(() => request_inject_payload(attempt + 1), RETRY_DELAY_MS);
        }
    });
})(0);

window.addEventListener("message", (event) => {
    // Data must come from our own webpage.
    if (event.source != window || !event.data) return;

    // Forward the requested proxy to the background.
    if (event.data.type == "FORWARD_TO_BACKGROUND" || event.data.type == "FORWARD_SETUP_PROXY") {
        chrome.runtime.sendMessage({
            action: "setup_proxy",
            proxy_details: event.data.proxy_details
        }, (response) => {
            if (response && response.success) {
                // Return message to the page telling the client the proxy is ready.
                // This effectively gives us an await on a promise or a mini-lock,
                // so we can wait for this to return before resuming execution of a task--
                // allowing us to await and connect to proxies.
                window.postMessage({ type: "PROXY_READY" }, "*");
            }
        });
    }
});
