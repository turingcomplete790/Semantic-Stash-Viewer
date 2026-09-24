//! Saved server profiles (data-model.md "ServerProfile", "ProfileDraft").
//!
//! The API key is plain profile configuration (constitution v3.0.0, Principle VII): it is saved
//! with the profile and sent to the UI as-is.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use crate::error::AppError;

/// Maximum display-name length, in characters, after trimming.
pub const DISPLAY_NAME_MAX_CHARS: usize = 64;

/// One saved Stash server, as persisted in `profiles.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerProfile {
    /// Generated on create; immutable.
    pub id: Uuid,
    /// 1–64 chars after trim; defaults to the host.
    pub display_name: String,
    /// Normalised final base URL after redirects: no trailing slash, no `/graphql` suffix.
    pub base_url: Url,
    /// Default `false` (FR-018).
    #[serde(default)]
    pub strict_tls: bool,
    /// Stored as entered, after trimming whitespace; `None` means no key.
    #[serde(default)]
    pub api_key: Option<String>,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub last_used_at: Option<DateTime<Utc>>,
}

/// What the connection form submits for `test_connection`, `create_profile`, `update_profile`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ProfileDraft {
    #[serde(default)]
    pub display_name: Option<String>,
    /// Raw user input; normalised by `connection::address`.
    pub address: String,
    /// Whitespace-trimmed; an empty string is treated as no key.
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub strict_tls: bool,
}

impl ProfileDraft {
    /// The key to send, or `None` when absent or blank.
    pub fn normalized_api_key(&self) -> Option<String> {
        normalize_api_key(self.api_key.as_deref())
    }
}

/// The profile as sent to the UI (contracts/tauri-commands.md `ProfileSummary`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    pub id: Uuid,
    pub display_name: String,
    pub base_url: String,
    pub strict_tls: bool,
    pub api_key: Option<String>,
    /// ISO 8601.
    pub last_used_at: Option<String>,
}

impl From<&ServerProfile> for ProfileSummary {
    fn from(p: &ServerProfile) -> Self {
        Self {
            id: p.id,
            display_name: p.display_name.clone(),
            base_url: display_url(&p.base_url),
            strict_tls: p.strict_tls,
            api_key: p.api_key.clone(),
            last_used_at: p.last_used_at.map(|t| t.to_rfc3339()),
        }
    }
}

/// Trim a key; blank becomes `None`.
pub fn normalize_api_key(key: Option<&str>) -> Option<String> {
    key.map(str::trim)
        .filter(|k| !k.is_empty())
        .map(str::to_owned)
}

/// Validate a display name, defaulting to the URL's host (and port, if any).
///
/// Rules: 1–64 chars after trim; defaults to the host (e.g. `192.168.1.10:9999`).
pub fn resolve_display_name(input: Option<&str>, base_url: &Url) -> Result<String, AppError> {
    match input.map(str::trim) {
        None | Some("") => Ok(default_display_name(base_url)),
        Some(name) if name.chars().count() > DISPLAY_NAME_MAX_CHARS => {
            Err(AppError::InvalidDisplayName {
                reason: format!("must be at most {DISPLAY_NAME_MAX_CHARS} characters"),
            })
        }
        Some(name) => Ok(name.to_owned()),
    }
}

/// `host` or `host:port`, used when the user gives no display name.
pub fn default_display_name(base_url: &Url) -> String {
    let host = base_url.host_str().unwrap_or("Stash");
    match base_url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    }
}

/// A base URL as shown to users: no trailing slash.
pub fn display_url(url: &Url) -> String {
    url.as_str().trim_end_matches('/').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).expect("test url")
    }

    #[test]
    fn display_name_defaults_to_host_and_port() {
        let u = url("http://192.168.1.10:9999");
        assert_eq!(
            resolve_display_name(None, &u).ok(),
            Some("192.168.1.10:9999".into())
        );
        assert_eq!(
            resolve_display_name(Some("   "), &u).ok(),
            Some("192.168.1.10:9999".into())
        );
        assert_eq!(
            resolve_display_name(None, &url("https://stash.example.com")).ok(),
            Some("stash.example.com".into())
        );
    }

    #[test]
    fn display_name_is_trimmed_and_limited_to_64_chars() {
        let u = url("http://localhost:9999");
        assert_eq!(
            resolve_display_name(Some("  Home  "), &u).ok(),
            Some("Home".into())
        );
        assert!(resolve_display_name(Some(&"a".repeat(64)), &u).is_ok());
        assert!(matches!(
            resolve_display_name(Some(&"a".repeat(65)), &u),
            Err(AppError::InvalidDisplayName { .. })
        ));
    }

    #[test]
    fn blank_api_key_is_none() {
        assert_eq!(normalize_api_key(Some("  ")), None);
        assert_eq!(normalize_api_key(None), None);
        assert_eq!(normalize_api_key(Some(" abc ")), Some("abc".into()));
    }
}
