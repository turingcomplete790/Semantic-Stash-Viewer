//! Watches one connected server's jobs (004 US3, research R5).
//!
//! Per session: seed from `jobQueue`, then follow `jobsSubscribe`. If the socket drops while the
//! session is still up, reconnect with backoff and reconcile. When the session goes offline,
//! active jobs become "status unknown" until it's back.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::task::JoinHandle;
use uuid::Uuid;

use super::{ws, JobTracker};
use crate::adapter::jobs::job_queue;
use crate::adapter::StashClient;
use crate::shell::notifications::NotificationCenter;

const MIN_BACKOFF: Duration = Duration::from_secs(2);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

pub struct JobsWatcher {
    center: Arc<NotificationCenter>,
    runtime: tokio::runtime::Handle,
    session: Mutex<Option<(Uuid, JoinHandle<()>)>>,
}

impl JobsWatcher {
    pub fn new(center: Arc<NotificationCenter>, runtime: tokio::runtime::Handle) -> Self {
        Self {
            center,
            runtime,
            session: Mutex::new(None),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<(Uuid, JoinHandle<()>)>> {
        self.session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The session for `profile` is up: start watching (no-op if already watching it).
    pub fn connected(&self, profile: Uuid, client: StashClient) {
        let mut session = self.lock();
        if session
            .as_ref()
            .is_some_and(|(p, h)| *p == profile && !h.is_finished())
        {
            return;
        }
        if let Some((old, handle)) = session.take() {
            handle.abort();
            JobTracker::mark_unknown(&self.center, old);
        }
        let center = Arc::clone(&self.center);
        let handle = self.runtime.spawn(async move {
            let mut tracker = JobTracker::new(profile, Arc::clone(&center));
            let mut backoff = MIN_BACKOFF;
            loop {
                match job_queue(&client).await {
                    Ok(queue) => tracker.reconcile(queue),
                    Err(e) => tracing::debug!(error = %e, "couldn't read the job queue"),
                }
                match ws::subscribe_jobs(&client).await {
                    Ok(mut updates) => {
                        backoff = MIN_BACKOFF;
                        while let Some(update) = updates.recv().await {
                            tracker.apply(update);
                        }
                        tracing::debug!("jobs subscription ended; reconnecting");
                    }
                    Err(e) => tracing::debug!(error = %e, "couldn't subscribe to jobs"),
                }
                JobTracker::mark_unknown(&center, profile);
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
        });
        *session = Some((profile, handle));
    }

    /// The session is down (offline, failed, disconnected): stop, and mark active jobs unknown.
    pub fn disconnected(&self) {
        if let Some((profile, handle)) = self.lock().take() {
            handle.abort();
            JobTracker::mark_unknown(&self.center, profile);
        }
    }
}

impl Drop for JobsWatcher {
    fn drop(&mut self) {
        if let Some((_, handle)) = self.lock().take() {
            handle.abort();
        }
    }
}
