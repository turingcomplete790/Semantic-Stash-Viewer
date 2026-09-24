//! Connection logic: address normalisation, the version gate, probing, and session state.

pub mod address;
pub mod connect;
pub mod failure;
pub mod version;

use serde::Serialize;

pub use failure::ConnectFailure;
pub use version::VersionStatus;

/// Library summary shown after connecting (FR-007).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LibraryCounts {
    pub scenes: u32,
    pub images: u32,
    pub galleries: u32,
    pub performers: u32,
}

/// Facts read from the server on connect (data-model.md "ServerInfo").
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    /// For example `v0.31.1`, or `unknown` if the server didn't report one.
    pub version: String,
    pub version_status: VersionStatus,
    pub app_schema: i32,
    pub counts: LibraryCounts,
}
