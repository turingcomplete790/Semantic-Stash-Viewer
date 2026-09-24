//! The Stash adapter: the **only** module in the workspace allowed to make network calls
//! (constitution Principle III). Everything else reaches Stash through `StashClient`.

pub mod probe;

use std::error::Error as _;
use std::time::Duration;

use url::Url;

use crate::connection::failure::ConnectFailure;
use crate::error::AppError;
use crate::profiles::model::display_url;

/// Overall budget for a single connection attempt (FR-006).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

/// Maximum redirects followed before giving up (research R4).
const MAX_REDIRECTS: usize = 5;

/// A client bound to one Stash base URL, TLS mode, and optional API key.
#[derive(Debug, Clone)]
pub struct StashClient {
    http: reqwest::Client,
    base_url: Url,
    strict_tls: bool,
    api_key: Option<String>,
}

impl StashClient {
    pub fn new(base_url: Url, strict_tls: bool, api_key: Option<String>) -> Result<Self, AppError> {
        Self::with_timeout(base_url, strict_tls, api_key, DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(
        base_url: Url,
        strict_tls: bool,
        api_key: Option<String>,
        timeout: Duration,
    ) -> Result<Self, AppError> {
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
            .timeout(timeout)
            // TLS verification is off unless the profile opts into strict mode (FR-018/019).
            .tls_danger_accept_invalid_certs(!strict_tls)
            .tls_danger_accept_invalid_hostnames(!strict_tls)
            .user_agent(concat!("semantic-stash-viewer/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| AppError::Internal {
                message: format!("could not build HTTP client: {e}"),
            })?;
        Ok(Self {
            http,
            base_url,
            strict_tls,
            api_key,
        })
    }

    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// The same client (connection pool, TLS mode, key) pointed at a different base URL.
    pub fn rebased(&self, base_url: Url) -> Self {
        Self {
            base_url,
            ..self.clone()
        }
    }

    pub fn strict_tls(&self) -> bool {
        self.strict_tls
    }

    pub fn has_api_key(&self) -> bool {
        self.api_key.is_some()
    }

    /// `{base}/{path}`, keeping any reverse-proxy sub-path in the base URL.
    pub fn endpoint(&self, path: &str) -> Url {
        endpoint(&self.base_url, path)
    }

    /// Start a POST to `{base}/graphql`, sending the `ApiKey` header only when `with_key` is
    /// true and a key is configured.
    pub(crate) fn graphql_request(&self, with_key: bool) -> reqwest::RequestBuilder {
        let mut req = self.http.post(self.endpoint("graphql"));
        if with_key {
            if let Some(key) = &self.api_key {
                req = req.header("ApiKey", key);
            }
        }
        req
    }

    /// Start a GET to `{base}/{path}` without authentication.
    pub(crate) fn get(&self, path: &str) -> reqwest::RequestBuilder {
        self.http.get(self.endpoint(path))
    }

    /// Classify a transport-level error (no HTTP response was received).
    pub(crate) fn classify_transport_error(&self, e: &reqwest::Error) -> ConnectFailure {
        if self.strict_tls && is_certificate_error(e) {
            return ConnectFailure::CertificateNotVerified;
        }
        if e.is_timeout() {
            return ConnectFailure::Timeout;
        }
        ConnectFailure::Unreachable {
            tried: vec![display_url(&self.base_url)],
        }
    }
}

/// `{base}/{path}` without dropping the base URL's last path segment (which `Url::join` would).
pub fn endpoint(base: &Url, path: &str) -> Url {
    let mut url = base.clone();
    {
        // http/https URLs always support path segments.
        if let Ok(mut segments) = url.path_segments_mut() {
            segments.pop_if_empty().push(path);
        }
    }
    url
}

/// Whether the error chain contains a TLS certificate/hostname verification failure.
///
/// rustls isn't a direct dependency, so this matches on the error messages in the source chain
/// (for example "invalid peer certificate: UnknownIssuer").
fn is_certificate_error(e: &reqwest::Error) -> bool {
    let mut source = e.source();
    while let Some(err) = source {
        let text = err.to_string().to_ascii_lowercase();
        if text.contains("certificate")
            || text.contains("unknownissuer")
            || text.contains("notvalidforname")
        {
            return true;
        }
        source = err.source();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_keeps_sub_path() {
        let base = Url::parse("https://host/stash").expect("url");
        assert_eq!(
            endpoint(&base, "graphql").as_str(),
            "https://host/stash/graphql"
        );
        let root = Url::parse("http://localhost:9999").expect("url");
        assert_eq!(
            endpoint(&root, "healthz").as_str(),
            "http://localhost:9999/healthz"
        );
    }
}
