//! iOS platform view implementation.
//!
//! Embeds native iOS `UIView` instances in the GPUI render tree using
//! hybrid composition. Native views are created here and can be inserted
//! into the view hierarchy by the iOS window code when a platform view
//! element is painted.

use crate::platform_view::{
    PlatformView, PlatformViewBounds, PlatformViewFactory, PlatformViewId, PlatformViewParams,
};
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "ios")]
use objc2::runtime::{AnyClass, AnyObject, ClassBuilder, Sel};
use objc2::{class, msg_send, sel};

#[cfg(target_os = "ios")]
use super::cg_types::ObjcCGRect;

/// iOS implementation of a platform view.
///
/// Wraps a `UIView` instance. The view is created during construction but
/// is NOT automatically added to the view hierarchy. Call
/// `native_view_ptr()` to get the raw `*mut AnyObject` pointer, then insert
/// it into the appropriate superview from the iOS window code.
///
/// TODO: The window/paint code should call `native_view_ptr()` and insert
/// the view as a subview of the Metal view's superview (or another
/// appropriate container) at the correct z-order position.
pub struct IosPlatformView {
    id: PlatformViewId,
    view_type: String,
    /// Pointer to the Objective-C UIView instance.
    /// Null if disposed.
    #[cfg(target_os = "ios")]
    native_view: std::sync::Mutex<*mut AnyObject>,
    disposed: AtomicBool,
    /// Whether this view has been inserted into the window's view hierarchy.
    inserted: AtomicBool,
    bounds: std::sync::Mutex<PlatformViewBounds>,
}

// Safety: UIView operations are dispatched to the main thread.
unsafe impl Send for IosPlatformView {}
unsafe impl Sync for IosPlatformView {}

impl IosPlatformView {
    /// Create a new iOS platform view.
    ///
    /// The UIView is allocated and initialized with the given bounds, but
    /// is NOT added to any view hierarchy. The caller is responsible for
    /// inserting the view by using `native_view_ptr()`.
    #[cfg(target_os = "ios")]
    pub fn new(view_type: &str, params: &PlatformViewParams) -> Result<Self, String> {
        let id = PlatformViewId::next();

        let native_view = Self::create_native_view(view_type, &params.bounds, params)?;

        Ok(Self {
            id,
            view_type: view_type.to_string(),
            native_view: std::sync::Mutex::new(native_view),
            disposed: AtomicBool::new(false),
            inserted: AtomicBool::new(false),
            bounds: std::sync::Mutex::new(params.bounds),
        })
    }

    /// Create the native UIView via Objective-C runtime.
    ///
    /// Dispatches to type-specific creation for known view types:
    /// - "video_player": Creates UIView with AVPlayerLayer (requires player_id in params)
    /// - "webview": Creates WKWebView (uses url/html from params)
    /// - "camera_preview": Creates UIView with AVCaptureVideoPreviewLayer (requires session_id in params)
    /// - Default: Creates a generic UIView container
    #[cfg(target_os = "ios")]
    fn create_native_view(
        view_type: &str,
        bounds: &PlatformViewBounds,
        params: &PlatformViewParams,
    ) -> Result<*mut AnyObject, String> {
        unsafe {
            let frame = ObjcCGRect::new(
                bounds.x as f64,
                bounds.y as f64,
                bounds.width as f64,
                bounds.height as f64,
            );

            let view: *mut AnyObject = match view_type {
                "video_player" => Self::create_video_player_view(frame, params)?,
                "webview" => Self::create_webview_view(frame, params)?,
                "camera_preview" => Self::create_camera_preview_view(frame, params)?,
                "text_field" => Self::create_text_field_view(frame, params)?,
                _ => Self::create_generic_view(frame)?,
            };

            if view.is_null() {
                return Err(format!("Failed to create UIView for type '{}'", view_type));
            }

            // Clip to bounds
            let _: () = msg_send![view, setClipsToBounds: true];

            log::info!(
                "IosPlatformView: created native UIView for type '{}' at ({}, {}, {}, {})",
                view_type,
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
            );

            Ok(view)
        }
    }

