//! HTTP calls used by the vulnerability radar.
//!
//! The core crate does not open connections. A caller supplies an
//! [`HttpClient`].

/// Largest response body the radar will accept.
pub const MAX_RESPONSE_BYTES: usize = 10 * 1024 * 1024;

/// `GET` or `POST`. The stored words stay uppercase because that is the
/// method text already written by callers and by the HTTP client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Method {
    /// An HTTP GET.
    #[serde(rename = "GET")]
    Get,
    /// An HTTP POST.
    #[serde(rename = "POST")]
    Post,
}

impl Method {
    /// `GET` or `POST`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
        }
    }

    /// Both methods.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[Self::Get, Self::Post]
    }
}

impl std::str::FromStr for Method {
    type Err = UnknownMethod;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "GET" => Ok(Self::Get),
            "POST" => Ok(Self::Post),
            _ => Err(UnknownMethod {
                value: text.to_owned(),
            }),
        }
    }
}

impl std::fmt::Display for Method {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Text that is neither `GET` nor `POST`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownMethod {
    value: String,
}

impl std::fmt::Display for UnknownMethod {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "unknown method: {}", self.value)
    }
}

impl std::error::Error for UnknownMethod {}

/// One request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpRequest<'a> {
    /// `GET` or `POST`.
    pub method: Method,
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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::Method;

    #[test]
    fn methods_round_trip_as_get_and_post() {
        for method in Method::all() {
            let word = serde_json::to_value(method).unwrap();
            assert_eq!(word.as_str(), Some(method.as_str()));
            assert_eq!(Method::from_str(method.as_str()).unwrap(), *method);
            assert_eq!(serde_json::from_value::<Method>(word).unwrap(), *method);
        }
        assert!(Method::from_str("get").is_err());
        assert!(serde_json::from_str::<Method>("\"PUT\"").is_err());
    }
}
