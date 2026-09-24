//! `test_connection`: try each candidate URL, apply the version gate, respect the overall
//! budget and cancellation (FR-003, FR-006).

use std::time::Duration;

use serde::Serialize;
use tokio_util::sync::CancellationToken;
use url::Url;

use super::address::candidates;
use super::failure::ConnectFailure;
use super::{version, ServerInfo};
use crate::adapter::probe::{probe, ProbeData};
use crate::adapter::{StashClient, DEFAULT_TIMEOUT};
use crate::error::AppError;
use crate::profiles::model::{display_url, ProfileDraft};

#[derive(Debug, Clone, Copy)]
pub struct ConnectOptions {
    /// Overall budget for the whole attempt, across all candidates.
    pub timeout: Duration,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
        }
    }
}

/// A successful connection check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectOutcome {
    /// Final base URL after redirects; this is what gets saved on the profile.
    pub base_url: Url,
    pub server: ServerInfo,
}

/// What `test_connection` returns to the UI (contracts/tauri-commands.md `TestResult`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    /// Final base URL after redirects, as shown to the user.
    pub normalized_url: String,
    pub server: ServerInfo,
}

impl From<&ConnectOutcome> for TestResult {
    fn from(o: &ConnectOutcome) -> Self {
        Self {
            normalized_url: display_url(&o.base_url),
            server: o.server.clone(),
        }
    }
}

/// Check a draft without saving anything.
///
/// Candidates are tried in order (https before http when no scheme was given). The first answer
/// that proves the server is Stash ends the search, whether it's a success or an auth/version
/// failure. With strict TLS on, a certificate failure also ends it, so a strict https attempt is
/// never silently downgraded to http.
pub async fn test_connection(
    draft: &ProfileDraft,
    cancel: &CancellationToken,
    options: ConnectOptions,
) -> Result<ConnectOutcome, AppError> {
    let urls = candidates(&draft.address)?;
    let api_key = draft.normalized_api_key();
    let attempt = try_candidates(urls, draft.strict_tls, api_key, options.timeout);

    tokio::select! {
        biased;
        () = cancel.cancelled() => Err(AppError::Cancelled),
        result = tokio::time::timeout(options.timeout, attempt) => match result {
            Ok(outcome) => outcome,
            Err(_elapsed) => Err(ConnectFailure::Timeout.into()),
        },
    }
}

async fn try_candidates(
    urls: Vec<Url>,
    strict_tls: bool,
    api_key: Option<String>,
    timeout: Duration,
) -> Result<ConnectOutcome, AppError> {
    let mut tried = Vec::new();
    let mut not_stash = None;
    let mut timed_out = false;

    for url in urls {
        let client = StashClient::with_timeout(url.clone(), strict_tls, api_key.clone(), timeout)?;
        match probe(&client).await {
            Ok(data) => return accept(data),
            Err(f) if f.proves_stash() || f == ConnectFailure::CertificateNotVerified => {
                return Err(f.into())
            }
            Err(ConnectFailure::Unreachable { tried: t }) => tried.extend(t),
            Err(ConnectFailure::Timeout) => {
                timed_out = true;
                tried.push(display_url(&url));
            }
            Err(f @ ConnectFailure::NotStash { .. }) => {
                not_stash.get_or_insert(f);
                tried.push(display_url(&url));
            }
            Err(other) => return Err(other.into()),
        }
    }

    Err(match (not_stash, timed_out) {
        (Some(f), _) => f,
        (None, true) => ConnectFailure::Timeout,
        (None, false) => ConnectFailure::Unreachable { tried },
    }
    .into())
}

/// Apply the version gate and build the outcome.
fn accept(data: ProbeData) -> Result<ConnectOutcome, AppError> {
    let server = server_info(&data)?;
    Ok(ConnectOutcome {
        base_url: data.final_base_url,
        server,
    })
}

/// Apply the version gate to probe data (shared with the connection manager's re-probe).
pub(crate) fn server_info(data: &ProbeData) -> Result<ServerInfo, ConnectFailure> {
    let version_status = version::check(data.version.as_deref(), data.app_schema, &data.status)?;
    Ok(ServerInfo {
        version: data.version.clone().unwrap_or_else(|| "unknown".into()),
        version_status,
        app_schema: i32::try_from(data.app_schema).unwrap_or(i32::MAX),
        counts: data.counts,
    })
}
