//! The native app's service layer (007 research R1): profiles, the connection, view caches,
//! thumbnails, notifications, Stash jobs, and the player. The state machine never calls these
//! directly; its effects do (`effects.rs`), off the UI thread, returning typed results.

pub mod cache;
pub mod paths;

use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use player::{CacheLimits, OpenRequest, Player, PlayerConfig};
use serde::de::DeserializeOwned;
use serde::Serialize;
use stash_core::adapter::scenes::{playable_scene, scene_screenshot_bytes};
use stash_core::adapter::StashClient;
use stash_core::cache::refresh::RefreshPolicy;
use stash_core::cache::Cached;
use stash_core::connection::connect::ConnectOptions;
use stash_core::connection::manager::{
    ConnectRequest, ConnectionManager, ManagerConfig, StashProber, Target,
};
use stash_core::connection::snapshot::{ConnectionSnapshot, SessionState};
use stash_core::jobs::watcher::JobsWatcher;
use stash_core::profiles::{service, ProfileDraft, ProfileStore, ServerProfile};
use stash_core::scenes::{is_direct_stream, PlayableScene};
use stash_core::shell::notifications::NotificationCenter;
use stash_core::thumbs::{SourceFetch, ThumbService};
use stash_core::AppError;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub use cache::{CacheChange, CacheRegistry};
pub use paths::{Paths, APP_ID};

pub type Manager = ConnectionManager<StashProber>;

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Everything the effects need from the core.
pub struct Services {
    pub paths: Paths,
    pub runtime: tokio::runtime::Handle,
    profiles: Arc<Mutex<ProfileStore>>,
    pub manager: Manager,
    pub caches: Arc<CacheRegistry>,
    pub notifications: Arc<NotificationCenter>,
    pub player: Option<Arc<Player>>,
    thumbs: Mutex<Option<(Uuid, Arc<ThumbService>)>>,
}

impl Services {
    pub fn open(paths: Paths, runtime: tokio::runtime::Handle) -> Result<Arc<Self>, AppError> {
        let mut store = ProfileStore::open(paths.profiles())?;
        if let Some(notice) = store.take_notice() {
            tracing::warn!(?notice, "profile store notice");
        }
        let profiles = Arc::new(Mutex::new(store));
        let manager = ConnectionManager::new(
            StashProber::default(),
            ManagerConfig::default(),
            runtime.clone(),
        );
        let notifications = Arc::new(NotificationCenter::open(paths.notifications()));
        // Connection alerts in the notification centre (004 US3).
        stash_core::connection::watch::spawn(
            manager.subscribe(),
            Arc::clone(&notifications),
            &runtime,
        );
        let caches = Arc::new(CacheRegistry::new(
            paths.cache_root(),
            Arc::clone(&notifications),
        ));
        caches.follow(&manager, &runtime);
        // "Last used" is what reconnects at the next launch (001 US2).
        let for_hook = Arc::clone(&profiles);
        manager.on_connected(move |id| {
            if let Err(e) = service::mark_used(&for_hook, id) {
                tracing::warn!(%id, error = %e, "could not record the last-used server");
            }
        });
        let jobs = Arc::new(JobsWatcher::new(
            Arc::clone(&notifications),
            runtime.clone(),
        ));
        watch_jobs(&manager, Arc::clone(&profiles), jobs, &runtime);
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
            profiles,
            manager,
            caches,
            notifications,
            player,
            thumbs: Mutex::new(None),
        }))
    }

    pub fn profiles(&self) -> Vec<ServerProfile> {
        lock(&self.profiles).list().to_vec()
    }

    pub fn profile(&self, id: Uuid) -> Option<ServerProfile> {
        lock(&self.profiles).get(id).cloned()
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

    /// Make `profile` the active server and connect (at launch or on a switch).
    pub fn connect(&self, profile: &ServerProfile, is_launch: bool) {
        if let Some(p) = &self.player {
            p.close();
        }
        self.manager.connect(ConnectRequest {
            target: Target::from(profile),
            is_launch,
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

    /// Clear the profile's view cache (thumbnails included); returns the bytes freed.
    pub fn clear_cache(&self, profile: Uuid) -> Result<u64, AppError> {
        self.caches.clear(profile)
    }

    /// Test a new server and save it (001 US1, US4). The core checks the address, the key, and
    /// the version before anything is saved.
    pub async fn create_profile(&self, draft: ProfileDraft) -> Result<ServerProfile, AppError> {
        let cancel = CancellationToken::new();
        service::create_profile(&self.profiles, &draft, &cancel, ConnectOptions::default())
            .await
            .map(|(profile, _)| profile)
    }

    /// Change a saved server; `force` saves even when the check fails.
    pub async fn update_profile(
        &self,
        id: Uuid,
        draft: ProfileDraft,
        force: bool,
    ) -> Result<ServerProfile, AppError> {
        let cancel = CancellationToken::new();
        service::update_profile(
            &self.profiles,
            id,
            &draft,
            force,
            &cancel,
            ConnectOptions::default(),
        )
        .await
    }

    pub fn delete_profile(&self, id: Uuid) -> Result<ServerProfile, AppError> {
        if self.manager.active_profile_id() == Some(id) {
            self.manager.disconnect();
        }
        let removed = service::delete_profile(&self.profiles, id)?;
        if let Err(e) = self.caches.clear(id) {
            tracing::warn!(error = %e, "couldn't clear a deleted server's cache");
        }
        Ok(removed)
    }

    pub fn reorder_profiles(&self, ids: &[Uuid]) -> Result<(), AppError> {
        service::reorder_profiles(&self.profiles, ids)
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

    /// Read `key` through the current profile's cache: cached data shows even while offline, and
    /// without a cache it reads the server. `force` fetches now.
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
        // The connection is the authority on whether we're online (003).
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

    /// Look up a scene and play its direct stream (never a transcode, Principle V).
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

    /// The current profile's thumbnail service (one per profile, so its concurrency limit covers
    /// every request).
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

/// Watch the connected server's Stash jobs (004 US3), following the session.
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
                    let profile = lock(&profiles).get(id).cloned();
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