    /// Create a generic transparent UIView container.
    #[cfg(target_os = "ios")]
    unsafe fn create_generic_view(frame: ObjcCGRect) -> Result<*mut AnyObject, String> {
        let uiview_class = class!(UIView);
        let view: *mut AnyObject = msg_send![uiview_class, alloc];
        let view: *mut AnyObject = msg_send![view, initWithFrame: frame];
        if view.is_null() {
            return Err("Failed to create UIView".into());
        }
        let clear_color: *mut AnyObject = msg_send![class!(UIColor), clearColor];
        let _: () = msg_send![view, setBackgroundColor: clear_color];
        Ok(view)
    }

    /// Create a UIView with an AVPlayerLayer for video playback.
    #[cfg(target_os = "ios")]
    unsafe fn create_video_player_view(
        frame: ObjcCGRect,
        params: &PlatformViewParams,
    ) -> Result<*mut AnyObject, String> {
        // Create a container UIView
        let uiview_class = class!(UIView);
        let view: *mut AnyObject = msg_send![uiview_class, alloc];
        let view: *mut AnyObject = msg_send![view, initWithFrame: frame];
        if view.is_null() {
            return Err("Failed to create UIView for video_player".into());
        }

        let black_color: *mut AnyObject = msg_send![class!(UIColor), blackColor];
        let _: () = msg_send![view, setBackgroundColor: black_color];

        // If a player_id is provided, try to get the AVPlayer and create AVPlayerLayer
        if let Some(player_id_str) = params.creation_params.get("player_id") {
            if let Ok(player_id) = player_id_str.parse::<u32>() {
                // Get AVPlayer from the video_player package's PLAYERS map
                if let Some(player_ptr) = crate::packages::video_player::ios_get_player(player_id) {
                    let player_layer: *mut AnyObject =
                        msg_send![class!(AVPlayerLayer), playerLayerWithPlayer: player_ptr];
                    if !player_layer.is_null() {
                        let _: () = msg_send![player_layer, setFrame: frame];
                        // Set video gravity to aspect fit
                        let gravity = Self::make_nsstring("AVLayerVideoGravityResizeAspect");
                        let _: () = msg_send![player_layer, setVideoGravity: gravity];
                        let _: () = msg_send![gravity, release];
                        let view_layer: *mut AnyObject = msg_send![view, layer];
                        let _: () = msg_send![view_layer, addSublayer: player_layer];
                    }
                }
            }
        }

        Ok(view)
    }

    /// Create a WKWebView.
    #[cfg(target_os = "ios")]
    unsafe fn create_webview_view(
        frame: ObjcCGRect,
        params: &PlatformViewParams,
    ) -> Result<*mut AnyObject, String> {
        let config: *mut AnyObject = msg_send![class!(WKWebViewConfiguration), alloc];
        let config: *mut AnyObject = msg_send![config, init];
        if config.is_null() {
            return Err("Failed to create WKWebViewConfiguration".into());
        }

        let js_enabled = params
            .creation_params
            .get("javascript_enabled")
            .map(|v| v == "true")
            .unwrap_or(true);

        let prefs: *mut AnyObject = msg_send![config, preferences];
        if !prefs.is_null() {
            let _: () = msg_send![prefs, setJavaScriptEnabled: js_enabled];
        }

        let webview: *mut AnyObject = msg_send![class!(WKWebView), alloc];
        let webview: *mut AnyObject =
            msg_send![webview, initWithFrame: frame, configuration: config];
        if webview.is_null() {
            return Err("Failed to create WKWebView".into());
        }

        // `WKWebView.navigationDelegate` is a weak reference, so the delegate
        // must be retained independently. We intentionally leak it for the
        // lifetime of the webview (matches the single-active-webview model
        // of `packages::webview`'s URL-changed callback).
        let delegate_class = Self::register_navigation_delegate_class();
        let delegate: *mut AnyObject = msg_send![delegate_class, alloc];
        let delegate: *mut AnyObject = msg_send![delegate, init];
        let _: () = msg_send![webview, setNavigationDelegate: delegate];

        // Load URL or HTML if provided
        if let Some(url) = params.creation_params.get("url") {
            if !url.is_empty() {
                let ns_url_str = Self::make_nsstring(url);
                let nsurl: *mut AnyObject = msg_send![class!(NSURL), URLWithString: ns_url_str];
                if !nsurl.is_null() {
                    let request: *mut AnyObject =
                        msg_send![class!(NSURLRequest), requestWithURL: nsurl];
                    let _: *mut AnyObject = msg_send![webview, loadRequest: request];
                }
                let _: () = msg_send![ns_url_str, release];
            }
        } else if let Some(html) = params.creation_params.get("html") {
            if !html.is_empty() {
                let ns_html = Self::make_nsstring(html);
                let base_url: *mut AnyObject = std::ptr::null_mut();
                let _: *mut AnyObject =
                    msg_send![webview, loadHTMLString: ns_html, baseURL: base_url];
                let _: () = msg_send![ns_html, release];
            }
        }

        Ok(webview)
    }

