//! `UIDocumentPickerViewController` delegate bridging picked URLs back to a
//! Rust `oneshot::Sender`, for [`Platform::prompt_for_paths`](gpui::Platform::prompt_for_paths).
//!
//! iOS has no synchronous file dialog: `UIDocumentPickerViewController` is
//! presented modally and reports back through an Objective-C delegate
//! protocol (`UIDocumentPickerDelegate`), asynchronously, on the main run
//! loop. This module implements that delegate as a tiny `NSObject` subclass
//! (built at runtime via `ClassBuilder`, the same pattern used for
//! `GPUIViewController`/`GPUIMetalView` in `window.rs`) that owns a boxed
//! `oneshot::Sender` and resolves it exactly once, from whichever delegate
//! method UIKit calls first.

use futures::channel::oneshot;
use gpui::Result;
use objc2::runtime::{AnyClass, AnyObject, ClassBuilder, Sel};
use objc2::{class, msg_send, sel};
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::Once;

const SENDER_IVAR: &str = "gpui_sender_ptr";

type PathsSender = oneshot::Sender<Result<Option<Vec<PathBuf>>>>;

static CLASS_REGISTERED: Once = Once::new();

/// Extracts and clears the boxed sender stored in `this`'s ivar, taking
/// ownership so it can be resolved (or dropped) exactly once even if UIKit
/// somehow calls more than one delegate method.
unsafe fn take_sender(this: *mut AnyObject) -> Option<PathsSender> {
    unsafe {
        #[allow(deprecated)]
        let slot = (*this).get_mut_ivar::<*mut c_void>(SENDER_IVAR);
        let ptr = *slot;
        *slot = std::ptr::null_mut();
        if ptr.is_null() {
            None
        } else {
            Some(*Box::from_raw(ptr as *mut PathsSender))
        }
    }
}

fn register_class() -> &'static AnyClass {
    CLASS_REGISTERED.call_once(|| {
        let superclass = class!(NSObject);
        let mut decl = ClassBuilder::new(c"GPUIDocumentPickerDelegate", superclass).unwrap();
        decl.add_ivar::<*mut c_void>(c"gpui_sender_ptr");

        extern "C" fn did_pick_documents(
            this: *mut AnyObject,
            _sel: Sel,
            _controller: *mut AnyObject,
            urls: *mut AnyObject,
        ) {
            unsafe {
                let Some(sender) = take_sender(this) else {
                    return;
                };

                let count: usize = msg_send![urls, count];
                let mut paths = Vec::with_capacity(count);
                for i in 0..count {
                    let url: *mut AnyObject = msg_send![urls, objectAtIndex: i];
                    if url.is_null() {
                        continue;
                    }
                    // The document picker hands back security-scoped URLs:
                    // the app must bracket access with
                    // `start`/`stopAccessingSecurityScopedResource`. We start
                    // it here (best-effort — the return value only indicates
                    // whether scoping was necessary at all, e.g. it's a
                    // no-op for files already inside the app sandbox) and
                    // document that the caller now owns balancing it with a
                    // matching `stop` once done with the path; this crate has
                    // no "finished with this path" hook to pair one with
                    // automatically.
                    let _: bool = msg_send![url, startAccessingSecurityScopedResource];
                    let path_ns: *mut AnyObject = msg_send![url, path];
                    if path_ns.is_null() {
                        continue;
                    }
                    let utf8: *const i8 = msg_send![path_ns, UTF8String];
                    if utf8.is_null() {
                        continue;
                    }
                    if let Ok(s) = std::ffi::CStr::from_ptr(utf8).to_str() {
                        paths.push(PathBuf::from(s));
                    }
                }
                let _ = sender.send(Ok(Some(paths)));
            }
        }

        extern "C" fn was_cancelled(this: *mut AnyObject, _sel: Sel, _controller: *mut AnyObject) {
            unsafe {
                if let Some(sender) = take_sender(this) {
                    let _ = sender.send(Ok(None));
                }
            }
        }

        extern "C" fn dealloc(this: *mut AnyObject, _sel: Sel) {
            unsafe {
                // If the picker (and thus this delegate) is torn down
                // without either delegate method firing — e.g. the user
                // dismisses it by swiping — free the boxed sender so it
                // isn't leaked. Dropping an unresolved `oneshot::Sender`
                // cancels the receiver, which callers already handle.
                let _ = take_sender(this);
                let superclass = class!(NSObject);
                let _: () = msg_send![super(this, superclass), dealloc];
            }
        }

        unsafe {
            decl.add_method(
                sel!(documentPicker:didPickDocumentsAtURLs:),
                did_pick_documents
                    as extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, *mut AnyObject),
            );
            decl.add_method(
                sel!(documentPickerWasCancelled:),
                was_cancelled as extern "C" fn(*mut AnyObject, Sel, *mut AnyObject),
            );
            decl.add_method(sel!(dealloc), dealloc as extern "C" fn(*mut AnyObject, Sel));
        }

        decl.register();
    });

    class!(GPUIDocumentPickerDelegate)
}

/// Creates a new delegate instance that will resolve `sender` exactly once,
/// with the paths the user picked (`Ok(Some(paths))`), `Ok(None)` if they
/// cancelled, or drop the sender (cancelling the receiver) if the picker is
/// torn down without either outcome.
pub fn make_delegate(sender: PathsSender) -> *mut AnyObject {
    unsafe {
        let class = register_class();
        let delegate: *mut AnyObject = msg_send![class, alloc];
        let delegate: *mut AnyObject = msg_send![delegate, init];
        let boxed = Box::into_raw(Box::new(sender)) as *mut c_void;
        #[allow(deprecated)]
        {
            *(*delegate).get_mut_ivar::<*mut c_void>(SENDER_IVAR) = boxed;
        }
        delegate
    }
}

/// Keeps `delegate` alive for as long as `picker` is, via
/// `objc_setAssociatedObject`. Necessary because
/// `UIDocumentPickerViewController.delegate` is a weak reference — without
/// an owner elsewhere, our delegate would be deallocated immediately after
/// this function returns (nothing else retains it), and UIKit would never
/// see its callbacks.
pub fn retain_delegate_on(picker: *mut AnyObject, delegate: *mut AnyObject) {
    unsafe {
        unsafe extern "C" {
            fn objc_setAssociatedObject(
                object: *mut AnyObject,
                key: *const c_void,
                value: *mut AnyObject,
                policy: usize,
            );
        }
        // OBJC_ASSOCIATION_RETAIN. Rust has no C-style leading-zero octal
        // literals (unlike the Objective-C runtime header this constant is
        // defined in) — `0o1401` is the correct octal spelling.
        const OBJC_ASSOCIATION_RETAIN: usize = 0o1401;
        static ASSOCIATION_KEY: u8 = 0;
        objc_setAssociatedObject(
            picker,
            &ASSOCIATION_KEY as *const u8 as *const c_void,
            delegate,
            OBJC_ASSOCIATION_RETAIN,
        );
    }
}
