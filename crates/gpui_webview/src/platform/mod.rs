use gpui::{Bounds, Pixels, SharedString};

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
    fn focus(&self);
}

mod wry_backend;

pub(crate) use wry_backend::*;
