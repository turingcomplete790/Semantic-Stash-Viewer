//! The view cache in the app (003 US1; contracts/cache-and-harness.md).
//!
//! One `Refresher` (over a `ViewCache` file) per server profile, opened lazily in the platform
//! cache directory. The connection decides when refreshes may run (online), and each connect
//! checks the server identity and stores the latest server summary. Cache problems never block a
//! read: without a cache, commands read from the server as before.

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::de::DeserializeOwned;
use serde::Serialize;
use stash_core::adapter::StashClient;
use stash_core::cache::refresh::{RefreshPolicy, Refresher};
use stash_core::cache::{delete_profile_cache, Cached, ViewCache};
use stash_core::connection::snapshot::SessionState;
use stash_core::connection::ServerInfo;
use stash_core::shell::notifications::NotificationCenter;
use stash_core::AppError;
use tauri::{AppHandle, State};
use tauri_specta::Event;
use uuid::Uuid;

use crate::state::{AppState, Manager};

/// A cached view's data changed (a refresh returned something different, or the cache was
/// cleared). `key` is `*` when everything changed. The UI re-reads the matching command.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "view-data-changed")]
#[serde(rename_all = "camelCase")]
pub struct ViewDataChangedEvent {
    pub profile_id: Uuid,
    pub key: String,
}

/// Called with (profile, key) when cached data changes.
pub type ProfileChangeFn = Arc<dyn Fn(Uuid, &str) + Send + Sync>;

pub struct CacheRegistry {
    root: PathBuf,
    center: Arc<NotificationCenter>,
    on_change: ProfileChangeFn,
    open: Mutex<HashMap<Uuid, Arc<Refresher>>>,
    /// The profile currently connected, if any (new refreshers start in step with it).
    online: Mutex<Option<Uuid>>,
}

impl CacheRegistry {
    pub fn new(root: PathBuf, center: Arc<NotificationCenter>, on_change: ProfileChangeFn) -> Self {
        Self {
            root,
            center,
            on_change,
            open: Mutex::new(HashMap::new()),
            online: Mutex::new(None),
        }
    }

    fn dir(&self, profile: Uuid) -> PathBuf {
        self.root.join(profile.to_string())
    }

    /// The profile's refresher, opening its cache file on first use. `None` if the cache can't
    /// be opened at all (reads then go straight to the server).
    pub fn for_profile(&self, profile: Uuid) -> Option<Arc<Refresher>> {
        let mut open = self
            .open
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(r) = open.get(&profile) {
            return Some(Arc::clone(r));
        }
        let cache = match ViewCache::open(self.dir(profile).join("cache.sqlite3")) {
            Ok(cache) => cache,
            Err(e) => {
                tracing::warn!(error = %e, "view cache unavailable; reading from the server");
                return None;
            }
        };
        let on_change = Arc::clone(&self.on_change);
        let refresher = Refresher::new(
            profile,
            Arc::new(Mutex::new(cache)),
            Arc::clone(&self.center),
            Arc::new(move |key: &str| on_change(profile, key)),
        );
        let online = *self
            .online
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        refresher.set_online(online == Some(profile));
        open.insert(profile, Arc::clone(&refresher));
        Some(refresher)
    }

    /// Forget a deleted profile's cache (FR-009).
    pub fn delete(&self, profile: Uuid) {
        self.open
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&profile);
        delete_profile_cache(&self.dir(profile));
    }

    fn set_online(&self, active: Option<Uuid>, online: bool) {
        *self
            .online
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = active.filter(|_| online);
        let open = self
            .open
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (id, r) in open.iter() {
            r.set_online(online && Some(*id) == active);
        }
    }

    /// Follow the connection: online state, the identity check, and the server summary.
    pub fn follow(self: &Arc<Self>, manager: &Manager, runtime: &tokio::runtime::Handle) {
        let mut rx = manager.subscribe();
        let this = Arc::clone(self);
        runtime.spawn(async move {
            loop {
                let snapshot = match rx.recv().await {
                    Ok(s) => s,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                };
                let connected = matches!(snapshot.state, SessionState::Connected);
                if let (true, Some(id), Some(info)) =
                    (connected, snapshot.profile_id, snapshot.server.as_ref())
                {
                    if let Some(r) = this.for_profile(id) {
                        let wiped = {
                            let mut cache = r.cache().lock().unwrap_or_else(|e| e.into_inner());
                            let wiped = cache.check_identity(&info.identity).unwrap_or(false);
                            if let Err(e) = cache.put("server:info", info) {
                                tracing::warn!(error = %e, "couldn't cache the server summary");
                            }
                            wiped
                        };
                        if wiped {
                            (this.on_change)(id, "*");
                        }
                    }
                }
                this.set_online(snapshot.profile_id, connected);
            }
        });
    }
}

