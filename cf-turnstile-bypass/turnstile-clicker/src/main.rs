// Cloudflare turnstile checkbox clicker. 

use std::{
    collections::VecDeque,
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
    thread,
    time::Duration,
};

use enigo::{Button, Coordinate, Enigo, Mouse, Settings};
use inputbot::KeybdKey;
use screenshots::Screen;
use rand::Rng;

// Some config to aid with detection.
// Note that though these are decent config values, depending on your specific graphics rendering these could require adjustment.
// You'll have to check for yourself, you can run this on any website with turnstile and see what config you need to detect the checkbox.

// Minimum area of the interior checkbox rect.
const MIN_INTERIOR_AREA: u32 = 200;
// Maximum area of the interior checkbox rect.
const MAX_INTERIOR_AREA: u32 = 200000;
// Border color greyscale target. For example, 74 -> RGB(74, 74, 74) is the target color for the checkbox border.
const BORDER_TARGET: u8 = 74;
// Border tolerance to ensure it can still be detected.
const BORDER_TOLERANCE: u8 = 30;
// The maximum variance between R, G, and B fields for the border, this ensures we maintain greyscale.
const BORDER_BALANCE: u8 = 20;
// Minimum amount of connected pixels detected within our previous greyscale config parameters that can be used to declare a checkbox border.
const MIN_BORDER_PIXELS: usize = 50;
// White interior general target pixel color -> RGB(target, target, target).
const WHITE_TARGET: u8 = 255;
// White tolerance.
const WHITE_TOLERANCE: u8 = 25;
// White balance.
const WHITE_BALANCE: u8 = 20;
// This infobox safety avoideance shift is a hacky solution I came up to fix an issue.
// Basically, when the verifying buffer on the turnstile widget is active, it can activate
// one of those html infoboxes for the element, which is actually detected by the checkbox clicker.
// To deal with this, I just made it so you can shift your mouse below a certain amount so that 
// the infobox doesbn't trigger. That's all this is.
// This was an issue because basically the infobox was just so well placed that you could
// Accidently click on one of the hrefs in the turnstile widget (there are Privacy and Help tabs),
// And this would cause you to go off the page. So this just fixes that.
const INFOBOX_AVOIDANCE_SAFETY_SHIFT: i32 = 20;

#[derive(Debug, Clone)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

// Pair a detected checkbox rect with its screen's DPI scale factor so click
// coordinates can be converted from image pixels to logical screen coordinates.
#[derive(Debug, Clone)]
pub struct ScaledRect {
    pub rect: Rect,
    pub scale: f32,
}

fn main() {
    let active = Arc::new(AtomicBool::new(false));
    let active_clone = Arc::clone(&active);

    // Press "F8" to toggle the scanner on/off for safety.
    KeybdKey::F8Key.bind(move || {
        let was = active_clone.fetch_xor(true, Ordering::SeqCst);
        println!(
            "Auto-clicker has been {}",
            if !was { "ACTIVATED." } else { "PAUSED." }
        );
    });

    // Inputbot's event loop must run on its own thread.
    thread::spawn(|| inputbot::handle_input_events());

    let mut enigo = Enigo::new(&Settings::default()).expect("Failed to create Enigo");

    println!("Press F8 to activate the auto-clicker.");

    loop {
        if !active.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(50));
            continue;
        }

        let queue = detect_checkboxes();
        println!("Detected {} checkbox(es)", queue.len());

        for scaled in &queue {
            // If active is false, the toggle will just stop us from clicking anything--but it'll still keep detecting checkboxes.
            if !active.load(Ordering::SeqCst) { break };

            // Divide image pixel coordinates by the DPI scale factor to convert them into
            // the logical screen coordinates that enigo expects.
            let mut rand = rand::thread_rng();
            let click_x = ((scaled.rect.x as f32 + scaled.rect.width as f32 * rand.gen::<f32>()) / scaled.scale) as i32;
            let click_y = ((scaled.rect.y as f32 + scaled.rect.height as f32 * rand.gen::<f32>()) / scaled.scale) as i32;

            enigo.move_mouse(click_x, click_y, Coordinate::Abs).expect("move_mouse failed");
            enigo.button(Button::Left, enigo::Direction::Click).expect("click failed");
            enigo.move_mouse(click_x, click_y + INFOBOX_AVOIDANCE_SAFETY_SHIFT, Coordinate::Abs).expect("move_mouse failed");

            thread::sleep(Duration::from_millis(50));
        }

        thread::sleep(Duration::from_millis(250));
    }
}

pub fn detect_checkboxes() -> VecDeque<ScaledRect> {
    let mut queue = VecDeque::new();

    let screens = match Screen::all() {
        Ok(s) => s,
        Err(e) => { eprintln!("Screen::all failed: {e}"); return queue },
    };

    for screen in screens {
        // Read the screen's DPI scale factor before capturing so we can attach
        // it to every rect found on this screen.
        let scale = screen.display_info.scale_factor;

        let image = match screen.capture() {
            Ok(img) => img,
            Err(e) => { eprintln!("capture failed: {e}"); continue },
        };

        let width = image.width();
        let height = image.height();
        let mut visited = vec![false; (width * height) as usize];

        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                if visited[idx] { continue };

                // Continue searching for a border color pixel.
                if !is_border(*image.get_pixel(x, y)) { continue };

                // If we find a border pixel, attempt to identify a connected ring, 
                // and check if the total pixel count is less than the min border pixels.
                let (count, outer) = find_border_outline(&image, &mut visited, x, y, width, height);

                if count < MIN_BORDER_PIXELS { continue };

                // Once we've found our border identify the interior from such, and do some checks.
                let Some(interior) = find_interior(&image, &outer) else { continue };

                // Ensure the interior area isn't too large.
                let area = interior.width * interior.height;
                if area < MIN_INTERIOR_AREA || area > MAX_INTERIOR_AREA { continue };

                // Wrap the rect and its screen scale factor together before queuing.
                queue.push_back(ScaledRect { rect: interior, scale });
            }
        }
    }

    queue
}

