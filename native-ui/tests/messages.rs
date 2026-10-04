//! 007 T014: every connection failure and app error reads plainly, and each failure kind has its
//! own title (behaviour that stays B6; 001 SC-003).

use std::collections::HashSet;

use semantic_stash_viewer_native::messages::{for_connect, for_error};
use stash_core::connection::failure::ConnectFailure;
use stash_core::AppError;
use uuid::Uuid;

fn all_failures() -> Vec<ConnectFailure> {
    vec![
        ConnectFailure::InvalidAddress {
            reason: "it has no host".into(),
        },
        ConnectFailure::Unreachable {
            tried: vec!["http://nas:9999".into()],
        },
        ConnectFailure::Timeout,
        ConnectFailure::NotStash { status: Some(404) },
        ConnectFailure::ApiKeyRequired,
        ConnectFailure::ApiKeyRejected,
        ConnectFailure::ApiKeyInvalidButNotRequired,
        ConnectFailure::UnsupportedVersion {
            found: "v0.28.1".into(),
            minimum: "v0.31.1".into(),
        },
        ConnectFailure::ServerNotReady {
            status: "NEEDS_MIGRATION".into(),
        },
        ConnectFailure::CertificateNotVerified,
        ConnectFailure::DuplicateProfile {
            existing_id: Uuid::nil(),
        },
    ]
}

#[test]
fn every_failure_kind_has_a_distinct_title() {
    let titles: Vec<String> = all_failures()
        .iter()
        .map(|f| for_connect(f).title)
        .collect();
    let unique: HashSet<&String> = titles.iter().collect();
    assert_eq!(unique.len(), titles.len(), "{titles:#?}");
    assert!(titles.iter().all(|t| !t.is_empty()));
}

#[test]
fn an_old_server_names_both_versions() {
    let m = for_connect(&ConnectFailure::UnsupportedVersion {
        found: "v0.28.1".into(),
        minimum: "v0.31.1".into(),
    });
    let all = format!("{} {} {}", m.title, m.detail, m.hint.unwrap_or_default());
    assert!(all.contains("v0.28.1") && all.contains("v0.31.1"), "{all}");
}

#[test]
fn a_certificate_failure_says_how_to_connect_anyway() {
    let m = for_connect(&ConnectFailure::CertificateNotVerified);
    assert!(m.hint.unwrap_or_default().to_lowercase().contains("strict"));
}

#[test]
fn an_unreachable_server_lists_what_was_tried() {
    let m = for_connect(&ConnectFailure::Unreachable {
        tried: vec!["http://nas:9999".into(), "https://nas:9999".into()],
    });
    assert!(m.detail.contains("http://nas:9999") && m.detail.contains("https://nas:9999"));
}

#[test]
fn app_errors_wrap_connection_failures_and_explain_the_rest() {
    assert_eq!(
        for_error(&AppError::Connect {
            failure: ConnectFailure::Timeout
        }),
        for_connect(&ConnectFailure::Timeout)
    );
    for e in [
        AppError::InvalidDisplayName {
            reason: "is too long".into(),
        },
        AppError::ProfileNotFound { id: Uuid::nil() },
        AppError::SceneNotFound { id: "7".into() },
        AppError::NoPlayableFile { id: "7".into() },
        AppError::UnsupportedProfilesVersion { found: 9 },
        AppError::Storage {
            message: "disk full".into(),
        },
        AppError::TabSetInvalid {
            reason: "empty".into(),
        },
        AppError::OpenFailed {
            detail: "no opener".into(),
        },
        AppError::NotConnected,
        AppError::Cancelled,
        AppError::Internal {
            message: "boom".into(),
        },
    ] {
        let m = for_error(&e);
        assert!(!m.title.is_empty() && !m.detail.is_empty(), "{e:?}");
    }
}
