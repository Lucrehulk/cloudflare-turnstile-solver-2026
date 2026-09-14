let _inject_config_written = false;
let _proxies_written = false;

// Native toString spoof helper.
let native_to_string = function toString() {
    return "function toString() { [native code] }";
};

// Generate hidden symbol references for anonymity.
const SPOOF_SYMBOL = Symbol("spoof_indicator");
const STORE_SYMBOL = Symbol("spoof_store");

// Store field overrides on window using a non-enumerable descriptor.
if (!window[STORE_SYMBOL]) {
    Object.defineProperty(window, STORE_SYMBOL, {
        value: {},
        writable: false,
        enumerable: false,
        configurable: false
    });
}

window.addEventListener("message", (event) => {
    // Data must come from our own webpage.
    if (event.source != window || !event.data) return;

    if (event.data.type == "SET_LOCALSTORAGE_INJECT") {
        try {
            let payload = event.data.payload;

            if (payload.inject_config && !_inject_config_written) {
                _inject_config_written = true;
                payload.inject_config.split("\n").forEach((line) => {
                    line = line.trim();
                    if (!line) return;
                    let idx = line.indexOf(":");
                    if (idx == -1) return;
                    let key = line.slice(0, idx).trim().toLowerCase();
                    let value = line.slice(idx + 1).trim();
                    if (key) localStorage[key] = value;
                });
            }

            if (payload.proxies != null && !_proxies_written) {
                _proxies_written = true;
                localStorage.proxies = payload.proxies;
            }
        } catch (e) {}
        return;
    }

    if (event.data.type == "SET_TAB_PROXY") {
        // Spoof JS fields by injecting a script overriding the fields into the page.
        if (event.data.js_API_spoof_fields) {
            let fields = event.data.js_API_spoof_fields;
            let data_store = window[STORE_SYMBOL];

            // Parse strings into primitive types if applicable.
            for (let key in fields) {
                let val = fields[key];
                if (val === "true") fields[key] = true;
                else if (val === "false") fields[key] = false;
                else if (typeof val === "string" && val.trim() != "" && !isNaN(Number(val))) fields[key] = Number(val);
            }

            for (let key in fields) {
                data_store[key] = fields[key];

                let parts = key.split(".");
                let prop = parts.pop();
                let obj = window;

                for (let i = 0; i < parts.length; i++) {
                    if (!obj[parts[i]]) {
                        obj[parts[i]] = {};
                    }
                    obj = obj[parts[i]];
                }

                // Locate the true prototype object to ensure correct application.
                let proto = obj;
                let descriptor = null;
                while (proto && !descriptor) {
                    descriptor = Object.getOwnPropertyDescriptor(proto, prop);
                    if (!descriptor) proto = Object.getPrototypeOf(proto);
                }
                let target_obj = proto || obj;

                // Check if we have already locked this property on a previous proxy setup using our hidden Symbol.
                if (!descriptor || !descriptor.get || !descriptor.get[SPOOF_SYMBOL]) {
                    let mock_get = function () { return window[STORE_SYMBOL][key]; };
                    let mock_set = function () {};

                    let native_get_to_string = function () { return "function get " + prop + "() { [native code] }"; };
                    let native_set_to_string = function () { return "function set " + prop + "() { [native code] }"; };

                    // Set string spoofs.
                    Object.defineProperty(mock_get, 'toString', { value: native_get_to_string, configurable: true, writable: true });
                    Object.defineProperty(mock_set, 'toString', { value: native_set_to_string, configurable: true, writable: true });
                    Object.defineProperty(mock_get.toString, 'toString', { value: native_to_string, configurable: true, writable: true });
                    Object.defineProperty(mock_set.toString, 'toString', { value: native_to_string, configurable: true, writable: true });

                    mock_get[SPOOF_SYMBOL] = true;

                    Object.defineProperty(target_obj, prop, {
                        get: mock_get,
                        set: mock_set,
                        configurable: true,
                        enumerable: descriptor ? descriptor.enumerable : true
                    });
                }
            }

            // matchMedia protection implementation. Triggers if spoofed window inner dimensions are set.
            if (data_store['window.innerWidth'] != undefined || data_store['window.innerHeight'] != undefined) {
                let orig_matchMedia = window.matchMedia;

                if (!orig_matchMedia[SPOOF_SYMBOL]) {
                    let mock_matchMedia = function matchMedia(query) {
                        let mql = orig_matchMedia.call(this, query);

                        let w = window[STORE_SYMBOL]['window.innerWidth'];
                        let h = window[STORE_SYMBOL]['window.innerHeight'];

                        let is_match = true;
                        let has_dimension_check = false;

                        // Parse standard pixel dimension queries.
                        let w_match = query.match(/(min-width|max-width|width)\s*:\s*(\d+)px/);
                        if (w_match) {
                            has_dimension_check = true;
                            let type = w_match[1];
                            let val = parseInt(w_match[2], 10);
                            if (type == 'width' && w != val) is_match = false;
                            if (type == 'min-width' && w < val) is_match = false;
                            if (type == 'max-width' && w > val) is_match = false;
                        }

                        let h_match = query.match(/(min-height|max-height|height)\s*:\s*(\d+)px/);
                        if (h_match) {
                            has_dimension_check = true;
                            let type = h_match[1];
                            let val = parseInt(h_match[2], 10);
                            if (type == 'height' && h != val) is_match = false;
                            if (type == 'min-height' && h < val) is_match = false;
                            if (type == 'max-height' && h > val) is_match = false;
                        }

                        // Override match result IF it is a dimension check.
                        if (has_dimension_check) {
                            Object.defineProperty(mql, 'matches', {
                                get: function() { return is_match; },
                                configurable: true,
                                enumerable: true
                            });
                        }

                        return mql;
                    };

                    let native_matchMedia_to_string = function () { return "function matchMedia() { [native code] }"; };
                    Object.defineProperty(mock_matchMedia, 'toString', { value: native_matchMedia_to_string, configurable: true, writable: true });
                    Object.defineProperty(mock_matchMedia.toString, 'toString', { value: native_to_string, configurable: true, writable: true });

                    mock_matchMedia[SPOOF_SYMBOL] = true;

                    Object.defineProperty(window, 'matchMedia', {
                        value: mock_matchMedia,
                        configurable: true,
                        writable: true
                    });
                }
            }
        }

        // Forward the requested proxy to the background.
        window.postMessage({
            type: "FORWARD_TO_BACKGROUND",
            proxy_details: event.data.proxy_details
        }, "*");
    }
});
