//! A small, platform-neutral HTTP abstraction.
//!
//! Every platform backend installs its own [`HttpClient`]: `fetch` on the
//! web, `NSURLSession` on iOS, and so on. Application code and GPUI's own
//! image loader only ever talk to the trait, so one app core can run on every
//! target without touching a platform HTTP library directly.
//!
//! Requests are described by [`HttpRequest`] (method, URL, headers, body) and
//! answered with a fully buffered [`HttpResponse`]. Streaming bodies are out of
//! scope for now; the response is small enough to hold in memory for the use
//! cases GPUI has today (images, JSON APIs).

use futures::future::BoxFuture;
use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// A fully buffered HTTP response.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// The HTTP status code.
    pub status: StatusCode,
    /// The response headers.
    pub headers: HeaderMap,
    /// The response body bytes.
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Create a response with the given status and body and no headers.
    pub fn new(status: StatusCode, body: Vec<u8>) -> Self {
        Self {
            status,
            headers: HeaderMap::new(),
            body,
        }
    }

    /// Whether the status code is in the 2xx range.
    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }

    /// Return an error if the status code is not in the 2xx range.
    ///
    /// The error message includes the status and the first line of the body,
    /// which is usually enough to see what an API complained about.
    pub fn error_for_status(self) -> anyhow::Result<Self> {
        if self.is_success() {
            Ok(self)
        } else {
            let body = String::from_utf8_lossy(&self.body);
            let first_line = body.lines().next().unwrap_or("").trim_end();
            anyhow::bail!("HTTP {}: {}", self.status, first_line)
        }
    }

    /// The body as UTF-8 text. Invalid sequences are replaced.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// Deserialize the body as JSON.
    pub fn json<T: DeserializeOwned>(&self) -> anyhow::Result<T> {
        Ok(serde_json::from_slice(&self.body)?)
    }

    /// The value of a header as a string, if present and valid UTF-8.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }
}

/// A request to be performed by an [`HttpClient`].
///
/// Build one with the constructors and the chained setters:
///
/// ```
/// # use gpui::http_client::HttpRequest;
/// let request = HttpRequest::post("https://example.com/api/items")
///     .header("Authorization", "Bearer token")
///     .body(b"{\"name\": \"widget\"}".to_vec());
/// assert_eq!(request.method, http::Method::POST);
/// ```
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// The HTTP method.
    pub method: Method,
    /// The absolute URL.
    pub url: String,
    /// Request headers.
    pub headers: HeaderMap,
    /// The request body. Empty for requests without a body.
    pub body: Vec<u8>,
    /// Whether the client should follow redirects automatically.
    pub follow_redirects: bool,
}

impl HttpRequest {
    /// Create a request with the given method and URL.
    pub fn new(method: Method, url: impl Into<String>) -> Self {
        Self {
            method,
            url: url.into(),
            headers: HeaderMap::new(),
            body: Vec::new(),
            follow_redirects: true,
        }
    }

    /// Create a GET request.
    pub fn get(url: impl Into<String>) -> Self {
        Self::new(Method::GET, url)
    }

    /// Create a POST request.
    pub fn post(url: impl Into<String>) -> Self {
        Self::new(Method::POST, url)
    }

    /// Create a PUT request.
    pub fn put(url: impl Into<String>) -> Self {
        Self::new(Method::PUT, url)
    }

    /// Create a PATCH request.
    pub fn patch(url: impl Into<String>) -> Self {
        Self::new(Method::PATCH, url)
    }

    /// Create a DELETE request.
    pub fn delete(url: impl Into<String>) -> Self {
        Self::new(Method::DELETE, url)
    }

    /// Add a header. Invalid names or values are ignored with a log message
    /// rather than panicking, since header values often come from user data.
    pub fn header(mut self, name: &str, value: &str) -> Self {
        match (HeaderName::try_from(name), HeaderValue::try_from(value)) {
            (Ok(name), Ok(value)) => {
                self.headers.append(name, value);
            }
            _ => log::warn!("ignoring invalid HTTP header {name:?}"),
        }
        self
    }

    /// The value of a header as a string, if present and valid UTF-8.
    pub fn header_value(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }

    /// Set the request body.
    pub fn body(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }

    /// Serialize `value` as the JSON body and set the content type.
    pub fn json<T: Serialize>(self, value: &T) -> anyhow::Result<Self> {
        let body = serde_json::to_vec(value)?;
        Ok(self.header("Content-Type", "application/json").body(body))
    }

