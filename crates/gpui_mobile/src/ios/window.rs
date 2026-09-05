//! iOS Window implementation using UIWindow and UIViewController.
//!
//! iOS windows are fundamentally different from desktop windows:
//! - Always fullscreen (or split-screen on iPad)
//! - No title bar or window chrome
//! - Touch-based input
//! - Safe area insets for notch/home indicator
//!
//! The window is backed by a UIWindow containing a UIViewController
//! whose view hosts a CAMetalLayer. Rendering is performed by
//! `gpui_wgpu::WgpuRenderer` which drives wgpu over the Metal backend.

use super::IosDisplay;
use super::events::*;
use gpui::{
    AnyWindowHandle, AtlasKey, AtlasTextureId, AtlasTextureKind, AtlasTile, Bounds, Capslock,
    DevicePixels, DispatchEventResult, GpuSpecs, Modifiers, Pixels, PlatformAtlas, PlatformDisplay,
    PlatformInput, PlatformInputHandler, PlatformWindow, Point, PromptButton, PromptLevel,
    RequestFrameOptions, Scene, Size, TileId, WindowAppearance, WindowBackgroundAppearance,
    WindowBounds, WindowControlArea, WindowParams, point, px, size,
};
use gpui_wgpu::{GpuContext, WgpuContext, WgpuRenderer, WgpuSurfaceConfig};
use objc2::encode::{Encode, Encoding, RefEncode};
use objc2::runtime::{AnyClass, AnyObject, ClassBuilder, Sel};
use objc2::{class, msg_send, sel};

use super::cg_types::ObjcCGRect;
use parking_lot::Mutex;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, UiKitDisplayHandle, UiKitWindowHandle};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::c_void,
    ptr::{self, NonNull},
    rc::Rc,
    sync::Arc,
};

const GPUI_WINDOW_IVAR: &str = "gpui_window_ptr";

/// Lightweight window handle for wgpu surface creation.
/// Stores the raw UIView pointer needed by wgpu to create a Metal surface.
/// Implements the traits required by `WgpuRenderer::new`.
#[derive(Debug, Clone, Copy)]
struct RawIosWindow {
    view: *mut c_void,
}

unsafe impl Send for RawIosWindow {}
unsafe impl Sync for RawIosWindow {}

impl HasWindowHandle for RawIosWindow {
    fn window_handle(
        &self,
    ) -> std::result::Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError>
    {
        let view = NonNull::new(self.view).ok_or(raw_window_handle::HandleError::Unavailable)?;
        let handle = UiKitWindowHandle::new(view);
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(handle.into()) })
    }
}

impl HasDisplayHandle for RawIosWindow {
    fn display_handle(
        &self,
    ) -> std::result::Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError>
    {
        let handle = UiKitDisplayHandle::new();
        Ok(unsafe { raw_window_handle::DisplayHandle::borrow_raw(handle.into()) })
    }
}

static METAL_VIEW_CLASS_REGISTERED: std::sync::Once = std::sync::Once::new();
static VC_CLASS_REGISTERED: std::sync::Once = std::sync::Once::new();

/// Global storage for the current status bar style.
/// 0 = default (dark content), 1 = light content.
/// Accessed from the main thread only.
static STATUS_BAR_STYLE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

/// Register a custom UIViewController subclass that allows overriding
/// `preferredStatusBarStyle` at runtime.
fn register_view_controller_class() -> &'static AnyClass {
    VC_CLASS_REGISTERED.call_once(|| {
        let superclass = class!(UIViewController);
        let mut decl = ClassBuilder::new(c"GPUIViewController", superclass).unwrap();

        // Override preferredStatusBarStyle
        extern "C" fn preferred_status_bar_style(_this: *mut AnyObject, _sel: Sel) -> isize {
            let style = STATUS_BAR_STYLE.load(std::sync::atomic::Ordering::Relaxed);
            if style == 1 {
                1 // UIStatusBarStyleLightContent
            } else {
                3 // UIStatusBarStyleDarkContent (iOS 13+)
            }
        }

        // Override viewDidLayoutSubviews — called by UIKit on rotation,
        // split-screen changes, and any other layout pass.
        extern "C" fn view_did_layout_subviews(this: *mut AnyObject, _sel: Sel) {
            // Call super
            unsafe {
                let superclass = class!(UIViewController);
                let _: () = msg_send![super(this, superclass), viewDidLayoutSubviews];
            }

            // Notify all registered GPUI windows about the layout change.
            if let Some(wrapper) = super::ffi::IOS_WINDOW_LIST.get() {
                unsafe {
                    let windows = &*wrapper.0.get();
                    for &window_ptr in windows.iter() {
                        if !window_ptr.is_null() {
                            let window = &*window_ptr;
                            window.handle_layout_change();
                        }
                    }
                }
            }
        }

        // Override viewSafeAreaInsetsDidChange — called whenever the safe
        // area changes independently of a full layout pass (e.g. a
        // floating/undocked external-keyboard toolbar). Reuses
        // `handle_layout_change`'s bounds/insets recomputation, which is a
        // cheap no-op when nothing has actually changed.
        extern "C" fn view_safe_area_insets_did_change(this: *mut AnyObject, _sel: Sel) {
            unsafe {
                let superclass = class!(UIViewController);
                let _: () = msg_send![super(this, superclass), viewSafeAreaInsetsDidChange];
            }

            if let Some(wrapper) = super::ffi::IOS_WINDOW_LIST.get() {
                unsafe {
                    let windows = &*wrapper.0.get();
                    for &window_ptr in windows.iter() {
                        if !window_ptr.is_null() {
                            let window = &*window_ptr;
                            window.handle_layout_change();
                            window.notify_insets_changed();
                        }
                    }
                }
            }
        }

        // Override traitCollectionDidChange: — fires for any trait change
        // (size class, display scale, user interface style/appearance...).
        // We only care about light/dark appearance here; `IosWindow`
        // de-duplicates against unrelated trait changes.
        extern "C" fn trait_collection_did_change(
            this: *mut AnyObject,
            _sel: Sel,
            previous: *mut AnyObject,
        ) {
            unsafe {
                let superclass = class!(UIViewController);
                let _: () = msg_send![super(this, superclass), traitCollectionDidChange: previous];
            }

            if let Some(wrapper) = super::ffi::IOS_WINDOW_LIST.get() {
                unsafe {
                    let windows = &*wrapper.0.get();
                    for &window_ptr in windows.iter() {
                        if !window_ptr.is_null() {
                            let window = &*window_ptr;
                            window.notify_appearance_changed_if_needed();
                        }
                    }
                }
            }
        }

        unsafe {
            decl.add_method(
                sel!(preferredStatusBarStyle),
                preferred_status_bar_style as extern "C" fn(*mut AnyObject, Sel) -> isize,
            );
            decl.add_method(
                sel!(viewDidLayoutSubviews),
                view_did_layout_subviews as extern "C" fn(*mut AnyObject, Sel),
            );
            decl.add_method(
                sel!(viewSafeAreaInsetsDidChange),
                view_safe_area_insets_did_change as extern "C" fn(*mut AnyObject, Sel),
            );
            decl.add_method(
                sel!(traitCollectionDidChange:),
                trait_collection_did_change as extern "C" fn(*mut AnyObject, Sel, *mut AnyObject),
            );
        }

        decl.register();
    });

    class!(GPUIViewController)
}

/// Set the iOS status bar content style (light or dark text/icons).
///
/// This updates the stored style and asks the root view controller
/// to re-query `preferredStatusBarStyle`.
pub fn set_status_bar_style(style: crate::StatusBarContentStyle) {
    use crate::StatusBarContentStyle;

    let value = match style {
        StatusBarContentStyle::Light => 1,
        StatusBarContentStyle::Dark => 0,
    };
    STATUS_BAR_STYLE.store(value, std::sync::atomic::Ordering::Relaxed);

    // Ask UIKit to re-query the status bar style
    unsafe {
        if let Some(wrapper) = super::ffi::IOS_WINDOW_LIST.get() {
            let windows = &*wrapper.0.get();
            if let Some(&window_ptr) = windows.last() {
                if !window_ptr.is_null() {
                    let window = &*window_ptr;
                    let vc = window.view_controller;
                    if !vc.is_null() {
                        let _: () = msg_send![vc, setNeedsStatusBarAppearanceUpdate];
                    }
                }
            }
        }
    }
}

