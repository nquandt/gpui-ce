use gpui::{Bounds, Pixels, SharedString, Window, App};
use raw_window_handle::HasWindowHandle;
use super::PlatformWebView;
use crate::WebViewConfig;

pub(crate) struct WryWebView {
    webview: wry::webview::WebView,
}

impl WryWebView {
    pub fn new(window: &Window, config: &WebViewConfig) -> Self {
        let mut builder = wry::webview::WebViewBuilder::new();

        let bounds = wry::Rect {
            position: wry::dpi::PhysicalPosition::new(
                config.bounds.origin.x.as_f32(),
                config.bounds.origin.y.as_f32(),
            )
            .into(),
            size: wry::dpi::PhysicalSize::new(
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

        if (config.zoom - 1.0).abs() > f32::EPSILON {
            builder = builder.with_zoom(config.zoom as f64);
        }

        let window_handle = window
            .window_handle()
            .expect("failed to get window handle for webview");

        let webview = builder
            .build_as_child(window_handle.raw_window_handle())
            .expect("failed to create webview");

        WryWebView { webview }
    }
}

impl PlatformWebView for WryWebView {
    fn set_bounds(&self, bounds: Bounds<Pixels>) {
        let _ = self.webview.set_bounds(wry::Rect {
            position: wry::dpi::PhysicalPosition::new(
                bounds.origin.x.as_f32(),
                bounds.origin.y.as_f32(),
            )
            .into(),
            size: wry::dpi::PhysicalSize::new(
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
        callback: Option<Box<dyn FnOnce(Result<String, String>)>>,
    ) {
        match callback {
            Some(cb) => {
                let _ = self
                    .webview
                    .evaluate_script_with_callback(script, move |result| {
                        cb(Ok(result));
                    });
            }
            None => {
                let _ = self.webview.evaluate_script(script);
            }
        }
    }

    fn title(&self) -> SharedString {
        // wry doesn't expose a synchronous title getter on WebView.
        // The title is tracked via with_document_title_changed_handler.
        // For now, return empty; a follow-up can store title in Rc<Cell>.
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
        // Runtime toggling is not supported by wry.
    }

    fn focus(&self) {
        let _ = self.webview.focus();
    }
}

pub(crate) fn create_platform_webview(
    window: &Window,
    config: &WebViewConfig,
    _cx: &mut App,
) -> Box<dyn PlatformWebView> {
    Box::new(WryWebView::new(window, config))
}
