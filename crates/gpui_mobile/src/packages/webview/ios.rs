use super::WebViewHandle;
use crate::ios::platform_view::IosPlatformView;
use objc2::msg_send;
use objc2::runtime::AnyObject;

#[link(name = "WebKit", kind = "framework")]
unsafe extern "C" {}

/// Get the raw `WKWebView*` behind a [`WebViewHandle`], if it still has a
/// live platform view backing it.
///
/// Mirrors the `handle.inner().as_any().downcast_ref::<IosPlatformView>()`
/// pattern used by `packages::text_field` to reach its native `UITextField`.
fn native_webview_ptr(handle: &WebViewHandle) -> Option<*mut AnyObject> {
    let platform_handle = handle.platform_handle.as_ref()?;
    let native = platform_handle
        .inner()
        .as_any()
        .downcast_ref::<IosPlatformView>()?;
    let ptr = native.native_view_ptr();
    if ptr.is_null() { None } else { Some(ptr) }
}

pub fn evaluate_javascript(handle: &WebViewHandle, script: &str) -> Result<(), String> {
    let Some(webview) = native_webview_ptr(handle) else {
        return Err("No active WebView".into());
    };
    unsafe {
        // SAFETY: `webview` is a live `WKWebView*` (checked non-null above),
        // and this is called from application code, which on iOS always
        // runs on the main thread for UI-facing APIs like this one.
        // `nsstring` returns an autoreleased NSString, so no manual release
        // is needed. The completion block is one-shot: it's dropped after
        // WebKit invokes it (or, if the webview is torn down first, WebKit
        // still calls completion handlers to avoid leaking them).
        let ns_script = crate::ios::util::nsstring(script);
        let block = block2::RcBlock::new(move |result: *mut AnyObject, error: *mut AnyObject| {
            let _ = result;
            if !error.is_null() {
                let desc: *mut AnyObject = msg_send![error, localizedDescription];
                if !desc.is_null() {
                    let utf8: *const std::ffi::c_char = msg_send![desc, UTF8String];
                    if !utf8.is_null() {
                        let msg = std::ffi::CStr::from_ptr(utf8).to_string_lossy();
                        log::warn!("evaluate_javascript: JS error: {}", msg);
                    }
                }
            }
        });
        let _: () = msg_send![webview,
            evaluateJavaScript: ns_script,
            completionHandler: &*block
        ];
    }
    Ok(())
}

pub fn go_back(handle: &WebViewHandle) -> Result<(), String> {
    let Some(webview) = native_webview_ptr(handle) else {
        return Err("No active WebView".into());
    };
    unsafe {
        let can_go_back: bool = msg_send![webview, canGoBack];
        if can_go_back {
            let _: *mut AnyObject = msg_send![webview, goBack];
        }
    }
    Ok(())
}

pub fn reload(handle: &WebViewHandle) -> Result<(), String> {
    let Some(webview) = native_webview_ptr(handle) else {
        return Err("No active WebView".into());
    };
    unsafe {
        let _: *mut AnyObject = msg_send![webview, reload];
    }
    Ok(())
}

pub fn stop_loading(handle: &WebViewHandle) -> Result<(), String> {
    let Some(webview) = native_webview_ptr(handle) else {
        return Err("No active WebView".into());
    };
    unsafe {
        let _: () = msg_send![webview, stopLoading];
    }
    Ok(())
}
