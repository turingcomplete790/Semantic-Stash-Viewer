//! Minimum-version gate (research R6, FR-004): Stash v0.31.1 or newer.

use serde::Serialize;

use super::failure::ConnectFailure;

/// Minimum supported Stash release (constitution: Technology & Platform Constraints).
pub const MINIMUM_STASH_VERSION: &str = "0.31.1";
/// `systemStatus.appSchema` reported by v0.31.1. Used when the version can't be parsed.
pub const MINIMUM_APP_SCHEMA: i64 = 85;

/// How confident we are that the server is compatible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum VersionStatus {
    /// A release at or above the minimum.
    Supported,
    /// A git-describe or pre-release build whose base version meets the minimum. Shown with a
    /// compatibility warning.
    DevelopmentBuild,
    /// The version couldn't be parsed, but the database schema is new enough. Shown with a
    /// warning.
    UnknownButCompatible,
}

/// Decide whether a server is usable from its probe response.
///
/// `status` is `systemStatus.status`; anything other than `OK` means setup or a migration is
/// pending and is refused first.
pub fn check(
    version: Option<&str>,
    app_schema: i64,
    status: &str,
) -> Result<VersionStatus, ConnectFailure> {
    if status != "OK" {
        return Err(ConnectFailure::ServerNotReady {
            status: status.to_owned(),
        });
    }

    let minimum = minimum();
    let found = version.unwrap_or("unknown");
    let unsupported = || ConnectFailure::UnsupportedVersion {
        found: found.to_owned(),
        minimum: format!("v{MINIMUM_STASH_VERSION}"),
    };

    match version.and_then(|v| semver::Version::parse(v.trim().trim_start_matches('v')).ok()) {
        Some(v) => {
            // Compare only the release core: `0.31.1-12-gabc1234` is a build *after* the
            // v0.31.1 tag, which plain semver would rank below 0.31.1.
            let core = semver::Version::new(v.major, v.minor, v.patch);
            if core < minimum {
                Err(unsupported())
            } else if v.pre.is_empty() && v.build.is_empty() {
                Ok(VersionStatus::Supported)
            } else {
                Ok(VersionStatus::DevelopmentBuild)
            }
        }
        None if app_schema >= MINIMUM_APP_SCHEMA => Ok(VersionStatus::UnknownButCompatible),
        None => Err(unsupported()),
    }
}

fn minimum() -> semver::Version {
    semver::Version::parse(MINIMUM_STASH_VERSION).unwrap_or_else(|_| semver::Version::new(0, 31, 1))
}
