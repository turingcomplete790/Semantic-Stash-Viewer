//! ConnectionManager state machine (data-model.md "ConnectionState and transitions").
//!
//! Uses a scripted fake `Prober` with paused tokio time: with real sockets, paused time would
//! auto-advance past reqwest's timeouts. The real HTTP path is covered by probe_classification.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use stash_core::connection::manager::{
    ConnectRequest, ConnectionManager, ManagerConfig, Prober, Target,
};
use stash_core::connection::snapshot::{ConnectionSnapshot, SessionState};
use stash_core::connection::{
    ConnectFailure, LibraryCounts, SecurityState, ServerInfo, VersionStatus,
};
use tokio::sync::broadcast;
use tokio::time::Instant;
use url::Url;
use uuid::Uuid;

type ProbeResult = Result<ServerInfo, ConnectFailure>;

/// Scripted responses; when a queue runs dry the last value repeats.
#[derive(Default)]
struct Script {
    probes: VecDeque<ProbeResult>,
    health: VecDeque<bool>,
    last_probe: Option<ProbeResult>,
    last_health: Option<bool>,
    probe_calls: Vec<Instant>,
    health_calls: Vec<Instant>,
}

#[derive(Clone, Default)]
struct FakeProber(Arc<Mutex<Script>>);

impl FakeProber {
    fn probes(self, results: impl IntoIterator<Item = ProbeResult>) -> Self {
        self.0.lock().expect("lock").probes.extend(results);
        self
    }
    fn health(self, results: impl IntoIterator<Item = bool>) -> Self {
        self.0.lock().expect("lock").health.extend(results);
        self
    }
    fn health_calls(&self) -> Vec<Instant> {
        self.0.lock().expect("lock").health_calls.clone()
    }
    fn probe_calls(&self) -> usize {
        self.0.lock().expect("lock").probe_calls.len()
    }
}

impl Prober for FakeProber {
    async fn probe(&self, target: &Target) -> Result<(Url, ServerInfo), ConnectFailure> {
        let result = {
            let mut s = self.0.lock().expect("lock");
            s.probe_calls.push(Instant::now());
            let next = s.probes.pop_front().or_else(|| s.last_probe.clone());
            s.last_probe = next.clone();
            next.unwrap_or(Err(ConnectFailure::Unreachable { tried: vec![] }))
        };
        result.map(|info| (target.base_url.clone(), info))
    }

    async fn health(&self, _target: &Target) -> bool {
        let mut s = self.0.lock().expect("lock");
        s.health_calls.push(Instant::now());
        let next = s.health.pop_front().or(s.last_health).unwrap_or(true);
        s.last_health = Some(next);
        next
    }
}

fn server(version: &str) -> ServerInfo {
    ServerInfo {
        identity: "0000000000000000".into(),
        version: version.into(),
        version_status: VersionStatus::Supported,
        app_schema: 85,
        counts: LibraryCounts {
            scenes: 1,
            images: 2,
            galleries: 3,
            performers: 4,
        },
    }
}

fn unreachable() -> ProbeResult {
    Err(ConnectFailure::Unreachable {
        tried: vec!["http://stash:9999".into()],
    })
}

fn target() -> Target {
    Target {
        profile_id: Uuid::new_v4(),
        base_url: Url::parse("http://stash:9999").expect("url"),
        strict_tls: false,
        api_key: None,
    }
}

fn request(t: &Target, is_launch: bool, connected_before: bool) -> ConnectRequest {
    ConnectRequest {
        target: t.clone(),
        is_launch,
        has_connected_before: connected_before,
    }
}

fn manager(
    prober: FakeProber,
) -> (
    ConnectionManager<FakeProber>,
    broadcast::Receiver<ConnectionSnapshot>,
) {
    let m = ConnectionManager::new(
        prober,
        ManagerConfig::default(),
        tokio::runtime::Handle::current(),
    );
    let rx = m.subscribe();
    (m, rx)
}

/// Next snapshot, advancing paused time as needed.
async fn next(rx: &mut broadcast::Receiver<ConnectionSnapshot>) -> ConnectionSnapshot {
    tokio::time::timeout(Duration::from_secs(600), rx.recv())
        .await
        .expect("a snapshot within 10 simulated minutes")
        .expect("channel open")
}

