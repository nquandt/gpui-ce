use gpui::{App, Bounds, Pixels, SharedString, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use super::PlatformWebView;
use crate::webview::WebViewConfig;

/// Shared cell holding the current navigation-interception callback. Wry
/// registers its navigation handler once at webview creation, but the
/// `WebView` element (and its `on_navigation` closure) is rebuilt every
/// frame, so the native handler reads through this cell instead of
/// capturing a closure directly.
type NavigationHandler = Rc<RefCell<Option<Box<dyn FnMut(&str) -> bool>>>>;

struct RawWindowHandleWrapper(RawWindowHandle);

impl HasWindowHandle for RawWindowHandleWrapper {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(self.0) })
    }
}

pub(crate) struct WryWebView {
    webview: wry::WebView,
    navigation_handler: NavigationHandler,
}

impl WryWebView {
    pub fn new(window: &Window, config: &WebViewConfig, cx: &mut App) -> Self {
        let mut builder = wry::WebViewBuilder::new();

        let bounds = wry::Rect {
            position: wry::dpi::LogicalPosition::new(
                config.bounds.origin.x.as_f32(),
                config.bounds.origin.y.as_f32(),
            )
            .into(),
            size: wry::dpi::LogicalSize::new(
                config.bounds.size.width.as_f32(),
                config.bounds.size.height.as_f32(),
            )
            .into(),
        };
        builder = builder.with_bounds(bounds);

        if let Some(url) = &config.url {
            builder = builder.with_url(url.as_str());
        } else if let Some(html) = &config.html {
            builder = builder.with_html(html.as_str());
        }

        if config.transparent {
            builder = builder.with_transparent(true);
        }

        if config.devtools {
            builder = builder.with_devtools(true);
        }

        // Scripts a consumer wants evaluated before any page script runs on
        // every navigation (analytics shims, polyfills, a future typed IPC
        // preload, etc). Order matches `WebView::init_script` call order.
        for script in &config.init_scripts {
            builder = builder.with_initialization_script(script.as_str());
        }

        let navigation_handler: NavigationHandler = Rc::new(RefCell::new(None));
        let navigation_handler_for_builder = navigation_handler.clone();
        builder = builder.with_navigation_handler(move |url| {
            match navigation_handler_for_builder.borrow_mut().as_mut() {
                Some(callback) => callback(&url),
                None => true,
            }
        });

        // NOTE(ipc): this is where a future JS<->Rust IPC bridge's own
        // initialization script and `with_ipc_handler` registration would be
        // wired in, ahead of any consumer-supplied init scripts above so the
        // bridge is always available to page code.

        // Wake the owning GPUI window whenever the webview starts or finishes a
        // page load, so its URL-detection poll runs promptly. Without this, the
        // poll only runs on frames the window happens to draw anyway (e.g. the
        // URL bar's caret blink), so a link-click navigation - which produces no
        // GPUI input event - could take hundreds of milliseconds to show up.
        // The wake is deferred through the foreground executor so we never
        // borrow the app re-entrantly from inside a WebView2 event callback.
        let app = cx.to_async();
        builder = builder.with_on_page_load_handler({
            move |event, url| {
                if std::env::var_os("GPUI_TRACE").is_some() {
                    let ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis())
                        .unwrap_or_default();
                    let phase = match event {
                        wry::PageLoadEvent::Started => "Started",
                        wry::PageLoadEvent::Finished => "Finished",
                    };
                    eprintln!("TRACE {ms}ms page_load {phase}: {url}");
                }
                let app = app.clone();
                let executor = app.foreground_executor().clone();
                executor
                    .spawn(async move { app.update(|cx| cx.refresh_windows()) })
                    .detach();
            }
        });

        let raw_handle = HasWindowHandle::window_handle(window)
            .expect("failed to get window handle for webview")
            .as_raw();

        let wrapper = RawWindowHandleWrapper(raw_handle);

        let webview = builder
            .build_as_child(&wrapper)
            .expect("failed to create webview");

        if (config.zoom - 1.0).abs() > f32::EPSILON {
            let _ = webview.zoom(config.zoom as f64);
        }

        WryWebView {
            webview,
            navigation_handler,
        }
    }

    /// The underlying wry webview, for capabilities this crate doesn't wrap
    /// yet. Reached publicly via `WebViewHandle::native()`.
    pub(crate) fn raw(&self) -> &wry::WebView {
        &self.webview
    }
}

impl PlatformWebView for WryWebView {
    fn set_bounds(&self, bounds: Bounds<Pixels>) {
        let _ = self.webview.set_bounds(wry::Rect {
            position: wry::dpi::LogicalPosition::new(
                bounds.origin.x.as_f32(),
                bounds.origin.y.as_f32(),
            )
            .into(),
            size: wry::dpi::LogicalSize::new(
                bounds.size.width.as_f32(),
                bounds.size.height.as_f32(),
            )
            .into(),
        });
    }

    fn set_visible(&self, visible: bool) {
        let _ = self.webview.set_visible(visible);
    }

    fn load_url(&self, url: &str) {
        let _ = self.webview.load_url(url);
    }

    fn load_html(&self, html: &str) {
        let _ = self.webview.load_html(html);
    }

    fn go_back(&self) {
        let _ = self.webview.evaluate_script("window.history.back()");
    }

    fn go_forward(&self) {
        let _ = self.webview.evaluate_script("window.history.forward()");
    }

    fn reload(&self) {
        let _ = self.webview.reload();
    }

    fn stop_loading(&self) {
        let _ = self.webview.evaluate_script("window.stop()");
    }

    fn evaluate_javascript(
        &self,
        script: &str,
        callback: Option<Box<dyn FnOnce(Result<String, String>) + Send>>,
    ) {
        match callback {
            Some(cb) => {
                let cb = Arc::new(Mutex::new(Some(cb)));
                let cb_clone = cb.clone();
                let _ = self
                    .webview
                    .evaluate_script_with_callback(script, move |result| {
                        if let Some(f) = cb_clone.lock().unwrap().take() {
                            f(Ok(result));
                        }
                    });
            }
            None => {
                let _ = self.webview.evaluate_script(script);
            }
        }
    }

    fn title(&self) -> SharedString {
        SharedString::default()
    }

    fn url(&self) -> SharedString {
        self.webview
            .url()
            .map(SharedString::from)
            .unwrap_or_default()
    }

    fn set_devtools_enabled(&self, _enabled: bool) {
        // wry devtools are enabled/disabled at creation time via the builder.
    }

    fn focus(&self) {
        let _ = self.webview.focus();
    }

    fn set_navigation_handler(&self, handler: Option<Box<dyn FnMut(&str) -> bool>>) {
        *self.navigation_handler.borrow_mut() = handler;
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub(crate) fn create_platform_webview(
    window: &Window,
    config: &WebViewConfig,
    cx: &mut App,
) -> Box<dyn PlatformWebView> {
    Box::new(WryWebView::new(window, config, cx))
}
