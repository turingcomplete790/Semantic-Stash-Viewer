//! The notification centre (constitution Principle IX; 004 US3, research R4).
//!
//! One list for connection alerts, background failures, and Stash jobs. A notification can carry
//! a key for an ongoing condition (`connection:<profile>`, `job:<profile>:<id>`): posting with
//! the same key updates it in place instead of adding another. The list is discardable local
//! state (Principle I), capped at 200, and broadcast in full on every change.

use std::sync::Mutex;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use uuid::Uuid;

use super::store::{JsonStore, Versioned};
use super::MAX_NOTIFICATIONS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum NotificationKind {
    Connection,
    Playback,
    Job,
    Background,
}

/// Ordered: `Info < Warning < Error`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum JobStatus {
    Queued,
    Running,
    Stopping,
    Finished,
    Failed,
    Cancelled,
    /// Can't be watched right now (disconnected, or just launched).
    Unknown,
}

impl JobStatus {
    /// Still going: shows the bell's activity marker and is never dropped by the cap.
    pub fn is_active(self) -> bool {
        matches!(self, Self::Queued | Self::Running | Self::Stopping)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct JobProgress {
    pub status: JobStatus,
    /// 0–1, when Stash reports it.
    pub progress: Option<f64>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: String,
    pub key: Option<String>,
    pub profile_id: Option<Uuid>,
    pub kind: NotificationKind,
    pub severity: Severity,
    /// Plain language, e.g. "Server unreachable".
    pub title: String,
    /// Plain language, with any next step.
    pub detail: Option<String>,
    /// ISO 8601.
    pub created_at: String,
    pub updated_at: String,
    pub read: bool,
    /// Show a toast for this post or update (live changes only; cleared on reload).
    pub toast: bool,
    pub job: Option<JobProgress>,
}

impl Notification {
    fn is_active_job(&self) -> bool {
        self.job.as_ref().is_some_and(|j| j.status.is_active())
    }
}

/// What a producer posts; the centre fills in id, times, and read state.
#[derive(Debug, Clone, PartialEq)]
pub struct NewNotification {
    pub key: Option<String>,
    pub profile_id: Option<Uuid>,
    pub kind: NotificationKind,
    pub severity: Severity,
    pub title: String,
    pub detail: Option<String>,
    pub toast: bool,
    pub job: Option<JobProgress>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationsFile {
    pub version: u32,
    /// Newest first.
    #[serde(default)]
    pub notifications: Vec<Notification>,
}

impl Default for NotificationsFile {
    fn default() -> Self {
        Self {
            version: Self::VERSION,
            notifications: Vec::new(),
        }
    }
}

impl Versioned for NotificationsFile {
    const VERSION: u32 = 1;
}

pub struct NotificationCenter {
    store: JsonStore<NotificationsFile>,
    list: Mutex<Vec<Notification>>,
    tx: watch::Sender<Vec<Notification>>,
}

impl NotificationCenter {
    /// Load `path` (discardable). Jobs that were active become `Unknown` until the next
    /// connection reconciles them, and no reloaded notification toasts again.
    pub fn open(path: impl Into<std::path::PathBuf>) -> Self {
        let store = JsonStore::<NotificationsFile>::new(path);
        let mut list = store.load().notifications;
        for n in &mut list {
            n.toast = false;
            if let Some(job) = n.job.as_mut().filter(|j| j.status.is_active()) {
                job.status = JobStatus::Unknown;
            }
        }
        let (tx, _) = watch::channel(list.clone());
        Self {
            store,
            list: Mutex::new(list),
            tx,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Notification>> {
        self.list
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Write and broadcast. `persist: false` skips the file for high-frequency updates (job
    /// progress); the next status change writes everything.
    fn changed(&self, list: &[Notification], persist: bool) {
        if persist {
            let file = NotificationsFile {
                notifications: list.to_vec(),
                ..NotificationsFile::default()
            };
            if let Err(e) = self.store.save(&file) {
                tracing::warn!(error = %e, "could not save notifications");
            }
        }
        self.tx.send_replace(list.to_vec());
    }

    /// Add a notification, or update the one with the same key. Returns the stored notification.
    pub fn post(&self, new: NewNotification) -> Notification {
        let now = Utc::now().to_rfc3339();
        let mut list = self.lock();
        let existing = new
            .key
            .as_ref()
            .and_then(|key| list.iter().position(|n| n.key.as_ref() == Some(key)));
        let (stored, persist) = match existing {
            Some(i) => {
                let old = list.remove(i);
                let progress_only = old.job.as_ref().map(|j| j.status)
                    == new.job.as_ref().map(|j| j.status)
                    && old.title == new.title
                    && old.severity == new.severity;
                let updated = Notification {
                    id: old.id,
                    key: new.key,
                    profile_id: new.profile_id,
                    kind: new.kind,
                    read: old.read && new.severity <= old.severity,
                    severity: new.severity,
                    title: new.title,
                    detail: new.detail,
                    created_at: old.created_at,
                    updated_at: now,
                    toast: new.toast,
                    job: new.job,
                };
                (updated, !progress_only)
            }
            None => (
                Notification {
                    id: Uuid::new_v4().to_string(),
                    key: new.key,
                    profile_id: new.profile_id,
                    kind: new.kind,
                    severity: new.severity,
                    title: new.title,
                    detail: new.detail,
                    created_at: now.clone(),
                    updated_at: now,
                    read: false,
                    toast: new.toast,
                    job: new.job,
                },
                true,
            ),
        };
        list.insert(0, stored.clone());
        // Cap: drop the oldest entries that aren't active jobs.
        while list.len() > MAX_NOTIFICATIONS {
            match list.iter().rposition(|n| !n.is_active_job()) {
                Some(i) => {
                    list.remove(i);
                }
                None => break,
            }
        }
        self.changed(&list, persist);
        stored
    }

    pub fn mark_read(&self, ids: &[String]) {
        let mut list = self.lock();
        let mut any = false;
        for n in list.iter_mut().filter(|n| !n.read && ids.contains(&n.id)) {
            n.read = true;
            n.toast = false;
            any = true;
        }
        if any {
            self.changed(&list, true);
        }
    }

    pub fn dismiss(&self, id: &str) {
        let mut list = self.lock();
        let before = list.len();
        list.retain(|n| n.id != id);
        if list.len() != before {
            self.changed(&list, true);
        }
    }

    /// Remove everything except jobs that are still going.
    pub fn dismiss_all(&self) {
        let mut list = self.lock();
        list.retain(Notification::is_active_job);
        self.changed(&list, true);
    }

    /// Notifications, newest first.
    pub fn list(&self) -> Vec<Notification> {
        self.lock().clone()
    }

    /// The notification with `key`, if any.
    pub fn find(&self, key: &str) -> Option<Notification> {
        self.lock()
            .iter()
            .find(|n| n.key.as_deref() == Some(key))
            .cloned()
    }

    /// The full list on every change.
    pub fn subscribe(&self) -> watch::Receiver<Vec<Notification>> {
        self.tx.subscribe()
    }
}
