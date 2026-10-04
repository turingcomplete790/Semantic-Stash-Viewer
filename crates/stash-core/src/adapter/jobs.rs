//! Read-only job queries (004 contracts "Stash GraphQL"): the current queue, and the
//! subscription document used over WebSocket (`jobs::ws`).

use cynic::QueryBuilder;

use super::gql::{schema, Time};
use super::StashClient;
use crate::error::AppError;
use crate::jobs::{status_from_stash, Job};

/// Stash's `JobStatus`; a value this build doesn't know (a newer Stash) becomes `Other`.
#[derive(cynic::Enum, Clone, Debug, PartialEq, Eq)]
#[cynic(graphql_type = "JobStatus")]
enum StashJobStatus {
    Ready,
    Running,
    Finished,
    Stopping,
    Cancelled,
    Failed,
    #[cynic(fallback)]
    Other(String),
}

impl StashJobStatus {
    fn as_stash(&self) -> &str {
        match self {
            Self::Ready => "READY",
            Self::Running => "RUNNING",
            Self::Finished => "FINISHED",
            Self::Stopping => "STOPPING",
            Self::Cancelled => "CANCELLED",
            Self::Failed => "FAILED",
            Self::Other(other) => other,
        }
    }
}

/// Only the fields the notification centre shows (Principle IV).
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Job")]
struct JobFields {
    id: cynic::Id,
    status: StashJobStatus,
    description: String,
    progress: Option<f64>,
    start_time: Option<Time>,
    end_time: Option<Time>,
    error: Option<String>,
}

/// Jobs currently queued or running, to seed the notification centre when a session connects
/// (004 research R5).
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
struct JobQueue {
    job_queue: Option<Vec<JobFields>>,
}

/// The `JobsSubscribe` document, sent as-is in the WebSocket `subscribe` message.
pub const JOBS_SUBSCRIBE: &str = include_str!("../../graphql/jobs_subscribe.graphql");

/// Jobs currently queued or running on the server (one request per connection).
pub async fn job_queue(client: &StashClient) -> Result<Vec<Job>, AppError> {
    let data = client.graphql(&JobQueue::build(())).await?;
    Ok(data
        .job_queue
        .unwrap_or_default()
        .into_iter()
        .map(|j| Job {
            status: status_from_stash(j.status.as_stash()),
            id: j.id.into_inner(),
            description: j.description,
            progress: j.progress,
            started_at: j.start_time.map(|t| t.0),
            ended_at: j.end_time.map(|t| t.0),
            error: j.error.filter(|e| !e.is_empty()),
        })
        .collect())
}
