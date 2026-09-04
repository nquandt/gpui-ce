//! Shared iOS utility helpers.

use objc2::runtime::AnyObject;
use objc2::{class, msg_send};

/// Create an autoreleased `NSString` from a Rust `&str`.
///
/// Uses `alloc + initWithBytes:length:encoding:` (NSUTF8StringEncoding = 4)
/// followed by `autorelease`, so callers never need to manually `release` the
/// returned pointer.
///
/// # Safety
/// Must be called from an Objective-C thread that has an active autorelease
/// pool (guaranteed by any UIKit entry-point or GCD-dispatched block).
pub unsafe fn nsstring(s: &str) -> *mut AnyObject {
    let ns: *mut AnyObject = msg_send![class!(NSString), alloc];
    let ns: *mut AnyObject = msg_send![ns,
        initWithBytes: s.as_ptr() as *const std::ffi::c_void,
        length: s.len(),
        encoding: 4u64 // NSUTF8StringEncoding
    ];
    msg_send![ns, autorelease]
}

// ── UniformTypeIdentifiers globals ──────────────────────────────────────────
//
// `UTType` content-type constants used by `UIDocumentPickerViewController`.
// These are `UTType *const` globals exported by the
// UniformTypeIdentifiers.framework (available since iOS 14).
#[link(name = "UniformTypeIdentifiers", kind = "framework")]
unsafe extern "C" {
    static UTTypeItem: *mut AnyObject;
    static UTTypeFolder: *mut AnyObject;
}

/// The `UTType` representing "any item" (files and packages) — used to let
/// `UIDocumentPickerViewController` open arbitrary files.
pub fn ut_type_item() -> *mut AnyObject {
    unsafe { UTTypeItem }
}

/// The `UTType` representing a folder — used to let
/// `UIDocumentPickerViewController` open directories only.
pub fn ut_type_folder() -> *mut AnyObject {
    unsafe { UTTypeFolder }
}
