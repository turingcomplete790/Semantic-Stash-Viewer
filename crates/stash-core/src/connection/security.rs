//! Connection security state (FR-020, research R4).

use serde::Serialize;
use url::Url;

/// How secure the current connection is, shown in the connection indicator at all times.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SecurityState {
    /// Plain http.
    Unencrypted,
    /// https with certificate checking off (the default): encrypted, but nothing was verified.
    EncryptedUnverified,
    /// https with strict checking on and a successful handshake.
    EncryptedVerified,
}

/// Derive the security state from the **final** URL (after redirects) and the profile's
/// strict setting.
///
/// With strict off, the viewer reports "not verified" even if the certificate happens to be
/// valid, because nothing was checked. With strict on, a connection only exists if the
/// certificate verified, so https means verified.
pub fn derive_security(final_url: &Url, strict_tls: bool) -> SecurityState {
    match (final_url.scheme(), strict_tls) {
        ("https", true) => SecurityState::EncryptedVerified,
        ("https", false) => SecurityState::EncryptedUnverified,
        _ => SecurityState::Unencrypted,
    }
}
