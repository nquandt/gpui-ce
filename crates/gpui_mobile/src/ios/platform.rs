//! iOS Platform implementation.
//!
//! This implements the Platform trait for iOS using UIKit.
//! Key differences from macOS:
//! - Uses UIApplication instead of NSApplication
//! - No menu bar (iOS apps don't have traditional menus)
//! - No windowed mode (iOS apps are always fullscreen on their display)
//! - Touch-based input instead of mouse
//! - System keyboard handling differs significantly

use super::{IosDispatcher, IosDisplay, IosWindow};
use anyhow::anyhow;
use futures::channel::oneshot;
use gpui::{
    Action, AnyWindowHandle, AppLifecyclePhase, BackgroundExecutor, ClipboardEntry, ClipboardItem,
    CursorStyle, DummyKeyboardMapper, ForegroundExecutor, GestureKinds, GestureTuning,
    HapticFeedbackStyle, Keymap, Menu, MenuItem, PathPromptOptions, Platform, PlatformDisplay,
    PlatformGestures, PlatformKeyboardLayout, PlatformKeyboardMapper, PlatformTextSystem,
    PlatformWindow, Result, Task, ThermalState, WindowAppearance, WindowParams, px,
};
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use parking_lot::Mutex;
use std::{
    cell::UnsafeCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, Once, OnceLock},
};

pub struct IosPlatform(Mutex<IosPlatformState>);

pub(crate) struct IosPlatformState {
    background_executor: BackgroundExecutor,
    foreground_executor: ForegroundExecutor,
    text_system: Arc<dyn PlatformTextSystem>,
    finish_launching: Option<Box<dyn FnOnce()>>,
    quit_callback: Option<Box<dyn FnMut() -> bool>>,
    /// `UIDocumentInteractionController`s kept alive while their "open with"
    /// menu is presented. iOS requires a strong reference to the controller
    /// for the duration of the menu's lifetime; we don't get a reliable
    /// dismissal callback wired up (that would need a delegate object), so
    /// we simply retain every controller ever created for the life of the
    /// process. This leaks one small object per `open_with_system` call,
    /// which is an acceptable tradeoff for a rarely-invoked, user-initiated
    /// action.
    document_interaction_controllers: Vec<usize>,
}

impl Default for IosPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl IosPlatform {
    pub fn new() -> Self {
        let dispatcher = Arc::new(IosDispatcher);

        let text_system: Arc<dyn PlatformTextSystem> = Arc::new(super::IosTextSystem::new());

        Self(Mutex::new(IosPlatformState {
            background_executor: BackgroundExecutor::new(dispatcher.clone()),
            foreground_executor: ForegroundExecutor::new(dispatcher),
            text_system,
            finish_launching: None,
            quit_callback: None,
            document_interaction_controllers: Vec::new(),
        }))
    }
}

/// A simple iOS keyboard layout.
struct IosKeyboardLayout;

impl PlatformKeyboardLayout for IosKeyboardLayout {
    fn id(&self) -> &str {
        "ios-default"
    }

    fn name(&self) -> &str {
        "iOS Default"
    }
}

// ── App-lifecycle / memory-warning / thermal-state callback storage ────────
//
// These are process-wide (not tied to a particular `IosPlatform` instance,
// though in practice there is only ever one), so they live in static cells
// rather than in `IosPlatformState`. This lets the FFI entry points in
// `ffi.rs` (which only have access to app-delegate-supplied pointers, not an
// `Rc<IosPlatform>`) and the `NSNotificationCenter` blocks registered below
// reach the callbacks registered through the `Platform` trait.
//
// Safety: all GPUI/UIKit work on iOS happens on the main thread, so the
// `UnsafeCell` accesses below are never actually concurrent even though the
// types are marked `Send`/`Sync` to satisfy `OnceLock`'s bounds.

struct LifecycleCell(UnsafeCell<Option<Box<dyn FnMut(AppLifecyclePhase)>>>);
unsafe impl Send for LifecycleCell {}
unsafe impl Sync for LifecycleCell {}

struct SimpleCallbackCell(UnsafeCell<Option<Box<dyn FnMut()>>>);
unsafe impl Send for SimpleCallbackCell {}
unsafe impl Sync for SimpleCallbackCell {}

struct OpenUrlsCell(UnsafeCell<Option<Box<dyn FnMut(Vec<String>)>>>);
unsafe impl Send for OpenUrlsCell {}
unsafe impl Sync for OpenUrlsCell {}

