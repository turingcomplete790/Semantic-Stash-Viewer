//! Errors returned to the UI by commands (contracts/tauri-commands.md, invariant 2).

use serde::Serialize;
use uuid::Uuid;

use crate::connection::failure::ConnectFailure;

/// Every fallible command returns this. Connection problems are wrapped as `Connect` so the UI
/// can show the matching plain-language message; the rest are app-level problems.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AppError {
    #[error(transparent)]
    Connect { failure: ConnectFailure },

    /// The display name is empty after trimming, or longer than 64 characters.
    #[error("invalid display name: {reason}")]
    InvalidDisplayName { reason: String },

    #[error("no profile with id {id}")]
    ProfileNotFound { id: Uuid },

    /// `profiles.json` has a newer schema version than this build understands. It is left
    /// untouched rather than overwritten.
    #[error("profiles.json has unsupported version {found}")]
    UnsupportedProfilesVersion { found: u32 },

    /// Reading or writing the local config failed.
    #[error("storage error: {message}")]
    Storage { message: String },

    /// The request was cancelled by the user.
    #[error("cancelled")]
    Cancelled,

    /// Anything else that shouldn't happen (for example the HTTP client failing to build).
    #[error("internal error: {message}")]
    Internal { message: String },
}

impl From<ConnectFailure> for AppError {
    fn from(failure: ConnectFailure) -> Self {
        Self::Connect { failure }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::Storage {
            message: e.to_string(),
        }
    }
}
