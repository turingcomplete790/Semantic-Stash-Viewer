//! Connection changes → notifications (004 US3, research R4).
//!
//! One keyed entry per profile (`connection:<profile>`) that updates as the state changes:
//! "Server unreachable" becomes "Reconnected" instead of a new alert for every retry. The first
//! successful connection at launch says nothing.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::broadcast;
use uuid::Uuid;

use super::snapshot::{ConnectionSnapshot, SessionState};
use crate::shell::notifications::{
    NewNotification, NotificationCenter, NotificationKind, Severity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Seen {
    Connected,
    Offline,
    AuthFailed,
    Failed,
}

/// Tracks the last meaningful state per profile.
#[derive(Debug, Default)]
pub struct ConnectionWatch {
    last: HashMap<Uuid, Seen>,
}

fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    let mut out = chars
        .next()
        .map(|c| c.to_uppercase().collect::<String>())
        .unwrap_or_default();
    out.push_str(chars.as_str());
    if !out.ends_with('.') {
        out.push('.');
    }
    out
}

impl ConnectionWatch {
    /// The notification this snapshot calls for, if any.
    pub fn observe(&mut self, snapshot: &ConnectionSnapshot) -> Option<NewNotification> {
        let profile = snapshot.profile_id?;
        let (seen, severity, title, detail) = match &snapshot.state {
            SessionState::Idle | SessionState::Connecting { .. } => return None,
            SessionState::Connected => (Seen::Connected, Severity::Info, "Reconnected", None),
            SessionState::Offline { .. } => (
                Seen::Offline,
                Severity::Warning,
                "Server unreachable",
                Some("The viewer keeps retrying and reconnects on its own.".to_owned()),
            ),
            SessionState::AuthFailed { failure } => (
                Seen::AuthFailed,
                Severity::Error,
                "API key rejected",
                Some(format!(
                    "{} Update the API key from the server menu or Settings.",
                    sentence(&failure.to_string())
                )),
            ),
            SessionState::Failed { failure } => (
                Seen::Failed,
                Severity::Error,
                "Couldn't connect to the server",
                Some(sentence(&failure.to_string())),
            ),
        };
        let previous = self.last.insert(profile, seen);
        if previous == Some(seen) {
            return None;
        }
        // Connected only notifies when it ends a problem; launching quietly succeeds.
        if seen == Seen::Connected && matches!(previous, None | Some(Seen::Connected)) {
            return None;
        }
        Some(NewNotification {
            key: Some(format!("connection:{profile}")),
            profile_id: Some(profile),
            kind: NotificationKind::Connection,
            severity,
            title: title.to_owned(),
            detail,
            toast: true,
            job: None,
        })
    }
}

/// Post connection notifications for every snapshot the manager broadcasts.
pub fn spawn(
    mut snapshots: broadcast::Receiver<ConnectionSnapshot>,
    center: Arc<NotificationCenter>,
    runtime: &tokio::runtime::Handle,
) -> tokio::task::JoinHandle<()> {
    runtime.spawn(async move {
        let mut watch = ConnectionWatch::default();
        loop {
            match snapshots.recv().await {
                Ok(snapshot) => {
                    if let Some(n) = watch.observe(&snapshot) {
                        center.post(n);
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    })
}