    /// Create a UIView with AVCaptureVideoPreviewLayer for camera preview.
    #[cfg(target_os = "ios")]
    unsafe fn create_camera_preview_view(
        frame: ObjcCGRect,
        params: &PlatformViewParams,
    ) -> Result<*mut AnyObject, String> {
        let uiview_class = class!(UIView);
        let view: *mut AnyObject = msg_send![uiview_class, alloc];
        let view: *mut AnyObject = msg_send![view, initWithFrame: frame];
        if view.is_null() {
            return Err("Failed to create UIView for camera_preview".into());
        }

        let black_color: *mut AnyObject = msg_send![class!(UIColor), blackColor];
        let _: () = msg_send![view, setBackgroundColor: black_color];

        // If a session_id is provided, try to get the AVCaptureSession and create preview layer
        if let Some(session_id_str) = params.creation_params.get("session_id") {
            if let Ok(session_id) = session_id_str.parse::<usize>() {
                if let Some(session_ptr) = crate::packages::camera::ios_get_session(session_id) {
                    let layer: *mut AnyObject =
                        msg_send![class!(AVCaptureVideoPreviewLayer), alloc];
                    let layer: *mut AnyObject = msg_send![layer, initWithSession: session_ptr];
                    if !layer.is_null() {
                        let _: () = msg_send![layer, setFrame: frame];
                        let gravity = Self::make_nsstring("AVLayerVideoGravityResizeAspectFill");
                        let _: () = msg_send![layer, setVideoGravity: gravity];
                        let _: () = msg_send![gravity, release];
                        let view_layer: *mut AnyObject = msg_send![view, layer];
                        let _: () = msg_send![view_layer, addSublayer: layer];
                    }
                }
            }
        }

        Ok(view)
    }

    /// Create a native `UITextField`.
    ///
    /// Unlike the manually-drawn GPUI text bars elsewhere in this crate,
    /// this is a real `UIResponder`/`UITextInput`, so it gets cursor
    /// placement, double-tap/drag selection, copy/paste, and autocorrect
    /// for free from UIKit.
    #[cfg(target_os = "ios")]
    unsafe fn create_text_field_view(
        frame: ObjcCGRect,
        params: &PlatformViewParams,
    ) -> Result<*mut AnyObject, String> {
        let field: *mut AnyObject = msg_send![class!(UITextField), alloc];
        let field: *mut AnyObject = msg_send![field, initWithFrame: frame];
        if field.is_null() {
            return Err("Failed to create UITextField".into());
        }

        let _: () = msg_send![field, setBorderStyle: 3_isize]; // UITextBorderStyleRoundedRect

        if let Some(text) = params.creation_params.get("text") {
            let ns_text = Self::make_nsstring(text);
            let _: () = msg_send![field, setText: ns_text];
        }
        if let Some(placeholder) = params.creation_params.get("placeholder") {
            let ns_placeholder = Self::make_nsstring(placeholder);
            let _: () = msg_send![field, setPlaceholder: ns_placeholder];
        }

        // Wire up a target for text-changed and return-key notifications.
        // `addTarget:action:forControlEvents:` keeps only an unretained
        // reference, so — like the navigation delegate above — we
        // intentionally leak the target for the field's lifetime.
        let target_class = Self::register_text_field_target_class();
        let target: *mut AnyObject = msg_send![target_class, alloc];
        let target: *mut AnyObject = msg_send![target, init];
        let _: () = msg_send![field,
            addTarget: target,
            action: sel!(gpuiTextChanged:),
            forControlEvents: 1usize << 17 // UIControlEventEditingChanged
        ];
        let _: () = msg_send![field,
            addTarget: target,
            action: sel!(gpuiDidEndOnExit:),
            forControlEvents: 1usize << 19 // UIControlEventEditingDidEndOnExit
        ];

        Ok(field)
    }