static OPEN_URLS_CALLBACK: OnceLock<OpenUrlsCell> = OnceLock::new();
static APP_LIFECYCLE_CALLBACK: OnceLock<LifecycleCell> = OnceLock::new();
static MEMORY_WARNING_CALLBACK: OnceLock<SimpleCallbackCell> = OnceLock::new();
static THERMAL_STATE_CALLBACK: OnceLock<SimpleCallbackCell> = OnceLock::new();

static MEMORY_WARNING_OBSERVER_REGISTERED: Once = Once::new();
static THERMAL_STATE_OBSERVER_REGISTERED: Once = Once::new();

/// Invoked from `ffi.rs`'s `gpui_ios_handle_open_url` to drive
/// [`Platform::on_open_urls`].
pub(crate) fn notify_open_urls(urls: Vec<String>) {
    let cell = OPEN_URLS_CALLBACK.get_or_init(|| OpenUrlsCell(UnsafeCell::new(None)));
    unsafe {
        if let Some(callback) = (*cell.0.get()).as_mut() {
            callback(urls);
        }
    }
}

/// Invoked from the `ffi.rs` app-delegate lifecycle entry points
/// (`gpui_ios_will_enter_foreground`, etc.) to drive [`Platform::on_app_lifecycle`].
pub(crate) fn notify_app_lifecycle(phase: AppLifecyclePhase) {
    let cell = APP_LIFECYCLE_CALLBACK.get_or_init(|| LifecycleCell(UnsafeCell::new(None)));
    unsafe {
        if let Some(callback) = (*cell.0.get()).as_mut() {
            callback(phase);
        }
    }
}

/// Reads `[[NSProcessInfo processInfo] thermalState]` and maps it onto
/// [`ThermalState`].
///
/// `NSProcessInfoThermalState`: 0 = nominal, 1 = fair, 2 = serious, 3 = critical.
fn query_thermal_state() -> ThermalState {
    unsafe {
        let process_info: *mut AnyObject = msg_send![class!(NSProcessInfo), processInfo];
        let state: i64 = msg_send![process_info, thermalState];
        match state {
            1 => ThermalState::Fair,
            2 => ThermalState::Serious,
            3 => ThermalState::Critical,
            _ => ThermalState::Nominal,
        }
    }
}

/// Registers an `NSNotificationCenter` observer for
/// `UIApplicationDidReceiveMemoryWarningNotification`, invoking the callback
/// stored in [`MEMORY_WARNING_CALLBACK`]. Idempotent — only the first call
/// actually registers the observer.
fn ensure_memory_warning_observer() {
    MEMORY_WARNING_OBSERVER_REGISTERED.call_once(|| unsafe {
        let notification_center: *mut AnyObject =
            msg_send![class!(NSNotificationCenter), defaultCenter];
        let name = super::util::nsstring("UIApplicationDidReceiveMemoryWarningNotification");

        let block = block2::RcBlock::new(move |_notification: *mut AnyObject| {
            log::warn!("GPUI iOS: Received memory warning");
            if let Some(cell) = MEMORY_WARNING_CALLBACK.get() {
                if let Some(callback) = (*cell.0.get()).as_mut() {
                    callback();
                }
            }
        });

        let _: *mut AnyObject = msg_send![notification_center,
            addObserverForName: name,
            object: std::ptr::null::<AnyObject>(),
            queue: std::ptr::null::<AnyObject>(),
            usingBlock: &*block
        ];
        std::mem::forget(block);
    });
}

/// Registers an `NSNotificationCenter` observer for
/// `NSProcessInfoThermalStateDidChangeNotification`, invoking the callback
/// stored in [`THERMAL_STATE_CALLBACK`]. Idempotent.
fn ensure_thermal_state_observer() {
    THERMAL_STATE_OBSERVER_REGISTERED.call_once(|| unsafe {
        let notification_center: *mut AnyObject =
            msg_send![class!(NSNotificationCenter), defaultCenter];
        let name = super::util::nsstring("NSProcessInfoThermalStateDidChangeNotification");

        let block = block2::RcBlock::new(move |_notification: *mut AnyObject| {
            log::info!(
                "GPUI iOS: Thermal state changed: {:?}",
                query_thermal_state()
            );
            if let Some(cell) = THERMAL_STATE_CALLBACK.get() {
                if let Some(callback) = (*cell.0.get()).as_mut() {
                    callback();
                }
            }
        });

        let _: *mut AnyObject = msg_send![notification_center,
            addObserverForName: name,
            object: std::ptr::null::<AnyObject>(),
            queue: std::ptr::null::<AnyObject>(),
            usingBlock: &*block
        ];
        std::mem::forget(block);
    });
}

