// Searches for popular CDP-compatible browser binaries and for each one found,
// generates dedicated anonymous profiles preloaded with a (given via CL) unpacked extension.
// Results are written to ./browser_binaries.txt in the format:
// key: command --args
// Keep in mind the custom clean profile with only the unpacked extension
// may not work for all browsers (e.g. Chrome) but allows you to automate most of them.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone)]
struct BrowserDef {
    key: &'static str,
    display: &'static str,
    candidates: &'static [&'static str],
    extra_args: &'static [&'static str],
    profile_flag: &'static str,
}

// Just store a bunch of browser definitions with
// data like args to run for them, paths to look in, etc.
fn browser_definitions() -> Vec<BrowserDef> {
    vec![
        BrowserDef {
            key: "chrome",
            display: "Google Chrome",
            candidates: &[
                "/usr/bin/google-chrome",
                "/usr/bin/google-chrome-stable",
                "/usr/bin/chromium",
                "/usr/bin/chromium-browser",
                "/snap/bin/chromium",
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                r"C:\Program Files\Google\Chrome\Application\chrome.exe",
                r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
            ],
            extra_args: &["--remote-debugging-port=0", "--no-first-run", "--no-default-browser-check"],
            profile_flag: "--user-data-dir=",
        },
        BrowserDef {
            key: "edge",
            display: "Microsoft Edge",
            candidates: &[
                "/usr/bin/microsoft-edge",
                "/usr/bin/microsoft-edge-stable",
                "/usr/bin/microsoft-edge-beta",
                "/usr/bin/microsoft-edge-dev",
                "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
                r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
                r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            ],
            extra_args: &["--remote-debugging-port=0", "--no-first-run", "--no-default-browser-check"],
            profile_flag: "--user-data-dir=",
        },
        BrowserDef {
            key: "brave",
            display: "Brave Browser",
            candidates: &[
                "/usr/bin/brave-browser",
                "/usr/bin/brave-browser-stable",
                "/usr/bin/brave",
                "/snap/bin/brave",
                "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
                r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe",
                r"C:\Program Files (x86)\BraveSoftware\Brave-Browser\Application\brave.exe",
            ],
            extra_args: &["--remote-debugging-port=0", "--no-first-run", "--no-default-browser-check"],
            profile_flag: "--user-data-dir=",
        },
        BrowserDef {
            key: "opera",
            display: "Opera",
            candidates: &[
                "/usr/bin/opera",
                "/usr/lib/x86_64-linux-gnu/opera/opera",
                "/snap/bin/opera",
                "/Applications/Opera.app/Contents/MacOS/Opera",
                r"C:\Program Files\Opera\opera.exe",
                r"C:\Program Files (x86)\Opera\opera.exe",
                r"C:\Users\%USERNAME%\AppData\Local\Programs\Opera\opera.exe",
            ],
            extra_args: &["--remote-debugging-port=0", "--no-first-run", "--no-default-browser-check"],
            profile_flag: "--user-data-dir=",
        },
        BrowserDef {
            key: "operagx",
            display: "Opera GX",
            candidates: &[
                "/usr/bin/opera-gx",
                "/Applications/Opera GX.app/Contents/MacOS/Opera GX",
                r"C:\Users\%USERNAME%\AppData\Local\Programs\Opera GX\opera.exe",
                r"C:\Program Files\Opera GX\opera.exe",
            ],
            extra_args: &["--remote-debugging-port=0", "--no-first-run", "--no-default-browser-check"],
            profile_flag: "--user-data-dir=",
        },
        BrowserDef {
            key: "vivaldi",
            display: "Vivaldi",
            candidates: &[
                "/usr/bin/vivaldi",
                "/usr/bin/vivaldi-stable",
                "/snap/bin/vivaldi",
                "/Applications/Vivaldi.app/Contents/MacOS/Vivaldi",
                r"C:\Program Files\Vivaldi\Application\vivaldi.exe",
                r"C:\Program Files (x86)\Vivaldi\Application\vivaldi.exe",
            ],
            extra_args: &["--remote-debugging-port=0", "--no-first-run", "--no-default-browser-check"],
            profile_flag: "--user-data-dir=",
        },
        BrowserDef {
            key: "arc",
            display: "Arc Browser",
            candidates: &[
                "/Applications/Arc.app/Contents/MacOS/Arc",
                r"C:\Users\%USERNAME%\AppData\Local\Programs\Arc\Arc.exe",
            ],
            extra_args: &["--remote-debugging-port=0", "--no-first-run", "--no-default-browser-check"],
            profile_flag: "--user-data-dir=",
        },
        BrowserDef {
            key: "chromium",
            display: "Chromium",
            candidates: &[
                "/usr/bin/chromium",
                "/usr/bin/chromium-browser",
                "/snap/bin/chromium",
                "/Applications/Chromium.app/Contents/MacOS/Chromium",
            ],
            extra_args: &["--remote-debugging-port=0", "--no-first-run", "--no-default-browser-check"],
            profile_flag: "--user-data-dir=",
        },
        BrowserDef {
            key: "thorium",
            display: "Thorium",
            candidates: &[
                "/usr/bin/thorium-browser",
                "/opt/thorium/thorium-browser",
                "/Applications/Thorium.app/Contents/MacOS/Thorium",
                r"C:\Program Files\Thorium\thorium.exe",
            ],
            extra_args: &["--remote-debugging-port=0", "--no-first-run", "--no-default-browser-check"],
            profile_flag: "--user-data-dir=",
        },
        BrowserDef {
            key: "ungoogled",
            display: "Ungoogled Chromium",
            candidates: &[
                "/usr/bin/ungoogled-chromium",
                "/opt/ungoogled-chromium/chrome",
                "/Applications/Ungoogled Chromium.app/Contents/MacOS/Chromium",
                r"C:\Program Files\Ungoogled Chromium\chrome.exe",
            ],
            extra_args: &["--remote-debugging-port=0", "--no-first-run", "--no-default-browser-check"],
            profile_flag: "--user-data-dir=",
        },
    ]
}

