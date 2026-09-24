//! `ConnectionManager`: owns the active session, health checks, and offline backoff
//! (research R8, data-model.md "ConnectionState and transitions").
//!
//! Every state transition publishes exactly one `ConnectionSnapshot` on a broadcast channel.
//! Successful health checks update `last_contact_at` silently, without publishing.

use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use chrono::Utc;
use tokio::runtime::Handle;
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

use super::connect::server_info;
use super::failure::ConnectFailure;
use super::snapshot::{ConnectionSnapshot, SessionState};
use super::ServerInfo;
use crate::adapter::{health, probe, StashClient, DEFAULT_TIMEOUT};
use crate::profiles::model::{display_url, ServerProfile};

/// Where to connect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub profile_id: Uuid,
    pub base_url: Url,
    pub strict_tls: bool,
    pub api_key: Option<String>,
}

impl From<&ServerProfile> for Target {
    fn from(p: &ServerProfile) -> Self {
        Self {
            profile_id: p.id,
            base_url: p.base_url.clone(),
            strict_tls: p.strict_tls,
            api_key: p.api_key.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConnectRequest {
    pub target: Target,
    /// Automatic connect at app launch.
    pub is_launch: bool,
    /// The profile has connected successfully before (its `last_used_at` is set). A launch
    /// auto-connect to such a profile that can't be reached goes Offline instead of Failed.
    pub has_connected_before: bool,
}

#[derive(Debug, Clone)]
pub struct ManagerConfig {
    /// Health-check interval while Connected.
    pub health_interval: Duration,
    /// Offline retry delays, in order; after the last one, `backoff_cap` repeats.
    pub backoff: Vec<Duration>,
    pub backoff_cap: Duration,
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            health_interval: Duration::from_secs(5),
            backoff: [1, 2, 4, 8, 16, 32].map(Duration::from_secs).to_vec(),
            backoff_cap: Duration::from_secs(60),
        }
    }
}

impl ManagerConfig {
    fn delay(&self, attempt: u32) -> Duration {
        let index = usize::try_from(attempt.saturating_sub(1)).unwrap_or(usize::MAX);
        self.backoff.get(index).copied().unwrap_or(self.backoff_cap)
    }
}

/// How the manager talks to Stash. Production uses [`StashProber`]; tests use a fake.
pub trait Prober: Send + Sync + 'static {
    /// Full probe plus version gate. Returns the final base URL and server info.
    fn probe(
        &self,
        target: &Target,
    ) -> impl Future<Output = Result<(Url, ServerInfo), ConnectFailure>> + Send;

    /// Cheap reachability check.
    fn health(&self, target: &Target) -> impl Future<Output = bool> + Send;
}

/// The real prober, backed by the Stash adapter.
#[derive(Debug, Default)]
pub struct StashProber {
    /// Reused between health checks so the connection pool survives.
    client: Mutex<Option<(Target, StashClient)>>,
}

impl StashProber {
    fn client_for(&self, target: &Target) -> Result<StashClient, ConnectFailure> {
        let mut cached = self.client.lock().unwrap_or_else(|p| p.into_inner());
        if let Some((t, c)) = cached.as_ref() {
            if t == target {
                return Ok(c.clone());
            }
        }
        let client = StashClient::with_timeout(
            target.base_url.clone(),
            target.strict_tls,
            target.api_key.clone(),
            DEFAULT_TIMEOUT,
        )
        .map_err(|_| ConnectFailure::Unreachable {
            tried: vec![display_url(&target.base_url)],
        })?;
        *cached = Some((target.clone(), client.clone()));
        Ok(client)
    }
}

impl Prober for StashProber {
    async fn probe(&self, target: &Target) -> Result<(Url, ServerInfo), ConnectFailure> {
        let client = self.client_for(target)?;
        let data = probe::probe(&client).await?;
        let info = server_info(&data)?;
        Ok((data.final_base_url, info))
    }

    async fn health(&self, target: &Target) -> bool {
        match self.client_for(target) {
            Ok(client) => health::health(&client).await,
            Err(_) => false,
        }
    }
}

type ConnectedHook = Arc<dyn Fn(Uuid) + Send + Sync>;

