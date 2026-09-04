use objc2::runtime::AnyObject;
use objc2::{class, msg_send};

pub fn share_text(text: &str, _subject: Option<&str>) -> Result<(), String> {
    unsafe {
        // SAFETY: `nsstring` builds the NSString via
        // `initWithBytes:length:encoding:`, so it's safe for `text` (a Rust
        // `&str`) which is not NUL-terminated — unlike
        // `stringWithUTF8String:`, which reads until a NUL byte and would
        // read out of bounds for arbitrary `&str` input.
        let ns_text = crate::ios::util::nsstring(text);
        if ns_text.is_null() {
            return Err("Failed to create NSString".into());
        }
        present_share_sheet(ns_text)
    }
}

unsafe fn present_share_sheet(text: *mut AnyObject) -> Result<(), String> {
    // NSArray *items = @[text];
    let items: *mut AnyObject = msg_send![class!(NSArray), arrayWithObject: text];
    if items.is_null() {
        return Err("Failed to create NSArray".into());
    }

    // UIActivityViewController *vc = [[UIActivityViewController alloc]
    //     initWithActivityItems:items applicationActivities:nil];
    let vc: *mut AnyObject = msg_send![class!(UIActivityViewController), alloc];
    let nil: *mut AnyObject = std::ptr::null_mut();
    let vc: *mut AnyObject = msg_send![vc,
        initWithActivityItems: items,
        applicationActivities: nil
    ];
    if vc.is_null() {
        return Err("Failed to create UIActivityViewController".into());
    }

    // Get the root view controller to present from.
    let app: *mut AnyObject = msg_send![class!(UIApplication), sharedApplication];
    let key_window: *mut AnyObject = msg_send![app, keyWindow];
    if key_window.is_null() {
        return Err("No key window available".into());
    }
    let root_vc: *mut AnyObject = msg_send![key_window, rootViewController];
    if root_vc.is_null() {
        return Err("No root view controller".into());
    }

    // [rootVC presentViewController:vc animated:YES completion:nil];
    let _: () = msg_send![root_vc,
        presentViewController: vc,
        animated: true,
        completion: nil
    ];

    Ok(())
}