/// Register a custom UIView subclass that uses CAMetalLayer as its backing layer.
/// This is required for Metal rendering on iOS.
fn register_metal_view_class() -> &'static AnyClass {
    METAL_VIEW_CLASS_REGISTERED.call_once(|| {
        let superclass = class!(UIView);
        let mut decl = ClassBuilder::new(c"GPUIMetalView", superclass).unwrap();

        // Add ivar to store window pointer for touch handling
        decl.add_ivar::<*mut std::ffi::c_void>(c"gpui_window_ptr");

        // Override layerClass to return CAMetalLayer
        extern "C" fn layer_class(_self: *const AnyClass, _sel: Sel) -> *const AnyClass {
            class!(CAMetalLayer) as *const AnyClass
        }

        // Touch handling methods
        extern "C" fn touches_began(
            this: *mut AnyObject,
            _sel: Sel,
            touches: *mut AnyObject,
            event: *mut AnyObject,
        ) {
            handle_touches(this, touches, event);
        }

        extern "C" fn touches_moved(
            this: *mut AnyObject,
            _sel: Sel,
            touches: *mut AnyObject,
            event: *mut AnyObject,
        ) {
            handle_touches(this, touches, event);
        }

        extern "C" fn touches_ended(
            this: *mut AnyObject,
            _sel: Sel,
            touches: *mut AnyObject,
            event: *mut AnyObject,
        ) {
            handle_touches(this, touches, event);
        }

        extern "C" fn touches_cancelled(
            this: *mut AnyObject,
            _sel: Sel,
            touches: *mut AnyObject,
            event: *mut AnyObject,
        ) {
            handle_touches(this, touches, event);
        }

        unsafe {
            // Add class method for layerClass
            decl.add_class_method(
                sel!(layerClass),
                layer_class as extern "C" fn(*const AnyClass, Sel) -> *const AnyClass,
            );

            // Add touch handling instance methods
            decl.add_method(
                sel!(touchesBegan:withEvent:),
                touches_began as extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, *mut AnyObject),
            );
            decl.add_method(
                sel!(touchesMoved:withEvent:),
                touches_moved as extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, *mut AnyObject),
            );
            decl.add_method(
                sel!(touchesEnded:withEvent:),
                touches_ended as extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, *mut AnyObject),
            );
            decl.add_method(
                sel!(touchesCancelled:withEvent:),
                touches_cancelled
                    as extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, *mut AnyObject),
            );
        }

        decl.register();
    });

    class!(GPUIMetalView)
}

/// Handle touch events from the GPUIMetalView
fn handle_touches(view: *mut AnyObject, touches: *mut AnyObject, event: *mut AnyObject) {
    unsafe {
        // Get the window pointer from the view's ivar
        #[allow(deprecated)]
        let window_ptr: *mut std::ffi::c_void = *(*view).get_ivar(GPUI_WINDOW_IVAR);
        if window_ptr.is_null() {
            log::warn!("GPUI iOS: Touch event but no window pointer set");
            return;
        }

        let window = &*(window_ptr as *const IosWindow);

        // Get all touches from the set
        let all_touches: *mut AnyObject = msg_send![touches, allObjects];
        let count: usize = msg_send![all_touches, count];

        for i in 0..count {
            let touch: *mut AnyObject = msg_send![all_touches, objectAtIndex: i];
            window.handle_touch(touch, event);
        }
    }
}

/// iOS Window backed by UIWindow + UIViewController.
#[allow(clippy::type_complexity)]
pub(crate) struct IosWindow {
    /// The UIWindow object
    window: *mut AnyObject,
    /// The UIViewController
    view_controller: *mut AnyObject,
    /// The Metal-backed UIView
    view: *mut AnyObject,
    /// The hidden text input view for keyboard input
    text_input_view: *mut AnyObject,
    /// Current bounds in pixels
    bounds: Cell<Bounds<Pixels>>,
    /// Scale factor
    scale_factor: Cell<f32>,
    /// Input handler for text input
    input_handler: RefCell<Option<PlatformInputHandler>>,
    /// Callback for frame requests
    /// Note: pub(super) to allow ffi.rs to access this for the display link callback
    pub(super) request_frame_callback: RefCell<Option<Box<dyn FnMut(RequestFrameOptions)>>>,
    /// Callback for input events
    input_callback: RefCell<Option<Box<dyn FnMut(PlatformInput) -> DispatchEventResult>>>,
    /// Callback for active status changes
    active_status_callback: RefCell<Option<Box<dyn FnMut(bool)>>>,
    /// Callback for hover status changes (not really applicable on iOS)
    hover_status_callback: RefCell<Option<Box<dyn FnMut(bool)>>>,
    /// Callback for resize events
    resize_callback: RefCell<Option<Box<dyn FnMut(Size<Pixels>, f32)>>>,
    /// Callback for move events (not applicable on iOS)
    moved_callback: RefCell<Option<Box<dyn FnMut()>>>,
    /// Callback for should close
    should_close_callback: RefCell<Option<Box<dyn FnMut() -> bool>>>,
    /// Callback for hit test
    hit_test_callback: RefCell<Option<Box<dyn FnMut() -> Option<WindowControlArea>>>>,
    /// Callback for close
    close_callback: RefCell<Option<Box<dyn FnOnce()>>>,
    /// Callback for appearance changes
    appearance_changed_callback: RefCell<Option<Box<dyn FnMut()>>>,
    /// Current mouse position (from touch)
    mouse_position: Cell<Point<Pixels>>,
    /// Current modifiers
    modifiers: Cell<Modifiers>,
    /// Track if a touch is currently pressed
    touch_pressed: Cell<bool>,
    /// Stable `TouchId`s for the `UITouch` objects currently on screen,
    /// keyed by object address, plus the next id to hand out. UIKit keeps a
    /// `UITouch` instance alive (same pointer) for the lifetime of one finger
    /// contact, so the address is a reliable per-contact key.
    active_touches: RefCell<HashMap<usize, u64>>,
    next_touch_id: Cell<u64>,
    /// The wgpu renderer (Metal backend on iOS).
    /// Wrapped in a `Mutex<Option<…>>` so that `draw()` (called from the
    /// `request_frame` callback) can acquire a mutable reference without
    /// conflicting with the outer `&self` borrow.
    renderer: Mutex<Option<WgpuRenderer>>,
    /// Callback invoked whenever [`PlatformWindow::insets`] changes — either
    /// exactly (safe-area/layout changes) or on every frame while a keyboard
    /// show/hide animation is interpolating (see `pump_insets_animation`).
    insets_changed_callback: RefCell<Option<Box<dyn FnMut(gpui::WindowInsets)>>>,
    /// In-flight keyboard inset animation, advanced once per frame from
    /// `pump_insets_animation`. `None` when at rest.
    keyboard_animation: Cell<Option<KeyboardAnimation>>,
    /// The current (settled) keyboard/IME inset in logical points, i.e. the
    /// value the animation above is animating *towards*, and the value
    /// reported once it settles.
    ime_inset_bottom: Cell<f32>,
    /// The appearance last reported via `on_appearance_changed`, so
    /// `GPUIViewController`'s `traitCollectionDidChange:` override (which
    /// fires for any trait change, not just dark/light) can suppress
    /// duplicate callbacks.
    last_appearance: Cell<WindowAppearance>,
}

/// Tracks an in-flight keyboard show/hide inset animation.
///
/// iOS reports the keyboard's animation curve/duration via
/// `UIKeyboardWillShow/HideNotification`'s `userInfo`, but delivers it as a
/// single instantaneous notification rather than a stream of frame updates
/// the way Android's `WindowInsetsAnimation` callback does. To honor
/// [`PlatformWindow::on_insets_changed`]'s contract ("fires continuously
/// during animated transitions... on iOS the platform interpolates the
/// keyboard animation curve on frame ticks"), we linearly interpolate
/// between the pre- and post-animation inset ourselves, driven by
/// `pump_insets_animation` on every `CADisplayLink` tick.
///
/// The interpolation is linear rather than matching UIKit's actual curve
/// (typically an ease-in-out) because `UIKeyboardAnimationCurveUserInfoKey`
/// reports a raw `UIViewAnimationCurve` enum value, not reusable easing
/// coefficients, and iOS 18 deprecated the API needed to convert it into a
/// `UIView` animation block we could sample. Linear interpolation over the
/// correct duration is a reasonable approximation and, crucially, always
/// reaches the exact end value.
#[derive(Clone, Copy, Debug)]
struct KeyboardAnimation {
    start_bottom: f32,
    end_bottom: f32,
    started_at: std::time::Instant,
    duration: std::time::Duration,
}

