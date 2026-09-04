use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, Hitbox, HitboxBehavior, InspectorElementId,
    IntoElement, LayoutId, Pixels, Refineable, SharedString, Style, StyleRefinement, Styled,
    Window,
};
use std::rc::Rc;

use crate::ipc::IpcRequest;
use crate::platform::{self, PlatformWebView};
use crate::webview_handle::WebViewHandle;

/// Configuration for creating a platform webview.
#[allow(dead_code)]
pub(crate) struct WebViewConfig {
    pub url: Option<String>,
    pub html: Option<String>,
    pub transparent: bool,
    pub devtools: bool,
    pub zoom: f32,
    pub bounds: Bounds<Pixels>,
    pub initial_title: Option<SharedString>,
    /// Scripts evaluated before page scripts run, on every navigation. See
    /// `WebView::init_script`.
    pub init_scripts: Vec<String>,
}

impl Default for WebViewConfig {
    fn default() -> Self {
        Self {
            url: None,
            html: None,
            transparent: false,
            devtools: false,
            zoom: 1.0,
            bounds: Bounds::default(),
            initial_title: None,
            init_scripts: Vec::new(),
        }
    }
}

/// Persistent state for a `WebView` element, stored across frames.
pub(crate) struct WebViewState {
    pub(crate) platform_webview: Rc<dyn PlatformWebView>,
    pub(crate) last_bounds: Bounds<Pixels>,
    pub(crate) last_known_url: SharedString,
}

/// A cross-platform native webview element for GPUI.
///
/// `WebView` renders a native webview (WebView2 on Windows, WKWebView on macOS)
/// as a child view embedded inside GPUI's layout system.
///
/// # Example
///
/// ```ignore
/// use gpui_webview::WebView;
///
/// fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
///     div().child(
///         WebView::new("my-webview")
///             .url("https://example.com")
///             .w(px(800))
///             .h(px(600))
///     )
/// }
/// ```
pub struct WebView {
    id: SharedString,
    config: WebViewConfig,
    on_navigation: Option<Box<dyn FnMut(&str) -> bool>>,
    on_page_load: Option<Box<dyn FnMut(&str)>>,
    on_url_changed: Option<Box<dyn FnMut(&str, &mut Window, &mut App)>>,
    on_create_handle: Option<Box<dyn FnMut(WebViewHandle, &mut Window, &mut App)>>,
    on_ipc_message: Option<Box<dyn FnMut(&IpcRequest, &WebViewHandle, &mut Window, &mut App)>>,
    style: StyleRefinement,
}

