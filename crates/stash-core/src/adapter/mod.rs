//! The Stash adapter: the **only** module in the workspace allowed to make network calls
//! (constitution Principle III). Everything else reaches Stash through `StashClient`.

pub mod activity;
pub mod chips;
pub mod choices;
pub mod filters;
pub mod gql;
pub mod health;
pub mod jobs;
pub mod operations;
pub mod probe;
pub mod record;
pub mod scene_page;
pub mod scenes;
pub mod semantic;

use std::error::Error as _;

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::Arc;
use std::time::{Duration, Instant};

use url::Url;

use crate::connection::failure::ConnectFailure;
use crate::error::AppError;
use crate::profiles::model::display_url;
use record::{operation_name_in, Recorder, RequestKind, RequestRecord};

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
    recorder: Option<Arc<dyn Recorder>>,
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
            recorder: None,
        })
    }

    /// The same client, reporting every request it sends to `recorder` (008 research R2).
    #[must_use]
    pub fn with_recorder(self, recorder: Arc<dyn Recorder>) -> Self {
        Self {
            recorder: Some(recorder),
            ..self
        }
    }

    /// Report one GET of a resource `kind` (`thumbnail`, `preview`, `sprite`, …; never the URL)
    /// to the recorder, if there is one.
    pub(crate) fn record_get(&self, kind: &str, response_bytes: usize, started: Instant) {
        if let Some(recorder) = &self.recorder {
            recorder.record(RequestRecord {
                operation: kind.to_owned(),
                kind: RequestKind::Get,
                request_bytes: 0,
                response_bytes,
                duration: started.elapsed(),
            });
        }
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

    pub(crate) fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
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

    /// Start a GET to `{base}/{path}` with the API key, when one is configured.
    pub(crate) fn get_with_key(&self, url: Url) -> reqwest::RequestBuilder {
        let mut req = self.http.get(url);
        if let Some(key) = &self.api_key {
            req = req.header("ApiKey", key);
        }
        req
    }

    /// Start a GET to `{base}/{path}` without authentication.
    pub(crate) fn get(&self, path: &str) -> reqwest::RequestBuilder {
        self.http.get(self.endpoint(path))
    }

    /// Send a typed GraphQL operation (Cynic, with the API key) and return its `data`.
    ///
    /// Transport failures and 401s map to connection failures; GraphQL errors and missing
    /// `data` map to `AppError::Internal` with Stash's message.
    pub(crate) async fn graphql<R, V>(
        &self,
        operation: &cynic::Operation<R, V>,
    ) -> Result<R, AppError>
    where
        R: DeserializeOwned,
        V: Serialize,
    {
        let body = serde_json::to_vec(operation).map_err(|e| AppError::Internal {
            message: format!("could not encode the request: {e}"),
        })?;
        let name = operation
            .operation_name
            .as_deref()
            .or_else(|| operation_name_in(&operation.query))
            .unwrap_or("anonymous");
        self.send_graphql(name, body).await
    }

    /// Send a GraphQL document built by hand (only where the text must follow data, such as the
    /// semantic vocabulary's aliases per colour rule) and return its `data` as JSON. Recorded
    /// under `name`, like typed operations.
    pub(crate) async fn graphql_document(
        &self,
        name: &str,
        query: String,
    ) -> Result<serde_json::Value, AppError> {
        let body = serde_json::to_vec(&serde_json::json!({ "query": query })).map_err(|e| {
            AppError::Internal {
                message: format!("could not encode the request: {e}"),
            }
        })?;
        self.send_graphql(name, body).await
    }

    async fn send_graphql<R: DeserializeOwned>(
        &self,
        name: &str,
        body: Vec<u8>,
    ) -> Result<R, AppError> {
        let request_bytes = body.len();
        let started = Instant::now();
        let result = self.graphql_bytes(body).await;
        if let Some(recorder) = &self.recorder {
            recorder.record(RequestRecord {
                operation: name.to_owned(),
                kind: RequestKind::GraphQl,
                request_bytes,
                response_bytes: result.as_ref().map_or(0, |(_, bytes)| bytes.len()),
                duration: started.elapsed(),
            });
        }
        let (status, bytes) = result?;
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(if self.has_api_key() {
                ConnectFailure::ApiKeyRejected
            } else {
                ConnectFailure::ApiKeyRequired
            }
            .into());
        }
        if !status.is_success() {
            return Err(AppError::Internal {
                message: format!("Stash answered HTTP {}", status.as_u16()),
            });
        }
        let parsed: cynic::GraphQlResponse<R> =
            serde_json::from_slice(&bytes).map_err(|e| AppError::Internal {
                message: format!("unexpected response from Stash: {e}"),
            })?;
        if let Some(error) = parsed.errors.as_ref().and_then(|errors| errors.first()) {
            return Err(AppError::Internal {
                message: format!("Stash reported an error: {}", error.message),
            });
        }
        parsed.data.ok_or_else(|| AppError::Internal {
            message: "Stash returned no data".into(),
        })
    }

    /// POST an encoded GraphQL body; the HTTP status and the whole response body.
    async fn graphql_bytes(
        &self,
        body: Vec<u8>,
    ) -> Result<(reqwest::StatusCode, Vec<u8>), AppError> {
        let response = self
            .graphql_request(true)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| AppError::from(self.classify_transport_error(&e)))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|e| AppError::from(self.classify_transport_error(&e)))?;
        Ok((status, bytes.to_vec()))
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
