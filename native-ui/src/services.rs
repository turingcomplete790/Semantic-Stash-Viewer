//! The native build's glue to the core (006 research R5).
//!
//! **A copy, on purpose.** These pieces mirror the web build's app glue in `src-tauri`
//! (`state.rs`, `cache_commands.rs`'s `CacheRegistry` and `read_cached`, `player_commands.rs`'s
//! `active_client` and `open_scene`, `thumb_scheme.rs`'s per-profile thumbnail service) so the web
//! build stays untouched (FR-001). The decision record lists what a port would move into a shared
//! crate. Views call these directly as tasks and get typed results back: no commands, no JSON.
//!
//! Read-only (FR-003): no writes to Stash, `profiles.json` is never saved (not even "last used"),
//! and the jobs watcher isn't started.

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use player::{CacheLimits, OpenRequest, Player, PlayerConfig};
use serde::de::DeserializeOwned;
use serde::Serialize;
use stash_core::adapter::scenes::{playable_scene, scene_screenshot_bytes};
use stash_core::adapter::StashClient;
use stash_core::cache::refresh::{RefreshPolicy, Refresher};
use stash_core::cache::{Cached, ViewCache};
use stash_core::connection::manager::{
    ConnectRequest, ConnectionManager, ManagerConfig, StashProber, Target,
};
use stash_core::connection::snapshot::{ConnectionSnapshot, SessionState};
use stash_core::profiles::{ProfileStore, ServerProfile};
use stash_core::scenes::{is_direct_stream, PlayableScene};
use stash_core::shell::notifications::NotificationCenter;
use stash_core::thumbs::{SourceFetch, ThumbService};
use stash_core::AppError;
use uuid::Uuid;

pub type Manager = ConnectionManager<StashProber>;

/// The web build's directory name under each platform directory (`src-tauri/src/lib.rs`).
pub const APP_DIR: &str = "semantic-stash-viewer";

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Where the native build reads and writes (T004). Same directories as the web build (Tauri's
/// `config_dir()`, `local_data_dir()`, `cache_dir()` on Linux, plus [`APP_DIR`]); its own tab
/// and notification files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub config: PathBuf,
    pub data: PathBuf,
    pub cache: PathBuf,
}

impl Paths {
    /// From the XDG variables, falling back to `$HOME` as the XDG spec says.
    pub fn resolve() -> Option<Self> {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let xdg = |var: &str, fallback: &str| -> Option<PathBuf> {
            std::env::var_os(var)
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .or_else(|| home.as_ref().map(|h| h.join(fallback)))
        };
        Some(Self::under(
            &xdg("XDG_CONFIG_HOME", ".config")?,
            &xdg("XDG_DATA_HOME", ".local/share")?,
            &xdg("XDG_CACHE_HOME", ".cache")?,
        ))
    }

    /// The app's directories under the given platform directories.
    pub fn under(config: &Path, data: &Path, cache: &Path) -> Self {
        Self {
            config: config.join(APP_DIR),
            data: data.join(APP_DIR),
            cache: cache.join(APP_DIR),
        }
    }

    pub fn profiles(&self) -> PathBuf {
        self.config.join("profiles.json")
    }

    /// The native build's tab sets (its view state differs from the web build's).
    pub fn tabs(&self) -> PathBuf {
        self.data.join("shell").join("tabs-native.json")
    }

    pub fn notifications(&self) -> PathBuf {
        self.data.join("shell").join("notifications-native.json")
    }

    pub fn logs(&self) -> PathBuf {
        self.data.join("logs")
    }

    /// Per-profile view caches, shared with the web build (used by one build at a time).
    pub fn cache_root(&self) -> PathBuf {
        self.cache.clone()
    }
}

/// Copy of the web build's `CacheRegistry`: one refresher per profile, following the connection.
pub struct CacheRegistry {
    root: PathBuf,
    center: Arc<NotificationCenter>,
    open: Mutex<HashMap<Uuid, Arc<Refresher>>>,
    online: Mutex<Option<Uuid>>,
}

impl CacheRegistry {
    pub fn new(root: PathBuf, center: Arc<NotificationCenter>) -> Self {
        Self {
            root,
            center,
            open: Mutex::new(HashMap::new()),
            online: Mutex::new(None),
        }
    }

