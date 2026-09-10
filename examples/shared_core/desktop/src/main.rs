//! Desktop host: Windows, macOS and Linux.
//!
//! `gpui_platform::application()` picks the native backend, installs a
//! file-backed key-value store named after this executable, and owns the run
//! loop, so `run` blocks until the app exits.

use gpui::App;

fn main() {
    env_logger::init();
    gpui_platform::application()
        .with_assets(app_core::Assets)
        .run(|cx: &mut App| {
            app_core::init(cx);
            app_core::open_main_window(cx);
        });
}
