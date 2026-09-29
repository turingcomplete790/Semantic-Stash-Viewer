//! Stash jobs in the notification centre (004 US3, research R5): read-only.
//!
//! `jobQueue` seeds current jobs on connect; the `jobsSubscribe` subscription (`ws`) then pushes
//! ADD / UPDATE / REMOVE. REMOVE carries the final status (observed on Stash v0.31).

pub mod watcher;
pub mod ws;

use std::collections::HashSet;
use std::sync::Arc;

use serde_json::Value;
use uuid::Uuid;

use crate::shell::notifications::{
    JobProgress, JobStatus, NewNotification, NotificationCenter, NotificationKind, Severity,
};

/// One Stash job, as the viewer needs it.
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub id: String,
    pub status: JobStatus,
    pub description: String,
    pub progress: Option<f64>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub error: Option<String>,
}

impl Job {
    /// From a GraphQL `Job` object (`id status description progress startTime endTime error`).
    pub fn from_json(v: &Value) -> Option<Self> {
        let text = |key: &str| v.get(key).and_then(Value::as_str).map(str::to_owned);
        Some(Self {
            id: match v.get("id")? {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            },
            status: status_from_stash(v.get("status")?.as_str()?),
            description: text("description").unwrap_or_default(),
            progress: v.get("progress").and_then(Value::as_f64),
            started_at: text("startTime"),
            ended_at: text("endTime"),
            error: text("error").filter(|e| !e.is_empty()),
        })
    }
}

/// Stash's `JobStatus` → the viewer's. Unknown values (a newer Stash) become `Unknown`.
pub fn status_from_stash(status: &str) -> JobStatus {
    match status {
        "READY" => JobStatus::Queued,
        "RUNNING" => JobStatus::Running,
        "STOPPING" => JobStatus::Stopping,
        "FINISHED" => JobStatus::Finished,
        "FAILED" => JobStatus::Failed,
        "CANCELLED" => JobStatus::Cancelled,
        _ => JobStatus::Unknown,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateKind {
    Add,
    Update,
    Remove,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JobUpdate {
    pub kind: UpdateKind,
    pub job: Job,
}

/// The job update in a `graphql-transport-ws` `next` message, if it is one.
pub fn parse_next(message: &Value) -> Option<JobUpdate> {
    if message.get("type")?.as_str()? != "next" {
        return None;
    }
    let update = message.pointer("/payload/data/jobsSubscribe")?;
    let kind = match update.get("type")?.as_str()? {
        "ADD" => UpdateKind::Add,
        "UPDATE" => UpdateKind::Update,
        "REMOVE" => UpdateKind::Remove,
        _ => return None,
    };
    Some(JobUpdate {
        kind,
        job: Job::from_json(update.get("job")?)?,
    })
}

const ENDED_WHILE_AWAY: &str = "Ended while the viewer was disconnected.";

fn key(profile: Uuid, id: &str) -> String {
    format!("job:{profile}:{id}")
}

/// Turns job updates for one server into notifications.
pub struct JobTracker {
    profile: Uuid,
    center: Arc<NotificationCenter>,
    /// Jobs seen as queued/running/stopping this session.
    active: HashSet<String>,
}

impl JobTracker {
    pub fn new(profile: Uuid, center: Arc<NotificationCenter>) -> Self {
        Self {
            profile,
            center,
            active: HashSet::new(),
        }
    }

    fn post(&self, job: &Job, status: JobStatus, detail: Option<String>, severity: Severity) {
        self.center.post(NewNotification {
            key: Some(key(self.profile, &job.id)),
            profile_id: Some(self.profile),
            kind: NotificationKind::Job,
            severity,
            title: if job.description.is_empty() {
                "Stash job".into()
            } else {
                job.description.clone()
            },
            detail,
            // Only failures pop up; progress and success stay in the centre.
            toast: status == JobStatus::Failed,
            job: Some(JobProgress {
                status,
                progress: job.progress,
                started_at: job.started_at.clone(),
                ended_at: job.ended_at.clone(),
            }),
        });
    }

    /// Apply one ADD / UPDATE / REMOVE.
    pub fn apply(&mut self, update: JobUpdate) {
        let job = &update.job;
        match update.kind {
            UpdateKind::Add | UpdateKind::Update if job.status.is_active() => {
                self.active.insert(job.id.clone());
                self.post(job, job.status, None, Severity::Info);
            }
            _ => {
                // REMOVE (or an update to a final status): the job is over.
                self.active.remove(&job.id);
                let status = match job.status {
                    s @ (JobStatus::Finished | JobStatus::Failed | JobStatus::Cancelled) => s,
                    _ => JobStatus::Finished,
                };
                let (severity, detail) = match status {
                    JobStatus::Failed => (
                        Severity::Error,
                        Some(
                            job.error
                                .clone()
                                .unwrap_or_else(|| "The job failed.".into()),
                        ),
                    ),
                    JobStatus::Cancelled => (Severity::Info, Some("Cancelled.".into())),
                    _ => (Severity::Info, Some("Finished.".into())),
                };
                self.post(job, status, detail, severity);
            }
        }
    }

    /// Seed or reconcile with the current queue (on every successful connection): queued jobs
    /// are posted, and jobs that were active but are no longer queued ended while disconnected.
    pub fn reconcile(&mut self, queue: Vec<Job>) {
        let present: HashSet<String> = queue.iter().map(|j| j.id.clone()).collect();
        let prefix = format!("job:{}:", self.profile);
        for n in self.center.list() {
            let Some(id) = n.key.as_deref().and_then(|k| k.strip_prefix(&prefix)) else {
                continue;
            };
            let was_active = n
                .job
                .as_ref()
                .is_some_and(|j| j.status.is_active() || j.status == JobStatus::Unknown);
            let already_ended = n.detail.as_deref() == Some(ENDED_WHILE_AWAY);
            if was_active && !already_ended && !present.contains(id) {
                self.center.post(NewNotification {
                    detail: Some(ENDED_WHILE_AWAY.into()),
                    toast: false,
                    job: n.job.clone().map(|j| JobProgress {
                        status: JobStatus::Unknown,
                        ..j
                    }),
                    key: n.key.clone(),
                    profile_id: n.profile_id,
                    kind: n.kind,
                    severity: Severity::Info,
                    title: n.title.clone(),
                });
                self.active.remove(id);
            }
        }
        for job in queue {
            self.apply(JobUpdate {
                kind: UpdateKind::Update,
                job,
            });
        }
    }

    /// The server can't be watched (offline): active jobs become "status unknown".
    pub fn mark_unknown(center: &NotificationCenter, profile: Uuid) {
        let prefix = format!("job:{profile}:");
        for n in center.list() {
            let active = n.job.as_ref().is_some_and(|j| j.status.is_active());
            if active && n.key.as_deref().is_some_and(|k| k.starts_with(&prefix)) {
                center.post(NewNotification {
                    key: n.key.clone(),
                    profile_id: n.profile_id,
                    kind: n.kind,
                    severity: n.severity,
                    title: n.title.clone(),
                    detail: Some("Status unknown while the server can't be reached.".into()),
                    toast: false,
                    job: n.job.clone().map(|j| JobProgress {
                        status: JobStatus::Unknown,
                        ..j
                    }),
                });
            }
        }
    }
}