impl KeyboardAnimation {
    /// Returns the interpolated inset for "now", and whether the animation
    /// has finished (in which case the returned value is exactly `end_bottom`).
    fn sample(&self) -> (f32, bool) {
        let elapsed = self.started_at.elapsed();
        if elapsed >= self.duration || self.duration.is_zero() {
            (self.end_bottom, true)
        } else {
            let t = elapsed.as_secs_f32() / self.duration.as_secs_f32();
            let value = self.start_bottom + (self.end_bottom - self.start_bottom) * t;
            (value, false)
        }
    }
}

// Required for raw_window_handle
unsafe impl Send for IosWindow {}
unsafe impl Sync for IosWindow {}

impl IosWindow {
    pub fn new(handle: AnyWindowHandle, _params: WindowParams) -> anyhow::Result<Self> {
        // Create the window on the main screen
        let screen = IosDisplay::main();
        let screen_bounds = screen.bounds();
        let scale_factor = screen.scale();

        unsafe {
            // Create UIWindow
            let screen_obj: *mut AnyObject = msg_send![class!(UIScreen), mainScreen];
            let screen_bounds_cg: ObjcCGRect = msg_send![screen_obj, bounds];
            let window: *mut AnyObject = msg_send![class!(UIWindow), alloc];
            let window: *mut AnyObject = msg_send![window, initWithFrame: screen_bounds_cg];

            // Create our custom UIViewController subclass that supports
            // dynamic `preferredStatusBarStyle` overrides.
            let vc_class = register_view_controller_class();
            let view_controller: *mut AnyObject = msg_send![vc_class, alloc];
            let view_controller: *mut AnyObject = msg_send![view_controller, init];

            // Create our custom Metal view using the registered class
            let metal_view_class = register_metal_view_class();
            let view: *mut AnyObject = msg_send![metal_view_class, alloc];
            let view: *mut AnyObject = msg_send![view, initWithFrame: screen_bounds_cg];

            // Configure the Metal layer — wgpu will use it for rendering but
            // we still need to set contentsScale so the drawable size is correct.
            let layer: *mut AnyObject = msg_send![view, layer];
            let scale: core_graphics::base::CGFloat = msg_send![screen_obj, scale];
            let _: () = msg_send![layer, setContentsScale: scale];

            // Auto-resize the Metal view when the parent view changes size
            // (e.g. rotation). UIViewAutoresizingFlexibleWidth | UIViewAutoresizingFlexibleHeight
            let _: () = msg_send![view, setAutoresizingMask: 18_usize]; // 0x02 | 0x10

            // Enable user interaction on the Metal view for touch handling
            let _: () = msg_send![view, setUserInteractionEnabled: true];
            let _: () = msg_send![view, setMultipleTouchEnabled: true];

            // Set the view as the view controller's view
            let _: () = msg_send![view_controller, setView: view];

            // Set the root view controller
            let _: () = msg_send![window, setRootViewController: view_controller];

            // Make the window visible
            let _: () = msg_send![window, makeKeyAndVisible];

            // Create a hidden text input view for keyboard handling.
            // Uses our custom GPUITextInputView which implements UIKeyInput
            // so iOS actually routes keyboard text to us.
            let text_input_class = super::text_input_view::register_text_input_view_class();
            let text_input_view: *mut AnyObject = msg_send![text_input_class, alloc];
            let text_input_frame = ObjcCGRect::new(0.0, 0.0, 1.0, 1.0);
            let text_input_view: *mut AnyObject =
                msg_send![text_input_view, initWithFrame: text_input_frame];
            let _: () = msg_send![text_input_view, setAlpha: 0.01_f64];
            let _: () = msg_send![text_input_view, setUserInteractionEnabled: true];
            let _: () = msg_send![view, addSubview: text_input_view];

            // --- Initialise the wgpu renderer (Metal backend) ---------------
            let pixel_w = (screen_bounds_cg.width * scale) as i32;
            let pixel_h = (screen_bounds_cg.height * scale) as i32;

            let _handle = handle; // consumed but not stored
            let ios_window = Self {
                window,
                view_controller,
                view,
                text_input_view,
                bounds: Cell::new(screen_bounds),
                scale_factor: Cell::new(scale_factor),
                input_handler: RefCell::new(None),
                request_frame_callback: RefCell::new(None),
                input_callback: RefCell::new(None),
                active_status_callback: RefCell::new(None),
                hover_status_callback: RefCell::new(None),
                resize_callback: RefCell::new(None),
                moved_callback: RefCell::new(None),
                should_close_callback: RefCell::new(None),
                hit_test_callback: RefCell::new(None),
                close_callback: RefCell::new(None),
                appearance_changed_callback: RefCell::new(None),
                mouse_position: Cell::new(Point::default()),
                modifiers: Cell::new(Modifiers::default()),
                touch_pressed: Cell::new(false),
                active_touches: RefCell::new(HashMap::new()),
                next_touch_id: Cell::new(1),
                renderer: Mutex::new(None),
                insets_changed_callback: RefCell::new(None),
                keyboard_animation: Cell::new(None),
                ime_inset_bottom: Cell::new(0.0),
                last_appearance: Cell::new(WindowAppearance::Light),
            };

            // Create the wgpu renderer using the Metal backend.
            //
            // `gpui_wgpu::WgpuContext::instance()` only enables Vulkan+GL,
            // so we create our own wgpu instance with Metal enabled, build
            // a surface from the UIView's raw window handle, construct the
            // WgpuContext with that instance, and finally create the renderer.
            let config = WgpuSurfaceConfig {
                size: size(DevicePixels(pixel_w), DevicePixels(pixel_h)),
                transparent: false,
                preferred_present_mode: None,
            };

            let metal_instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::METAL,
                flags: wgpu::InstanceFlags::default(),
                backend_options: wgpu::BackendOptions::default(),
                memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
                display: None,
            });

            let raw_window = RawIosWindow {
                view: ios_window.view as *mut c_void,
            };

            // Build a temporary surface for WgpuContext initialisation
            // (adapter selection needs a surface to test compatibility).
            let window_handle = raw_window
                .window_handle()
                .expect("iOS window handle unavailable");
            let display_handle = raw_window
                .display_handle()
                .expect("iOS display handle unavailable");