struct Session {
    generation: u64,
    cancel: CancellationToken,
    reports: mpsc::UnboundedSender<ConnectFailure>,
}

struct Inner<P> {
    prober: P,
    config: ManagerConfig,
    handle: Handle,
    tx: broadcast::Sender<ConnectionSnapshot>,
    snapshot: Mutex<ConnectionSnapshot>,
    session: Mutex<(u64, Option<Session>)>,
    on_connected: Mutex<Option<ConnectedHook>>,
}

/// Cheap to clone; clones share the same session.
pub struct ConnectionManager<P: Prober> {
    inner: Arc<Inner<P>>,
}

impl<P: Prober> Clone for ConnectionManager<P> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

enum Offline {
    Recovered(Url, ServerInfo),
    Stop,
}

impl<P: Prober> ConnectionManager<P> {
    /// `handle` is the tokio runtime the session tasks run on.
    pub fn new(prober: P, config: ManagerConfig, handle: Handle) -> Self {
        let (tx, _) = broadcast::channel(64);
        Self {
            inner: Arc::new(Inner {
                prober,
                config,
                handle,
                tx,
                snapshot: Mutex::new(ConnectionSnapshot::default()),
                session: Mutex::new((0, None)),
                on_connected: Mutex::new(None),
            }),
        }
    }

    /// Called with the profile id after every successful (re)connect, e.g. to update
    /// `last_used_at` / `last_used_profile_id`.
    pub fn on_connected(&self, hook: impl Fn(Uuid) + Send + Sync + 'static) {
        *lock(&self.inner.on_connected) = Some(Arc::new(hook));
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ConnectionSnapshot> {
        self.inner.tx.subscribe()
    }

    pub fn snapshot(&self) -> ConnectionSnapshot {
        lock(&self.inner.snapshot).clone()
    }

    pub fn active_profile_id(&self) -> Option<Uuid> {
        lock(&self.inner.snapshot).profile_id
    }

    /// Start (or restart) a session. Any previous session is cancelled first.
    pub fn connect(&self, request: ConnectRequest) {
        let cancel = CancellationToken::new();
        let (reports_tx, reports_rx) = mpsc::unbounded_channel();
        let generation = {
            let mut session = lock(&self.inner.session);
            if let Some(old) = session.1.take() {
                old.cancel.cancel();
            }
            session.0 += 1;
            session.1 = Some(Session {
                generation: session.0,
                cancel: cancel.clone(),
                reports: reports_tx,
            });
            session.0
        };

        self.inner.publish(
            generation,
            ConnectionSnapshot {
                profile_id: Some(request.target.profile_id),
                state: SessionState::Connecting {
                    attempt_url: display_url(&request.target.base_url),
                },
                ..Default::default()
            },
        );

        let inner = Arc::clone(&self.inner);
        self.inner.handle.spawn(async move {
            tokio::select! {
                () = cancel.cancelled() => {}
                () = inner.run(generation, request, reports_rx) => {}
            }
        });
    }

    /// End the session and return to Idle.
    pub fn disconnect(&self) {
        let generation = {
            let mut session = lock(&self.inner.session);
            if let Some(old) = session.1.take() {
                old.cancel.cancel();
            }
            session.0 += 1;
            session.0
        };
        // Publish directly: there is no live session to match the generation.
        self.inner
            .publish_unchecked(generation, ConnectionSnapshot::default());
    }

    /// Tell the manager an ordinary request failed. A network-level failure moves Connected to
    /// Offline; an auth failure moves it to AuthFailed.
    pub fn report_request_failure(&self, failure: ConnectFailure) {
        if let Some(session) = lock(&self.inner.session).1.as_ref() {
            let _ = session.reports.send(failure);
        }
    }
}

impl<P: Prober> Inner<P> {
    /// Publish if `generation` is still the live session.
    fn publish(&self, generation: u64, snapshot: ConnectionSnapshot) {
        let session = lock(&self.session);
        let live = session
            .1
            .as_ref()
            .is_some_and(|s| s.generation == generation);
        if live {
            *lock(&self.snapshot) = snapshot.clone();
            let _ = self.tx.send(snapshot);
        }
    }