    /// Register (once) the target object handling `UITextField` control
    /// events for the "text_field" platform view type.
    #[cfg(target_os = "ios")]
    fn register_text_field_target_class() -> &'static AnyClass {
        static REGISTERED: std::sync::Once = std::sync::Once::new();
        REGISTERED.call_once(|| {
            let superclass = class!(NSObject);
            let mut decl = ClassBuilder::new(c"GPUITextFieldTarget", superclass).unwrap();

            unsafe extern "C" fn text_changed(_this: *mut AnyObject, _sel: Sel, sender: *mut AnyObject) {
                unsafe {
                    if sender.is_null() {
                        return;
                    }
                    let text: *mut AnyObject = msg_send![sender, text];
                    if text.is_null() {
                        return;
                    }
                    let utf8: *const i8 = msg_send![text, UTF8String];
                    if utf8.is_null() {
                        return;
                    }
                    let text_str = std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned();
                    crate::packages::text_field::dispatch_text_changed(&text_str);
                }
            }

            unsafe extern "C" fn did_end_on_exit(_this: *mut AnyObject, _sel: Sel, _sender: *mut AnyObject) {
                crate::packages::text_field::dispatch_submit();
            }

            unsafe {
                decl.add_method(
                    sel!(gpuiTextChanged:),
                    text_changed as unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject),
                );
                decl.add_method(
                    sel!(gpuiDidEndOnExit:),
                    did_end_on_exit as unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject),
                );
            }

