//! Shared app state managed by Tauri.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use player::Player;
use stash_core::adapter::StashClient;
use stash_core::connection::manager::{ConnectionManager, ManagerConfig, StashProber};
use stash_core::connection::snapshot::SessionState;
use stash_core::jobs::watcher::JobsWatcher;
use stash_core::profiles::{service, ProfileStore};
use stash_core::shell::notifications::NotificationCenter;
use stash_core::shell::tabs::TabsStore;
use stash_core::AppError;

use crate::cache_commands::{CacheRegistry, ProfileChangeFn};
use tokio_util::sync::CancellationToken;

pub type Manager = ConnectionManager<StashProber>;

pub struct AppState {
    pub profiles: Arc<Mutex<ProfileStore>>,
    /// In-flight cancellable requests, keyed by the UI's request id.
    pub requests: Mutex<HashMap<String, CancellationToken>>,
    pub manager: Manager,
    /// The mpv player (spike 002). `None` if libmpv failed to start; the app still runs.
    pub player: Option<Arc<Player>>,
    /// Per-profile tab sets (004 US2), discardable.
    pub tabs: Arc<TabsStore>,
    /// The notification centre (004 US3), discardable.
    pub notifications: Arc<NotificationCenter>,
    /// Per-profile view caches (003 US1), discardable.
    pub caches: Arc<CacheRegistry>,
}

impl AppState {
    pub fn open(
        profiles_path: &Path,
        tabs_path: &Path,
        notifications_path: &Path,
        cache_root: &Path,
        on_cache_change: ProfileChangeFn,
        runtime: tokio::runtime::Handle,
        player: Option<Arc<Player>>,
    ) -> Result<Self, AppError> {
        let mut store = ProfileStore::open(profiles_path)?;
        if let Some(notice) = store.take_notice() {
            tracing::warn!(?notice, "profile store notice");
        }
        let known: Vec<_> = store.list().iter().map(|p| p.id).collect();
        let profiles = Arc::new(Mutex::new(store));

        let tabs = Arc::new(TabsStore::new(tabs_path));
        if let Err(e) = tabs.prune(&known) {
            tracing::warn!(error = %e, "could not prune saved tabs");
        }

        let manager = ConnectionManager::new(
            StashProber::default(),
            ManagerConfig::default(),
            runtime.clone(),
        );

        // Notifications: connection changes and Stash jobs (004 US3, research R4–R5).
        let notifications = Arc::new(NotificationCenter::open(notifications_path));
        stash_core::connection::watch::spawn(
            manager.subscribe(),
            Arc::clone(&notifications),
            &runtime,
        );
        // Watches the connected server's jobs (read-only); the task below owns it.
        let jobs = Arc::new(JobsWatcher::new(
            Arc::clone(&notifications),
            runtime.clone(),
        ));
        watch_jobs(&manager, Arc::clone(&profiles), jobs, &runtime);

        // View caches, following the connection (003 US1).
        let caches = Arc::new(CacheRegistry::new(
            cache_root.to_path_buf(),
            Arc::clone(&notifications),
            on_cache_change,
        ));
        caches.follow(&manager, &runtime);
        let for_hook = Arc::clone(&profiles);
        manager.on_connected(move |id| {
            if let Err(e) = service::mark_used(&for_hook, id) {
                tracing::warn!(%id, error = %e, "could not record last-used profile");
            }
        });

        Ok(Self {
            profiles,
            requests: Mutex::new(HashMap::new()),
            manager,
            player,
            tabs,
            notifications,
            caches,
        })
    }
}

/// Start watching jobs when a session connects, and stop when it goes down.
fn watch_jobs(
    manager: &Manager,
    profiles: Arc<Mutex<ProfileStore>>,
    jobs: Arc<JobsWatcher>,
    runtime: &tokio::runtime::Handle,
) {
    let mut rx = manager.subscribe();
    runtime.spawn(async move {
        loop {
            let snapshot = match rx.recv().await {
                Ok(s) => s,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            };
            match (&snapshot.state, snapshot.profile_id) {
                (SessionState::Connected, Some(id)) => {
                    let profile = profiles.lock().ok().and_then(|s| s.get(id).cloned());
                    let Some(profile) = profile else { continue };
                    match StashClient::new(profile.base_url, profile.strict_tls, profile.api_key) {
                        Ok(client) => jobs.connected(id, client),
                        Err(e) => tracing::warn!(error = %e, "couldn't build a client for jobs"),
                    }
                }
                (SessionState::Connecting { .. }, _) => {}
                _ => jobs.disconnected(),
            }
        }
    });
}
