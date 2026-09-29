//! Shared app state managed by Tauri.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use player::Player;
use stash_core::connection::manager::{ConnectionManager, ManagerConfig, StashProber};
use stash_core::profiles::{service, ProfileStore};
use stash_core::shell::tabs::TabsStore;
use stash_core::AppError;
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
}

impl AppState {
    pub fn open(
        profiles_path: &Path,
        tabs_path: &Path,
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

        let manager =
            ConnectionManager::new(StashProber::default(), ManagerConfig::default(), runtime);
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
        })
    }
}