fn expand_path(raw: &str) -> PathBuf {
    let s = if let Some(stripped) = raw.strip_prefix('~') {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_default();
        format!("{}{}", home, stripped)
    } else {
        raw.to_string()
    };

    #[cfg(target_os = "windows")]
    let s = {
        let mut result = s.clone();
        for (key, val) in std::env::vars() {
            result = result.replace(&format!("%{}%", key), &val);
        }
        result
    };

    PathBuf::from(s)
}

fn binary_exists(path: &str) -> bool {
    let p = expand_path(path);
    p.exists() && p.is_file()
}

// Each key gets its own fresh, otherwise-empty profile directory (created once,
// reused on every future run) so the loaded extension's storage and granted
// permissions persist between launches without using a real browser
// profile. For systems like Recaptcha, this is obviously unviable as they look at account
// data and cookies and that sort of stuff.
fn ensure_profile_dir(key: &str) -> PathBuf {
    let cwd = std::env::current_dir().expect("failed to get current directory");
    let dir = cwd.join("browser_profiles").join(key);
    fs::create_dir_all(&dir).expect("failed to create profile directory");
    dir
}

// Build the shell command that gets written to ./browser_binaries.txt,
// so we can load these browsers via shorthand names for main.rs
fn build_command(binary: &str, def: &BrowserDef, key: &str, ext_path: &str) -> String {
    let mut parts: Vec<String> = vec![format!("\"{}\"", binary)];

    if !def.profile_flag.is_empty() {
        let profile_dir = ensure_profile_dir(key);
        parts.push(format!("\"{}{}\"", def.profile_flag, profile_dir.to_string_lossy()));
    }

    parts.push(format!("\"--load-extension={}\"", ext_path));

    for arg in def.extra_args {
        parts.push(arg.to_string());
    }
    parts.join(" ")
}

fn resolve_extension_path(raw: &str) -> PathBuf {
    let expanded = expand_path(raw);
    match fs::canonicalize(&expanded) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("Extension path does not exist: {}", expanded.display());
            std::process::exit(1);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage: find_browsers <browser> <count> [<browser> <count> ...] <ext_path>");
        println!("Example: find_browsers edge 3 chrome 2 C:\\ext\\my_extension");
        return;
    }

    let ext_path = resolve_extension_path(args.last().unwrap());
    let ext_path = ext_path.to_string_lossy().to_string();

    let pair_args = &args[1..args.len() - 1];
    if pair_args.len() % 2 != 0 {
        eprintln!("Error: browser/count args must come in pairs (e.g. edge 3 chrome 2).");
        std::process::exit(1);
    }

    // Parse the profile creation requests into (browser_key, count).
    let mut requests: Vec<(&str, u32)> = Vec::new();
    let mut i = 0;
    while i < pair_args.len() {
        let browser = pair_args[i].as_str();
        let count: u32 = match pair_args[i + 1].parse() {
            Ok(n) if n > 0 => n,
            _ => {
                eprintln!("Error: expected a positive count after '{}', got '{}'.", browser, pair_args[i + 1]);
                std::process::exit(1);
            }
        };
        requests.push((browser, count));
        i += 2;
    }

    // Wipe the profiles directory on each run so every browser gets a truly
    // fresh profile, the same way browser_binaries.txt is regenerated from scratch.
    let profiles_dir = std::env::current_dir()
        .expect("failed to get current directory")
        .join("browser_profiles");
    if profiles_dir.exists() {
        fs::remove_dir_all(&profiles_dir).expect("failed to clear browser_profiles directory");
    }

    let defs = browser_definitions();
    let def_map: std::collections::HashMap<&str, &BrowserDef> =
        defs.iter().map(|d| (d.key, d)).collect();

    let mut entries: Vec<(String, String)> = Vec::new();

    for (browser_key, count) in &requests {
        let Some(def) = def_map.get(browser_key) else {
            eprintln!("Unknown browser key '{}'. Known keys: {}.",
                browser_key,
                defs.iter().map(|d| d.key).collect::<Vec<_>>().join(", "));
            continue;
        };

        let Some(binary) = def.candidates.iter().find(|c| binary_exists(c)) else {
            eprintln!("Browser '{}' not found on this system.", browser_key);
            continue;
        };

        println!("Found {} at {}, creating {} profile(s).", def.display, binary, count);

        for i in 1..=*count {
            let key = format!("{}{}", def.key, i);
            let cmd = build_command(binary, def, &key, &ext_path);
            entries.push((key, cmd));
        }
    }

    if entries.is_empty() {
        eprintln!("No entries generated; nothing written.");
        std::process::exit(1);
    }

    let output_path = "./browser_binaries.txt";
    let mut file = fs::File::create(output_path).expect("Failed to create browser_binaries.txt");
    for (key, cmd) in &entries {
        writeln!(file, "{}: {}", key, cmd).expect("Failed to write to browser_binaries.txt");
    }
    println!("Wrote {} entries to {}.", entries.len(), output_path);
}