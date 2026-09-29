//! Watching Stash jobs (004 US3, research R5), from recorded fixtures and a mock WebSocket.

use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use stash_core::adapter::jobs::job_queue;
use stash_core::adapter::StashClient;
use stash_core::jobs::{parse_next, status_from_stash, ws, JobTracker};
use stash_core::shell::notifications::{JobStatus, NotificationCenter, Severity};
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::Message;
use url::Url;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stash-v0.31.1");

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{FIXTURES}/{name}")).expect("fixture")
}

fn events(sequence: &str) -> Vec<Value> {
    let all: Value = serde_json::from_str(&fixture("jobs-subscribe-events.json")).expect("json");
    all[sequence].as_array().expect("sequence").clone()
}

fn center() -> (tempfile::TempDir, Arc<NotificationCenter>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let c = Arc::new(NotificationCenter::open(dir.path().join("n.json")));
    (dir, c)
}

fn job_status(c: &NotificationCenter, profile: Uuid, id: &str) -> Option<JobStatus> {
    c.find(&format!("job:{profile}:{id}"))
        .and_then(|n| n.job)
        .map(|j| j.status)
}

#[tokio::test]
async fn parses_the_job_queue() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(fixture("job-queue.json"), "application/json"),
        )
        .mount(&server)
        .await;
    let client =
        StashClient::new(Url::parse(&server.uri()).expect("url"), false, None).expect("client");
    let jobs = job_queue(&client).await.expect("queue");
    assert_eq!(jobs.len(), 2);
    assert_eq!(jobs[0].status, JobStatus::Running);
    assert_eq!(jobs[0].description, "Generating...");
    assert_eq!(jobs[1].status, JobStatus::Queued);
}

#[test]
fn maps_stash_job_statuses() {
    assert_eq!(status_from_stash("READY"), JobStatus::Queued);
    assert_eq!(status_from_stash("RUNNING"), JobStatus::Running);
    assert_eq!(status_from_stash("STOPPING"), JobStatus::Stopping);
    assert_eq!(status_from_stash("FINISHED"), JobStatus::Finished);
    assert_eq!(status_from_stash("FAILED"), JobStatus::Failed);
    assert_eq!(status_from_stash("CANCELLED"), JobStatus::Cancelled);
    assert_eq!(status_from_stash("SOMETHING_NEW"), JobStatus::Unknown);
}

#[test]
fn replays_recorded_events_into_notifications() {
    let (_dir, c) = center();
    let p = Uuid::new_v4();
    let mut tracker = JobTracker::new(p, Arc::clone(&c));
    for message in events("cancelledAndFinished") {
        if let Some(update) = parse_next(&message) {
            tracker.apply(update);
        }
    }
    // Job 5 (generate) was stopped; job 6 (scan) finished.
    let generate = c.find(&format!("job:{p}:5")).expect("job 5");
    assert_eq!(
        generate.job.as_ref().map(|j| j.status),
        Some(JobStatus::Cancelled)
    );
    assert_eq!(generate.severity, Severity::Info);
    assert!(!generate.toast);
    let scan = c.find(&format!("job:{p}:6")).expect("job 6");
    assert_eq!(
        scan.job.as_ref().map(|j| j.status),
        Some(JobStatus::Finished)
    );
    assert_eq!(scan.title, "Scanning...");

    for message in events("failed") {
        if let Some(update) = parse_next(&message) {
            tracker.apply(update);
        }
    }
    let failed = c.find(&format!("job:{p}:7")).expect("job 7");
    assert_eq!(failed.severity, Severity::Error);
    assert!(failed.toast, "failures toast");
    assert_eq!(
        failed.detail.as_deref(),
        Some("scan failed: path not found")
    );
}

#[test]
fn progress_updates_keep_one_entry() {
    let (_dir, c) = center();
    let p = Uuid::new_v4();
    let mut tracker = JobTracker::new(p, Arc::clone(&c));
    let seq = events("cancelledAndFinished");
    // ADD 5, UPDATE 5 RUNNING, ADD 6, UPDATE 5 …: two jobs, one entry each.
    for message in &seq[..6] {
        if let Some(update) = parse_next(message) {
            tracker.apply(update);
        }
    }
    assert_eq!(c.list().len(), 2);
    assert_eq!(job_status(&c, p, "5"), Some(JobStatus::Running));
    assert_eq!(job_status(&c, p, "6"), Some(JobStatus::Queued));
}