            let target = wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(display_handle.as_raw()),
                raw_window_handle: window_handle.as_raw(),
            };

            let surface_result = metal_instance.create_surface_unsafe(target);
            match surface_result {
                Ok(surface) => match WgpuContext::new(metal_instance, &surface, None, None) {
                    Ok(context) => {
                        // Pre-populate gpu_context so WgpuRenderer::new()
                        // reuses our Metal-backed context (and its instance)
                        // instead of creating a Vulkan+GL one.
                        let gpu_context: GpuContext = Rc::new(RefCell::new(Some(context)));
                        drop(surface); // no longer needed — new() creates its own

                        match WgpuRenderer::new(gpu_context, &raw_window, config, None, None) {
                            Ok(renderer) => {
                                log::info!("iOS wgpu renderer created (Metal)");
                                *ios_window.renderer.lock() = Some(renderer);
                            }
                            Err(e) => {
                                log::error!("Failed to create iOS wgpu renderer: {e:#}");
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to create iOS WgpuContext: {e:#}");
                    }
                },
                Err(e) => {
                    log::error!("Failed to create iOS wgpu Metal surface: {e:#}");
                }
            }

            Ok(ios_window)
        }
    }

    /// Get the raw pointer to the UIViewController.
    pub fn view_controller_ptr(&self) -> *mut AnyObject {
        self.view_controller
    }

    /// Get the raw pointer to the GPUIMetalView.
    #[allow(dead_code)]
    pub fn metal_view_ptr(&self) -> *mut AnyObject {
        self.view
    }

    /// Register this window with the FFI layer after it's been stored.
    /// This must be called after the window is placed at a stable address
    /// (e.g., in a Box or Arc).
    pub(crate) fn register_with_ffi(&self) {
        super::ffi::register_window(self as *const Self);

        // Set the window pointer on the view so touch events can find us,
        // and on the text input view so keyboard input can find us.
        unsafe {
            let window_ptr = self as *const Self as *mut std::ffi::c_void;
            #[allow(deprecated)]
            {
                *(*self.view).get_mut_ivar::<*mut c_void>(GPUI_WINDOW_IVAR) = window_ptr;
            }
            #[allow(deprecated)]
            {
                *(*self.text_input_view).get_mut_ivar::<*mut c_void>(GPUI_WINDOW_IVAR) = window_ptr;
            }
            log::info!(
                "GPUI iOS: Set window pointer {:p} on view {:p} and text input {:p}",
                window_ptr,
                self.view,
                self.text_input_view
            );
        }

        // Listen for keyboard show/hide so we can expose the keyboard height.
        self.register_keyboard_observers();
    }

    /// Register for keyboard show/hide notifications so we can track the
    /// keyboard height and allow the UI to shift content above the keyboard.
    pub(crate) fn register_keyboard_observers(&self) {
        unsafe {
            let notification_center: *mut AnyObject =
                msg_send![class!(NSNotificationCenter), defaultCenter];

            let show_name = crate::ios::util::nsstring("UIKeyboardWillShowNotification");
            let hide_name = crate::ios::util::nsstring("UIKeyboardWillHideNotification");

            // Captured as a `usize` (not a typed pointer) purely so the
            // `'static` block2 closures below don't need `Send`/`Sync`
            // impls for `*const Self` — it's only ever dereferenced back on
            // the main thread, same as every other pointer in this module.
            let window_ptr = self as *const Self as usize;

            // Extracts the animation duration from a keyboard notification's
            // userInfo, defaulting to UIKit's standard 0.25s if absent.
            unsafe fn animation_duration_secs(user_info: *mut AnyObject) -> f64 {
                unsafe {
                    if user_info.is_null() {
                        return 0.25;
                    }
                    let duration_key =
                        crate::ios::util::nsstring("UIKeyboardAnimationDurationUserInfoKey");
                    let duration_value: *mut AnyObject =
                        msg_send![user_info, objectForKey: duration_key];
                    if duration_value.is_null() {
                        0.25
                    } else {
                        msg_send![duration_value, doubleValue]
                    }
                }
            }

            // Block that fires when the keyboard is about to appear —
            // extracts the end-frame height and the animation duration, and
            // kicks off an interpolated inset animation on the window that
            // registered this observer.
            let show_block = block2::RcBlock::new(move |notification: *mut AnyObject| {
                if notification.is_null() {
                    return;
                }
                let user_info: *mut AnyObject = msg_send![notification, userInfo];
                if user_info.is_null() {
                    return;
                }
                let frame_key = crate::ios::util::nsstring("UIKeyboardFrameEndUserInfoKey");
                let frame_value: *mut AnyObject = msg_send![user_info, objectForKey: frame_key];
                // frame_key is autoreleased by util::nsstring — no manual release needed
                let _ = frame_key;
                if frame_value.is_null() {
                    return;
                }
                let frame: ObjcCGRect = msg_send![frame_value, CGRectValue];
                let height = frame.height as f32;
                let duration = animation_duration_secs(user_info);

                log::info!(
                    "GPUI iOS: Keyboard will show, height={}, duration={:.3}s",
                    height,
                    duration
                );
                crate::set_keyboard_height(height);

                let window = &*(window_ptr as *const Self);
                window.start_keyboard_inset_animation(height, duration);
            });

            let hide_block = block2::RcBlock::new(move |notification: *mut AnyObject| {
                log::info!("GPUI iOS: Keyboard will hide");
                crate::set_keyboard_height(0.0);

                let user_info: *mut AnyObject = if notification.is_null() {
                    std::ptr::null_mut()
                } else {
                    msg_send![notification, userInfo]
                };
                let duration = animation_duration_secs(user_info);

                let window = &*(window_ptr as *const Self);
                window.start_keyboard_inset_animation(0.0, duration);
            });

            let _: *mut AnyObject = msg_send![notification_center,
                addObserverForName: show_name,
                object: std::ptr::null::<AnyObject>(),
                queue: std::ptr::null::<AnyObject>(),
                usingBlock: &*show_block
            ];
            let _: *mut AnyObject = msg_send![notification_center,
                addObserverForName: hide_name,
                object: std::ptr::null::<AnyObject>(),
                queue: std::ptr::null::<AnyObject>(),
                usingBlock: &*hide_block
            ];
            // show_name and hide_name are autoreleased by util::nsstring

            // Leak the blocks so they live for the app lifetime.
            std::mem::forget(show_block);
            std::mem::forget(hide_block);
        }
    }

    /// Computes the current [`gpui::WindowInsets`] from the live safe-area
    /// insets plus the settled (not mid-animation) IME inset. Used both by
    /// [`PlatformWindow::insets`] and to report the *final*, exact value
    /// once a keyboard animation completes.
    fn compute_settled_insets(&self) -> gpui::WindowInsets {
        let (top, bottom, left, right) = self.safe_area_insets();
        gpui::WindowInsets {
            safe_area: gpui::Edges {
                top: px(top),
                bottom: px(bottom),
                left: px(left),
                right: px(right),
            },
            ime: gpui::Edges {
                top: px(0.),
                bottom: px(self.ime_inset_bottom.get()),
                left: px(0.),
                right: px(0.),
            },
        }
    }

    /// Starts (or retargets) the keyboard inset animation towards
    /// `end_bottom` over `duration_secs`. If `duration_secs` is ~0 (some
    /// devices report a zero duration for an already-visible keyboard
    /// changing type), the inset is applied immediately instead.
    fn start_keyboard_inset_animation(&self, end_bottom: f32, duration_secs: f64) {
        let start_bottom = self
            .keyboard_animation
            .get()
            .map(|anim| anim.sample().0)
            .unwrap_or_else(|| self.ime_inset_bottom.get());

        if duration_secs <= 0.001 {
            self.keyboard_animation.set(None);
            self.ime_inset_bottom.set(end_bottom);
            self.notify_insets_changed();
            return;
        }

        self.keyboard_animation.set(Some(KeyboardAnimation {
            start_bottom,
            end_bottom,
            started_at: std::time::Instant::now(),
            duration: std::time::Duration::from_secs_f64(duration_secs),
        }));
    }

    /// Advances the in-flight keyboard inset animation by one frame,
    /// firing `on_insets_changed` with the interpolated value. Called from
    /// `gpui_ios_request_frame` on every `CADisplayLink` tick, alongside
    /// `pump_momentum`. A no-op when no animation is in flight.
    pub(crate) fn pump_insets_animation(&self) {
        let Some(animation) = self.keyboard_animation.get() else {
            return;
        };
        let (value, finished) = animation.sample();
        self.ime_inset_bottom.set(value);
        if finished {
            self.keyboard_animation.set(None);
        }
        self.notify_insets_changed();
    }

    /// Fires the `on_insets_changed` callback with the current insets, if a
    /// callback is registered. Intersects the IME inset against the window's
    /// own bounds implicitly (via `ime_inset_bottom`, which is only ever set
    /// from `UIKeyboardWillShow/HideNotification` frames that UIKit already
    /// clips to the screen) — an external or floating/undocked iPad keyboard
    /// reports a frame outside the window's bounds and UIKit's notification
    /// height for those is 0, so no special-casing is needed here.
    pub(crate) fn notify_insets_changed(&self) {
        let insets = self.compute_settled_insets();
        if let Some(callback) = self.insets_changed_callback.borrow_mut().as_mut() {
            callback(insets);
        }
    }

    /// Fires `on_appearance_changed` if the current appearance differs from
    /// the last one reported. Called from `traitCollectionDidChange:`,
    /// which fires for *any* trait change (size class, scale, etc.), not
    /// just light/dark mode.
    pub(crate) fn notify_appearance_changed_if_needed(&self) {
        let current = PlatformWindow::appearance(self);
        if current != self.last_appearance.get() {
            self.last_appearance.set(current);
            if let Some(callback) = self.appearance_changed_callback.borrow_mut().as_mut() {
                callback();
            }
        }
    }

    /// Forward one `UITouch` update to GPUI as a raw [`gpui::TouchEvent`].
    ///
    /// GPUI's window owns touch gesture recognition (`TouchGestureRecognizer`
    /// in `crates/gpui/src/gestures.rs`): it turns raw touches into taps,
    /// pans with momentum, long presses and touch drags using the tuning from
    /// `Platform::gestures()`. The iOS layer therefore does no tap-versus-
    /// scroll disambiguation of its own; it only assigns a stable `TouchId`
    /// per finger contact and reports position, phase and force.
    pub fn handle_touch(&self, touch: *mut AnyObject, _event: *mut AnyObject) {
        use gpui::{TouchEvent, TouchId, TouchPhase};

        let position = touch_location_in_view(touch, self.view);
        let phase = match touch_phase(touch) {
            UITouchPhase::Began => TouchPhase::Started,
            UITouchPhase::Moved => TouchPhase::Moved,
            // UIKit reports Stationary for fingers that did not move while
            // another finger did; GPUI has no use for it.
            UITouchPhase::Stationary => return,
            UITouchPhase::Ended => TouchPhase::Ended,
            UITouchPhase::Cancelled => TouchPhase::Cancelled,
        };

        let key = touch as usize;
        let id = match phase {
            TouchPhase::Started => {
                let id = self.next_touch_id.get();
                self.next_touch_id.set(id + 1);
                self.active_touches.borrow_mut().insert(key, id);
                self.touch_pressed.set(true);
                id
            }
            TouchPhase::Moved => match self.active_touches.borrow().get(&key) {
                Some(&id) => id,
                None => return,
            },
            TouchPhase::Ended | TouchPhase::Cancelled => {
                let removed = self.active_touches.borrow_mut().remove(&key);
                if self.active_touches.borrow().is_empty() {
                    self.touch_pressed.set(false);
                }
                match removed {
                    Some(id) => id,
                    None => return,
                }
            }
        };

        self.mouse_position.set(position);

        let force = unsafe {
            let max: f64 = msg_send![touch, maximumPossibleForce];
            if max > 0.0 {
                let force: f64 = msg_send![touch, force];
                Some((force / max).clamp(0.0, 1.0) as f32)
            } else {
                None
            }
        };

        let event = PlatformInput::Touch(TouchEvent {
            id: TouchId(id),
            phase,
            position,
            predicted_position: None,
            force,
        });
        if let Some(callback) = self.input_callback.borrow_mut().as_mut() {
            callback(event);
        }
    }

    /// Query the safe area insets from the UIView.
    ///
    /// Returns `(top, bottom, left, right)` in logical points.
    /// These represent the areas occupied by system UI (status bar,
    /// home indicator, camera notch) that content should avoid.
    pub fn safe_area_insets(&self) -> (f32, f32, f32, f32) {
        if self.view.is_null() {
            return (0.0, 0.0, 0.0, 0.0);
        }
        unsafe {
            // UIEdgeInsets { top, left, bottom, right } — all CGFloat
            #[repr(C)]
            #[derive(Debug, Clone, Copy)]
            struct UIEdgeInsets {
                top: f64,
                left: f64,
                bottom: f64,
                right: f64,
            }

            unsafe impl Encode for UIEdgeInsets {
                const ENCODING: Encoding = Encoding::Struct(
                    "UIEdgeInsets",
                    &[
                        Encoding::Double,
                        Encoding::Double,
                        Encoding::Double,
                        Encoding::Double,
                    ],
                );
            }

            unsafe impl RefEncode for UIEdgeInsets {
                const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
            }

            let insets: UIEdgeInsets = msg_send![self.view, safeAreaInsets];
            (
                insets.top as f32,
                insets.bottom as f32,
                insets.left as f32,
                insets.right as f32,
            )
        }
    }

    /// Show the software keyboard with the specified keyboard type.
    ///
    /// The actual `becomeFirstResponder` call is deferred to the next run-loop
    /// iteration via `performSelector:withObject:afterDelay:` to avoid re-entering
    /// GPUI's event dispatch while an entity lease is active (UIKit's keyboard
    /// presentation can synchronously trigger layout callbacks).
    pub fn show_keyboard_with_type(&self, keyboard_type: crate::KeyboardType) {
        log::info!("GPUI iOS: Showing keyboard (type={:?})", keyboard_type);
        unsafe {
            use crate::KeyboardType;
            let kb_type: isize = match keyboard_type {
                KeyboardType::Default => 0,      // UIKeyboardTypeDefault
                KeyboardType::EmailAddress => 7, // UIKeyboardTypeEmailAddress
                KeyboardType::Phone => 5,        // UIKeyboardTypePhonePad
                KeyboardType::NumberPad => 4,    // UIKeyboardTypeNumberPad
                KeyboardType::URL => 3,          // UIKeyboardTypeURL
                KeyboardType::Decimal => 8,      // UIKeyboardTypeDecimalPad
            };
            log::info!(
                "GPUI iOS: text_input_view={:p}, setKeyboardType: {}",
                self.text_input_view,
                kb_type
            );
            if self.text_input_view.is_null() {
                log::error!("GPUI iOS: text_input_view is NULL!");
                return;
            }
            let _: () = msg_send![self.text_input_view, setKeyboardType: kb_type];
            log::info!("GPUI iOS: setAutocorrectionType");
            let _: () = msg_send![self.text_input_view, setAutocorrectionType: 1_isize];
            log::info!("GPUI iOS: setAutocapitalizationType");
            let _: () = msg_send![self.text_input_view, setAutocapitalizationType: 0_isize];
            // UIReturnKeyGo (6) for URL fields (matches Safari/typical browser
            // UX), UIReturnKeyDefault (0) otherwise.
            let return_key_type: isize = if matches!(keyboard_type, KeyboardType::URL) {
                6
            } else {
                0
            };
            let _: () = msg_send![self.text_input_view, setReturnKeyType: return_key_type];
            log::info!("GPUI iOS: scheduling becomeFirstResponder");

            // Defer becomeFirstResponder to the next run-loop iteration.
            let _: () = msg_send![self.text_input_view,
                performSelector: sel!(becomeFirstResponder),
                withObject: ptr::null::<AnyObject>(),
                afterDelay: 0.0_f64
            ];
            log::info!("GPUI iOS: show_keyboard_with_type done");
        }
    }

    /// Hide the software keyboard.
    ///
    /// Deferred to the next run-loop iteration (like `show_keyboard_with_type`)
    /// to avoid re-entering GPUI event dispatch.
    pub fn hide_keyboard(&self) {
        log::info!("GPUI iOS: Hiding keyboard");
        unsafe {
            let _: () = msg_send![self.text_input_view,
                performSelector: sel!(resignFirstResponder),
                withObject: ptr::null::<AnyObject>(),
                afterDelay: 0.0_f64
            ];
        }
    }

    /// Runs `f` against the current `PlatformInputHandler`, if one is set.
    ///
    /// Follows the same take-then-restore pattern as `gpui_macos`'s
    /// `with_input_handler`: the handler is removed from `input_handler`
    /// for the duration of the call, so a re-entrant call (e.g. UIKit
    /// synchronously asking for `selectedTextRange` from inside a callback
    /// we're already driving) sees "no handler" instead of double-borrowing
    /// the `RefCell`, and a `cx.update` failure inside `PlatformInputHandler`
    /// (GPUI mid-update) is already surfaced as `None`/no-op by its methods
    /// rather than panicking.
    pub(crate) fn with_input_handler<R>(
        &self,
        f: impl FnOnce(&mut PlatformInputHandler) -> R,
    ) -> Option<R> {
        let taken = self.input_handler.borrow_mut().take();
        let mut handler = taken?;
        let result = f(&mut handler);
        *self.input_handler.borrow_mut() = Some(handler);
        Some(result)
    }

    /// Whether a real `EntityInputHandler`-backed `PlatformInputHandler` is
    /// currently set. When `false`, text input falls back to the legacy
    /// global-callback bridge (`crate::dispatch_text_input`) for backward
    /// compatibility with `components::material::text_input` and
    /// `examples/ios_browser`.
    pub(crate) fn has_real_input_handler(&self) -> bool {
        self.input_handler.borrow().is_some()
    }

    /// Handle text input from the software keyboard (`UIKeyInput::insertText:`).
    ///
    /// When a real `PlatformInputHandler` is set (a GPUI view using
    /// `EntityInputHandler`), input is routed through it: a literal newline
    /// is first dispatched as an `enter` `KeyDown` through the window's
    /// input callback (so `on_action`/keymap bindings fire, mirroring
    /// desktop key-then-IME ordering), and only inserted as text if that
    /// event was not `default_prevented`. Otherwise this falls back to the
    /// legacy global-callback bridge for callers that haven't migrated to
    /// `EntityInputHandler` yet.
    pub fn handle_text_input(&self, text: *mut AnyObject) {
        if text.is_null() {
            return;
        }

        unsafe {
            // Convert NSString to Rust String
            let utf8: *const i8 = msg_send![text, UTF8String];
            if utf8.is_null() {
                return;
            }

            let text_str = std::ffi::CStr::from_ptr(utf8)
                .to_string_lossy()
                .into_owned();

            log::info!("GPUI iOS: Text input: {:?}", text_str);

            if self.has_real_input_handler() {
                if text_str == "\n" {
                    let keystroke = gpui::Keystroke {
                        modifiers: Modifiers::default(),
                        key: "enter".to_string(),
                        key_char: Some("\n".to_string()),
                    };
                    let event = PlatformInput::KeyDown(gpui::KeyDownEvent {
                        keystroke,
                        is_held: false,
                        prefer_character_input: false,
                    });
                    let result = self
                        .input_callback
                        .borrow_mut()
                        .as_mut()
                        .map(|callback| callback(event));
                    if let Some(result) = result {
                        if result.default_prevented {
                            return;
                        }
                    }
                }
                self.with_input_handler(|handler| handler.replace_text_in_range(None, &text_str));
                return;
            }

            // Legacy fallback: the global text input callback (for
            // `components::material::text_input` and older callers that
            // haven't migrated to `EntityInputHandler`).
            crate::dispatch_text_input(&text_str);

            // Send key events through GPUI's input callback so GPUI triggers
            // a re-render cycle (which runs drain_pending_text and updates
            // the UI).
            for c in text_str.chars() {
                let keystroke = gpui::Keystroke {
                    modifiers: Modifiers::default(),
                    key: c.to_string(),
                    key_char: Some(c.to_string()),
                };

                let event = PlatformInput::KeyDown(gpui::KeyDownEvent {
                    keystroke,
                    is_held: false,
                    prefer_character_input: true,
                });

                if let Some(callback) = self.input_callback.borrow_mut().as_mut() {
                    callback(event);
                }
            }
        }
    }

    /// Handle the delete-backward action from the software keyboard.
    ///
    /// With a real `PlatformInputHandler`, deletes one character before the
    /// caret (surrogate-pair aware, never splitting a UTF-16 low/high
    /// surrogate pair) via `replace_text_in_range`, or deletes the current
    /// selection if it is non-empty. Otherwise falls back to the legacy
    /// global-callback sentinel (`"\x08"`) for older callers.
    pub fn handle_delete_backward(&self) {
        log::info!("GPUI iOS: deleteBackward");

        if self.has_real_input_handler() {
            let handled = self.with_input_handler(|handler| {
                let selection = handler.selected_text_range(true);
                let Some(selection) = selection else {
                    return false;
                };
                if !selection.range.is_empty() {
                    handler.replace_text_in_range(Some(selection.range), "");
                    return true;
                }
                let caret = selection.range.start;
                if caret == 0 {
                    return true;
                }
                // Delete one UTF-16 code unit, or two if the unit
                // immediately before the caret is a low surrogate (so we
                // never split a surrogate pair).
                let mut adjusted = None;
                let delete_len = handler
                    .text_for_range(caret.saturating_sub(2)..caret, &mut adjusted)
                    .and_then(|s| {
                        let units: Vec<u16> = s.encode_utf16().collect();
                        units.last().map(|&last| {
                            if (0xDC00..=0xDFFF).contains(&last) && units.len() >= 2 {
                                2
                            } else {
                                1
                            }
                        })
                    })
                    .unwrap_or(1);
                let start = caret.saturating_sub(delete_len);
                handler.replace_text_in_range(Some(start..caret), "");
                true
            });
            if handled == Some(true) {
                return;
            }
        }

        // Legacy fallback: global callback sentinel (backspace = "\x08").
        crate::dispatch_text_input("\x08");

        // Always send a Backspace KeyDown event through GPUI to trigger
        // a re-render cycle (which runs drain_pending_text).
        let keystroke = gpui::Keystroke {
            modifiers: Modifiers::default(),
            key: "backspace".to_string(),
            key_char: None,
        };
        let event = PlatformInput::KeyDown(gpui::KeyDownEvent {
            keystroke,
            is_held: false,
            prefer_character_input: false,
        });
        if let Some(callback) = self.input_callback.borrow_mut().as_mut() {
            callback(event);
        }
    }

    /// Handle a key event from an external keyboard
    pub fn handle_key_event(&self, key_code: u32, modifier_flags: u32, is_key_down: bool) {
        use super::text_input::{
            key_code_to_key_down, key_code_to_key_up, key_code_to_string,
            modifier_flags_to_modifiers,
        };

        let key = key_code_to_string(key_code);
        let modifiers = modifier_flags_to_modifiers(modifier_flags);

        log::info!(
            "GPUI iOS: Key event - key: {:?}, modifiers: {:?}, down: {}",
            key,
            modifiers,
            is_key_down
        );

        // On key-down, dispatch cursor-movement control codes through the
        // legacy global text input callback so TextField-based components
        // receive them. Skipped when a real `PlatformInputHandler` is set —
        // those views get arrow/home/end via ordinary `KeyDown` keymap
        // bindings instead.
        if is_key_down && !self.has_real_input_handler() {
            match key_code {
                0x50 => {
                    crate::dispatch_text_input("\x1b[D");
                } // Left arrow
                0x4F => {
                    crate::dispatch_text_input("\x1b[C");
                } // Right arrow
                0x4A => {
                    crate::dispatch_text_input("\x1b[H");
                } // Home
                0x4D => {
                    crate::dispatch_text_input("\x1b[F");
                } // End
                _ => {}
            }
        }

        let event = if is_key_down {
            key_code_to_key_down(key_code, modifier_flags)
        } else {
            key_code_to_key_up(key_code, modifier_flags)
        };

        if let Some(callback) = self.input_callback.borrow_mut().as_mut() {
            callback(event);
        }
    }

    /// Notify the window of active status changes (foreground/background).
    ///
    /// This is called by the FFI layer when the app transitions between
    /// foreground and background states.
    pub fn notify_active_status_change(&self, is_active: bool) {
        log::info!("GPUI iOS: Window active status changed to: {}", is_active);

        if let Some(callback) = self.active_status_callback.borrow_mut().as_mut() {
            callback(is_active);
        }
    }

    /// Handle a layout change (e.g. rotation, split-screen resize).
    ///
    /// Called from `viewDidLayoutSubviews` on the GPUIViewController.
    /// Queries the current UIView bounds, updates the stored bounds/scale,
    /// reconfigures the Metal layer + wgpu surface, and fires the resize callback.
    pub fn handle_layout_change(&self) {
        // Safe-area insets can change independently of bounds/scale (e.g. a
        // rotation that doesn't change the notch's logical side, or a
        // split-view resize) — recompute and notify unconditionally, ahead
        // of the bounds/scale early-return below.
        self.notify_insets_changed();

        unsafe {
            let view_bounds: ObjcCGRect = msg_send![self.view, bounds];
            let screen: *mut AnyObject = msg_send![class!(UIScreen), mainScreen];
            let scale: core_graphics::base::CGFloat = msg_send![screen, scale];

            let new_w = view_bounds.width as f32;
            let new_h = view_bounds.height as f32;
            let new_scale = scale as f32;

            let old_bounds = self.bounds.get();
            let old_scale = self.scale_factor.get();

            let new_size = size(px(new_w), px(new_h));

            // Only process if something actually changed.
            if old_bounds.size == new_size && (old_scale - new_scale).abs() < 0.01 {
                return;
            }

            log::info!(
                "GPUI iOS: Layout changed — {:?} @{:.1}x → {:?} @{:.1}x",
                old_bounds.size,
                old_scale,
                new_size,
                new_scale,
            );

            // Update stored bounds (in logical pixels, matching GPUI convention).
            let new_bounds = Bounds {
                origin: Default::default(),
                size: new_size,
            };
            self.bounds.set(new_bounds);
            self.scale_factor.set(new_scale);

            // Update the Metal layer's contentsScale so the drawable has the
            // correct pixel dimensions.
            let layer: *mut AnyObject = msg_send![self.view, layer];
            let _: () = msg_send![layer, setContentsScale: scale];

            // Update the wgpu renderer's surface configuration.
            let pixel_w = (new_w * new_scale) as i32;
            let pixel_h = (new_h * new_scale) as i32;
            {
                let mut guard = self.renderer.lock();
                if let Some(renderer) = guard.as_mut() {
                    renderer
                        .update_drawable_size(size(DevicePixels(pixel_w), DevicePixels(pixel_h)));
                }
            }

            // Fire the resize callback so GPUI re-layouts at the new size.
            let cb = self.resize_callback.borrow_mut().take();
            if let Some(mut cb) = cb {
                cb(new_size, new_scale);
                // Restore the callback for future resize events.
                let mut slot = self.resize_callback.borrow_mut();
                if slot.is_none() {
                    *slot = Some(cb);
                }
            }
        }
    }
}

