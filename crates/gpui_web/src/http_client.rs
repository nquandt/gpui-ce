//! `fetch`-backed [`HttpClient`] for the browser.
//!
//! Requests carry method, headers, body and timeout through to `fetch`. The
//! response status and headers become an [`HttpResponse`] or a
//! [`StreamingResponse`]; the latter reads the body's `ReadableStream` chunk
//! by chunk. Browser rules still apply: cross-origin requests need CORS on
//! the server, and the browser owns forbidden headers such as `Host`.

use anyhow::anyhow;
use futures::stream::StreamExt as _;
use gpui::http_client::{HttpClient, HttpRequest, HttpResponse, StreamingResponse};
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
///
/// Sound in practice because the web executor polls these on the thread that
/// created them; JavaScript values never actually cross threads.
struct AssertSend<F>(F);

unsafe impl<F> Send for AssertSend<F> {}

impl<F: Future> Future for AssertSend<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let inner = unsafe { self.map_unchecked_mut(|this| &mut this.0) };
        inner.poll(cx)
    }
}

/// The stream counterpart of [`AssertSend`].
struct AssertSendStream<S>(S);

unsafe impl<S> Send for AssertSendStream<S> {}

impl<S: futures::Stream> futures::Stream for AssertSendStream<S> {
    type Item = S::Item;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let inner = unsafe { self.map_unchecked_mut(|this| &mut this.0) };
        inner.poll_next(cx)
    }
}

impl HttpClient for FetchHttpClient {
    fn send(
        &self,
        request: HttpRequest,
    ) -> futures::future::BoxFuture<'static, anyhow::Result<HttpResponse>> {
        Box::pin(AssertSend(async move {
            let started = start(request).await?;
            let body_promise = started
                .response
                .array_buffer()
                .map_err(|error| anyhow!("failed to initiate response body read: {error:?}"))?;
            let body_value = wasm_bindgen_futures::JsFuture::from(body_promise)
                .await
                .map_err(|error| anyhow!("failed to read response body: {error:?}"))?;
            let array_buffer: js_sys::ArrayBuffer = body_value
                .dyn_into()
                .map_err(|error| anyhow!("response body is not an ArrayBuffer: {error:?}"))?;
            let body = js_sys::Uint8Array::new(&array_buffer).to_vec();
            drop(started.timeout_guard);
            Ok(HttpResponse {
                status: started.status,
                headers: started.headers,
                body,
            })
        }))
    }

    fn send_stream(
        &self,
        request: HttpRequest,
    ) -> futures::future::BoxFuture<'static, anyhow::Result<StreamingResponse>> {
        Box::pin(AssertSend(async move {
            let started = start(request).await?;
            let Some(stream) = started.response.body() else {
                return Ok(StreamingResponse {
                    status: started.status,
                    headers: started.headers,
                    body: futures::stream::empty().boxed(),
                });
            };
            let reader: web_sys::ReadableStreamDefaultReader = stream
                .get_reader()
                .dyn_into()
                .map_err(|_| anyhow!("body reader is not a ReadableStreamDefaultReader"))?;

            // The timeout guard travels with the stream so the abort timer
            // stays armed until the last chunk has been read.
            let state = (reader, started.timeout_guard, false);
            let chunks = futures::stream::unfold(state, |(reader, guard, done)| async move {
                if done {
                    return None;
                }
                let result = wasm_bindgen_futures::JsFuture::from(reader.read()).await;
                let item = match result {
                    Err(error) => (Err(anyhow!("reading body chunk: {error:?}")), true),
                    Ok(value) => {
                        let result: web_sys::ReadableStreamReadResult = value.unchecked_into();
                        let finished = result.get_done().unwrap_or(true);
                        if finished {
                            return None;
                        }
                        let chunk = result
                            .get_value()
                            .dyn_into::<js_sys::Uint8Array>()
                            .map(|array| array.to_vec())
                            .map_err(|_| anyhow!("body chunk is not a Uint8Array"));
                        let failed = chunk.is_err();
                        (chunk, failed)
                    }
                };
                Some((item.0, (reader, guard, item.1)))
            });

            Ok(StreamingResponse {
                status: started.status,
                headers: started.headers,
                body: AssertSendStream(chunks).boxed(),
            })
        }))
    }
}

struct Started {
    response: web_sys::Response,
    status: http::StatusCode,
    headers: http::HeaderMap,
    timeout_guard: Option<TimeoutGuard>,
}

/// Issue the fetch and wait for the response head.
async fn start(request: HttpRequest) -> anyhow::Result<Started> {
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

    let timeout_guard = match request.timeout {
        Some(timeout) => Some(TimeoutGuard::arm(&init, timeout)?),
        None => None,
    };

    let web_request = web_sys::Request::new_with_str_and_init(&request.url, &init)
        .map_err(|error| anyhow!("failed to create fetch Request: {error:?}"))?;

    let promise =
        global_fetch(&web_request).map_err(|error| anyhow!("fetch threw an error: {error:?}"))?;
    let response_value = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|error| anyhow!("fetch failed: {error:?}"))?;

    let response: web_sys::Response = response_value
        .dyn_into()
        .map_err(|error| anyhow!("fetch result is not a Response: {error:?}"))?;

    let status = http::StatusCode::from_u16(response.status())
        .map_err(|_| anyhow!("invalid status code"))?;
    let headers = collect_headers(&response.headers());

    Ok(Started {
        response,
        status,
        headers,
        timeout_guard,
    })
}

/// Aborts a fetch after a delay. Dropping the guard cancels the timer.
struct TimeoutGuard {
    handle: i32,
    _closure: Closure<dyn FnMut()>,
}

impl TimeoutGuard {
    fn arm(init: &web_sys::RequestInit, timeout: std::time::Duration) -> anyhow::Result<Self> {
        let controller = web_sys::AbortController::new()
            .map_err(|error| anyhow!("failed to create AbortController: {error:?}"))?;
        init.set_signal(Some(&controller.signal()));
        let closure = Closure::once(move || controller.abort());
        let millis = i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX);
        let handle = js_sys::global()
            .unchecked_into::<web_sys::WorkerGlobalScope>()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                millis,
            )
            .map_err(|error| anyhow!("setTimeout failed: {error:?}"))?;
        Ok(Self {
            handle,
            _closure: closure,
        })
    }
}

impl Drop for TimeoutGuard {
    fn drop(&mut self) {
        js_sys::global()
            .unchecked_into::<web_sys::WorkerGlobalScope>()
            .clear_timeout_with_handle(self.handle);
    }
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