/// [`PlatformGestures`] for iOS: touch-driven tuning constants tuned to feel
/// native (10pt slop matches `UIScrollView`'s drag threshold, 500ms long
/// press matches `UILongPressGestureRecognizer`'s default), and no native
/// recognizers — GPUI's own touch state machine in `IosWindow::handle_touch`
/// (not a `UIGestureRecognizer`) drives taps/scrolls/flings directly, so the
/// portable recognizer arena in gpui core handles everything else.
pub(crate) struct IosGestures;

impl PlatformGestures for IosGestures {
    fn tuning(&self) -> GestureTuning {
        GestureTuning {
            touch_slop: px(10.),
            long_press_duration: std::time::Duration::from_millis(500),
            scroll_physics: gpui::ScrollPhysics::ios(),
            ..Default::default()
        }
    }

    fn native_recognizers(&self) -> GestureKinds {
        GestureKinds::NONE
    }
}

// ── iOS Keychain (Security.framework) ───────────────────────────────────────
//
// Declares the small slice of Security.framework's C API needed for
// generic-password keychain items. iOS's Security.framework mirrors macOS's
// closely (see `gpui_macos::platform::security` for the desktop equivalent),
// but we use `kSecClassGenericPassword` (service + account) rather than
// `kSecClassInternetPassword` (server) since iOS apps store credentials
// keyed by an arbitrary service identifier, not necessarily a real host.
mod security {
    #![allow(non_upper_case_globals)]

    use core_foundation::base::{CFTypeRef, OSStatus};
    use core_foundation::dictionary::CFDictionaryRef;
    use core_foundation::string::CFStringRef;

    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        pub static kSecClass: CFStringRef;
        pub static kSecClassGenericPassword: CFStringRef;
        pub static kSecAttrService: CFStringRef;
        pub static kSecAttrAccount: CFStringRef;
        pub static kSecAttrAccessible: CFStringRef;
        pub static kSecAttrAccessibleAfterFirstUnlock: CFStringRef;
        pub static kSecValueData: CFStringRef;
        pub static kSecReturnAttributes: CFStringRef;
        pub static kSecReturnData: CFStringRef;

        pub fn SecItemAdd(attributes: CFDictionaryRef, result: *mut CFTypeRef) -> OSStatus;
        pub fn SecItemUpdate(query: CFDictionaryRef, attributes: CFDictionaryRef) -> OSStatus;
        pub fn SecItemDelete(query: CFDictionaryRef) -> OSStatus;
        pub fn SecItemCopyMatching(query: CFDictionaryRef, result: *mut CFTypeRef) -> OSStatus;
    }

    pub const errSecSuccess: OSStatus = 0;
    pub const errSecUserCanceled: OSStatus = -128;
    pub const errSecItemNotFound: OSStatus = -25300;
}

impl Platform for IosPlatform {
    fn background_executor(&self) -> BackgroundExecutor {
        self.0.lock().background_executor.clone()
    }