impl HasWindowHandle for IosWindow {
    fn window_handle(
        &self,
    ) -> std::result::Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError>
    {
        let view = NonNull::new(self.view as *mut c_void)
            .ok_or(raw_window_handle::HandleError::Unavailable)?;
        let handle = UiKitWindowHandle::new(view);
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(handle.into()) })
    }
}

impl HasDisplayHandle for IosWindow {
    fn display_handle(
        &self,
    ) -> std::result::Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError>
    {
        let handle = UiKitDisplayHandle::new();
        Ok(unsafe { raw_window_handle::DisplayHandle::borrow_raw(handle.into()) })
    }
}

impl PlatformWindow for IosWindow {
    fn bounds(&self) -> Bounds<Pixels> {
        self.bounds.get()
    }

    fn is_maximized(&self) -> bool {
        true // iOS windows are always "maximized"
    }

    fn window_bounds(&self) -> WindowBounds {
        WindowBounds::Fullscreen(self.bounds.get())
    }

    fn content_size(&self) -> Size<Pixels> {
        self.bounds.get().size
    }

    fn resize(&mut self, _size: Size<Pixels>) {
        // iOS windows cannot be resized programmatically
    }

    fn scale_factor(&self) -> f32 {
        self.scale_factor.get()
    }

    fn appearance(&self) -> WindowAppearance {
        unsafe {
            let trait_collection: *mut AnyObject = msg_send![self.view, traitCollection];
            let style: i64 = msg_send![trait_collection, userInterfaceStyle];
            match style {
                2 => WindowAppearance::Dark,
                _ => WindowAppearance::Light,
            }
        }
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(Rc::new(IosDisplay::main()))
    }

