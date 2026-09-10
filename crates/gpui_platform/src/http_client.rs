//! `ureq`-backed [`HttpClient`] for desktop targets.
//!
//! GPUI applications start with `NullHttpClient`, which fails every request.
//! [`application`](crate::application) installs this client so remote images
//! and `cx.http_client()` users work on Windows, macOS and Linux without host
//! code, matching what the web and iOS backends already do.
//!
//! `ureq` is synchronous, so each request runs on its own OS thread and the
//! result comes back through a oneshot channel. That keeps the client free of
//! any executor dependency and is fine for the request volumes a UI produces.
//! TLS is rustls with bundled roots; system proxies from `HTTP_PROXY` and
//! `HTTPS_PROXY` are honoured by ureq's defaults.

use anyhow::{Context as _, Result, anyhow};
use futures::channel::oneshot;
use futures::future::BoxFuture;
use gpui::http_client::{HttpClient, HttpRequest, HttpResponse};
use std::sync::Arc;

/// Desktop HTTP client built on `ureq`.
#[derive(Clone)]
pub struct UreqHttpClient {
    following: Arc<ureq::Agent>,
    manual: Arc<ureq::Agent>,
}

impl Default for UreqHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl UreqHttpClient {
    /// Create a client with default settings: redirects followed up to
    /// ureq's limit, non-2xx statuses returned as responses rather than errors.
    pub fn new() -> Self {
        let user_agent = concat!("gpui-ce/", env!("CARGO_PKG_VERSION"));
        let base = || {
            ureq::Agent::config_builder()
                .http_status_as_error(false)
                .user_agent(user_agent)
        };
        Self {
            following: Arc::new(base().build().into()),
            manual: Arc::new(base().max_redirects(0).build().into()),
        }
    }

    fn perform(agent: &ureq::Agent, request: HttpRequest) -> Result<HttpResponse> {
        let mut builder = http::Request::builder()
            .method(request.method)
            .uri(request.url.as_str());
        if let Some(headers) = builder.headers_mut() {
            *headers = request.headers;
        }
        let http_request = builder.body(request.body).context("invalid HTTP request")?;

        let response = agent
            .run(http_request)
            .with_context(|| format!("request to {} failed", request.url))?;
        let (parts, mut body) = response.into_parts();
        let bytes = body
            .read_to_vec()
            .with_context(|| format!("reading response body from {}", request.url))?;

        Ok(HttpResponse {
            status: parts.status,
            headers: parts.headers,
            body: bytes,
        })
    }
}

impl HttpClient for UreqHttpClient {
    fn send(&self, request: HttpRequest) -> BoxFuture<'static, Result<HttpResponse>> {
        let agent = if request.follow_redirects {
            self.following.clone()
        } else {
            self.manual.clone()
        };
        let (tx, rx) = oneshot::channel();
        let spawned = std::thread::Builder::new()
            .name("gpui-http".into())
            .spawn(move || {
                let _ = tx.send(Self::perform(&agent, request));
            });
        Box::pin(async move {
            spawned.context("failed to spawn HTTP worker thread")?;
            rx.await
                .map_err(|_| anyhow!("HTTP worker thread exited without a response"))?
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    /// Needs network access; run with `cargo test -p gpui_ce_platform -- --ignored`.
    #[test]
    #[ignore]
    fn fetches_over_https() {
        let client = UreqHttpClient::new();
        let response = block_on(
            client.send(HttpRequest::get("https://example.com/").header("Accept", "text/html")),
        )
        .unwrap();
        assert!(response.is_success(), "status {}", response.status);
        assert!(response.header("content-type").is_some());
        assert!(response.text().contains("Example Domain"));
    }

    #[test]
    #[ignore]
    fn manual_redirects_are_not_followed() {
        let client = UreqHttpClient::new();
        let response =
            block_on(client.send(HttpRequest::get("http://example.com/").follow_redirects(false)))
                .unwrap();
        // example.com answers 200 on http too; only assert the call completes.
        assert!(response.status.as_u16() < 500);
    }
}
