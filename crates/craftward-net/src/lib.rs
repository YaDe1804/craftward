//! HTTPS client used by the Craftward command line.
//!
//! This is the only crate that links an HTTP library. TLS uses rustls with the
//! `ring` provider. Root certificates come from the operating system store.

use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

use craftward_core::{
    HttpClient, HttpError, HttpRequest, HttpResponse, MAX_RESPONSE_BYTES, Method,
};
use ureq::Agent;
use ureq::tls::{Certificate, RootCerts, TlsConfig, TlsProvider};

/// ureq agent that only talks HTTPS.
pub struct UreqClient {
    agent: Agent,
}

impl UreqClient {
    /// Builds a client with a 10 second connect timeout and a 30 second
    /// response timeout.
    ///
    /// `user_agent` is sent on every request. Root certificates are loaded
    /// from the operating system store.
    ///
    /// # Errors
    ///
    /// Returns a message when the certificate store cannot be read or contains
    /// no certificates.
    pub fn new(user_agent: &str) -> Result<Self, String> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let loaded = rustls_native_certs::load_native_certs();
        if loaded.certs.is_empty() {
            return Err(
                "the operating system certificate store has no root certificates".to_owned(),
            );
        }
        let certs: Vec<Certificate<'static>> = loaded
            .certs
            .iter()
            .map(|cert| Certificate::from_der(cert.as_ref()).to_owned())
            .collect();
        let tls = TlsConfig::builder()
            .provider(TlsProvider::Rustls)
            .root_certs(RootCerts::new_with_certs(&certs))
            .unversioned_rustls_crypto_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .build();
        let agent = Agent::config_builder()
            .tls_config(tls)
            .https_only(true)
            .user_agent(user_agent)
            .timeout_connect(Some(Duration::from_secs(10)))
            .timeout_recv_response(Some(Duration::from_secs(30)))
            .timeout_recv_body(Some(Duration::from_secs(30)))
            .timeout_send_request(Some(Duration::from_secs(30)))
            .timeout_send_body(Some(Duration::from_secs(30)))
            .build()
            .new_agent();
        Ok(Self { agent })
    }
}

impl HttpClient for UreqClient {
    fn execute(&self, request: &HttpRequest<'_>) -> Result<HttpResponse, HttpError> {
        if !request.url.starts_with("https://") {
            return Err(HttpError {
                message: format!("refusing non-HTTPS URL {}", request.url),
            });
        }
        let response = match request.method {
            Method::Post => self
                .agent
                .post(request.url)
                .header("content-type", "application/json")
                .send(request.body.unwrap_or_default()),
            Method::Get => self.agent.get(request.url).call(),
        };
        let response = response.map_err(|error| HttpError {
            message: error.to_string(),
        })?;
        let status = response.status().as_u16();
        let mut reader = response.into_body().into_reader();
        let mut body = Vec::new();
        let mut chunk = [0_u8; 8192];
        loop {
            let read = reader.read(&mut chunk).map_err(|error| HttpError {
                message: error.to_string(),
            })?;
            if read == 0 {
                break;
            }
            if body.len().saturating_add(read) > MAX_RESPONSE_BYTES {
                return Err(HttpError {
                    message: format!("response exceeded {MAX_RESPONSE_BYTES} bytes"),
                });
            }
            body.extend_from_slice(&chunk[..read]);
        }
        Ok(HttpResponse { status, body })
    }
}

#[cfg(test)]
mod tests {
    use super::UreqClient;
    use craftward_core::{HttpClient, HttpRequest, Method};

    #[test]
    #[ignore = "contacts osv.dev"]
    fn live_query_reaches_osv() {
        let client = UreqClient::new("craftward/0.1.0").expect("client");
        let body = br#"{"package":{"ecosystem":"crates.io","name":"libc"},"version":"0.2.0"}"#;
        let response = client
            .execute(&HttpRequest {
                method: Method::Post,
                url: "https://api.osv.dev/v1/query",
                body: Some(body),
            })
            .expect("query");
        let text = String::from_utf8(response.body).expect("utf-8");
        assert!(text.contains("vulns") || text.contains("{}"), "{text}");
    }
}