    fn mouse_position(&self) -> Point<Pixels> {
        self.mouse_position.get()
    }

    fn modifiers(&self) -> Modifiers {
        self.modifiers.get()
    }

    fn capslock(&self) -> Capslock {
        // Would need to check UIKeyModifierFlags
        Capslock { on: false }
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        *self.input_handler.borrow_mut() = Some(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.input_handler.borrow_mut().take()
    }

    fn set_text_input_configuration(&mut self, configuration: gpui::TextInputConfiguration) {
        use gpui::{Autocapitalize, TextInputAction};
        unsafe {
            if self.text_input_view.is_null() {
                return;
            }
            // UITextAutocorrectionType: 0=Default,1=No,2=Yes
            let autocorrection_type: isize = if configuration.autocorrect { 2 } else { 1 };
            let _: () = msg_send![
                self.text_input_view,
                setAutocorrectionType: autocorrection_type
            ];

            // UITextAutocapitalizationType: 0=None,1=Words,2=Sentences,3=AllChars
            let autocapitalization_type: isize = match configuration.autocapitalize {
                Autocapitalize::None => 0,
                Autocapitalize::Words => 1,
                Autocapitalize::Sentences => 2,
                Autocapitalize::Characters => 3,
            };
            let _: () = msg_send![
                self.text_input_view,
                setAutocapitalizationType: autocapitalization_type
            ];

            // UITextSpellCheckingType: 0=Default,1=No,2=Yes
            let spell_checking_type: isize = if configuration.suggestions { 2 } else { 1 };
            let _: () = msg_send![
                self.text_input_view,
                setSpellCheckingType: spell_checking_type
            ];
            // Disable smart quotes/dashes along with autocorrect — matches
            // the intent of "no text assistance" rather than half-applying it.
            // UITextSmartQuotesType / UITextSmartDashesType: 0=Default,1=No,2=Yes
            let smart_type: isize = if configuration.autocorrect { 2 } else { 1 };
            let _: () = msg_send![self.text_input_view, setSmartQuotesType: smart_type];
            let _: () = msg_send![self.text_input_view, setSmartDashesType: smart_type];

            // UIReturnKeyType
            let return_key_type: isize = match configuration.input_action {
                TextInputAction::Go => 6,
                TextInputAction::Done => 9,
                TextInputAction::Search => 4,
                TextInputAction::Send => 7,
                TextInputAction::Next => 5,
                TextInputAction::Enter | TextInputAction::Unspecified => 0,
                TextInputAction::Previous => 0,
            };
            let _: () = msg_send![self.text_input_view, setReturnKeyType: return_key_type];

            let is_first_responder: bool = msg_send![self.text_input_view, isFirstResponder];
            if is_first_responder {
                let _: () = msg_send![self.text_input_view, reloadInputViews];
            }
        }
    }

    fn show_soft_keyboard(&self) {
        unsafe {
            if self.text_input_view.is_null() {
                return;
            }
            let _: () = msg_send![self.text_input_view,
                performSelector: sel!(becomeFirstResponder),
                withObject: ptr::null::<AnyObject>(),
                afterDelay: 0.0_f64
            ];
        }
    }

    fn hide_soft_keyboard(&self) {
        unsafe {
            if self.text_input_view.is_null() {
                return;
            }
            let _: () = msg_send![self.text_input_view,
                performSelector: sel!(resignFirstResponder),
                withObject: ptr::null::<AnyObject>(),
                afterDelay: 0.0_f64
            ];
        }
    }

    fn text_input_state_changed(&self, change: gpui::TextInputStateChange) {
        use gpui::TextInputStateChange;
        unsafe {
            if self.text_input_view.is_null() {
                return;
            }
            match change {
                TextInputStateChange::FocusGained => {
                    self.show_soft_keyboard();
                }
                TextInputStateChange::FocusLost => {
                    self.hide_soft_keyboard();
                }
                TextInputStateChange::SelectionChanged => {
                    let delegate: *mut AnyObject = msg_send![self.text_input_view, inputDelegate];
                    if !delegate.is_null() {
                        let _: () = msg_send![
                            delegate,
                            selectionWillChange: self.text_input_view
                        ];
                        let _: () = msg_send![
                            delegate,
                            selectionDidChange: self.text_input_view
                        ];
                    }
                }
                TextInputStateChange::ContentChanged => {
                    let delegate: *mut AnyObject = msg_send![self.text_input_view, inputDelegate];
                    if !delegate.is_null() {
                        let _: () = msg_send![delegate, textWillChange: self.text_input_view];
                        let _: () = msg_send![delegate, textDidChange: self.text_input_view];
                    }
                }
            }
        }
    }

    fn prompt(
        &self,
        level: PromptLevel,
        msg: &str,
        detail: Option<&str>,
        answers: &[PromptButton],
    ) -> Option<futures::channel::oneshot::Receiver<usize>> {
        let (tx, rx) = futures::channel::oneshot::channel();

        unsafe {
            let title = msg;
            let message = detail.unwrap_or("");

            // UIAlertControllerStyleAlert. `Critical` gets the same style —
            // iOS has no distinct "critical" alert chrome — but we still
            // prefix the title so the distinction isn't lost entirely.
            let alert_style: i64 = 1;
            let title = match level {
                PromptLevel::Critical => format!("⚠️ {title}"),
                PromptLevel::Warning | PromptLevel::Info => title.to_string(),
            };

            // NUL-safe: `nsstring` copies exactly `s.len()` bytes rather
            // than scanning for a NUL terminator the way
            // `stringWithUTF8String:` does on a non-NUL-terminated `&str`
            // pointer (undefined behavior the previous implementation had).
            let title_str = crate::ios::util::nsstring(&title);
            let message_str = crate::ios::util::nsstring(message);

            let alert: *mut AnyObject = msg_send![
                class!(UIAlertController),
                alertControllerWithTitle: title_str,
                message: message_str,
                preferredStyle: alert_style
            ];

            // Shared across every button's handler block: whichever fires
            // first takes the sender and resolves it; `Rc<RefCell<Option<_>>>`
            // rather than a plain `oneshot::Sender` because `UIAlertAction`
            // handler blocks are `Fn`, not `FnOnce`, and UIKit only ever
            // invokes exactly one of them per alert.
            let sender: Rc<RefCell<Option<futures::channel::oneshot::Sender<usize>>>> =
                Rc::new(RefCell::new(Some(tx)));

            for (index, button) in answers.iter().enumerate() {
                let button_title = crate::ios::util::nsstring(button.label().as_ref());

                // UIAlertActionStyleCancel = 1, UIAlertActionStyleDefault = 0.
                let action_style: i64 = if button.is_cancel() { 1 } else { 0 };

                let sender_for_action = sender.clone();
                let handler = block2::RcBlock::new(move |_action: *mut AnyObject| {
                    if let Some(sender) = sender_for_action.borrow_mut().take() {
                        let _ = sender.send(index);
                    }
                });

                let action: *mut AnyObject = msg_send![
                    class!(UIAlertAction),
                    actionWithTitle: button_title,
                    style: action_style,
                    handler: &*handler
                ];

                let _: () = msg_send![alert, addAction: action];

                // Leak the handler block: `UIAlertAction` retains it for its
                // own lifetime (bounded by the alert, which UIKit owns once
                // presented), but we have no Rust-side owner to keep it
                // alive until then otherwise.
                std::mem::forget(handler);
            }

            // Present the alert
            let _: () = msg_send![
                self.view_controller,
                presentViewController: alert,
                animated: true,
                completion: ptr::null::<AnyObject>()
            ];
        }

        Some(rx)
    }

    fn activate(&self) {
        unsafe {
            let _: () = msg_send![self.window, makeKeyAndVisible];
        }
    }

    fn is_active(&self) -> bool {
        unsafe {
            let app: *mut AnyObject = msg_send![class!(UIApplication), sharedApplication];
            let key_window: *mut AnyObject = msg_send![app, keyWindow];
            self.window == key_window
        }
    }

    fn is_hovered(&self) -> bool {
        // Hover isn't really applicable on iOS
        false
    }

    fn set_title(&mut self, _title: &str) {
        // iOS apps don't have window titles
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        WindowBackgroundAppearance::Opaque
    }

    fn set_background_appearance(&self, _background_appearance: WindowBackgroundAppearance) {
        // Could adjust view background color
    }

    fn minimize(&self) {
        // iOS apps cannot be minimized
    }

    fn zoom(&self) {
        // iOS apps cannot be zoomed
    }

    fn toggle_fullscreen(&self) {
        // iOS apps are always fullscreen
    }

    fn is_fullscreen(&self) -> bool {
        true
    }

    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        *self.request_frame_callback.borrow_mut() = Some(callback);
    }