fn kind(s: &ConnectionSnapshot) -> &'static str {
    match s.state {
        SessionState::Idle => "idle",
        SessionState::Connecting { .. } => "connecting",
        SessionState::Connected => "connected",
        SessionState::Offline { .. } => "offline",
        SessionState::AuthFailed { .. } => "authFailed",
        SessionState::Failed { .. } => "failed",
    }
}

#[tokio::test(start_paused = true)]
async fn connects_then_goes_offline_on_health_failure() {
    let prober = FakeProber::default()
        .probes([Ok(server("v0.31.1"))])
        .health([true, false]);
    let (m, mut rx) = manager(prober.clone());
    let t = target();
    m.connect(request(&t, false, false));

    assert_eq!(kind(&next(&mut rx).await), "connecting");
    let connected = next(&mut rx).await;
    assert_eq!(kind(&connected), "connected");
    assert_eq!(connected.profile_id, Some(t.profile_id));
    assert_eq!(
        connected.server.as_ref().map(|s| s.version.as_str()),
        Some("v0.31.1")
    );
    assert_eq!(connected.final_url.as_deref(), Some("http://stash:9999"));
    assert_eq!(connected.security, Some(SecurityState::Unencrypted));

    let offline = next(&mut rx).await;
    assert_eq!(kind(&offline), "offline");
    // Health runs every 5 s while connected: first ok at +5 s, failure at +10 s.
    let calls = prober.health_calls();
    assert_eq!(calls[1] - calls[0], Duration::from_secs(5));
}

#[tokio::test(start_paused = true)]
async fn backoff_is_1_2_4_8_16_32_then_60() {
    // Connected, then the server stays down.
    let prober = FakeProber::default()
        .probes([Ok(server("v0.31.1"))])
        .health([false]);
    let (m, mut rx) = manager(prober.clone());
    m.connect(request(&target(), false, false));

    let mut attempts = Vec::new();
    while attempts.len() < 9 {
        let s = next(&mut rx).await;
        if let SessionState::Offline { attempt, .. } = s.state {
            attempts.push(attempt);
        }
    }
    assert_eq!(attempts, (1..=9).collect::<Vec<u32>>());

    // health_calls[0] is the failing check while connected; the rest are backoff probes.
    let calls = prober.health_calls();
    let gaps: Vec<u64> = calls.windows(2).map(|w| (w[1] - w[0]).as_secs()).collect();
    assert_eq!(&gaps[..8], &[1, 2, 4, 8, 16, 32, 60, 60]);
}

#[tokio::test(start_paused = true)]
async fn recovery_reruns_the_full_probe() {
    let prober = FakeProber::default()
        .probes([Ok(server("v0.31.1")), Ok(server("v0.31.2"))])
        .health([false, false, true]);
    let (m, mut rx) = manager(prober.clone());
    m.connect(request(&target(), false, false));

    let mut kinds = Vec::new();
    loop {
        let s = next(&mut rx).await;
        kinds.push(kind(&s));
        if kinds.iter().filter(|k| **k == "connected").count() == 2 {
            assert_eq!(s.server.map(|x| x.version), Some("v0.31.2".into()));
            break;
        }
    }
    assert_eq!(
        kinds,
        ["connecting", "connected", "offline", "offline", "connected"]
    );
    assert_eq!(prober.probe_calls(), 2);
}

#[tokio::test(start_paused = true)]
async fn auth_failure_on_reprobe_stops_retrying() {
    let prober = FakeProber::default()
        .probes([Ok(server("v0.31.1")), Err(ConnectFailure::ApiKeyRejected)])
        .health([false, true]);
    let (m, mut rx) = manager(prober.clone());
    m.connect(request(&target(), false, false));

    let mut last = next(&mut rx).await;
    while !matches!(last.state, SessionState::AuthFailed { .. }) {
        last = next(&mut rx).await;
    }
    assert_eq!(
        last.state,
        SessionState::AuthFailed {
            failure: ConnectFailure::ApiKeyRejected
        }
    );

    let probes = prober.probe_calls();
    let health = prober.health_calls().len();
    tokio::time::sleep(Duration::from_secs(600)).await;
    assert_eq!(prober.probe_calls(), probes, "no further probes");
    assert_eq!(
        prober.health_calls().len(),
        health,
        "no further health checks"
    );
    assert!(rx.try_recv().is_err(), "no further snapshots");
}

