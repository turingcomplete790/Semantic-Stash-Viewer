//! `ConnectionSnapshot`: the connection state sent to the UI on every transition
//! (contracts/tauri-commands.md).

use serde::Serialize;
use uuid::Uuid;

use super::failure::ConnectFailure;
use super::security::SecurityState;
use super::ServerInfo;

/// Session state (data-model.md "ConnectionState and transitions").
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SessionState {
    #[default]
    Idle,
    Connecting {
        attempt_url: String,
    },
    Connected,
    /// Retrying with backoff. `next_retry_at` is ISO 8601 wall-clock time.
    Offline {
        attempt: u32,
        next_retry_at: String,
    },
    /// The key was missing, rejected, or wrong. Retries stop until the key is updated (FR-017).
    AuthFailed {
        failure: ConnectFailure,
    },
    Failed {
        failure: ConnectFailure,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSnapshot {
    pub profile_id: Option<Uuid>,
    pub state: SessionState,
    /// Known after the first successful handshake (FR-020).
    pub security: Option<SecurityState>,
    /// Base URL after redirects, once known.
    pub final_url: Option<String>,
    /// From the last successful probe.
    pub server: Option<ServerInfo>,
    /// ISO 8601; last successful health check or probe.
    pub last_contact_at: Option<String>,
}