#[test]
fn offline_marks_active_jobs_unknown_and_reconnect_reconciles() {
    let (_dir, c) = center();
    let p = Uuid::new_v4();
    let mut tracker = JobTracker::new(p, Arc::clone(&c));
    let seq = events("cancelledAndFinished");
    for message in &seq[..4] {
        if let Some(update) = parse_next(message) {
            tracker.apply(update);
        }
    }
    assert_eq!(job_status(&c, p, "5"), Some(JobStatus::Running));
    assert_eq!(job_status(&c, p, "6"), Some(JobStatus::Queued));

    JobTracker::mark_unknown(&c, p);
    assert_eq!(job_status(&c, p, "5"), Some(JobStatus::Unknown));
    assert_eq!(job_status(&c, p, "6"), Some(JobStatus::Unknown));

    // Back online: job 6 is still queued, job 5 is gone (it ended while we were away).
    let queue: Value = serde_json::from_str(&fixture("job-queue.json")).expect("json");
    let still = queue["data"]["jobQueue"][1].clone(); // id 6, READY
    let jobs = vec![stash_core::jobs::Job::from_json(&still).expect("job")];
    tracker.reconcile(jobs);
    assert_eq!(job_status(&c, p, "6"), Some(JobStatus::Queued));
    let gone = c.find(&format!("job:{p}:5")).expect("job 5");
    assert_eq!(gone.job.map(|j| j.status), Some(JobStatus::Unknown));
    assert_eq!(
        gone.detail.as_deref(),
        Some("Ended while the viewer was disconnected.")
    );
}

// The handshake callback's error type is tungstenite's `ErrorResponse`, fixed by its API.
#[allow(clippy::result_large_err)]
#[tokio::test]
async fn subscribes_over_graphql_transport_ws_with_the_api_key() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let seen: Arc<Mutex<Vec<(String, String)>>> = Arc::default();
    let seen_server = Arc::clone(&seen);
    let next = events("cancelledAndFinished")[1].clone();

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut ws =
            tokio_tungstenite::accept_hdr_async(stream, |req: &Request, mut res: Response| {
                let mut headers = seen_server.lock().expect("lock");
                for (name, value) in req.headers() {
                    headers.push((
                        name.as_str().to_owned(),
                        value.to_str().unwrap_or("").to_owned(),
                    ));
                }
                res.headers_mut().insert(
                    "sec-websocket-protocol",
                    "graphql-transport-ws".parse().expect("hv"),
                );
                Ok(res)
            })
            .await
            .expect("handshake");

        let init: Value = recv_json(&mut ws).await;
        assert_eq!(init["type"], "connection_init");
        ws.send(Message::text(
            json!({ "type": "connection_ack" }).to_string(),
        ))
        .await
        .expect("ack");
        let sub: Value = recv_json(&mut ws).await;
        assert_eq!(sub["type"], "subscribe");
        assert!(sub["payload"]["query"]
            .as_str()
            .is_some_and(|q| q.contains("jobsSubscribe")));
        ws.send(Message::text(json!({ "type": "ping" }).to_string()))
            .await
            .expect("ping");
        let pong: Value = recv_json(&mut ws).await;
        assert_eq!(pong["type"], "pong");
        ws.send(Message::text(next.to_string()))
            .await
            .expect("next");
        // Keep the socket open until the client has read the update.
        let _ = ws.next().await;
    });

    let base = Url::parse(&format!("http://127.0.0.1:{port}")).expect("url");
    let client = StashClient::new(base, false, Some("secret".into())).expect("client");
    let mut updates = ws::subscribe_jobs(&client).await.expect("subscribe");
    let update = tokio::time::timeout(std::time::Duration::from_secs(5), updates.recv())
        .await
        .expect("in time")
        .expect("an update");
    assert_eq!(update.job.id, "5");
    assert_eq!(update.job.status, JobStatus::Queued);
    drop(updates);
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), server).await;

    let headers = seen.lock().expect("lock");
    let header = |name: &str| {
        headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone())
    };
    assert_eq!(
        header("sec-websocket-protocol").as_deref(),
        Some("graphql-transport-ws")
    );
    assert_eq!(header("apikey").as_deref(), Some("secret"));
}

async fn recv_json(ws: &mut tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>) -> Value {
    loop {
        match ws.next().await.expect("message").expect("ok") {
            Message::Text(text) => return serde_json::from_str(&text).expect("json"),
            Message::Ping(_) | Message::Pong(_) => continue,
            other => panic!("unexpected {other:?}"),
        }
    }
}
