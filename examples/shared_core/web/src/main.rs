//! Web host: wasm32 in the browser.
//!
//! The browser owns the event loop, so the platform's `run` returns at once.
//! `run_embedded` returns a handle that keeps the app alive; it is leaked so
//! the app lives as long as the page. `application_with_web_backend` also
//! installs the fetch HTTP client and the `localStorage` key-value store.

use gpui::App;

fn main() {
    gpui_platform::web_init();
    let handle =
        gpui_platform::application_with_web_backend(gpui_platform::WebBackendPreference::Auto)
            .with_assets(app_core::Assets)
            .run_embedded(|cx: &mut App| {
                app_core::init(cx);
                app_core::open_main_window(cx);
            });
    std::mem::forget(handle);
}