    fn on_input(&self, callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>) {
        *self.input_callback.borrow_mut() = Some(callback);
    }

    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        *self.active_status_callback.borrow_mut() = Some(callback);
    }

    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        *self.hover_status_callback.borrow_mut() = Some(callback);
    }

    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        *self.resize_callback.borrow_mut() = Some(callback);
    }

    fn on_moved(&self, callback: Box<dyn FnMut()>) {
        *self.moved_callback.borrow_mut() = Some(callback);
    }

    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        *self.should_close_callback.borrow_mut() = Some(callback);
    }

    fn on_hit_test_window_control(&self, callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
        *self.hit_test_callback.borrow_mut() = Some(callback);
    }

    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        *self.close_callback.borrow_mut() = Some(callback);
    }

    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>) {
        *self.appearance_changed_callback.borrow_mut() = Some(callback);
    }

    fn draw(&self, scene: &Scene) {
        let mut guard = self.renderer.lock();
        if let Some(renderer) = guard.as_mut() {
            renderer.draw(scene);
        } else {
            log::trace!("GPUI iOS: draw called but no renderer available");
        }
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        let guard = self.renderer.lock();
        if let Some(renderer) = guard.as_ref() {
            renderer.sprite_atlas().clone()
        } else {
            // Fallback: return a dummy atlas so GPUI doesn't panic before
            // the renderer is initialised.
            Arc::new(FallbackAtlas::new())
        }
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        let guard = self.renderer.lock();
        guard
            .as_ref()
            .map(|r| r.supports_dual_source_blending())
            .unwrap_or(false)
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        let guard = self.renderer.lock();
        guard.as_ref().map(|r| r.gpu_specs())
    }

    fn update_ime_position(&self, bounds: Bounds<Pixels>) {
        // Move the (transparent, non-interactive) text input view over the
        // focused element's bounds so UIKit positions autocorrect bubbles,
        // the predictive-text candidate bar, and dictation UI correctly.
        // The view stays alpha~0 and userInteractionEnabled=NO so touches
        // still reach the Metal view underneath.
        unsafe {
            if self.text_input_view.is_null() {
                return;
            }
            let frame = ObjcCGRect::new(
                f64::from(bounds.origin.x),
                f64::from(bounds.origin.y),
                f64::from(bounds.size.width).max(1.0),
                f64::from(bounds.size.height).max(1.0),
            );
            let _: () = msg_send![self.text_input_view, setFrame: frame];
        }
    }

    fn insets(&self) -> gpui::WindowInsets {
        self.compute_settled_insets()
    }

    fn on_insets_changed(&self, callback: Box<dyn FnMut(gpui::WindowInsets)>) {
        *self.insets_changed_callback.borrow_mut() = Some(callback);
        // Fire immediately with the current value so callers that only
        // register a callback (and never separately call `insets()`) still
        // get the current state rather than waiting for the next change.
        self.notify_insets_changed();
    }

    fn request_attention(&self) {
        // iOS has no window-attention affordance for a foreground app (no
        // Dock bounce equivalent); the closest analogue is a local
        // notification, which would require notification authorization the
        // app may not have requested. If the app is backgrounded, posting a
        // notification is the right tool and belongs in
        // `Platform::show_system_notification`, which callers can already
        // reach directly — so this is a deliberate no-op rather than a
        // surprising side effect.
    }
}

