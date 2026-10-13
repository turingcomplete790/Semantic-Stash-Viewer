//! The native app's service layer (007 research R1): profiles, the connection, view caches,
//! thumbnails, notifications, Stash jobs, and the player. The state machine never calls these
//! directly; its effects do (`effects.rs`), off the UI thread, returning typed results.

pub mod cache;
pub mod paths;
pub mod settings;

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
use stash_core::images::neighbours::Neighbours;
use stash_core::jobs::watcher::JobsWatcher;
use stash_core::playback::history::{flush, FlushOutcome, HistoryQueue, PlayHistoryReport};
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
    /// Play history waiting for Stash, on disk (008 contracts/play-history.md).
    pub history: Arc<Mutex<HistoryQueue>>,
    /// The card preview generation being decoded (0: none).
    preview_wanted: std::sync::atomic::AtomicU64,
    /// The image viewer's neighbour files, for the current profile (012 R4).
    neighbours: Mutex<Option<(Uuid, Arc<Neighbours>)>>,
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
        let history = Arc::new(Mutex::new(HistoryQueue::open(paths.play_history())));
        Ok(Arc::new(Self {
            paths,
            runtime,
            profiles,
            manager,
            caches,
            notifications,
            player,
            thumbs: Mutex::new(None),
            history,
            preview_wanted: std::sync::atomic::AtomicU64::new(0),
            neighbours: Mutex::new(None),
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

    /// Clear the profile's view cache (thumbnails and the viewer's files included); returns the
    /// bytes freed.
    pub fn clear_cache(&self, profile: Uuid) -> Result<u64, AppError> {
        let files = match lock(&self.neighbours).as_ref() {
            Some((id, n)) if *id == profile => n.clear(),
            _ => 0,
        };
        Ok(self.caches.clear(profile)? + files)
    }

    /// The profile's cache size, the viewer's files included.
    pub fn cache_size(&self, profile: Uuid) -> u64 {
        let files = match lock(&self.neighbours).as_ref() {
            Some((id, n)) if *id == profile => n.size_bytes(),
            _ => 0,
        };
        self.caches.size(profile) + files
    }

    /// The current profile's neighbour files (in its cache directory, `images/`).
    pub fn neighbours(&self) -> Option<Arc<Neighbours>> {
        let profile = self.current_profile()?;
        let mut current = lock(&self.neighbours);
        if let Some((id, n)) = current.as_ref() {
            if *id == profile {
                return Some(Arc::clone(n));
            }
        }
        let dir = self
            .paths
            .cache_root()
            .join(profile.to_string())
            .join("images");
        let n = Arc::new(Neighbours::open(dir));
        *current = Some((profile, Arc::clone(&n)));
        Some(n)
    }

    /// Show an image in the player (012 R3, R4): its file on disk when a neighbour fetch has it,
    /// otherwise Stash's `/image/<id>/image` with the key in a header (fetched to disk alongside,
    /// so the way back is local). `looped`: it animates or is a clip.
    pub async fn open_image(&self, id: &str, title: &str, looped: bool) -> Result<(), AppError> {
        let player = Arc::clone(self.player()?);
        let (client, api_key, strict_tls) = self.active_client()?;
        let neighbours = self.neighbours();
        let local = neighbours.as_ref().and_then(|n| n.ready(id));
        let (source, api_key) = match local {
            Some(path) => (path.to_string_lossy().into_owned(), None),
            None => {
                if !matches!(self.manager.snapshot().state, SessionState::Connected) {
                    return Err(AppError::NotConnected);
                }
                let mut url = client.endpoint("image");
                if let Ok(mut segments) = url.path_segments_mut() {
                    segments.push(id).push("image");
                }
                if let Some(n) = neighbours {
                    let id = id.to_owned();
                    self.runtime.spawn(async move {
                        if let Err(e) = n.fetch(&client, &id).await {
                            tracing::debug!(error = %e, "image file not kept");
                        }
                    });
                }
                (url.to_string(), api_key)
            }
        };
        tracing::info!(image = %id, local = api_key.is_none(), "opening image");
        player.open(OpenRequest {
            source,
            scene_id: None,
            title: Some(title.to_owned()),
            api_key,
            strict_tls,
            cache: None,
            start_seconds: None,
            transcoded: false,
            kind: player::MediaKind::Image { looped },
        });
        Ok(())
    }

    /// Fetch the viewer's neighbours' files to disk, in order (012 R4).
    pub async fn prefetch_image_files(&self, ids: Vec<String>) {
        let (Some(n), Ok((client, _, _))) = (self.neighbours(), self.active_client()) else {
            return;
        };
        for id in ids {
            if let Err(e) = n.fetch(&client, &id).await {
                tracing::debug!(error = %e, "neighbour file not fetched");
            }
        }
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
        let mut client = StashClient::new(
            profile.base_url.clone(),
            profile.strict_tls,
            profile.api_key.clone(),
        )?;
        if let Some(log) = crate::measure::request_log() {
            client = client.with_recorder(log);
        }
        Ok((client, profile.api_key, profile.strict_tls))
    }

    /// A client for any saved server (a server switch flushes the old server's play history).
    pub fn client_for(&self, id: Uuid) -> Result<StashClient, AppError> {
        let profile = lock(&self.profiles)
            .get(id)
            .cloned()
            .ok_or(AppError::ProfileNotFound { id })?;
        let mut client = StashClient::new(profile.base_url, profile.strict_tls, profile.api_key)?;
        if let Some(log) = crate::measure::request_log() {
            client = client.with_recorder(log);
        }
        Ok(client)
    }

    /// Queue a play history report for `profile`: in memory at once (no disk on the UI thread),
    /// written to disk by the flush that follows, before anything is sent.
    pub fn queue_play_history(&self, profile: Uuid, report: PlayHistoryReport) {
        lock(&self.history).stage(&profile.to_string(), report);
    }

    /// Send `profile`'s queued play history in order (until `deadline`, if any). Each confirmed
    /// report patches that scene's cached page (resume point, playing time, play count), so it
    /// shows the new values without a request.
    pub async fn flush_play_history(
        &self,
        profile: Uuid,
        deadline: Option<std::time::Instant>,
    ) -> FlushOutcome {
        // Written before anything is sent (contracts/play-history.md).
        let history = Arc::clone(&self.history);
        let written = tokio::task::spawn_blocking(move || lock(&history).persist()).await;
        if let Ok(Err(e)) = written {
            tracing::warn!(error = %e, "couldn't write the play history queue");
        }
        let client = match self.client_for(profile) {
            Ok(c) => c,
            Err(e) => {
                return FlushOutcome {
                    sent: 0,
                    failed: Some(e.to_string()),
                    left: lock(&self.history).pending(&profile.to_string()).len(),
                }
            }
        };
        let caches = self.caches.for_profile(profile);
        flush(&self.history, &profile.to_string(), deadline, |report| {
            let client = client.clone();
            let caches = caches.clone();
            async move {
                let count = stash_core::adapter::activity::send_report(&client, &report).await?;
                if let Some(r) = caches {
                    let key = stash_core::cache::keys::scene_details(&report.scene_id);
                    let mut cache = lock(r.cache());
                    if let Some(mut cached) = cache.get::<stash_core::scenes::SceneDetails>(&key) {
                        stash_core::adapter::activity::patch_details(
                            &mut cached.data,
                            &report,
                            count,
                        );
                        if let Err(e) = cache.put(&key, &cached.data) {
                            tracing::warn!(error = %e, "couldn't patch the cached scene page");
                        }
                    }
                }
                Ok(())
            }
        })
        .await
    }

    /// The resume point cached with a scene's page (no request; patched after each save).
    fn cached_resume_point(&self, scene_id: &str) -> Option<f64> {
        let r = self
            .current_profile()
            .and_then(|id| self.caches.for_profile(id))?;
        let key = stash_core::cache::keys::scene_details(scene_id);
        let details = lock(r.cache()).get::<stash_core::scenes::SceneDetails>(&key)?;
        details.data.resume_time.filter(|t| *t > 0.0)
    }

    /// The last server summary read for the current profile, or the live connection's.
    pub fn cached_server_info(&self) -> Option<stash_core::connection::ServerInfo> {
        let cached = self
            .current_profile()
            .and_then(|id| self.caches.for_profile(id))
            .and_then(|r| lock(r.cache()).get("server:info").map(|c| c.data));
        cached.or_else(|| self.manager.snapshot().server)
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

    /// Save the tag colour rules (the one settings write, 008 FR-003g) and patch the cached UI
    /// settings with what Stash now holds, so no read follows.
    pub async fn save_tag_colours(
        &self,
        rules: Vec<stash_core::semantic::ColourRule>,
    ) -> Result<stash_core::adapter::filters::UiSettings, AppError> {
        let (client, _, _) = self.active_client()?;
        let settings = stash_core::adapter::filters::save_tag_colours(&client, &rules).await?;
        if let Some(r) = self
            .current_profile()
            .and_then(|id| self.caches.for_profile(id))
        {
            if let Err(e) = lock(r.cache()).put(stash_core::cache::keys::UI_SETTINGS, &settings) {
                tracing::warn!(error = %e, "couldn't update the cached UI settings");
            }
        }
        Ok(settings)
    }

    pub fn player(&self) -> Result<&Arc<Player>, AppError> {
        self.player.as_ref().ok_or_else(|| AppError::Internal {
            message: "the video player isn't available (libmpv failed to start; see the log)"
                .into(),
        })
    }

    /// Look up a scene and play its direct stream (never a transcode, Principle V). Without a
    /// `start`, and with `resume`, it starts at the scene's cached resume point (008 FR-012);
    /// returns the scene and that resume point when one was used.
    pub async fn open_scene(
        &self,
        scene_id: &str,
        start: Option<f64>,
        resume: bool,
    ) -> Result<(PlayableScene, Option<f64>), AppError> {
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
        let resumed = match start {
            None if resume => {
                player::start_position(self.cached_resume_point(&scene.id), scene.duration_seconds)
            }
            _ => None,
        };
        tracing::info!(scene = %scene.id, resumed = resumed.is_some(), "opening scene");
        player.open(OpenRequest {
            source: scene.stream_url.to_string(),
            scene_id: Some(scene.id.clone()),
            title: Some(scene.title.clone()),
            api_key,
            strict_tls,
            cache,
            start_seconds: player::start_position(start.or(resumed), scene.duration_seconds),
            transcoded: false,
            kind: player::MediaKind::Video,
        });
        Ok((scene, resumed))
    }

    /// Play Stash's MP4 transcode of a scene, after its direct stream couldn't be decoded and the
    /// user chose to (008 R10). `reason` is mpv's, for the log (no title or path there).
    pub async fn open_transcoded(&self, scene_id: &str, reason: &str) -> Result<(), AppError> {
        let player = Arc::clone(self.player()?);
        let (client, api_key, strict_tls) = self.active_client()?;
        let id = scene_id.trim().to_owned();
        let title = self
            .current_profile()
            .and_then(|p| self.caches.for_profile(p))
            .and_then(|r| lock(r.cache()).get::<PlayableScene>(&format!("scene:{id}")))
            .map(|c| c.data.title);
        let url = stash_core::adapter::scenes::transcode_url(client.base_url(), &id);
        tracing::info!(scene = %id, %reason, "playing the server's transcode");
        player.open(OpenRequest {
            source: url.to_string(),
            scene_id: Some(id),
            title,
            api_key,
            strict_tls,
            cache: None,
            start_seconds: None,
            transcoded: true,
            kind: player::MediaKind::Video,
        });
        Ok(())
    }

    /// A scene's cover (its screenshot or uploaded cover) through the current profile's cache
    /// (`cover:<id>`); `None` when Stash has none or it can't be read. Read-only.
    pub async fn scene_cover(&self, id: &str) -> Option<Vec<u8>> {
        let key = format!("cover:{id}");
        let refresher = self
            .current_profile()
            .and_then(|profile| self.caches.for_profile(profile));
        if let Some(bytes) = refresher
            .as_ref()
            .and_then(|r| lock(r.cache()).get_bytes(&key))
        {
            return Some(bytes);
        }
        let (client, _, _) = self.active_client().ok()?;
        let (bytes, _type) = stash_core::adapter::scenes::scene_screenshot_bytes(&client, id)
            .await
            .ok()
            .flatten()?;
        if let Some(r) = refresher {
            if let Err(e) = lock(r.cache()).put_bytes(&key, &bytes) {
                tracing::warn!(error = %e, "couldn't cache a cover");
            }
        }
        Some(bytes)
    }

    /// Bytes through the current profile's disk cache under `key`, fetched with `fetch` on a miss.
    async fn cached_bytes<F, Fut>(&self, key: &str, fetch: F) -> Result<Option<Vec<u8>>, AppError>
    where
        F: FnOnce(StashClient) -> Fut,
        Fut: Future<Output = Result<Option<Vec<u8>>, AppError>>,
    {
        let refresher = self
            .current_profile()
            .and_then(|profile| self.caches.for_profile(profile));
        if let Some(bytes) = refresher
            .as_ref()
            .and_then(|r| lock(r.cache()).get_bytes(key))
        {
            return Ok(Some(bytes));
        }
        let (client, _, _) = self.active_client()?;
        let Some(bytes) = fetch(client).await? else {
            return Ok(None);
        };
        if let Some(r) = refresher {
            if let Err(e) = lock(r.cache()).put_bytes(key, &bytes) {
                tracing::warn!(error = %e, "couldn't cache {key}");
            }
        }
        Ok(Some(bytes))
    }

    /// A scene's animated preview file (008 FR-019): one GET the first time, then the disk
    /// cache. `None` when Stash has no preview.
    pub async fn preview_file(&self, scene_id: &str) -> Result<Option<Vec<u8>>, AppError> {
        let id = scene_id.to_owned();
        let key = stash_core::cache::keys::preview(scene_id);
        self.cached_bytes(&key, |client| async move {
            stash_core::previews::preview_bytes(&client, &id).await
        })
        .await
    }

    /// The preview that may decode now; others stop at their next frame (the pointer left).
    pub fn want_preview(&self, generation: Option<u64>) {
        self.preview_wanted
            .store(generation.unwrap_or(0), std::sync::atomic::Ordering::SeqCst);
    }

    pub fn preview_wanted(&self, generation: u64) -> bool {
        self.preview_wanted
            .load(std::sync::atomic::Ordering::SeqCst)
            == generation
    }

    /// A scene's sprite sheet and index for the seek bar (008 FR-021): two GETs the first time,
    /// then the disk cache; decoded off the UI thread. `None` when Stash hasn't generated them.
    pub async fn sprite_sheet(
        &self,
        scene_id: &str,
    ) -> Result<Option<stash_core::sprites::SpriteSheet>, AppError> {
        let key = stash_core::cache::keys::sprite(scene_id);
        let vtt_key = format!("{key}:vtt");
        let refresher = self
            .current_profile()
            .and_then(|profile| self.caches.for_profile(profile));
        let cached = refresher.as_ref().and_then(|r| {
            let cache = lock(r.cache());
            Some((cache.get_bytes(&vtt_key)?, cache.get_bytes(&key)?))
        });
        let (vtt, sheet) = match cached {
            Some((vtt, sheet)) => (String::from_utf8_lossy(&vtt).into_owned(), sheet),
            None => {
                let (client, _, _) = self.active_client()?;
                let Some((vtt, sheet)) =
                    stash_core::sprites::sprite_sources(&client, scene_id).await?
                else {
                    return Ok(None);
                };
                if let Some(r) = refresher {
                    let mut cache = lock(r.cache());
                    if let Err(e) = cache
                        .put_bytes(&vtt_key, vtt.as_bytes())
                        .and_then(|()| cache.put_bytes(&key, &sheet))
                    {
                        tracing::warn!(error = %e, "couldn't cache a sprite sheet");
                    }
                }
                (vtt, sheet)
            }
        };
        let decoded =
            tokio::task::spawn_blocking(move || stash_core::sprites::decode_sheet(&vtt, &sheet))
                .await
                .map_err(|e| AppError::Internal {
                    message: e.to_string(),
                })?
                .map_err(|message| AppError::Internal { message })?;
        Ok(Some(decoded))
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
        let fetch: SourceFetch = Arc::new(move |kind: &'static str, id: String| {
            let this = this.clone();
            Box::pin(async move {
                let client = this
                    .upgrade()
                    .ok_or(AppError::NotConnected)?
                    .active_client()
                    .map(|(client, _, _)| client)?;
                match kind {
                    "performer" => {
                        stash_core::adapter::scenes::performer_image_bytes(&client, &id).await
                    }
                    "image" => {
                        stash_core::adapter::scenes::image_thumbnail_bytes(&client, &id).await
                    }
                    _ => scene_screenshot_bytes(&client, &id).await,
                }
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