    pub fn for_profile(&self, profile: Uuid) -> Option<Arc<Refresher>> {
        let mut open = lock(&self.open);
        if let Some(r) = open.get(&profile) {
            return Some(Arc::clone(r));
        }
        let path = self.root.join(profile.to_string()).join("cache.sqlite3");
        let cache = match ViewCache::open(path) {
            Ok(cache) => cache,
            Err(e) => {
                tracing::warn!(error = %e, "view cache unavailable; reading from the server");
                return None;
            }
        };
        let refresher = Refresher::new(
            profile,
            Arc::new(Mutex::new(cache)),
            Arc::clone(&self.center),
            Arc::new(|_key: &str| {}),
        );
        refresher.set_online(*lock(&self.online) == Some(profile));
        open.insert(profile, Arc::clone(&refresher));
        Some(refresher)
    }

    fn set_online(&self, active: Option<Uuid>, online: bool) {
        *lock(&self.online) = active.filter(|_| online);
        for (id, r) in lock(&self.open).iter() {
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
                        let mut cache = lock(r.cache());
                        let _ = cache.check_identity(&info.identity);
                        if let Err(e) = cache.put("server:info", info) {
                            tracing::warn!(error = %e, "couldn't cache the server summary");
                        }
                    }
                }
                this.set_online(snapshot.profile_id, connected);
            }
        });
    }
}

/// Everything the views need from the core.
pub struct Services {
    pub paths: Paths,
    pub runtime: tokio::runtime::Handle,
    profiles: Mutex<ProfileStore>,
    pub manager: Manager,
    pub caches: Arc<CacheRegistry>,
    pub player: Option<Arc<Player>>,
    thumbs: Mutex<Option<(Uuid, Arc<ThumbService>)>>,
}

impl Services {
    pub fn open(paths: Paths, runtime: tokio::runtime::Handle) -> Result<Arc<Self>, AppError> {
        let mut store = ProfileStore::open(paths.profiles())?;
        if let Some(notice) = store.take_notice() {
            tracing::warn!(?notice, "profile store notice");
        }
        let manager = ConnectionManager::new(
            StashProber::default(),
            ManagerConfig::default(),
            runtime.clone(),
        );
        let notifications = Arc::new(NotificationCenter::open(paths.notifications()));
        let caches = Arc::new(CacheRegistry::new(paths.cache_root(), notifications));
        caches.follow(&manager, &runtime);
        let player = match Player::new(PlayerConfig::render()) {
            Ok(p) => Some(Arc::new(p)),
            Err(e) => {
                tracing::error!(error = %e, "video player unavailable");
                None
            }
        };
        Ok(Arc::new(Self {
            paths,
            runtime,
            profiles: Mutex::new(store),
            manager,
            caches,
            player,
            thumbs: Mutex::new(None),
        }))
    }

    pub fn profiles(&self) -> Vec<ServerProfile> {
        lock(&self.profiles).list().to_vec()
    }

    pub fn profile_named(&self, name: &str) -> Option<ServerProfile> {
        lock(&self.profiles)
            .list()
            .iter()
            .find(|p| p.display_name == name)
            .cloned()
    }

    pub fn last_used(&self) -> Option<ServerProfile> {
        let store = lock(&self.profiles);
        let id = store.last_used_profile_id()?;
        store.get(id).cloned()
    }

    /// Connect (launch auto-connect). Unlike the web build, "last used" is not recorded.
    pub fn connect(&self, profile: &ServerProfile) {
        if let Some(p) = &self.player {
            p.close();
        }
        self.manager.connect(ConnectRequest {
            target: Target::from(profile),
            is_launch: true,
            has_connected_before: profile.last_used_at.is_some(),
        });
    }

    pub fn snapshot(&self) -> ConnectionSnapshot {
        self.manager.snapshot()
    }

    /// The active profile, or the last-used one while disconnected.
    pub fn current_profile(&self) -> Option<Uuid> {
        self.manager
            .active_profile_id()
            .or_else(|| lock(&self.profiles).last_used_profile_id())
    }

