// Our Z-index Orderer will lock the z-index of all browser windows we spawn. 
// This massively increases the amount of tabs we can spawn as the gui overlap problems
// for the os-level clicking are effectively negated. 

#![cfg(target_os = "windows")]

// Config for enforce z order loop rate.
const ENFORCE_Z_ORDER_RATE: u64 = 100;

use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use windows::Win32::{
    Foundation::{BOOL, HWND, LPARAM},
    UI::{
        WindowsAndMessaging::{
            EnumWindows, GetWindowTextLengthW, IsIconic, IsWindowVisible,
            SetWindowPos, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOSENDCHANGING,
        },
    },
};

use inputbot::KeybdKey;

// Note we need to implement unsafe Send and Sync for our hwnds, which have a *mut c_void.
// This is why we wrap them in a struct--because we can then give the struct these unsafe traits. 
// This will allow us to send the data to our loop/working threads: the z-order enforcement loop and the new page checker loop.
// Otherwise we'd catch an error. 
#[derive(Clone, Copy)]
struct SendHwnd(HWND);
unsafe impl Send for SendHwnd {}
unsafe impl Sync for SendHwnd {}

#[derive(Clone)]
struct TrackedWindow {
    hwnd: SendHwnd,
    // Insertion order for placement. lower -> newer -> higher up/more top.
    insertion_order: usize,
}

type WindowList = Arc<Mutex<Vec<TrackedWindow>>>;

// Find all visible windows in a list of hwnds,
// and return a list of hwnds for these windows.
fn snapshot_visible() -> Vec<SendHwnd> {
    let mut hwnds: Vec<SendHwnd> = Vec::new();

    unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let list = &mut *(lparam.0 as *mut Vec<SendHwnd>);
        if IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool() {
            if GetWindowTextLengthW(hwnd) > 0 {
                list.push(SendHwnd(hwnd));
            }
        }
        BOOL(1)
    }

    unsafe {
        let ptr = &mut hwnds as *mut Vec<SendHwnd> as isize;
        let _ = EnumWindows(Some(callback), LPARAM(ptr));
    }

    hwnds
}

// Sort z-index for each window so that it is below the z-index of the current (or z_prev + 1).
// We specifically insert it after it's predecessor.
// The setWindowPos method allows us to set the window position for a certain hwnd based on where we can to place it *after*, 
// which is why we take the previous entry. 
unsafe fn enforce_order(sorted: &[TrackedWindow]) {
    for i in (0..sorted.len()).rev() {
        let win = &sorted[i];
        if !IsWindowVisible(win.hwnd.0).as_bool() {
            continue;
        }
        let insert_after: HWND = if i == 0 { HWND_TOP } else { sorted[i - 1].hwnd.0 };
        SetWindowPos(
            win.hwnd.0,
            insert_after,
            0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOSENDCHANGING,
        )
        .ok();
    }
}

fn main() {
    // We store pre-existing windows and use this as a reference list of handles.
    let pre_existing: HashSet<isize> = snapshot_visible()
        .into_iter()
        .map(|h| h.0 .0 as isize)
        .collect();

    let tracked: WindowList = Arc::new(Mutex::new(Vec::new()));

    let enforcer_active = Arc::new(AtomicBool::new(false));
    let checker_enabled = Arc::new(AtomicBool::new(true));

    // We implement a global counter, so each new window gets the current value, then it decrements.
    // So the newest window always has the lowest number and sorts to the top.
    let next_order = Arc::new(AtomicUsize::new(usize::MAX));

    // Press "F7" to toggle the scanner on/off.
    // This can allow you to boost performance by turning it off once you've set up all pages.
    {
        let checker_enabled = Arc::clone(&checker_enabled);
        KeybdKey::F7Key.bind(move || {
            let prev = checker_enabled.fetch_xor(true, Ordering::Relaxed);
            println!(
                "[F7] New-page checker {}",
                if prev { "disabled" } else { "enabled" }
            );
        });
    }

    // Press "F9" to toggle the Z-order enforce loop on/off.
    // Useful for dealing with some buggy behavior and if you need to turn it off.
    {
        let enforcer_active = Arc::clone(&enforcer_active);
        KeybdKey::F9Key.bind(move || {
            let was = enforcer_active.fetch_xor(true, Ordering::SeqCst);
            println!(
                "Z-order enforcer has been {}",
                if !was { "ACTIVATED." } else { "PAUSED." }
            );
        });
    }

    thread::spawn(|| inputbot::handle_input_events());

    // Enforce z-ordering for all windows.
    {
        let tracked = Arc::clone(&tracked);
        let enforcer_active = Arc::clone(&enforcer_active);
        thread::spawn(move || loop {
            // Should be short while minimizing performance impact.
            thread::sleep(Duration::from_millis(ENFORCE_Z_ORDER_RATE));

            if !enforcer_active.load(Ordering::SeqCst) {
                continue;
            }

            let list = tracked.lock().unwrap();
            if list.is_empty() {
                continue;
            }

            let live: Vec<TrackedWindow> = list
                .iter()
                .filter(|w| unsafe { IsWindowVisible(w.hwnd.0).as_bool() })
                .cloned()
                .collect();

            if live.len() >= 2 {
                unsafe { enforce_order(&live) };
            }
        });
    }

    // Check for new windows.
    loop {
        thread::sleep(Duration::from_millis(10));

        // Only run this loop if the checker is enabled.
        if !checker_enabled.load(Ordering::Relaxed) {
            continue;
        }

        let current = snapshot_visible();
        let mut list = tracked.lock().unwrap();
        let known: HashSet<isize> = list.iter().map(|w| w.hwnd.0 .0 as isize).collect();

        // We must ensure pre existing windows are not included by this,
        // which we do by simply ensuring the hwnd isn't already found in the pre-existing windows list.
        let new_hwnds: Vec<SendHwnd> = current
            .iter()
            .copied()
            .filter(|h| {
                let hwnd = h.0 .0 as isize;
                !pre_existing.contains(&hwnd) && !known.contains(&hwnd)
            })
            .collect();

        for hwnd in new_hwnds {
            // We subtract one from our global order so each new page gets a lower insertion order--meaning it is more topmost.
            let insertion_order = next_order.fetch_sub(1, Ordering::Relaxed);
            list.push(TrackedWindow { hwnd, insertion_order });
        }

        // Sort by insertion_order in ascending order.
        list.sort_by_key(|w| w.insertion_order);
        list.retain(|w| unsafe { IsWindowVisible(w.hwnd.0).as_bool() });
    }
}