    fn foreground_executor(&self) -> ForegroundExecutor {
        self.0.lock().foreground_executor.clone()
    }

    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.0.lock().text_system.clone()
    }

    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>) {
        // Store the callback for later invocation via FFI.
        // The callback will be invoked when gpui_ios_did_finish_launching() is called
        // from the iOS app delegate's applicationDidFinishLaunchingWithOptions:.
        self.0.lock().finish_launching = Some(on_finish_launching);

        // On iOS, the app lifecycle is managed by UIApplicationMain which must be
        // called from main() before any Rust code runs. The Application::run() method
        // is called during app initialization, before UIApplicationMain starts its
        // event loop.
        //
        // The finish_launching callback is stored and will be invoked when the iOS
        // app delegate calls gpui_ios_did_finish_launching() via FFI.
        //
        // Unlike macOS where we call NSApplication.run() here, on iOS we don't need
        // to start the run loop - UIApplicationMain handles that.
        //
        // The callback is forwarded to the FFI layer so it can be invoked from Obj-C.
        if let Some(callback) = self.0.lock().finish_launching.take() {
            super::ffi::set_finish_launching_callback(callback);
        }

        log::info!("GPUI iOS: Platform::run() completed, waiting for app delegate callback");
    }

    fn quit(&self) {
        // iOS apps do not own their process lifetime: only the user
        // (swiping up in the app switcher) or the system (jetsam, on
        // resource pressure) terminates an app. There is no supported API
        // to request termination programmatically — `exit()` is possible
        // but is rejected by App Review and skips the orderly
        // `applicationWillTerminate:` teardown, so we deliberately no-op
        // here rather than call it.
        log::warn!("iOS apps cannot programmatically quit");
    }

    fn restart(&self, _binary_path: Option<PathBuf>, _arguments: Vec<std::ffi::OsString>) {
        // iOS apps cannot restart themselves — there is no exec/relaunch
        // API available to third-party apps. Only the user or the system
        // can relaunch the app (e.g. after a crash or a manual force-quit).
        log::warn!("iOS apps cannot restart themselves");
    }

    fn activate(&self, _ignoring_other_apps: bool) {
        // iOS handles app activation automatically
    }

    fn hide(&self) {
        // iOS apps cannot hide themselves
    }

    fn hide_other_apps(&self) {
        // Not applicable on iOS
    }

    fn unhide_other_apps(&self) {
        // Not applicable on iOS
    }

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        IosDisplay::all()
            .map(|display| Rc::new(display) as Rc<dyn PlatformDisplay>)
            .collect()
    }

    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(Rc::new(IosDisplay::main()))
    }

    fn active_window(&self) -> Option<AnyWindowHandle> {
        // iOS typically has a single `UIWindow`; GPUI's window handles are
        // not tracked by the platform layer (only `IosWindow` pointers are,
        // via `IOS_WINDOW_LIST`, which don't carry an `AnyWindowHandle`).
        // Since apps only ever open one GPUI window in practice, there is
        // no ambiguity to resolve, but plumbing the handle through would
        // require `IosWindow` to store its own `AnyWindowHandle` — left
        // unimplemented since nothing in this crate consumes it yet.
        None
    }

    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> anyhow::Result<Box<dyn PlatformWindow>> {
        let window = Box::new(IosWindow::new(handle, options)?);
        // Register the window with FFI layer so Objective-C can access it for rendering
        window.register_with_ffi();
        Ok(window)
    }

    fn set_window_appearance(&self, appearance: Option<WindowAppearance>) {
        // UIUserInterfaceStyle: 0 = unspecified (follow system), 1 = light, 2 = dark
        let style: isize = match appearance {
            None => 0,
            Some(WindowAppearance::Light | WindowAppearance::VibrantLight) => 1,
            Some(WindowAppearance::Dark | WindowAppearance::VibrantDark) => 2,
        };
        unsafe {
            let app: *mut AnyObject = msg_send![class!(UIApplication), sharedApplication];
            let windows: *mut AnyObject = msg_send![app, windows];
            if windows.is_null() {
                return;
            }
            let count: usize = msg_send![windows, count];
            for index in 0..count {
                let window: *mut AnyObject = msg_send![windows, objectAtIndex: index];
                if !window.is_null() {
                    let _: () = msg_send![window, setOverrideUserInterfaceStyle: style];
                }
            }
        }
    }

    fn window_appearance(&self) -> WindowAppearance {
        unsafe {
            let style: i64 = {
                let app: *mut AnyObject = msg_send![class!(UIApplication), sharedApplication];
                let key_window: *mut AnyObject = msg_send![app, keyWindow];
                if key_window.is_null() {
                    return WindowAppearance::Light;
                }
                let trait_collection: *mut AnyObject = msg_send![key_window, traitCollection];
                msg_send![trait_collection, userInterfaceStyle]
            };

            // UIUserInterfaceStyle: 0 = unspecified, 1 = light, 2 = dark
            match style {
                2 => WindowAppearance::Dark,
                _ => WindowAppearance::Light,
            }
        }
    }

    fn open_url(&self, url: &str) {
        unsafe {
            let url_string = super::util::nsstring(url);
            let url: *mut AnyObject = msg_send![class!(NSURL), URLWithString: url_string];
            if url.is_null() {
                log::warn!("GPUI iOS: open_url: invalid URL: {:?}", url_string);
                return;
            }
            let app: *mut AnyObject = msg_send![class!(UIApplication), sharedApplication];
            let _: () = msg_send![app, openURL: url, options: std::ptr::null::<AnyObject>(), completionHandler: std::ptr::null::<AnyObject>()];
        }
    }

    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>) {
        let cell = OPEN_URLS_CALLBACK.get_or_init(|| OpenUrlsCell(UnsafeCell::new(None)));
        unsafe {
            *cell.0.get() = Some(callback);
        }
    }

    fn register_url_scheme(&self, _url: &str) -> Task<Result<()>> {
        // URL schemes on iOS are registered in Info.plist, not programmatically
        Task::ready(Ok(()))
    }

    fn prompt_for_paths(
        &self,
        options: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        let (tx, rx) = oneshot::channel();
        unsafe {
            let vc: *mut AnyObject = active_root_view_controller();
            if vc.is_null() {
                let _ = tx.send(Err(anyhow!(
                    "prompt_for_paths: no active view controller to present from"
                )));
                return rx;
            }

            // `UTTypeItem`/`UTTypeFolder` are `UTType *const` globals exported
            // by UniformTypeIdentifiers.framework (iOS 14+).
            let content_type: *mut AnyObject = if options.directories && !options.files {
                super::util::ut_type_folder()
            } else {
                super::util::ut_type_item()
            };

            let types_array: *mut AnyObject =
                msg_send![class!(NSArray), arrayWithObject: content_type];

            let picker: *mut AnyObject = msg_send![class!(UIDocumentPickerViewController), alloc];
            let picker: *mut AnyObject = msg_send![picker, initForOpeningContentTypes: types_array];
            let _: () = msg_send![picker, setAllowsMultipleSelection: options.multiple];

            let delegate = super::document_picker::make_delegate(tx);
            let _: () = msg_send![picker, setDelegate: delegate];

            // Keep the delegate alive for as long as the picker is presented
            // by stashing it as an associated object on the picker itself.
            super::document_picker::retain_delegate_on(picker, delegate);

            let _: () = msg_send![vc,
                presentViewController: picker,
                animated: true,
                completion: std::ptr::null::<AnyObject>()
            ];
        }
        rx
    }

    fn prompt_for_new_path(
        &self,
        directory: &Path,
        suggested_name: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        // iOS has no save-panel equivalent available to third-party apps
        // (there is no writable, user-browsable Finder-style UI outside of
        // `UIDocumentPickerViewController`'s *export* mode, which requires
        // the file to already exist on disk before it can be "saved
        // elsewhere"). Rather than fail, we return a path inside the app's
        // sandboxed Documents directory — which is itself visible to the
        // user via the Files app when `UIFileSharingEnabled`/
        // `LSSupportsOpeningDocumentsInPlace` is set in Info.plist — so
        // callers can still write to a real, persistent location.
        let (tx, rx) = oneshot::channel();
        let name = suggested_name.unwrap_or("Untitled");
        let path = directory.join(name);
        let _ = tx.send(Ok(Some(path)));
        rx
    }

    fn can_select_mixed_files_and_dirs(&self) -> bool {
        false
    }

    fn reveal_path(&self, _path: &Path) {
        // iOS doesn't have a file manager like Finder
    }

    fn open_with_system(&self, path: &Path) {
        let Some(path_str) = path.to_str() else {
            log::warn!("GPUI iOS: open_with_system: non-UTF8 path");
            return;
        };
        unsafe {
            let vc: *mut AnyObject = active_root_view_controller();
            if vc.is_null() {
                log::warn!("GPUI iOS: open_with_system: no active view controller");
                return;
            }

            let path_ns = super::util::nsstring(path_str);
            let url: *mut AnyObject = msg_send![class!(NSURL), fileURLWithPath: path_ns];
            let controller: *mut AnyObject = msg_send![class!(UIDocumentInteractionController), interactionControllerWithURL: url];

            let view: *mut AnyObject = msg_send![vc, view];
            let bounds: super::cg_types::ObjcCGRect = msg_send![view, bounds];
            let _: bool = msg_send![controller,
                presentOptionsMenuFromRect: bounds,
                inView: view,
                animated: true
            ];

            // `UIDocumentInteractionController` does not retain itself, and
            // its menu is dismissed asynchronously with no delegate wired up
            // here to tell us when. We retain it for the lifetime of the
            // process (one small object per invocation of this rarely-called,
            // user-initiated action) rather than risk the menu's controller
            // being deallocated out from under UIKit mid-presentation.
            let retained: *mut AnyObject = msg_send![controller, retain];
            self.0
                .lock()
                .document_interaction_controllers
                .push(retained as usize);
        }
    }

    fn on_quit(&self, callback: Box<dyn FnMut() -> bool>) {
        self.0.lock().quit_callback = Some(callback);
    }

    fn on_system_wake(&self, _callback: Box<dyn FnMut()>) {
        // Not applicable on iOS.
    }

    fn hide_cursor_until_mouse_moves(&self) {
        // iOS has no cursor.
    }

    fn is_cursor_visible(&self) -> bool {
        false
    }

    fn on_reopen(&self, _callback: Box<dyn FnMut()>) {
        // iOS handles app reopening through scene lifecycle
    }

    fn set_menus(&self, _menus: Vec<Menu>, _keymap: &Keymap) {
        // iOS doesn't have a menu bar
        // Could potentially integrate with UIMenuBuilder for context menus
    }

    fn set_dock_menu(&self, _menu: Vec<MenuItem>, _keymap: &Keymap) {
        // iOS doesn't have a dock menu
    }

    fn on_app_menu_action(&self, _callback: Box<dyn FnMut(&dyn Action)>) {
        // Not applicable on iOS
    }

    fn on_will_open_app_menu(&self, _callback: Box<dyn FnMut()>) {
        // Not applicable on iOS
    }

    fn on_validate_app_menu_command(&self, _callback: Box<dyn FnMut(&dyn Action) -> bool>) {
        // Not applicable on iOS
    }

    fn app_path(&self) -> Result<PathBuf> {
        unsafe {
            let bundle: *mut AnyObject = msg_send![class!(NSBundle), mainBundle];
            let path: *mut AnyObject = msg_send![bundle, bundlePath];
            let utf8: *const i8 = msg_send![path, UTF8String];
            if utf8.is_null() {
                return Err(anyhow!("Failed to get bundle path"));
            }
            let path_str = std::ffi::CStr::from_ptr(utf8).to_str()?;
            Ok(PathBuf::from(path_str))
        }
    }

    fn path_for_auxiliary_executable(&self, name: &str) -> Result<PathBuf> {
        let app_path = self.app_path()?;
        Ok(app_path.join(name))
    }

    fn set_cursor_style(&self, _style: CursorStyle) {
        // iOS doesn't have visible cursors (except for Apple Pencil hover on iPad)
    }

    fn should_auto_hide_scrollbars(&self) -> bool {
        true // iOS always auto-hides scrollbars
    }

    fn write_to_clipboard(&self, item: ClipboardItem) {
        unsafe {
            let pasteboard: *mut AnyObject = msg_send![class!(UIPasteboard), generalPasteboard];

            for entry in item.entries() {
                match entry {
                    ClipboardEntry::String(string) => {
                        let ns_string = super::util::nsstring(&string.text);
                        let _: () = msg_send![pasteboard, setString: ns_string];
                    }
                    ClipboardEntry::Image(image) => {
                        let uti = match image.format {
                            gpui::ImageFormat::Png => "public.png",
                            gpui::ImageFormat::Jpeg => "public.jpeg",
                            gpui::ImageFormat::Gif => "com.compuserve.gif",
                            gpui::ImageFormat::Webp => "org.webmproject.webp",
                            gpui::ImageFormat::Bmp => "com.microsoft.bmp",
                            gpui::ImageFormat::Tiff => "public.tiff",
                            gpui::ImageFormat::Svg => "public.svg-image",
                            gpui::ImageFormat::Ico => "com.microsoft.ico",
                            gpui::ImageFormat::Pnm => "public.image",
                        };
                        let bytes = image.bytes.as_slice();
                        let data: *mut AnyObject = msg_send![class!(NSData), alloc];
                        let data: *mut AnyObject = msg_send![data,
                            initWithBytes: bytes.as_ptr() as *const std::ffi::c_void,
                            length: bytes.len()
                        ];
                        let uti_ns = super::util::nsstring(uti);
                        let _: () = msg_send![pasteboard, setData: data, forPasteboardType: uti_ns];
                    }
                    ClipboardEntry::ExternalPaths(_) => {
                        // Not supported on iOS: there is no cross-app file
                        // reference the pasteboard can hold without also
                        // handing over a security-scoped bookmark, which
                        // ClipboardItem has no representation for.
                    }
                }
            }
        }
    }

    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        unsafe {
            let pasteboard: *mut AnyObject = msg_send![class!(UIPasteboard), generalPasteboard];

            let has_strings: bool = msg_send![pasteboard, hasStrings];
            if has_strings {
                let string: *mut AnyObject = msg_send![pasteboard, string];
                if !string.is_null() {
                    let utf8: *const i8 = msg_send![string, UTF8String];
                    if !utf8.is_null() {
                        if let Ok(text) = std::ffi::CStr::from_ptr(utf8).to_str() {
                            return Some(ClipboardItem::new_string(text.to_string()));
                        }
                    }
                }
            }

            let has_images: bool = msg_send![pasteboard, hasImages];
            if has_images {
                let image: *mut AnyObject = msg_send![pasteboard, image];
                if !image.is_null() {
                    // Prefer PNG (lossless). `pngData()` only exists in Swift;
                    // the Objective-C runtime API is the C function
                    // `UIImagePNGRepresentation`, so a `pngData` selector
                    // aborts with "method not found".
                    unsafe extern "C" {
                        fn UIImagePNGRepresentation(image: *mut AnyObject) -> *mut AnyObject;
                    }
                    let png_data: *mut AnyObject = UIImagePNGRepresentation(image);
                    if !png_data.is_null() {
                        let length: usize = msg_send![png_data, length];
                        let bytes_ptr: *const std::ffi::c_void = msg_send![png_data, bytes];
                        let bytes_ptr = bytes_ptr as *const u8;
                        if !bytes_ptr.is_null() && length > 0 {
                            let bytes = std::slice::from_raw_parts(bytes_ptr, length).to_vec();
                            let image = gpui::Image::from_bytes(gpui::ImageFormat::Png, bytes);
                            return Some(ClipboardItem::new_image(&image));
                        }
                    }
                }
            }

            None
        }
    }

    fn write_credentials(&self, url: &str, username: &str, password: &[u8]) -> Task<Result<()>> {
        let url = url.to_string();
        let username = username.to_string();
        let password = password.to_vec();
        self.background_executor().spawn(async move {
            unsafe {
                use core_foundation::base::TCFType;
                use core_foundation::data::CFData;
                use core_foundation::dictionary::CFMutableDictionary;
                use core_foundation::string::CFString;
                use security::*;

                let service = CFString::from(url.as_str());
                let account = CFString::from(username.as_str());
                let password_data = CFData::from_buffer(&password);

                let mut query_attrs = CFMutableDictionary::with_capacity(3);
                query_attrs.set(kSecClass as *const _, kSecClassGenericPassword as *const _);
                query_attrs.set(kSecAttrService as *const _, service.as_CFTypeRef());
                query_attrs.set(kSecAttrAccount as *const _, account.as_CFTypeRef());

                let mut attrs = CFMutableDictionary::with_capacity(5);
                attrs.set(kSecClass as *const _, kSecClassGenericPassword as *const _);
                attrs.set(kSecAttrService as *const _, service.as_CFTypeRef());
                attrs.set(kSecAttrAccount as *const _, account.as_CFTypeRef());
                attrs.set(kSecValueData as *const _, password_data.as_CFTypeRef());
                attrs.set(
                    kSecAttrAccessible as *const _,
                    kSecAttrAccessibleAfterFirstUnlock as *const _,
                );

                let mut verb = "updating";
                let mut update_attrs = CFMutableDictionary::with_capacity(1);
                update_attrs.set(kSecValueData as *const _, password_data.as_CFTypeRef());
                let mut status = SecItemUpdate(
                    query_attrs.as_concrete_TypeRef(),
                    update_attrs.as_concrete_TypeRef(),
                );

                if status == errSecItemNotFound {
                    verb = "creating";
                    status = SecItemAdd(attrs.as_concrete_TypeRef(), std::ptr::null_mut());
                }
                anyhow::ensure!(
                    status == errSecSuccess,
                    "{verb} keychain item failed: {status}"
                );
            }
            Ok(())
        })
    }

    fn read_credentials(&self, url: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        let url = url.to_string();
        self.background_executor().spawn(async move {
            unsafe {
                use core_foundation::base::{CFType, CFTypeRef, TCFType};
                use core_foundation::boolean::CFBoolean;
                use core_foundation::data::CFData;
                use core_foundation::dictionary::{CFDictionary, CFMutableDictionary};
                use core_foundation::string::CFString;
                use security::*;

                let service = CFString::from(url.as_str());
                let cf_true = CFBoolean::true_value().as_CFTypeRef();

                let mut attrs = CFMutableDictionary::with_capacity(4);
                attrs.set(kSecClass as *const _, kSecClassGenericPassword as *const _);
                attrs.set(kSecAttrService as *const _, service.as_CFTypeRef());
                attrs.set(kSecReturnAttributes as *const _, cf_true);
                attrs.set(kSecReturnData as *const _, cf_true);

                let mut result = CFTypeRef::from(std::ptr::null());
                let status = SecItemCopyMatching(attrs.as_concrete_TypeRef(), &mut result);
                match status {
                    security::errSecSuccess => {}
                    security::errSecItemNotFound | security::errSecUserCanceled => {
                        return Ok(None);
                    }
                    _ => anyhow::bail!("reading keychain item failed: {status}"),
                }

                let result = CFType::wrap_under_create_rule(result)
                    .downcast::<CFDictionary>()
                    .ok_or_else(|| anyhow!("keychain item was not a dictionary"))?;
                let account = result
                    .find(kSecAttrAccount as *const _)
                    .ok_or_else(|| anyhow!("account was missing from keychain item"))?;
                let account = CFType::wrap_under_get_rule(*account)
                    .downcast::<CFString>()
                    .ok_or_else(|| anyhow!("account was not a string"))?;
                let password = result
                    .find(kSecValueData as *const _)
                    .ok_or_else(|| anyhow!("password was missing from keychain item"))?;
                let password = CFType::wrap_under_get_rule(*password)
                    .downcast::<CFData>()
                    .ok_or_else(|| anyhow!("password was not data"))?;

                Ok(Some((account.to_string(), password.bytes().to_vec())))
            }
        })
    }

    fn delete_credentials(&self, url: &str) -> Task<Result<()>> {
        let url = url.to_string();
        self.background_executor().spawn(async move {
            unsafe {
                use core_foundation::base::TCFType;
                use core_foundation::dictionary::CFMutableDictionary;
                use core_foundation::string::CFString;
                use security::*;

                let service = CFString::from(url.as_str());
                let mut query_attrs = CFMutableDictionary::with_capacity(2);
                query_attrs.set(kSecClass as *const _, kSecClassGenericPassword as *const _);
                query_attrs.set(kSecAttrService as *const _, service.as_CFTypeRef());

                let status = SecItemDelete(query_attrs.as_concrete_TypeRef());
                anyhow::ensure!(
                    status == errSecSuccess || status == errSecItemNotFound,
                    "deleting keychain item failed: {status}"
                );
            }
            Ok(())
        })
    }

    fn on_keyboard_layout_change(&self, _callback: Box<dyn FnMut()>) {
        // iOS handles keyboard layout changes differently
    }

    fn thermal_state(&self) -> ThermalState {
        query_thermal_state()
    }

    fn on_thermal_state_change(&self, callback: Box<dyn FnMut()>) {
        let cell = THERMAL_STATE_CALLBACK.get_or_init(|| SimpleCallbackCell(UnsafeCell::new(None)));
        unsafe {
            *cell.0.get() = Some(callback);
        }
        ensure_thermal_state_observer();
    }

    fn on_memory_warning(&self, callback: Box<dyn FnMut()>) {
        let cell =
            MEMORY_WARNING_CALLBACK.get_or_init(|| SimpleCallbackCell(UnsafeCell::new(None)));
        unsafe {
            *cell.0.get() = Some(callback);
        }
        ensure_memory_warning_observer();
    }

    fn on_app_lifecycle(&self, callback: Box<dyn FnMut(AppLifecyclePhase)>) {
        let cell = APP_LIFECYCLE_CALLBACK.get_or_init(|| LifecycleCell(UnsafeCell::new(None)));
        unsafe {
            *cell.0.get() = Some(callback);
        }
    }

    fn gestures(&self) -> Option<Rc<dyn PlatformGestures>> {
        Some(Rc::new(IosGestures))
    }

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        Box::new(IosKeyboardLayout)
    }

    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        Rc::new(DummyKeyboardMapper)
    }

    fn supports_haptic_feedback(&self) -> bool {
        // Haptics are unavailable on iPad (no Taptic Engine) but
        // `UIFeedbackGenerator` subclasses simply no-op there, so we report
        // support on any iOS device — including the simulator, where the
        // generators also silently no-op. This mirrors how call sites treat
        // `supports_haptic_feedback` as "safe to call `play_haptic_feedback`
        // unconditionally", not a strict hardware capability check.
        true
    }

    fn play_haptic_feedback(&self, style: HapticFeedbackStyle) {
        unsafe {
            match style {
                HapticFeedbackStyle::Generic => {
                    // UIImpactFeedbackStyleMedium = 1
                    let generator: *mut AnyObject =
                        msg_send![class!(UIImpactFeedbackGenerator), alloc];
                    let generator: *mut AnyObject = msg_send![generator, initWithStyle: 1_isize];
                    let _: () = msg_send![generator, prepare];
                    let _: () = msg_send![generator, impactOccurred];
                }
                HapticFeedbackStyle::Alignment => {
                    let generator: *mut AnyObject =
                        msg_send![class!(UISelectionFeedbackGenerator), alloc];
                    let generator: *mut AnyObject = msg_send![generator, init];
                    let _: () = msg_send![generator, prepare];
                    let _: () = msg_send![generator, selectionChanged];
                }
                HapticFeedbackStyle::LevelChange => {
                    // UIImpactFeedbackStyleLight = 0 — a lighter tap than
                    // `Generic`, appropriate for discrete slider/toggle steps.
                    let generator: *mut AnyObject =
                        msg_send![class!(UIImpactFeedbackGenerator), alloc];
                    let generator: *mut AnyObject = msg_send![generator, initWithStyle: 0_isize];
                    let _: () = msg_send![generator, prepare];
                    let _: () = msg_send![generator, impactOccurred];
                }
            }
        }
    }
}

/// Returns the root view controller of the key window, or null if there is
/// no key window yet (e.g. very early in launch, before the first
/// `IosWindow` has called `makeKeyAndVisible`).
unsafe fn active_root_view_controller() -> *mut AnyObject {
    unsafe {
        let app: *mut AnyObject = msg_send![class!(UIApplication), sharedApplication];
        let window: *mut AnyObject = msg_send![app, keyWindow];
        if window.is_null() {
            return std::ptr::null_mut();
        }
        msg_send![window, rootViewController]
    }
}
