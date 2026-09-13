#![cfg(target_os = "windows")]

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
    UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, IsIconic, IsWindowVisible,
        SetWindowPos, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOSENDCHANGING,
    },
};

use inputbot::KeybdKey;

#[derive(Clone, Copy)]
struct SendHwnd(HWND);
unsafe impl Send for SendHwnd {}
unsafe impl Sync for SendHwnd {}

#[derive(Clone)]
struct TrackedWindow {
    hwnd: SendHwnd,
    insertion_order: usize,
}

type WindowList = Arc<Mutex<Vec<TrackedWindow>>>;

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
    let pre_existing: HashSet<isize> = snapshot_visible()
        .into_iter()
        .map(|h| h.0 .0 as isize)
        .collect();

    let tracked: WindowList = Arc::new(Mutex::new(Vec::new()));
    let enforcer_active = Arc::new(AtomicBool::new(false));
    let next_order = Arc::new(AtomicUsize::new(usize::MAX));

    // Press F9 to toggle the Z-order enforce loop on/off.
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

    println!("Press F9 to activate the Z-order enforcer.");

    // Z-order enforcement loop.
    {
        let tracked = Arc::clone(&tracked);
        let enforcer_active = Arc::clone(&enforcer_active);
        thread::spawn(move || loop {
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

    // New-window checker loop (always runs).
    loop {
        thread::sleep(Duration::from_millis(10));

        let current = snapshot_visible();
        let mut list = tracked.lock().unwrap();
        let known: HashSet<isize> = list.iter().map(|w| w.hwnd.0 .0 as isize).collect();

        let new_hwnds: Vec<SendHwnd> = current
            .iter()
            .copied()
            .filter(|h| {
                let hwnd = h.0 .0 as isize;
                !pre_existing.contains(&hwnd) && !known.contains(&hwnd)
            })
            .collect();

        for hwnd in new_hwnds {
            let insertion_order = next_order.fetch_sub(1, Ordering::Relaxed);
            list.push(TrackedWindow { hwnd, insertion_order });
        }

        list.sort_by_key(|w| w.insertion_order);
        list.retain(|w| unsafe { IsWindowVisible(w.hwnd.0).as_bool() });
    }
}