use gpui::SharedString;
use serde::Serialize;
use std::rc::Rc;

use crate::ipc::{self, IpcRequest, IpcResult};
use crate::platform::PlatformWebView;

/// A handle for controlling a [`WebView`] after creation.
///
/// Obtained via the `on_create_handle` callback or by querying the element tree.
/// This handle is cloneable and can be used to control the webview from anywhere
/// in your application.
#[derive(Clone)]
pub struct WebViewHandle {
    inner: Rc<dyn PlatformWebView>,
}

impl WebViewHandle {
    pub(crate) fn new(inner: Rc<dyn PlatformWebView>) -> Self {
        Self { inner }
    }

    /// Navigate to a URL.
    pub fn load_url(&self, url: &str) {
        self.inner.load_url(url);
    }

    /// Navigate to a data URL with HTML content.
    pub fn load_html(&self, html: &str) {
        self.inner.load_html(html);
    }

    /// Navigate back.
    pub fn go_back(&self) {
        self.inner.go_back();
    }

    /// Navigate forward.
    pub fn go_forward(&self) {
        self.inner.go_forward();
    }

    /// Reload the current page.
    pub fn reload(&self) {
        self.inner.reload();
    }

    /// Stop loading.
    pub fn stop_loading(&self) {
        self.inner.stop_loading();
    }

    /// Execute JavaScript in the webview.
    pub fn evaluate_javascript(
        &self,
        script: &str,
        callback: Option<Box<dyn FnOnce(Result<String, String>) + Send>>,
    ) {
        self.inner.evaluate_javascript(script, callback);
    }

    /// Get the current page title.
    pub fn title(&self) -> SharedString {
        self.inner.title()
    }

    /// Get the current page URL.
    pub fn url(&self) -> SharedString {
        self.inner.url()
    }

    /// Focus the webview.
    pub fn focus(&self) {
        self.inner.focus();
    }

    /// Access the underlying `wry::WebView` for capabilities this crate
    /// doesn't wrap yet (custom protocol handlers, download handlers,
    /// platform-specific extension traits, and so on).
    ///
    /// Returns `None` if the "webview" feature is disabled, or if a future
    /// non-wry backend is active.
    #[cfg(feature = "webview")]
    pub fn native(&self) -> Option<&wry::WebView> {
        self.inner
            .as_any()
            .downcast_ref::<crate::platform::WryWebView>()
            .map(|webview| webview.raw())
    }

    /// Settle the promise a page's `window.invoke()` call for `request` is
    /// waiting on, with the given [`IpcResult`]. This is the reply half of
    /// [`crate::WebView::on_ipc_message`].
    pub fn reply(&self, request: &IpcRequest, result: IpcResult) {
        let script = ipc::resolve_script(request.id, &result);
        self.inner.evaluate_javascript(&script, None);
    }

    /// Reply to `request` with a success value.
    pub fn reply_ok<T: Serialize>(&self, request: &IpcRequest, value: T) {
        self.reply(request, ipc::ok(value));
    }

    /// Reply to `request` with an error message.
    pub fn reply_err(&self, request: &IpcRequest, message: impl Into<String>) {
        self.reply(request, Err(message.into()));
    }
}