            decl.register();
        });

        class!(GPUITextFieldTarget)
    }

    #[cfg(target_os = "ios")]
    unsafe fn make_nsstring(s: &str) -> *mut AnyObject {
        crate::ios::util::nsstring(s)
    }

    /// Register (once) a `WKNavigationDelegate` that forwards URL changes
    /// to `packages::webview`'s `dispatch_url_changed`, so app code can
    /// observe in-page navigation (e.g. tapping a link) via
    /// `packages::webview::set_on_url_changed`.
    #[cfg(target_os = "ios")]
    fn register_navigation_delegate_class() -> &'static AnyClass {
        static REGISTERED: std::sync::Once = std::sync::Once::new();
        REGISTERED.call_once(|| {
            let superclass = class!(NSObject);
            let mut decl = ClassBuilder::new(c"GPUIWebViewNavigationDelegate", superclass).unwrap();

            if let Some(protocol) = objc2::runtime::AnyProtocol::get(c"WKNavigationDelegate") {
                decl.add_protocol(protocol);
            }

            unsafe fn report_current_url(web_view: *mut AnyObject) {
                unsafe {
                    if web_view.is_null() {
                        return;
                    }
                    let url: *mut AnyObject = msg_send![web_view, URL];
                    if url.is_null() {
                        return;
                    }
                    let absolute_string: *mut AnyObject = msg_send![url, absoluteString];
                    if absolute_string.is_null() {
                        return;
                    }
                    let utf8: *const i8 = msg_send![absolute_string, UTF8String];
                    if utf8.is_null() {
                        return;
                    }
                    let url_str = std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned();
                    crate::packages::webview::dispatch_url_changed(&url_str);
                }
            }

            // `didCommitNavigation` fires as soon as the navigation is
            // confirmed (URL bar should update here, like Safari does) —
            // well before the page finishes loading all its resources.
            // `didFinishNavigation` is still handled as a fallback/final
            // sync (e.g. for URL changes via client-side redirects that
            // don't go through a full navigation commit).
            unsafe extern "C" fn did_commit_navigation(
                _this: *mut AnyObject,
                _sel: Sel,
                web_view: *mut AnyObject,
                _navigation: *mut AnyObject,
            ) {
                unsafe { report_current_url(web_view) };
            }

            unsafe extern "C" fn did_finish_navigation(
                _this: *mut AnyObject,
                _sel: Sel,
                web_view: *mut AnyObject,
                _navigation: *mut AnyObject,
            ) {
                unsafe { report_current_url(web_view) };
            }

            unsafe {
                decl.add_method(
                    sel!(webView:didCommitNavigation:),
                    did_commit_navigation
                        as unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, *mut AnyObject),
                );
                decl.add_method(
                    sel!(webView:didFinishNavigation:),
                    did_finish_navigation
                        as unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, *mut AnyObject),
                );
            }

            decl.register();
        });

        class!(GPUIWebViewNavigationDelegate)
    }

    /// Returns the raw pointer to the underlying `UIView`.
    ///
    /// Use this to insert the view into the iOS view hierarchy from
    /// the window/paint code. Returns null if the view has been disposed.
    #[cfg(target_os = "ios")]
    pub fn native_view_ptr(&self) -> *mut AnyObject {
        *self.native_view.lock().unwrap()
    }

    /// Set the `text` property on the underlying view (e.g. a `UITextField`
    /// created for the "text_field" view type). No-op if the view is
    /// disposed or doesn't respond to `setText:`.
    #[cfg(target_os = "ios")]
    pub fn set_text(&self, text: &str) {
        let view = self.native_view_ptr();
        if view.is_null() {
            return;
        }
        unsafe {
            let ns_text = Self::make_nsstring(text);
            let _: () = msg_send![view, setText: ns_text];
        }
    }

    /// Read the `text` property from the underlying view. Returns `None` if
    /// the view is disposed or doesn't respond to `text`.
    #[cfg(target_os = "ios")]
    pub fn text(&self) -> Option<String> {
        let view = self.native_view_ptr();
        if view.is_null() {
            return None;
        }
        unsafe {
            let ns_text: *mut AnyObject = msg_send![view, text];
            if ns_text.is_null() {
                return None;
            }
            let utf8: *const i8 = msg_send![ns_text, UTF8String];
            if utf8.is_null() {
                return None;
            }
            Some(std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned())
        }
    }

    /// Whether the underlying view is currently the first responder
    /// (has keyboard focus).
    #[cfg(target_os = "ios")]
    pub fn is_first_responder(&self) -> bool {
        let view = self.native_view_ptr();
        if view.is_null() {
            return false;
        }
        unsafe {
            let is_first: objc2::runtime::Bool = msg_send![view, isFirstResponder];
            is_first.as_bool()
        }
    }

    /// Resign first responder (dismiss the keyboard if this view has it).
    #[cfg(target_os = "ios")]
    pub fn resign_first_responder(&self) {
        let view = self.native_view_ptr();
        if view.is_null() {
            return;
        }
        unsafe {
            let _: objc2::runtime::Bool = msg_send![view, resignFirstResponder];
        }
    }

    /// Insert this view into the GPUI window's view hierarchy.
    ///
    /// Adds the UIView as a subview of the view controller's view, above
    /// the Metal view (whose opaque `CAMetalLayer` would otherwise cover
    /// it every frame).
    ///
    /// This should be called once after creation, typically from the
    /// platform view element's first paint. Subsequent calls are no-ops.
    #[cfg(target_os = "ios")]
    pub fn insert_into_window(&self) -> Result<(), String> {
        // Guard against double-insertion.
        if self.inserted.swap(true, Ordering::Relaxed) {
            return Ok(());
        }

        let native_view = *self.native_view.lock().unwrap();
        if native_view.is_null() {
            self.inserted.store(false, Ordering::Relaxed);
            return Err("View is null (disposed?)".to_string());
        }

        unsafe {
            if let Some(wrapper) = super::ffi::IOS_WINDOW_LIST.get() {
                let windows = &*wrapper.0.get();
                if let Some(&window_ptr) = windows.last() {
                    if !window_ptr.is_null() {
                        let window = &*window_ptr;
                        // Get the view controller's view (parent of Metal view)
                        let vc: *mut AnyObject = window.view_controller_ptr();
                        let vc_view: *mut AnyObject = msg_send![vc, view];
                        if !vc_view.is_null() {
                            // The Metal view's CAMetalLayer is opaque and covers the
                            // full window every frame, so a platform view inserted
                            // below it would never be visible. Insert above instead.
                            let _: () = msg_send![vc_view, addSubview: native_view];
                            log::info!(
                                "IosPlatformView: inserted view {} into window hierarchy",
                                self.id
                            );
                            return Ok(());
                        }
                    }
                }
            }
        }
        self.inserted.store(false, Ordering::Relaxed);
        Err("No GPUI window available to host platform view".to_string())
    }

    /// Update the native view's frame.
    #[cfg(target_os = "ios")]
    fn update_native_frame(&self, bounds: &PlatformViewBounds) {
        let view = *self.native_view.lock().unwrap();
        if view.is_null() {
            return;
        }
        unsafe {
            let frame = ObjcCGRect::new(
                bounds.x as f64,
                bounds.y as f64,
                bounds.width as f64,
                bounds.height as f64,
            );
            let _: () = msg_send![view, setFrame: frame];
        }
    }
}