// ── Fallback atlas ────────────────────────────────────────────────────────────

/// A minimal fallback `PlatformAtlas` used until a real Blade/Metal renderer is
/// wired up.  It records tiles in memory but does not upload texture data to the
/// GPU — just enough to satisfy GPUI's atlas queries without panicking.
struct FallbackAtlas {
    state: Mutex<FallbackAtlasState>,
}

struct FallbackAtlasState {
    next_id: u32,
    tiles: HashMap<AtlasKey, AtlasTile>,
}

impl FallbackAtlas {
    fn new() -> Self {
        Self {
            state: Mutex::new(FallbackAtlasState {
                next_id: 1,
                tiles: HashMap::new(),
            }),
        }
    }
}

impl PlatformAtlas for FallbackAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> anyhow::Result<
            Option<(Size<DevicePixels>, std::borrow::Cow<'a, [u8]>)>,
        >,
    ) -> anyhow::Result<Option<AtlasTile>> {
        let mut state = self.state.lock();

        if let Some(tile) = state.tiles.get(key) {
            return Ok(Some(*tile));
        }

        let data = build()?;
        if let Some((size, _pixels)) = data {
            let id = state.next_id;
            state.next_id += 1;

            let tile = AtlasTile {
                texture_id: AtlasTextureId {
                    index: 0,
                    kind: AtlasTextureKind::Monochrome,
                },
                tile_id: TileId(id),
                padding: 0,
                bounds: Bounds {
                    origin: point(DevicePixels(0), DevicePixels(0)),
                    size,
                },
            };

            state.tiles.insert(key.clone(), tile);
            Ok(Some(tile))
        } else {
            Ok(None)
        }
    }

    fn remove(&self, key: &AtlasKey) {
        self.state.lock().tiles.remove(key);
    }
}