    /// Set whether redirects are followed. Defaults to `true`.
    pub fn follow_redirects(mut self, follow: bool) -> Self {
        self.follow_redirects = follow;
        self
    }
}

/// A trait for making HTTP requests.
///
/// Implementors provide [`send`](HttpClient::send). The convenience methods
/// have default implementations built on it.
pub trait HttpClient: 'static + Send + Sync {
    /// Perform a request and return the full response.
    fn send(&self, request: HttpRequest) -> BoxFuture<'static, anyhow::Result<HttpResponse>>;

    /// Perform a GET request and return the full response.
    fn get(
        &self,
        url: &str,
        follow_redirects: bool,
    ) -> BoxFuture<'static, anyhow::Result<HttpResponse>> {
        self.send(HttpRequest::get(url).follow_redirects(follow_redirects))
    }

    /// Perform a POST request with the given body.
    fn post(&self, url: &str, body: Vec<u8>) -> BoxFuture<'static, anyhow::Result<HttpResponse>> {
        self.send(HttpRequest::post(url).body(body))
    }
}

/// An HTTP client that always returns an error.
pub struct NullHttpClient;

impl HttpClient for NullHttpClient {
    fn send(&self, _request: HttpRequest) -> BoxFuture<'static, anyhow::Result<HttpResponse>> {
        Box::pin(async { anyhow::bail!("No HttpClient available") })
    }
}

/// An HTTP client that blocks all requests.
pub struct BlockedHttpClient;

impl BlockedHttpClient {
    /// Create a new `BlockedHttpClient`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for BlockedHttpClient {
    fn default() -> Self {
        Self
    }
}

impl HttpClient for BlockedHttpClient {
    fn send(&self, _request: HttpRequest) -> BoxFuture<'static, anyhow::Result<HttpResponse>> {
        Box::pin(async {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "BlockedHttpClient disallowed request",
            )
            .into())
        })
    }
}

/// A fake HTTP client for testing.
#[cfg(any(test, feature = "test-support"))]
pub struct FakeHttpClient {
    status: StatusCode,
}

#[cfg(any(test, feature = "test-support"))]
impl FakeHttpClient {
    /// Create a fake client that returns 404 responses.
    pub fn with_404_response() -> std::sync::Arc<dyn HttpClient> {
        std::sync::Arc::new(Self {
            status: StatusCode::NOT_FOUND,
        })
    }

    /// Create a fake client that returns 200 responses.
    pub fn with_200_response() -> std::sync::Arc<dyn HttpClient> {
        std::sync::Arc::new(Self {
            status: StatusCode::OK,
        })
    }
}

#[cfg(any(test, feature = "test-support"))]
impl HttpClient for FakeHttpClient {
    fn send(&self, _request: HttpRequest) -> BoxFuture<'static, anyhow::Result<HttpResponse>> {
        let status = self.status;
        Box::pin(async move { Ok(HttpResponse::new(status, Vec::new())) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_builder_sets_fields() {
        let request = HttpRequest::put("https://example.com")
            .header("X-Test", "1")
            .header("bad header", "x")
            .body(vec![1, 2, 3])
            .follow_redirects(false);
        assert_eq!(request.method, Method::PUT);
        assert_eq!(request.headers.len(), 1);
        assert_eq!(request.body, vec![1, 2, 3]);
        assert!(!request.follow_redirects);
    }

    #[test]
    fn json_helpers_round_trip() {
        #[derive(Serialize, serde::Deserialize, PartialEq, Debug)]
        struct Item {
            name: String,
        }
        let item = Item {
            name: "widget".into(),
        };
        let request = HttpRequest::post("https://example.com")
            .json(&item)
            .unwrap();
        assert_eq!(
            request.header_value("content-type"),
            Some("application/json")
        );
        let response = HttpResponse::new(StatusCode::OK, request.body.clone());
        assert_eq!(response.json::<Item>().unwrap(), item);
    }

    #[test]
    fn error_for_status_reports_first_line() {
        let response = HttpResponse::new(StatusCode::BAD_GATEWAY, b"upstream down\nmore".to_vec());
        let error = response.error_for_status().unwrap_err();
        assert_eq!(error.to_string(), "HTTP 502 Bad Gateway: upstream down");
    }
}