#[tokio::test(start_paused = true)]
async fn downgraded_server_on_reprobe_fails() {
    let old = ConnectFailure::UnsupportedVersion {
        found: "v0.30.1".into(),
        minimum: "v0.31.1".into(),
    };
    let prober = FakeProber::default()
        .probes([Ok(server("v0.31.1")), Err(old.clone())])
        .health([false, true]);
    let (m, mut rx) = manager(prober);
    m.connect(request(&target(), false, false));

    let mut last = next(&mut rx).await;
    while !matches!(last.state, SessionState::Failed { .. }) {
        last = next(&mut rx).await;
    }
    assert_eq!(last.state, SessionState::Failed { failure: old });
}

#[tokio::test(start_paused = true)]
async fn launch_auto_connect_to_unreachable_known_server_goes_offline() {
    let prober = FakeProber::default()
        .probes([unreachable()])
        .health([false]);
    let (m, mut rx) = manager(prober);
    m.connect(request(&target(), true, true));

    assert_eq!(kind(&next(&mut rx).await), "connecting");
    assert_eq!(kind(&next(&mut rx).await), "offline");
}

#[tokio::test(start_paused = true)]
async fn unreachable_otherwise_fails() {
    for (is_launch, before) in [(false, true), (true, false), (false, false)] {
        let prober = FakeProber::default().probes([unreachable()]);
        let (m, mut rx) = manager(prober);
        m.connect(request(&target(), is_launch, before));
        assert_eq!(kind(&next(&mut rx).await), "connecting");
        assert_eq!(
            kind(&next(&mut rx).await),
            "failed",
            "launch={is_launch} before={before}"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn auth_failure_on_first_connect_is_auth_failed() {
    let prober = FakeProber::default().probes([Err(ConnectFailure::ApiKeyRequired)]);
    let (m, mut rx) = manager(prober);
    m.connect(request(&target(), true, true));
    assert_eq!(kind(&next(&mut rx).await), "connecting");
    assert_eq!(kind(&next(&mut rx).await), "authFailed");
}

#[tokio::test(start_paused = true)]
async fn disconnect_returns_to_idle_and_stops_work() {
    let prober = FakeProber::default()
        .probes([Ok(server("v0.31.1"))])
        .health([true]);
    let (m, mut rx) = manager(prober.clone());
    m.connect(request(&target(), false, false));
    assert_eq!(kind(&next(&mut rx).await), "connecting");
    assert_eq!(kind(&next(&mut rx).await), "connected");

    m.disconnect();
    let idle = next(&mut rx).await;
    assert_eq!(kind(&idle), "idle");
    assert_eq!(idle.profile_id, None);
    assert_eq!(m.snapshot(), idle);

    let calls = prober.health_calls().len();
    tokio::time::sleep(Duration::from_secs(60)).await;
    assert_eq!(prober.health_calls().len(), calls, "health loop stopped");
}

#[tokio::test(start_paused = true)]
async fn request_failure_report_moves_connected_to_offline() {
    let prober = FakeProber::default()
        .probes([Ok(server("v0.31.1"))])
        .health([true]);
    let (m, mut rx) = manager(prober);
    m.connect(request(&target(), false, false));
    assert_eq!(kind(&next(&mut rx).await), "connecting");
    assert_eq!(kind(&next(&mut rx).await), "connected");

    m.report_request_failure(ConnectFailure::Unreachable { tried: vec![] });
    assert_eq!(kind(&next(&mut rx).await), "offline");
}

#[tokio::test(start_paused = true)]
async fn each_transition_emits_exactly_one_snapshot() {
    let prober = FakeProber::default()
        .probes([Ok(server("v0.31.1"))])
        .health([true, true, true]);
    let (m, mut rx) = manager(prober);
    m.connect(request(&target(), false, false));
    assert_eq!(kind(&next(&mut rx).await), "connecting");
    assert_eq!(kind(&next(&mut rx).await), "connected");

    // Successful health checks don't emit.
    tokio::time::sleep(Duration::from_secs(14)).await;
    assert!(rx.try_recv().is_err());
}