// Check if our given RGB color matches a border pixel based on the limitations set within our config.
fn is_border(p: image::Rgba<u8>) -> bool {
    let (r, g, b) = (p[0], p[1], p[2]);
    let near = |ch: u8| (ch as i16 - BORDER_TARGET as i16).unsigned_abs() <= BORDER_TOLERANCE as u16;
    if !near(r) || !near(g) || !near(b) { return false };
    let spread = r.max(g).max(b) - r.min(g).min(b);
    spread <= BORDER_BALANCE
}

// Check if our given RGB color matches a white interior pixel, just like is_border does for the border color.
fn is_white(p: image::Rgba<u8>) -> bool {
    let (r, g, b) = (p[0], p[1], p[2]);
    let near = |ch: u8| (ch as i16 - WHITE_TARGET as i16).unsigned_abs() <= WHITE_TOLERANCE as u16;
    if !near(r) || !near(g) || !near(b) { return false };
    let spread = r.max(g).max(b) - r.min(g).min(b);
    spread <= WHITE_BALANCE
}

// Solves for and identifies the border outline and pixels by continually branching out and searching for connected border pixels,
// in a DFS until the path either ends or all pixels have been visited (reconnects).
fn find_border_outline(
    image: &image::RgbaImage,
    visited: &mut [bool],
    start_x: u32,
    start_y: u32,
    width: u32,
    height: u32,
) -> (usize, Rect) {
    let mut stack = vec![(start_x, start_y)];
    let mut count = 0;
    let mut min_x = start_x;
    let mut min_y = start_y;
    let mut max_x = start_x;
    let mut max_y = start_y;

    // Pop off each grey pixel
    while let Some((x, y)) = stack.pop() {
        if x >= width || y >= height { continue };
        let idx = (y * width + x) as usize;
        // If already visited (ring completed or turned back) OR it's not a border pixel anymore (thus not connected),
        // Then quit skip out.
        if visited[idx] || !is_border(*image.get_pixel(x, y)) { continue };

        visited[idx] = true;
        count += 1;

        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);

        // Push out children in each of the four directions for new branches to test the closed loop.
        if x > 0 { stack.push((x - 1, y)) };
        if x + 1 < width { stack.push((x + 1, y)) };
        if y > 0 { stack.push((x, y - 1)) };
        if y + 1 < height { stack.push((x, y + 1)) };
    }

    (count, Rect { x: min_x, y: min_y, width: max_x - min_x + 1, height: max_y - min_y + 1 })
}

// Finally, identify the interior area after we've identified our border. 
fn find_interior(image: &image::RgbaImage, outer: &Rect) -> Option<Rect> {
    let width = image.width();
    let height = image.height();

    let center_x = outer.x + outer.width / 2;
    let center_y = outer.y + outer.height / 2;
    if center_x >= width || center_y >= height { return None };

    // We scan over from the left side of the rectangle to the right until we find where we're no longer at a border,
    // yielding our left inner bound.
    let inner_left = (outer.x..outer.x + outer.width).find(|&x| x < width && !is_border(*image.get_pixel(x, center_y)))?;
    // Verify that this pixel is actually white and not just some other stray non-border color (noise, an icon, text, etc).
    // If it isn't white, this isn't a checkbox and we abort. We run this for the other sides too--all four sides must have white interiors.
    if !is_white(*image.get_pixel(inner_left, center_y)) { return None };

    // We perform the same scan as before, but reverse it, meaning the first white pixel is now the furthest right, 
    // or hence this is our inner right border.
    let inner_right = (outer.x..outer.x + outer.width).rev().find(|&x| x < width && !is_border(*image.get_pixel(x, center_y)))?;
    if !is_white(*image.get_pixel(inner_right, center_y)) { return None };

    // Applying the same idea to the y-axis, we now scan from the top y all the way to the bottom y, 
    // and our first indice that is white is our inner border at the top.
    let inner_top = (outer.y..outer.y + outer.height).find(|&y| y < height && !is_border(*image.get_pixel(center_x, y)))?;
    if !is_white(*image.get_pixel(center_x, inner_top)) { return None };

    // Finally, to get the bottom border we do the same trick as we did with the right side and reverse the top search,
    // so that the first returned item is now the white pixel closest to the bottom.
    let inner_bottom = (outer.y..outer.y + outer.height).rev().find(|&y| y < height && !is_border(*image.get_pixel(center_x, y)))?;
    if !is_white(*image.get_pixel(center_x, inner_bottom)) { return None };

    Some(Rect {
        x: inner_left,
        y: inner_top,
        width: inner_right - inner_left + 1,
        height: inner_bottom - inner_top + 1
    })
}
