chrome.runtime.onMessage.addListener((message, sender, send_response) => {
    // Read the inject config file text,
    // and send back to the background.
    if (message.action == "read_inject_config_file") {
        let { file_path } = message;

        // Normalize the path into a file:// URL.
        let file_url = file_path;
        if (!file_url.startsWith("file://")) {
            let normalized = file_path.replace(/\\/g, "/");
            file_url = normalized.startsWith("/") ? "file://" + normalized : "file:///" + normalized;
        }

        fetch(file_url)
        .then((res) => {
            if (!res.ok) throw new Error("HTTP " + res.status);
            return res.text();
        })
        .then((content) => {
            chrome.runtime.sendMessage({ action: "inject_config_file_content", content });
        })
        .catch((err) => {
            chrome.runtime.sendMessage({ action: "inject_config_file_content", content: null, error: err.message });
        });

        return false;
    }

    if (message.action != "read_override_file") return false;

    let { file_path, request_id, tab_id } = message;

    // Normalize the path into a file:// URL.
    let file_url = file_path;
    if (!file_url.startsWith("file://")) {
        let normalized = file_path.replace(/\\/g, "/");
        file_url = normalized.startsWith("/") ? "file://" + normalized : "file:///" + normalized;
    }

    fetch(file_url)
    .then((res) => {
        if (!res.ok) throw new Error("HTTP " + res.status);
        return res.text();
    })
    .then((content) => {
        chrome.runtime.sendMessage({
            action: "override_file_content",
            request_id,
            tab_id,
            content
        });
    })
    .catch((err) => {
        chrome.runtime.sendMessage({
            action: "override_file_content",
            request_id,
            tab_id,
            content: null,
            error: err.message
        });
    });

    return false;
});