/// The active profile, or the last-used one while disconnected (for Settings and Home).
pub(crate) fn current_profile(state: &AppState) -> Option<Uuid> {
    state.manager.active_profile_id().or_else(|| {
        state
            .profiles
            .lock()
            .ok()
            .and_then(|s| s.last_used_profile_id())
    })
}

/// Read `key` through the current profile's cache: cached data shows even while offline, and
/// without a cache (or a connection) it falls back to reading the server. `force` fetches now.
pub(crate) async fn read_cached<T, F, Fut>(
    state: &AppState,
    key: &str,
    policy: RefreshPolicy,
    force: bool,
    fetch: F,
) -> Result<Cached<T>, AppError>
where
    T: Serialize + DeserializeOwned + Send + 'static,
    F: Fn(StashClient) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<T, AppError>> + Send + 'static,
{
    let client = crate::player_commands::active_client(state).map(|(client, _, _)| client);
    let Some(r) = current_profile(state).and_then(|id| state.caches.for_profile(id)) else {
        return Ok(Cached::fresh(fetch(client?).await?));
    };
    let client = client.ok();
    let fetcher = move || {
        let pending = client.clone().map(&fetch);
        async move {
            match pending {
                Some(fut) => fut.await,
                None => Err(AppError::NotConnected),
            }
        }
    };
    if force {
        r.fetch_now(key, fetcher).await
    } else {
        r.get_or_fetch(key, policy, fetcher).await
    }
}

/// The last known server summary. (A named type: the bindings generator mangles
/// `Option<Cached<ServerInfo>>`.)
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LastServerInfo {
    pub server: ServerInfo,
    /// When it was read from the server (ISO 8601).
    pub fetched_at: String,
}

/// The last known server summary, for Home while connecting or offline (research R6).
#[tauri::command]
#[specta::specta]
pub fn cached_server_info(state: State<'_, AppState>) -> Option<LastServerInfo> {
    let r = state.caches.for_profile(current_profile(&state)?)?;
    let cache = r
        .cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    cache
        .get::<ServerInfo>("server:info")
        .map(|c| LastServerInfo {
            server: c.data,
            fetched_at: c.fetched_at,
        })
}

/// The cache's size in bytes (Settings → Troubleshooting). A float because the bindings have
/// no 64-bit integers; sizes stay exact far past any cache limit.
#[tauri::command]
#[specta::specta]
#[allow(clippy::cast_precision_loss)]
pub fn cache_size(state: State<'_, AppState>) -> f64 {
    current_profile(&state)
        .and_then(|id| state.caches.for_profile(id))
        .map_or(0.0, |r| {
            r.cache()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .size_bytes() as f64
        })
}

/// Clear the cache (FR-005). Returns the bytes freed; open views reload from the server.
#[tauri::command]
#[specta::specta]
#[allow(clippy::cast_precision_loss)]
pub fn clear_cache(app: AppHandle, state: State<'_, AppState>) -> Result<f64, AppError> {
    let Some(id) = current_profile(&state) else {
        return Ok(0.0);
    };
    let Some(r) = state.caches.for_profile(id) else {
        return Ok(0.0);
    };
    let freed = r
        .cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear()?;
    let _ = ViewDataChangedEvent {
        profile_id: id,
        key: "*".into(),
    }
    .emit(&app);
    Ok(freed as f64)
}
