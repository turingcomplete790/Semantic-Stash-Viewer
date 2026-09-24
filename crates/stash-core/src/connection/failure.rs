//! Why a connection attempt failed. Each variant maps 1:1 to a plain-language message in the UI
//! (spec FR-005, data-model.md "ConnectFailure").

use serde::Serialize;
use uuid::Uuid;

/// A classified connection failure. No variant carries a raw response body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ConnectFailure {
    /// The address can't be used (no host, bad scheme). Detected before any network call.
    #[error("invalid address: {reason}")]
    InvalidAddress { reason: String },

    /// Nothing answered at any of the tried URLs.
    #[error("nothing answered at {tried:?}")]
    Unreachable { tried: Vec<String> },

    /// No response within the overall connection budget (15 s).
    #[error("the server did not respond in time")]
    Timeout,

    /// Something answered, but it isn't Stash.
    #[error("the server is not Stash (HTTP status {status:?})")]
    NotStash { status: Option<u16> },

    /// Stash requires an API key and none was sent.
    #[error("the server requires an API key")]
    ApiKeyRequired,

    /// Stash rejected the API key that was sent.
    #[error("the API key was rejected")]
    ApiKeyRejected,

    /// Stash has no authentication configured, but rejected the (wrong) key that was sent.
    #[error("the server doesn't need an API key, but the one entered is wrong")]
    ApiKeyInvalidButNotRequired,

    /// Stash is older than the minimum supported version.
    #[error("Stash {found} is older than the minimum supported {minimum}")]
    UnsupportedVersion { found: String, minimum: String },

    /// Stash reports it needs setup or a migration (`systemStatus.status` is not `OK`).
    #[error("Stash is not ready ({status})")]
    ServerNotReady { status: String },

    /// Strict certificate checking is on and the server's certificate couldn't be verified.
    #[error("the server's certificate couldn't be verified")]
    CertificateNotVerified,

    /// A profile for this server already exists.
    #[error("a profile for this server already exists")]
    DuplicateProfile { existing_id: Uuid },
}

impl ConnectFailure {
    /// True for failures that prove the server is Stash (it answered with a Stash-shaped
    /// response), so trying further candidate URLs is pointless.
    pub fn proves_stash(&self) -> bool {
        matches!(
            self,
            Self::ApiKeyRequired
                | Self::ApiKeyRejected
                | Self::ApiKeyInvalidButNotRequired
                | Self::UnsupportedVersion { .. }
                | Self::ServerNotReady { .. }
        )
    }

    /// True for authentication failures (the session goes to `AuthFailed`, not `Failed`).
    pub fn is_auth(&self) -> bool {
        matches!(
            self,
            Self::ApiKeyRequired | Self::ApiKeyRejected | Self::ApiKeyInvalidButNotRequired
        )
    }
}
