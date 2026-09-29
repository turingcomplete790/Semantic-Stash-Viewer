//! Read-only job queries (004 contracts "Stash GraphQL"): the current queue, and the
//! subscription document used over WebSocket (`jobs::ws`).

use graphql_client::GraphQLQuery;

use super::StashClient;
use crate::error::AppError;
use crate::jobs::Job;

/// Stash's `Time` scalar (RFC 3339 text).
type Time = String;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.json",
    query_path = "graphql/job_queue.graphql",
    response_derives = "Debug, Serialize"
)]
pub struct JobQueue;

/// The `JobsSubscribe` document, sent as-is in the WebSocket `subscribe` message.
pub const JOBS_SUBSCRIBE: &str = include_str!("../../graphql/jobs_subscribe.graphql");

/// Jobs currently queued or running on the server (one request per connection).
pub async fn job_queue(client: &StashClient) -> Result<Vec<Job>, AppError> {
    let data: job_queue::ResponseData = client
        .graphql(&JobQueue::build_query(job_queue::Variables))
        .await?;
    Ok(data
        .job_queue
        .unwrap_or_default()
        .iter()
        .filter_map(|j| serde_json::to_value(j).ok())
        .filter_map(|v| Job::from_json(&v))
        .collect())
}
