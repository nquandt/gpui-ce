#![cfg(target_os = "ios")]

//! `ios_gallery` — a feature-gallery test app for the `gpui_mobile` iOS
//! backend. One screen per feature area, a pass/fail verdict + notes
//! recorded per screen (persisted via shared preferences), and a Report
//! screen that exports everything as markdown. See `src/screens/mod.rs`
//! for the screen registry.

pub mod log;
pub mod screens;
pub mod shell;
pub mod store;

use gpui::{App, WindowOptions, prelude::*};
use shell::Gallery;

/// C entry point called from the iOS app delegate after `gpui_ios_initialize()`.
#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_gallery_main() {
    // Launch-argument automation hook: `xcrun simctl launch booted
    // com.gpuice.iosgallery --screen buttons` opens directly to a screen.
    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--screen")
        && let Some(screen_id) = args.get(pos + 1)
    {
        shell::request_navigation(screen_id.clone());
    }

    // Deep links (`gpuigallery://screen/<id>`) can arrive before the app
    // callback runs (cold launch) or later while the app is foregrounded;
    // both paths funnel through `shell::request_navigation`, which is safe
    // to call from any thread.
    gpui_mobile::packages::deeplink::set_deep_link_handler(|url| {
        if let Some(id) = screen_id_from_url(url) {
            shell::request_navigation(id);
        }
    });
    if let Ok(Some(initial)) = gpui_mobile::packages::deeplink::get_initial_link()
        && let Some(id) = screen_id_from_url(&initial)
    {
        shell::request_navigation(id);
    }

    gpui_mobile::ios::ffi::set_app_callback(Box::new(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |window, cx| {
            cx.new(|cx| Gallery::new(window, cx))
        })
        .expect("failed to open gallery window");
        cx.activate(true);
    }));

    gpui_mobile::ios::ffi::gpui_ios_run_demo();
}

/// Parse `gpuigallery://screen/<id>` into `<id>`.
fn screen_id_from_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix("gpuigallery://screen/")?;
    let id = rest.split(['/', '?', '#']).next()?;
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}
