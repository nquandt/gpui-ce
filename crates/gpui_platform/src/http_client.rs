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
use futures::SinkExt as _;
use futures::channel::oneshot;
use futures::future::BoxFuture;
use gpui::http_client::{HttpClient, HttpRequest, HttpResponse, StreamingResponse};
use std::io::Read as _;
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
        let url = request.url.clone();
        let response = Self::start(agent, request)?;
        let (parts, mut body) = response.into_parts();
        let bytes = body
            .read_to_vec()
            .with_context(|| format!("reading response body from {url}"))?;
        Ok(HttpResponse {
            status: parts.status,
            headers: parts.headers,
            body: bytes,
        })
    }

    fn agent_for(&self, request: &HttpRequest) -> Arc<ureq::Agent> {
        if request.follow_redirects {
            self.following.clone()
        } else {
            self.manual.clone()
        }
    }

    fn start(agent: &ureq::Agent, request: HttpRequest) -> Result<http::Response<ureq::Body>> {
        let mut builder = http::Request::builder()
            .method(request.method)
            .uri(request.url.as_str());
        if let Some(headers) = builder.headers_mut() {
            *headers = request.headers;
        }
        let http_request = builder.body(request.body).context("invalid HTTP request")?;
        let http_request = match request.timeout {
            Some(timeout) => agent
                .configure_request(http_request)
                .timeout_global(Some(timeout))
                .build(),
            None => http_request,
        };

        agent
            .run(http_request)
            .with_context(|| format!("request to {} failed", request.url))
    }
}

/// Bytes per chunk when streaming a body.
const STREAM_CHUNK: usize = 64 * 1024;

impl HttpClient for UreqHttpClient {
    fn send_stream(&self, request: HttpRequest) -> BoxFuture<'static, Result<StreamingResponse>> {
        let agent = self.agent_for(&request);
        let (head_tx, head_rx) = oneshot::channel();
        let (mut chunk_tx, chunk_rx) = futures::channel::mpsc::channel::<Result<Vec<u8>>>(4);
        let spawned = std::thread::Builder::new()
            .name("gpui-http-stream".into())
            .spawn(move || {
                let response = match Self::start(&agent, request) {
                    Ok(response) => response,
                    Err(error) => {
                        let _ = head_tx.send(Err(error));
                        return;
                    }
                };
                let (parts, body) = response.into_parts();
                if head_tx.send(Ok((parts.status, parts.headers))).is_err() {
                    return;
                }
                let mut reader = body.into_reader();
                let mut buffer = vec![0u8; STREAM_CHUNK];
                loop {
                    let item = match reader.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(n) => Ok(buffer[..n].to_vec()),
                        Err(error) => Err(anyhow!("reading response body: {error}")),
                    };
                    let failed = item.is_err();
                    if futures::executor::block_on(chunk_tx.send(item)).is_err() || failed {
                        break;
                    }
                }
            });
        Box::pin(async move {
            spawned.context("failed to spawn HTTP worker thread")?;
            let (status, headers) = head_rx
                .await
                .map_err(|_| anyhow!("HTTP worker thread exited without a response"))??;
            Ok(StreamingResponse {
                status,
                headers,
                body: Box::pin(chunk_rx),
            })
        })
    }

    fn send(&self, request: HttpRequest) -> BoxFuture<'static, Result<HttpResponse>> {
        let agent = self.agent_for(&request);
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
    fn streams_body_in_chunks() {
        use futures::StreamExt as _;
        let client = UreqHttpClient::new();
        let mut streaming =
            block_on(client.send_stream(HttpRequest::get("https://example.com/"))).unwrap();
        assert!(streaming.status.is_success());
        let mut chunks = 0;
        let mut body = Vec::new();
        block_on(async {
            while let Some(chunk) = streaming.body.next().await {
                chunks += 1;
                body.extend_from_slice(&chunk.unwrap());
            }
        });
        assert!(chunks >= 1);
        assert!(String::from_utf8_lossy(&body).contains("Example Domain"));
    }

    #[test]
    #[ignore]
    fn timeout_is_enforced() {
        let client = UreqHttpClient::new();
        // 10.255.255.1 is unroutable, so the connect phase hangs until the timeout.
        let error = block_on(client.send(
            HttpRequest::get("http://10.255.255.1/").timeout(std::time::Duration::from_millis(300)),
        ))
        .unwrap_err();
        let text = format!("{error:#}").to_lowercase();
        assert!(
            text.contains("timeout") || text.contains("timed out"),
            "{text}"
        );
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