impl PlatformView for IosPlatformView {
    fn id(&self) -> PlatformViewId {
        self.id
    }

    fn view_type(&self) -> &str {
        &self.view_type
    }

    fn set_bounds(&self, bounds: PlatformViewBounds) {
        if self.disposed.load(Ordering::Relaxed) {
            return;
        }
        *self.bounds.lock().unwrap() = bounds;
        #[cfg(target_os = "ios")]
        {
            if !self.inserted.load(Ordering::Relaxed) {
                if let Err(e) = self.insert_into_window() {
                    log::warn!("IosPlatformView: failed to insert into window: {e}");
                }
            }
            self.update_native_frame(&bounds);
        }
    }

    fn set_visible(&self, visible: bool) {
        if self.disposed.load(Ordering::Relaxed) {
            return;
        }
        #[cfg(target_os = "ios")]
        {
            let view = *self.native_view.lock().unwrap();
            if !view.is_null() {
                unsafe {
                    let _: () = msg_send![view, setHidden: !visible];
                }
            }
        }
        #[cfg(not(target_os = "ios"))]
        {
            let _ = visible;
        }
    }

    fn set_z_index(&self, z_index: i32) {
        if self.disposed.load(Ordering::Relaxed) {
            return;
        }
        #[cfg(target_os = "ios")]
        {
            let view = *self.native_view.lock().unwrap();
            if !view.is_null() {
                unsafe {
                    let layer: *mut AnyObject = msg_send![view, layer];
                    if !layer.is_null() {
                        let z = z_index as f64;
                        let _: () = msg_send![layer, setZPosition: z];
                    }
                }
            }
        }
        #[cfg(not(target_os = "ios"))]
        {
            let _ = z_index;
        }
    }

    fn dispose(&self) {
        if self.disposed.swap(true, Ordering::Relaxed) {
            return;
        }
        #[cfg(target_os = "ios")]
        {
            let view = *self.native_view.lock().unwrap();
            if !view.is_null() {
                unsafe {
                    // Remove from superview if it was added to one.
                    let _: () = msg_send![view, removeFromSuperview];
                }
            }
        }
        log::info!("IosPlatformView: disposed view {}", self.id);
    }

    fn is_disposed(&self) -> bool {
        self.disposed.load(Ordering::Relaxed)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// iOS platform view factory.
pub struct IosPlatformViewFactory {
    view_type: String,
}

impl IosPlatformViewFactory {
    pub fn new(view_type: &str) -> Self {
        Self {
            view_type: view_type.to_string(),
        }
    }
}

impl PlatformViewFactory for IosPlatformViewFactory {
    fn create(&self, params: &PlatformViewParams) -> Result<Box<dyn PlatformView>, String> {
        #[cfg(target_os = "ios")]
        {
            let view = IosPlatformView::new(&self.view_type, params)?;
            Ok(Box::new(view))
        }
        #[cfg(not(target_os = "ios"))]
        {
            let _ = params;
            Err("iOS platform views are only available on iOS".to_string())
        }
    }

    fn view_type(&self) -> &str {
        &self.view_type
    }
}