impl WebView {
    /// Create a new WebView element with a unique ID.
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            config: WebViewConfig::default(),
            on_navigation: None,
            on_page_load: None,
            on_url_changed: None,
            on_create_handle: None,
            on_ipc_message: None,
            style: StyleRefinement::default(),
        }
    }

    /// Set the initial URL to load.
    pub fn url(mut self, url: &str) -> Self {
        self.config.url = Some(url.to_string());
        self
    }

    /// Set initial HTML content.
    pub fn html(mut self, html: &str) -> Self {
        self.config.html = Some(html.to_string());
        self
    }

    /// Set whether the webview background is transparent.
    pub fn transparent(mut self, transparent: bool) -> Self {
        self.config.transparent = transparent;
        self
    }

    /// Set whether the webview is dev-tools-enabled.
    pub fn devtools(mut self, enabled: bool) -> Self {
        self.config.devtools = enabled;
        self
    }

    /// Set zoom factor.
    pub fn zoom(mut self, factor: f32) -> Self {
        self.config.zoom = factor;
        self
    }

    /// Add a script evaluated before any page script runs, on every
    /// navigation. Call multiple times to add several scripts; they run in
    /// the order added. Useful for polyfills, analytics shims, or a
    /// consumer-defined preload API.
    pub fn init_script(mut self, script: impl Into<String>) -> Self {
        self.config.init_scripts.push(script.into());
        self
    }

    /// Callback fired when navigation is attempted, and reused as the trust
    /// check that gates IPC dispatch (see [`WebView::on_ipc_message`]).
    /// Return false to block the navigation, or drop an IPC message from a
    /// page whose URL fails the check.
    ///
    /// Without this callback the webview behaves like a classic, open
    /// browser: any URL may load, and any loaded page may call
    /// `window.invoke`. Set it to scope the webview into an embedded-app
    /// mode where only approved URLs (e.g. your own bundled origin) can
    /// navigate or talk to native code, the way a Tauri app scopes its IPC
    /// bridge to its own origin.
    pub fn on_navigation(mut self, callback: impl FnMut(&str) -> bool + 'static) -> Self {
        self.on_navigation = Some(Box::new(callback));
        self
    }

    /// Callback fired when a page finishes loading.
    pub fn on_page_load(mut self, callback: impl FnMut(&str) + 'static) -> Self {
        self.on_page_load = Some(Box::new(callback));
        self
    }

    /// Callback fired whenever the webview's main-frame URL changes.
    ///
    /// The URL is polled each frame and the callback is invoked with the new
    /// URL. This is useful for keeping an address bar in sync with internal
    /// page navigation (link clicks, redirects, etc.).
    pub fn on_url_changed(
        mut self,
        callback: impl FnMut(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_url_changed = Some(Box::new(callback));
        self
    }

    /// Callback fired when the webview is first created, providing a handle.
    pub fn on_create_handle(
        mut self,
        callback: impl FnMut(WebViewHandle, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_create_handle = Some(Box::new(callback));
        self
    }

    /// Callback fired for each command the page sends via
    /// `window.invoke(cmd, payload)`, decoded into an [`IpcRequest`].
    ///
    /// Reply with [`WebViewHandle::reply`] (or `reply_ok` / `reply_err`)
    /// using the same request to settle the matching promise on the page,
    /// the way a Tauri command handler replies to `invoke()`.
    pub fn on_ipc_message(
        mut self,
        callback: impl FnMut(&IpcRequest, &WebViewHandle, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_ipc_message = Some(Box::new(callback));
        self
    }

    fn element_id(&self) -> ElementId {
        ElementId::from(self.id.clone())
    }
}

impl IntoElement for WebView {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for WebView {
    type RequestLayoutState = Style;
    type PrepaintState = (Option<WebViewHandle>, Hitbox);

    fn id(&self) -> Option<ElementId> {
        Some(self.element_id())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.refine(&self.style);
        let layout_id = window.request_layout(style.clone(), [], cx);
        (layout_id, style)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // The native webview manages its own cursor (e.g. showing a pointer
        // over links); this hitbox lets us opt its region out of GPUI's own
        // cursor management so the two don't fight each other.
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);

        let handle = window.with_optional_element_state::<WebViewState, Option<WebViewHandle>>(
            id,
            |state, window| {
                let is_first_creation = !matches!(state, Some(Some(_)));
                let mut webview_state = match state {
                    Some(Some(ref prev)) => {
                        // Update bounds if changed
                        if prev.last_bounds != bounds {
                            prev.platform_webview.set_bounds(bounds);
                        }
                        WebViewState {
                            platform_webview: prev.platform_webview.clone(),
                            last_bounds: bounds,
                            last_known_url: prev.last_known_url.clone(),
                        }
                    }
                    _ => {
                        // First prepaint: create the native webview
                        self.config.bounds = bounds;
                        let platform_webview =
                            platform::create_platform_webview(window, &self.config, cx);
                        let rc: Rc<dyn PlatformWebView> = Rc::from(platform_webview);
                        WebViewState {
                            platform_webview: rc,
                            last_bounds: bounds,
                            last_known_url: SharedString::default(),
                        }
                    }
                };

                let handle = WebViewHandle::new(webview_state.platform_webview.clone());

                // The native navigation handler is registered once at webview
                // creation (see `wry_backend::WryWebView::new`), but `self`
                // (and its `on_navigation` closure) is rebuilt every frame,
                // so refresh the callback the handler reads through each
                // prepaint.
                webview_state
                    .platform_webview
                    .set_navigation_handler(self.on_navigation.take());

                // Detect main-frame URL changes (e.g. internal navigation) and
                // notify observers. `url()` reports the top-level document URL,
                // so sub-frame navigations don't trigger spurious updates.
                let current_url = webview_state.platform_webview.url();
                if current_url != webview_state.last_known_url {
                    let changed_url = current_url.clone();
                    webview_state.last_known_url = current_url;
                    // Ignore the transient blank state right after creation.
                    if !changed_url.is_empty()
                        && changed_url != "about:blank"
                        && let Some(ref mut callback) = self.on_url_changed
                    {
                        callback(&changed_url, window, cx);
                        // Callbacks fired from prepaint run mid-draw, where
                        // entity-change notifications can't schedule a
                        // redraw, so request one explicitly.
                        window.defer(cx, |window, _cx| window.refresh());
                    }
                }

                // Fire the on_create_handle callback on first creation
                if is_first_creation && let Some(ref mut callback) = self.on_create_handle {
                    callback(handle.clone(), window, cx);
                }

                // Dispatch any `window.invoke()` calls the page made since
                // the last frame to the on_ipc_message callback. Messages
                // that fail to decode are silently dropped; the page-side
                // promise will simply never resolve, which surfaces as a
                // hang during development rather than a crash at runtime.
                //
                // Messages are always drained, even from an untrusted page,
                // so a page that gets navigated away can't build up a queue
                // of commands that fire the moment a trusted URL loads.
                let pending_ipc = webview_state.platform_webview.take_ipc_messages();
                if let Some(ref mut callback) = self.on_ipc_message {
                    let is_trusted = webview_state
                        .platform_webview
                        .is_url_trusted(&webview_state.last_known_url);
                    if is_trusted {
                        for message in pending_ipc {
                            if let Some(request) = IpcRequest::parse(&message) {
                                callback(&request, &handle, window, cx);
                            }
                        }
                    }
                }

                (Some(handle), Some(webview_state))
            },
        );

        (handle, hitbox)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        // Native webview renders itself via the OS compositor. Leave cursor
        // management to it while the mouse is over its bounds.
        let (_, hitbox) = prepaint;
        window.disable_cursor_style(hitbox);
    }
}

impl Styled for WebView {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// Shorthand for creating a [`WebView`] element.
pub fn webview(id: impl Into<SharedString>) -> WebView {
    WebView::new(id)
}
