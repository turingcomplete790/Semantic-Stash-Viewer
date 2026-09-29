//! Connection changes → notifications (004 US3, research R4).

use stash_core::connection::snapshot::{ConnectionSnapshot, SessionState};
use stash_core::connection::watch::ConnectionWatch;
use stash_core::connection::ConnectFailure;
use stash_core::shell::notifications::{NotificationKind, Severity};
use uuid::Uuid;

fn snap(profile: Uuid, state: SessionState) -> ConnectionSnapshot {
    ConnectionSnapshot {
        profile_id: Some(profile),
        state,
        ..ConnectionSnapshot::default()
    }
}

fn offline(attempt: u32) -> SessionState {
    SessionState::Offline {
        attempt,
        next_retry_at: "2026-09-28T00:00:00Z".into(),
    }
}

#[test]
fn first_connection_at_launch_says_nothing() {
    let p = Uuid::new_v4();
    let mut w = ConnectionWatch::default();
    assert!(w
        .observe(&snap(
            p,
            SessionState::Connecting {
                attempt_url: "http://x".into()
            }
        ))
        .is_none());
    assert!(w.observe(&snap(p, SessionState::Connected)).is_none());
}

#[test]
fn losing_and_regaining_the_server_is_one_keyed_entry() {
    let p = Uuid::new_v4();
    let mut w = ConnectionWatch::default();
    w.observe(&snap(p, SessionState::Connected));

    let lost = w.observe(&snap(p, offline(1))).expect("unreachable");
    assert_eq!(
        lost.key.as_deref(),
        Some(format!("connection:{p}").as_str())
    );
    assert_eq!(lost.kind, NotificationKind::Connection);
    assert_eq!(lost.severity, Severity::Warning);
    assert_eq!(lost.title, "Server unreachable");
    assert!(lost.toast);
    assert_eq!(lost.profile_id, Some(p));

    // More retries: nothing new.
    assert!(w.observe(&snap(p, offline(2))).is_none());
    assert!(w
        .observe(&snap(
            p,
            SessionState::Connecting {
                attempt_url: "http://x".into()
            }
        ))
        .is_none());
    assert!(w.observe(&snap(p, offline(3))).is_none());

    let back = w
        .observe(&snap(p, SessionState::Connected))
        .expect("reconnected");
    assert_eq!(back.key, lost.key);
    assert_eq!(back.severity, Severity::Info);
    assert_eq!(back.title, "Reconnected");
    assert!(back.toast);
}

#[test]
fn a_rejected_key_is_an_error_with_a_hint() {
    let p = Uuid::new_v4();
    let mut w = ConnectionWatch::default();
    let n = w
        .observe(&snap(
            p,
            SessionState::AuthFailed {
                failure: ConnectFailure::ApiKeyRejected,
            },
        ))
        .expect("auth");
    assert_eq!(n.severity, Severity::Error);
    assert_eq!(n.title, "API key rejected");
    assert!(n
        .detail
        .as_deref()
        .is_some_and(|d| d.contains("Update the API key")));
    assert!(n.toast);
}

#[test]
fn a_failed_connection_uses_the_failure_message() {
    let p = Uuid::new_v4();
    let mut w = ConnectionWatch::default();
    let n = w
        .observe(&snap(
            p,
            SessionState::Failed {
                failure: ConnectFailure::UnsupportedVersion {
                    found: "v0.30.0".into(),
                    minimum: "v0.31.1".into(),
                },
            },
        ))
        .expect("failed");
    assert_eq!(n.severity, Severity::Error);
    assert_eq!(n.title, "Couldn't connect to the server");
    assert_eq!(
        n.detail.as_deref(),
        Some("Stash v0.30.0 is older than the minimum supported v0.31.1.")
    );
}

#[test]
fn idle_and_connecting_say_nothing() {
    let p = Uuid::new_v4();
    let mut w = ConnectionWatch::default();
    assert!(w.observe(&ConnectionSnapshot::default()).is_none());
    assert!(w
        .observe(&snap(
            p,
            SessionState::Connecting {
                attempt_url: "http://x".into()
            }
        ))
        .is_none());
}