    fn publish_unchecked(&self, generation: u64, snapshot: ConnectionSnapshot) {
        let session = lock(&self.session);
        if session.0 == generation {
            *lock(&self.snapshot) = snapshot.clone();
            let _ = self.tx.send(snapshot);
        }
    }

    /// Update `last_contact_at` without publishing (not a transition).
    fn touch(&self, generation: u64) {
        let session = lock(&self.session);
        if session
            .1
            .as_ref()
            .is_some_and(|s| s.generation == generation)
        {
            lock(&self.snapshot).last_contact_at = Some(now_iso());
        }
    }

    fn base(&self, target: &Target) -> ConnectionSnapshot {
        let current = lock(&self.snapshot).clone();
        ConnectionSnapshot {
            profile_id: Some(target.profile_id),
            final_url: current.final_url,
            server: current.server,
            last_contact_at: current.last_contact_at,
            ..Default::default()
        }
    }

    fn stop(&self, generation: u64, target: &Target, failure: ConnectFailure) {
        let state = if failure.is_auth() {
            SessionState::AuthFailed { failure }
        } else {
            SessionState::Failed { failure }
        };
        self.publish(
            generation,
            ConnectionSnapshot {
                state,
                ..self.base(target)
            },
        );
    }

    async fn run(
        self: &Arc<Self>,
        generation: u64,
        request: ConnectRequest,
        mut reports: mpsc::UnboundedReceiver<ConnectFailure>,
    ) {
        let mut target = request.target;

        let (mut url, mut info) = match self.prober.probe(&target).await {
            Ok(ok) => ok,
            Err(failure) => {
                let offline_ok = request.is_launch
                    && request.has_connected_before
                    && matches!(
                        failure,
                        ConnectFailure::Unreachable { .. } | ConnectFailure::Timeout
                    );
                if !offline_ok {
                    return self.stop(generation, &target, failure);
                }
                match self.offline(generation, &target).await {
                    Offline::Recovered(u, i) => (u, i),
                    Offline::Stop => return,
                }
            }
        };

        loop {
            target.base_url = url;
            self.publish(
                generation,
                ConnectionSnapshot {
                    profile_id: Some(target.profile_id),
                    state: SessionState::Connected,
                    final_url: Some(display_url(&target.base_url)),
                    server: Some(info),
                    last_contact_at: Some(now_iso()),
                },
            );
            let hook = lock(&self.on_connected).clone();
            if let Some(hook) = hook {
                hook(target.profile_id);
            }

            // Connected: health loop until something fails.
            loop {
                tokio::select! {
                    () = tokio::time::sleep(self.config.health_interval) => {
                        if self.prober.health(&target).await {
                            self.touch(generation);
                        } else {
                            break;
                        }
                    }
                    Some(failure) = reports.recv() => {
                        if failure.is_auth() {
                            return self.stop(generation, &target, failure);
                        }
                        break;
                    }
                }
            }

            match self.offline(generation, &target).await {
                Offline::Recovered(u, i) => (url, info) = (u, i),
                Offline::Stop => return,
            }
        }
    }

    /// Retry with backoff until the server is back (full re-probe) or a non-network failure.
    async fn offline(&self, generation: u64, target: &Target) -> Offline {
        let mut attempt: u32 = 1;
        loop {
            let delay = self.config.delay(attempt);
            let next = Utc::now()
                + chrono::Duration::from_std(delay).unwrap_or_else(|_| chrono::Duration::zero());
            self.publish(
                generation,
                ConnectionSnapshot {
                    state: SessionState::Offline {
                        attempt,
                        next_retry_at: next.to_rfc3339(),
                    },
                    ..self.base(target)
                },
            );
            tokio::time::sleep(delay).await;

            if self.prober.health(target).await {
                match self.prober.probe(target).await {
                    Ok((url, info)) => return Offline::Recovered(url, info),
                    Err(ConnectFailure::Unreachable { .. } | ConnectFailure::Timeout) => {}
                    Err(failure) => {
                        self.stop(generation, target, failure);
                        return Offline::Stop;
                    }
                }
            }
            attempt = attempt.saturating_add(1);
        }
    }
}
