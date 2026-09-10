//! `fetch`-backed [`HttpClient`] for the browser.
//!
//! Requests carry method, headers and body through to `fetch`; the response
//! status, headers and body are buffered into an [`HttpResponse`]. Browser
//! rules still apply: cross-origin requests need CORS on the server, and the
//! browser owns forbidden headers such as `Host` or `Cookie`.

use anyhow::anyhow;
use gpui::http_client::{HttpClient, HttpRequest, HttpResponse};
use http::{HeaderName, HeaderValue};
use std::future::Future;
use std::pin::Pin;
use std::task::Poll;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_name = "fetch")]
    fn global_fetch(input: &web_sys::Request) -> Result<js_sys::Promise, JsValue>;
}

pub struct FetchHttpClient;

impl Default for FetchHttpClient {
    fn default() -> Self {
        Self
    }
}

#[cfg(feature = "multithreaded")]
impl FetchHttpClient {
    pub unsafe fn new() -> Self {
        Self
    }
}

#[cfg(not(feature = "multithreaded"))]
impl FetchHttpClient {
    pub fn new() -> Self {
        Self
    }
}

/// Wraps a `!Send` future to satisfy the `Send` bound on `BoxFuture`.
struct AssertSend<F>(F);

unsafe impl<F> Send for AssertSend<F> {}

impl<F: Future> Future for AssertSend<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let inner = unsafe { self.map_unchecked_mut(|this| &mut this.0) };
        inner.poll(cx)
    }
}

impl HttpClient for FetchHttpClient {
    fn send(
        &self,
        request: HttpRequest,
    ) -> futures::future::BoxFuture<'static, anyhow::Result<HttpResponse>> {
        Box::pin(AssertSend(async move { perform(request).await }))
    }
}

async fn perform(request: HttpRequest) -> anyhow::Result<HttpResponse> {
    let init = web_sys::RequestInit::new();
    init.set_method(request.method.as_str());

    if !request.follow_redirects {
        init.set_redirect(web_sys::RequestRedirect::Manual);
    }

    let headers = web_sys::Headers::new()
        .map_err(|error| anyhow!("failed to create fetch Headers: {error:?}"))?;
    for (name, value) in &request.headers {
        let value = value
            .to_str()
            .map_err(|_| anyhow!("header {name} is not valid UTF-8"))?;
        headers
            .append(name.as_str(), value)
            .map_err(|error| anyhow!("failed to set header {name}: {error:?}"))?;
    }
    init.set_headers(&headers);

    if !request.body.is_empty() {
        let body = js_sys::Uint8Array::from(request.body.as_slice());
        init.set_body(&body);
    }

    let web_request = web_sys::Request::new_with_str_and_init(&request.url, &init)
        .map_err(|error| anyhow!("failed to create fetch Request: {error:?}"))?;

    let promise =
        global_fetch(&web_request).map_err(|error| anyhow!("fetch threw an error: {error:?}"))?;
    let response_value = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|error| anyhow!("fetch failed: {error:?}"))?;

    let web_response: web_sys::Response = response_value
        .dyn_into()
        .map_err(|error| anyhow!("fetch result is not a Response: {error:?}"))?;

    let status = http::StatusCode::from_u16(web_response.status())
        .map_err(|_| anyhow!("invalid status code"))?;

    let response_headers = collect_headers(&web_response.headers());

    let body_promise = web_response
        .array_buffer()
        .map_err(|error| anyhow!("failed to initiate response body read: {error:?}"))?;
    let body_value = wasm_bindgen_futures::JsFuture::from(body_promise)
        .await
        .map_err(|error| anyhow!("failed to read response body: {error:?}"))?;
    let array_buffer: js_sys::ArrayBuffer = body_value
        .dyn_into()
        .map_err(|error| anyhow!("response body is not an ArrayBuffer: {error:?}"))?;
    let body = js_sys::Uint8Array::new(&array_buffer).to_vec();

    Ok(HttpResponse {
        status,
        headers: response_headers,
        body,
    })
}

/// Copy the entries of a `Headers` object into an `http::HeaderMap`.
///
/// `Headers` is iterable, yielding `[name, value]` pairs. Entries the `http`
/// crate rejects are skipped rather than failing the whole response.
fn collect_headers(headers: &web_sys::Headers) -> http::HeaderMap {
    let mut map = http::HeaderMap::new();
    let Ok(Some(iterator)) = js_sys::try_iter(headers) else {
        return map;
    };
    for entry in iterator.flatten() {
        let pair = js_sys::Array::from(&entry);
        let name = pair.get(0).as_string();
        let value = pair.get(1).as_string();
        if let (Some(name), Some(value)) = (name, value)
            && let (Ok(name), Ok(value)) = (
                HeaderName::try_from(name.as_str()),
                HeaderValue::try_from(value.as_str()),
            )
        {
            map.append(name, value);
        }
    }
    map
}
