//! `NSURLSession`-backed [`HttpClient`] for iOS.
//!
//! GPUI applications start with `NullHttpClient`, which fails every request,
//! so remote images (`img("https://…")`), animated GIFs and anything else
//! using `cx.http_client()` are dead on iOS unless a client is installed.
//! [`run_app`](super::ffi::run_app) installs [`IosHttpClient`] before the
//! app callback runs; hosts that want something else can call
//! `cx.set_http_client` from their callback.
//!
//! Requests go through `NSURLSession.sharedSession`, so App Transport
//! Security, system proxies and the device's certificate store all apply.
//! Redirects are always followed (the session default); the
//! `follow_redirects` flag is accepted but not enforced.

use anyhow::{Result, anyhow};
use futures::channel::oneshot;
use futures::future::BoxFuture;
use gpui::http_client::{HttpClient, HttpResponse};
use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send};
use parking_lot::Mutex;
use std::ffi::c_void;
use std::sync::Arc;

/// HTTP client that performs GET requests with `NSURLSession`.
#[derive(Debug, Default, Clone, Copy)]
pub struct IosHttpClient;

type ResponseSender = oneshot::Sender<Result<HttpResponse>>;

impl HttpClient for IosHttpClient {
    fn get(&self, url: &str, _follow_redirects: bool) -> BoxFuture<'static, Result<HttpResponse>> {
        let url = url.to_string();
        Box::pin(async move {
            let (tx, rx) = oneshot::channel();
            // SAFETY: NSURLSession is thread-safe; the completion block only
            // touches its own captures and Foundation objects handed to it.
            unsafe { start_request(&url, tx)? };
            rx.await
                .map_err(|_| anyhow!("NSURLSession completion handler was dropped"))?
        })
    }
}

unsafe fn start_request(url: &str, sender: ResponseSender) -> Result<()> {
    unsafe {
        let url_string = super::util::nsstring(url);
        let ns_url: *mut AnyObject = msg_send![class!(NSURL), URLWithString: url_string];
        if ns_url.is_null() {
            return Err(anyhow!("invalid URL: {url}"));
        }

        let request: *mut AnyObject =
            msg_send![class!(NSMutableURLRequest), requestWithURL: ns_url];
        if request.is_null() {
            return Err(anyhow!("could not create NSMutableURLRequest for {url}"));
        }
        let user_agent = super::util::nsstring(concat!("gpui_mobile/", env!("CARGO_PKG_VERSION")));
        let header = super::util::nsstring("User-Agent");
        let _: () = msg_send![request, setValue: user_agent, forHTTPHeaderField: header];

        let session: *mut AnyObject = msg_send![class!(NSURLSession), sharedSession];
        if session.is_null() {
            return Err(anyhow!("NSURLSession.sharedSession unavailable"));
        }

        let sender = Arc::new(Mutex::new(Some(sender)));
        let url_for_error = url.to_string();
        let block = block2::RcBlock::new(
            move |data: *mut AnyObject, response: *mut AnyObject, error: *mut AnyObject| {
                let result = completion_result(data, response, error, &url_for_error);
                if let Some(sender) = sender.lock().take() {
                    let _ = sender.send(result);
                }
            },
        );

        let task: *mut AnyObject =
            msg_send![session, dataTaskWithRequest: request, completionHandler: &*block];
        if task.is_null() {
            return Err(anyhow!(
                "NSURLSession refused to create a data task for {url}"
            ));
        }
        let _: () = msg_send![task, resume];
        // NSURLSession copies the block, so our reference may go away now.
        drop(block);
        Ok(())
    }
}

/// Runs on an `NSURLSession` delegate queue: convert the Foundation objects
/// into owned Rust values before returning, since none of them outlive the
/// block invocation.
unsafe fn completion_result(
    data: *mut AnyObject,
    response: *mut AnyObject,
    error: *mut AnyObject,
    url: &str,
) -> Result<HttpResponse> {
    unsafe {
        if !error.is_null() {
            let description: *mut AnyObject = msg_send![error, localizedDescription];
            let message = nsstring_to_string(description)
                .unwrap_or_else(|| "unknown NSURLSession error".to_string());
            return Err(anyhow!("request to {url} failed: {message}"));
        }

        let mut status = 200_u16;
        if !response.is_null() {
            let is_http: Bool = msg_send![response, isKindOfClass: class!(NSHTTPURLResponse)];
            if is_http.as_bool() {
                let code: isize = msg_send![response, statusCode];
                status = u16::try_from(code).unwrap_or(0);
            }
        }
        let status = http::StatusCode::from_u16(status)
            .map_err(|_| anyhow!("request to {url} returned invalid status {status}"))?;

        let mut body = Vec::new();
        if !data.is_null() {
            let length: usize = msg_send![data, length];
            let bytes: *const c_void = msg_send![data, bytes];
            if !bytes.is_null() && length > 0 {
                body = std::slice::from_raw_parts(bytes as *const u8, length).to_vec();
            }
        }

        Ok(HttpResponse { status, body })
    }
}

unsafe fn nsstring_to_string(string: *mut AnyObject) -> Option<String> {
    unsafe {
        if string.is_null() {
            return None;
        }
        let utf8: *const std::ffi::c_char = msg_send![string, UTF8String];
        if utf8.is_null() {
            return None;
        }
        Some(
            std::ffi::CStr::from_ptr(utf8)
                .to_string_lossy()
                .into_owned(),
        )
    }
}