    /// Clear the profile's view cache (thumbnails included), for harness runs.
    pub fn clear_cache(&self, profile: Uuid) {
        if let Some(r) = self.caches.for_profile(profile) {
            if let Err(e) = lock(r.cache()).clear() {
                tracing::warn!(error = %e, "couldn't clear the cache for the harness run");
            }
        }
    }

    /// A client for the active profile, its key, and its TLS setting.
    pub fn active_client(&self) -> Result<(StashClient, Option<String>, bool), AppError> {
        let id = self
            .manager
            .active_profile_id()
            .ok_or(AppError::NotConnected)?;
        let profile = lock(&self.profiles)
            .get(id)
            .cloned()
            .ok_or(AppError::ProfileNotFound { id })?;
        let client = StashClient::new(
            profile.base_url.clone(),
            profile.strict_tls,
            profile.api_key.clone(),
        )?;
        Ok((client, profile.api_key, profile.strict_tls))
    }

    /// Copy of the web build's `read_cached`.
    pub async fn read_cached<T, F, Fut>(
        &self,
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
        let client = self.active_client().map(|(client, _, _)| client);
        let Some((profile, r)) = self
            .current_profile()
            .and_then(|id| self.caches.for_profile(id).map(|r| (id, r)))
        else {
            return Ok(Cached::fresh(fetch(client?).await?));
        };
        let snapshot = self.manager.snapshot();
        r.set_online(
            matches!(snapshot.state, SessionState::Connected)
                && snapshot.profile_id == Some(profile),
        );
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

    pub fn player(&self) -> Result<&Arc<Player>, AppError> {
        self.player.as_ref().ok_or_else(|| AppError::Internal {
            message: "the video player isn't available (libmpv failed to start; see the log)"
                .into(),
        })
    }

    /// Copy of the web build's `open_scene`: look up the scene and play its direct stream.
    pub async fn open_scene(&self, scene_id: &str) -> Result<PlayableScene, AppError> {
        let player = Arc::clone(self.player()?);
        let (_, api_key, strict_tls) = self.active_client()?;
        if !matches!(self.manager.snapshot().state, SessionState::Connected) {
            return Err(AppError::NotConnected);
        }
        let id = scene_id.trim().to_owned();
        let scene = self
            .read_cached(
                &format!("scene:{id}"),
                RefreshPolicy::Auto,
                false,
                move |client| {
                    let id = id.clone();
                    async move { playable_scene(&client, &id).await }
                },
            )
            .await?
            .data;
        if !is_direct_stream(&scene.stream_url) {
            return Err(AppError::Internal {
                message: "refusing to play a non-direct stream".into(),
            });
        }
        let cache = scene.seek_cache().map(|c| CacheLimits {
            forward_bytes: c.forward_bytes,
            back_bytes: c.back_bytes,
        });
        tracing::info!(scene = %scene.id, "opening scene");
        player.open(OpenRequest {
            source: scene.stream_url.to_string(),
            scene_id: Some(scene.id.clone()),
            title: Some(scene.title.clone()),
            api_key,
            strict_tls,
            cache,
        });
        Ok(scene)
    }

    /// The current profile's thumbnail service (one per profile, so its limit covers every
    /// request), as `src-tauri/src/thumb_scheme.rs`.
    pub fn thumbs(self: &Arc<Self>) -> Option<Arc<ThumbService>> {
        let profile = self.current_profile()?;
        let mut current = lock(&self.thumbs);
        if let Some((id, service)) = current.as_ref() {
            if *id == profile {
                return Some(Arc::clone(service));
            }
        }
        let refresher = self.caches.for_profile(profile)?;
        let this = Arc::downgrade(self);
        let fetch: SourceFetch = Arc::new(move |id: String| {
            let this = this.clone();
            Box::pin(async move {
                let client = this
                    .upgrade()
                    .ok_or(AppError::NotConnected)?
                    .active_client()
                    .map(|(client, _, _)| client)?;
                scene_screenshot_bytes(&client, &id).await
            })
        });
        let service = Arc::new(ThumbService::new(Arc::clone(refresher.cache()), fetch));
        *current = Some((profile, Arc::clone(&service)));
        Some(service)
    }
}
