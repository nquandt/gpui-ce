use gpui::{Bounds, Pixels, SharedString};

#[allow(dead_code)]
pub(crate) trait PlatformWebView {
    fn set_bounds(&self, bounds: Bounds<Pixels>);
    fn set_visible(&self, visible: bool);
    fn load_url(&self, url: &str);
    fn load_html(&self, html: &str);
    fn go_back(&self);
    fn go_forward(&self);
    fn reload(&self);
    fn stop_loading(&self);
    fn evaluate_javascript(
        &self,
        script: &str,
        callback: Option<Box<dyn FnOnce(Result<String, String>) + Send>>,
    );
    fn title(&self) -> SharedString;
    fn url(&self) -> SharedString;
    fn set_devtools_enabled(&self, enabled: bool);
    fn focus(&self);
    /// Replace the navigation-interception callback. Called every prepaint
    /// with the latest closure from `WebView::on_navigation`, so the native
    /// handler (registered once at webview creation) always calls through to
    /// the current frame's callback.
    fn set_navigation_handler(&self, handler: Option<Box<dyn FnMut(&str) -> bool>>);
    /// Downcast escape hatch so `WebViewHandle::native()` can reach the
    /// concrete backend (e.g. `wry::WebView`) for APIs this crate doesn't
    /// wrap yet.
    fn as_any(&self) -> &dyn std::any::Any;
    /// Drain and return IPC messages sent from JS via `window.ipc.postMessage`.
    fn take_ipc_messages(&self) -> Vec<String>;
}

#[cfg(feature = "webview")]
mod wry_backend;

#[cfg(feature = "webview")]
pub(crate) use wry_backend::*;

#[cfg(not(feature = "webview"))]
mod stub;

#[cfg(not(feature = "webview"))]
pub(crate) use stub::*;
