//! HTTP calls used by the vulnerability radar.
//!
//! The core crate does not open connections. A caller supplies an
//! [`HttpClient`].

/// Largest response body the radar will accept.
pub const MAX_RESPONSE_BYTES: usize = 10 * 1024 * 1024;

/// One request. `method` is `GET` or `POST`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpRequest<'a> {
    /// `GET` or `POST`.
    pub method: &'static str,
    /// Absolute `https` URL.
    pub url: &'a str,
    /// Request body for `POST`.
    pub body: Option<&'a [u8]>,
}

/// A completed response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response body, at most [`MAX_RESPONSE_BYTES`].
    pub body: Vec<u8>,
}

/// Why a request did not produce a usable response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpError {
    /// English explanation, without a trailing newline.
    pub message: String,
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for HttpError {}

/// Sends one request.
pub trait HttpClient {
    /// Performs `request`.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError`] when the connection fails, the URL is not HTTPS,
    /// a redirect leaves HTTPS, the status is not successful, or the body
    /// exceeds [`MAX_RESPONSE_BYTES`].
    fn execute(&self, request: &HttpRequest<'_>) -> Result<HttpResponse, HttpError>;
}
