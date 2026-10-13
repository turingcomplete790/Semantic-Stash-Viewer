//! Effects: the only way work leaves the state machine (007 research R2).
//!
//! A transition returns [`Effect`] values (data). [`run`] turns each into an iced `Task` over the
//! service layer, where the core fetches and deserializes off the UI thread, and the typed result
//! comes back as a [`Message`] addressed to the state that asked. `update` therefore never does
//! I/O, and tests assert on effects without running anything.

use std::sync::Arc;

use iced::{window, Task};
use stash_core::cache::refresh::RefreshPolicy;
use stash_core::connection::ServerInfo;
use stash_core::profiles::{ProfileDraft, ServerProfile};
use stash_core::scenes::query::SceneQuery;
use stash_core::scenes::ScenePage;
use stash_core::AppError;
use uuid::Uuid;

use crate::app::Message;
use crate::player::controls::Direction;
use crate::screens::scenes::page_key;
use crate::screens::scenes::thumbs::{key_of, thumb_key, ThumbKey, WARM_LIMIT};
use crate::services::Services;
use crate::session::snapshot::{self, SavedSession};
use crate::shell::TabId;

/// How long after the last change the session is saved.
pub const SAVE_DELAY: std::time::Duration = std::time::Duration::from_millis(500);

/// A player command (the 006 controls); sent to mpv by the effect runner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayerAction {
    TogglePause,
    SeekRelative(f64),
    Seek { seconds: f64, exact: bool },
    SetVolume(f64),
    SetMuted(bool),
    SetSpeed(f64),
    FrameStep(Direction),
    Replay,
    Close,
}

/// A change to the notification centre, asked for by the UI.
#[derive(Debug, Clone, PartialEq)]
pub enum NotificationAction {
    MarkRead(Vec<String>),
    Dismiss(String),
    DismissAll,
}

/// Work a transition asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Make a saved server active and connect (at launch, or a switch).
    Connect {
        profile: Uuid,
        launch: bool,
    },
    /// Test a new server and save it; the result goes back to onboarding.
    CreateProfile(ProfileDraft),
    /// Re-test and save a changed server (a new API key from the key prompt).
    UpdateProfile {
        id: Uuid,
        draft: ProfileDraft,
    },
    /// Read the saved servers (for the server menu).
    LoadProfiles,
    /// Move keyboard focus to a widget (an entry action).
    Focus(&'static str),
    /// Move keyboard focus to the next or previous control (Tab, Shift+Tab).
    FocusNext,
    FocusPrevious,
    /// Read the server summary for a tab's Home screen, cache first.
    LoadSummary {
        tab: TabId,
    },
    /// Read a page of scenes through the cache, for the screen in `tab` that asked (`generation`).
    LoadScenesPage {
        tab: TabId,
        generation: u64,
        query: SceneQuery,
        page: u32,
        size: u32,
    },
    /// Read neighbouring pages into the cache; warm the thumbnails of page `warm`.
    PrefetchScenes {
        query: SceneQuery,
        pages: Vec<u32>,
        size: u32,
        warm: Option<u32>,
    },
    /// Read a page of images through the cache, for the screen in `tab` that asked (012).
    LoadImagesPage {
        tab: TabId,
        generation: u64,
        query: stash_core::images::ImageQuery,
        page: u32,
        size: u32,
    },
    /// Read neighbouring pages of images into the cache; warm the thumbnails of page `warm`.
    PrefetchImages {
        query: stash_core::images::ImageQuery,
        pages: Vec<u32>,
        size: u32,
        warm: Option<u32>,
    },
    /// Scenes' thumbnails (the core's, through its disk cache), delivered in batches so arriving
    /// thumbnails don't rebuild the view on every frame.
    LoadThumbnails {
        keys: Vec<ThumbKey>,
        width: u32,
    },
    /// Read the server's UI settings through the cache (008: default filter, play settings,
    /// tag colour rules).
    LoadUiSettings,
    /// Read the saved scene filters through the cache.
    LoadSavedFilters,
    /// Read the saved image filters through the cache (012).
    LoadSavedImageFilters,
    /// Read a picker's list through the cache.
    LoadChoices(stash_core::adapter::choices::ChoiceKind),
    /// Read the semantic vocabulary (and these colour rules' matches) through the cache.
    LoadVocabulary(Vec<stash_core::semantic::ColourRule>),
    /// The search box's debounce: report back after `after` (008 FR-001).
    SearchDue {
        tab: TabId,
        generation: u64,
        after: std::time::Duration,
    },
    /// Read a scene's "All files" section through the cache.
    LoadSceneFiles {
        tab: TabId,
        id: String,
    },
    /// Read a scene's galleries through the cache.
    LoadSceneGalleries {
        tab: TabId,
        id: String,
    },
    /// Read one chip's card through the cache.
    LoadChipDetails {
        kind: stash_core::adapter::chips::ChipKind,
        id: String,
    },
    /// A chip hover's rest: report back after `after` (008 FR-011b: 400 ms).
    ChipRestDue {
        tab: TabId,
        generation: u64,
        after: std::time::Duration,
    },
    /// The queue's selection rest: report back after `after` (010 research R3: 250 ms).
    QueueRestDue {
        tab: TabId,
        generation: u64,
        after: std::time::Duration,
    },
    /// Save the tag colour rules to Stash (the one settings write, FR-003g).
    SaveTagColours(Vec<stash_core::semantic::ColourRule>),
    /// Read a scene's details through the cache, for the Scene screen in `tab`.
    LoadSceneDetails {
        tab: TabId,
        id: String,
    },
    /// Read a scene's cover (its screenshot) through the cache.
    LoadCover {
        tab: TabId,
        id: String,
    },
    /// Post to the notification centre (a playback failure).
    Notify {
        title: String,
        detail: Option<String>,
    },
    /// Re-test and save a server from Settings (`force`: save even if the test fails).
    SaveProfile {
        id: Uuid,
        draft: ProfileDraft,
        force: bool,
    },
    /// Delete a saved server (and its cache).
    DeleteProfile(Uuid),
    /// Save the servers' order.
    ReorderProfiles(Vec<Uuid>),
    /// Read the current server's cache size.
    ReadCacheSize,
    /// Clear the current server's cache.
    ClearCache,
    /// Open the log folder in the file manager.
    OpenLogFolder,
    /// The UI bench resumes after this long (it idles without its frame clock).
    WakeBench(std::time::Duration),
    /// Mark read or dismiss entries in the notification centre.
    Notifications(NotificationAction),
    /// Hide a toast after a while (`stamp`: the entry's version that toasted).
    ExpireToast {
        id: String,
        stamp: String,
        after: std::time::Duration,
    },
    /// Scroll a scrollable to `y`.
    ScrollTo {
        id: String,
        y: f32,
    },
    /// Read a profile's saved session.
    LoadSession {
        profile: Uuid,
    },
    /// Ask for a save after [`SAVE_DELAY`]; only the latest generation is written (debounce).
    SaveSessionLater {
        generation: u64,
    },
    /// Write a profile's session in the background.
    SaveSession {
        profile: Uuid,
        session: SavedSession,
    },
    /// Write it before anything else runs (quit, server switch).
    SaveSessionNow {
        profile: Uuid,
        session: SavedSession,
    },
    /// Enter or leave fullscreen.
    SetFullscreen(bool),
    /// Open a scene in the player, at `start` seconds (a marker) or, with `resume`, at its
    /// cached resume point, otherwise from the beginning.
    OpenScene {
        id: String,
        start: Option<f64>,
        resume: bool,
    },
    /// Play Stash's transcode after a decode failure the user chose to work around (008 R10).
    OpenTranscoded {
        id: String,
        reason: String,
    },
    /// Queue a play history report for a server and send what's queued (008 contracts/play-history.md).
    SavePlayHistory {
        profile: Uuid,
        report: stash_core::playback::history::PlayHistoryReport,
    },
    /// Send a server's queued play history (on reconnect; for `deadline` after a server switch).
    FlushPlayHistory {
        profile: Uuid,
        deadline: Option<std::time::Duration>,
    },
    /// Play history couldn't be saved yet: one notification per server, updated.
    PlayHistoryUnsaved {
        profile: Uuid,
        left: usize,
    },
    /// The bench (Testing only): send a simulated 10-minute viewing's play history.
    SimulateViewing {
        scene_id: String,
    },
    /// Report back after the preview rest (008 FR-019).
    PreviewDue {
        generation: u64,
        after: std::time::Duration,
    },
    /// Load and decode a scene's preview at `width` pixels.
    LoadPreview {
        scene_id: String,
        generation: u64,
        width: u32,
    },
    /// Stop decoding a preview that was left.
    StopPreview,
    /// Load and decode the playing scene's seek-bar sprites (008 FR-021).
    LoadSprite {
        scene_id: String,
    },
    /// Read Settings → Playback from disk.
    LoadPlaybackPreferences,
    /// Write Settings → Playback.
    SavePlaybackPreferences(crate::services::settings::PlaybackPreferences),
    /// A playback control.
    Player(PlayerAction),
    /// Stop playback and the video path, then leave the app.
    Quit,
    /// The measurement's close-while-playing run: step `n` after a while.
    MeasureStep {
        n: u8,
        after: std::time::Duration,
    },
}

/// The result of an effect that reports back.
#[derive(Debug, Clone)]
pub enum Reply {
    ProfileCreated(Result<ServerProfile, AppError>),
    ProfileUpdated(Result<ServerProfile, AppError>),
    Profiles(Vec<ServerProfile>),
    /// A scene opened; the resume point it started at, if it resumed.
    SceneOpened(Result<Option<f64>, AppError>),
    /// A flush of a server's play history finished.
    PlayHistoryFlushed {
        profile: Uuid,
        outcome: stash_core::playback::history::FlushOutcome,
    },
    PlaybackPreferences(crate::services::settings::PlaybackPreferences),
    PreviewDue(u64),
    /// More of a preview's frames (`done`: that's all of them; none at all: the scene has none).
    Preview {
        generation: u64,
        frames: crate::screens::scenes::preview::Frames,
        done: bool,
    },
    /// The playing scene's seek-bar sprites (`None`: it has none).
    Sprite {
        scene_id: String,
        sheet: Option<crate::player::view::Sprites>,
    },
    Summary {
        tab: TabId,
        info: Option<ServerInfo>,
    },
    ScenesPage {
        tab: TabId,
        generation: u64,
        result: Result<ScenePage, AppError>,
    },
    ImagesPage {
        tab: TabId,
        generation: u64,
        result: Result<stash_core::images::ImagePage, AppError>,
    },
    /// Thumbnails that arrived, decoded (`None`: the core had nothing for that scene).
    Thumbnails(Vec<(ThumbKey, Option<iced::widget::image::Handle>)>),
    SceneDetails {
        tab: TabId,
        id: String,
        /// Boxed: details are large next to the other replies.
        result: Box<Result<stash_core::scenes::SceneDetails, AppError>>,
    },
    Cover {
        tab: TabId,
        id: String,
        cover: Option<iced::widget::image::Handle>,
    },
    ProfileSaved {
        id: Uuid,
        result: Result<ServerProfile, AppError>,
    },
    ProfileDeleted(Result<ServerProfile, AppError>),
    CacheSize(u64),
    CacheCleared(Result<u64, AppError>),
    /// Thumbnails worth having before they're shown (the next page's).
    Warm(Vec<ThumbKey>),
    SessionLoaded {
        profile: Uuid,
        saved: Option<SavedSession>,
    },
    UiSettings(Result<stash_core::adapter::filters::UiSettings, AppError>),
    SavedFilters(Result<Vec<stash_core::scenes::filter::saved::SavedFilter>, AppError>),
    SavedImageFilters(
        Result<
            Vec<stash_core::scenes::filter::saved::SavedFilterOf<stash_core::images::ImageSort>>,
            AppError,
        >,
    ),
    Choices {
        kind: stash_core::adapter::choices::ChoiceKind,
        result: Result<stash_core::adapter::choices::ChoiceList, AppError>,
    },
    Vocabulary(Result<stash_core::semantic::SemanticVocabulary, AppError>),
    /// A search's debounce is over.
    SearchDue {
        tab: TabId,
        generation: u64,
    },
    SceneFiles {
        tab: TabId,
        id: String,
        result: Result<Vec<stash_core::scenes::FileFacts>, AppError>,
    },
    SceneGalleries {
        tab: TabId,
        id: String,
        result: Result<Vec<stash_core::scenes::GalleryChip>, AppError>,
    },
    ChipDetails {
        kind: stash_core::adapter::chips::ChipKind,
        id: String,
        result: Result<stash_core::adapter::chips::ChipDetails, AppError>,
    },
    ChipRestDue {
        tab: TabId,
        generation: u64,
    },
    QueueRestDue {
        tab: TabId,
        generation: u64,
    },
    TagColoursSaved(Result<stash_core::adapter::filters::UiSettings, AppError>),
}

/// Turn an effect into a task.
pub fn run(effect: Effect, services: &Arc<Services>, window: Option<window::Id>) -> Task<Message> {
    match effect {
        Effect::Connect { profile, launch } => {
            if let Some(p) = services.profile(profile) {
                services.connect(&p, launch);
            }
            Task::none()
        }
        Effect::CreateProfile(draft) => {
            let services = Arc::clone(services);
            Task::perform(
                async move { services.create_profile(draft).await },
                |result| Message::Reply(Reply::ProfileCreated(result)),
            )
        }
        Effect::UpdateProfile { id, draft } => {
            let services = Arc::clone(services);
            Task::perform(
                async move { services.update_profile(id, draft, false).await },
                |result| Message::Reply(Reply::ProfileUpdated(result)),
            )
        }
        Effect::LoadProfiles => {
            let services = Arc::clone(services);
            Task::perform(async move { services.profiles() }, |list| {
                Message::Reply(Reply::Profiles(list))
            })
        }
        Effect::Focus(id) => iced::widget::operation::focus(id),
        Effect::FocusNext => iced::widget::operation::focus_next(),
        Effect::FocusPrevious => iced::widget::operation::focus_previous(),
        Effect::LoadSummary { tab } => {
            let services = Arc::clone(services);
            blocking(
                move || services.cached_server_info(),
                move |info| Message::Reply(Reply::Summary { tab, info }),
            )
        }
        Effect::LoadScenesPage {
            tab,
            generation,
            query,
            page,
            size,
        } => {
            let services = Arc::clone(services);
            Task::perform(
                async move { scenes_page(&services, query, page, size).await },
                move |result| {
                    Message::Reply(Reply::ScenesPage {
                        tab,
                        generation,
                        result,
                    })
                },
            )
        }
        Effect::PrefetchScenes {
            query,
            pages,
            size,
            warm,
        } => Task::batch(pages.into_iter().map(|page| {
            let services = Arc::clone(services);
            let query = query.clone();
            Task::perform(
                async move { scenes_page(&services, query, page, size).await },
                move |result| match result {
                    Ok(p) if Some(page) == warm => Message::Reply(Reply::Warm(
                        p.items
                            .iter()
                            .take(WARM_LIMIT)
                            .filter_map(thumb_key)
                            .collect(),
                    )),
                    _ => Message::Idle,
                },
            )
        })),
        Effect::LoadImagesPage {
            tab,
            generation,
            query,
            page,
            size,
        } => {
            let services = Arc::clone(services);
            Task::perform(
                async move { images_page(&services, query, page, size).await },
                move |result| {
                    Message::Reply(Reply::ImagesPage {
                        tab,
                        generation,
                        result,
                    })
                },
            )
        }
        Effect::PrefetchImages {
            query,
            pages,
            size,
            warm,
        } => Task::batch(pages.into_iter().map(|page| {
            let services = Arc::clone(services);
            let query = query.clone();
            Task::perform(
                async move { images_page(&services, query, page, size).await },
                move |result| match result {
                    Ok(p) if Some(page) == warm => Message::Reply(Reply::Warm(
                        p.items
                            .iter()
                            .take(WARM_LIMIT)
                            .filter_map(|c| c.thumb.as_deref().and_then(key_of))
                            .collect(),
                    )),
                    _ => Message::Idle,
                },
            )
        })),
        Effect::LoadThumbnails { keys, width } => {
            let services = Arc::clone(services);
            Task::run(thumbnail_batches(services, keys, width), |batch| {
                Message::Reply(Reply::Thumbnails(batch))
            })
        }
        Effect::LoadUiSettings => {
            let services = Arc::clone(services);
            Task::perform(
                async move {
                    services
                        .read_cached(
                            stash_core::cache::keys::UI_SETTINGS,
                            RefreshPolicy::Auto,
                            false,
                            |client| async move {
                                stash_core::adapter::filters::ui_settings(&client).await
                            },
                        )
                        .await
                        .map(|c| c.data)
                },
                |result| Message::Reply(Reply::UiSettings(result)),
            )
        }
        Effect::LoadSavedFilters => {
            let services = Arc::clone(services);
            Task::perform(
                async move {
                    services
                        .read_cached(
                            stash_core::cache::keys::SAVED_SCENE_FILTERS,
                            RefreshPolicy::Auto,
                            false,
                            |client| async move {
                                stash_core::adapter::filters::saved_scene_filters(&client).await
                            },
                        )
                        .await
                        .map(|c| c.data)
                },
                |result| Message::Reply(Reply::SavedFilters(result)),
            )
        }
        Effect::LoadSavedImageFilters => {
            let services = Arc::clone(services);
            Task::perform(
                async move {
                    services
                        .read_cached(
                            stash_core::cache::keys::SAVED_IMAGE_FILTERS,
                            RefreshPolicy::Auto,
                            false,
                            |client| async move {
                                stash_core::adapter::filters::saved_image_filters(&client).await
                            },
                        )
                        .await
                        .map(|c| c.data)
                },
                |result| Message::Reply(Reply::SavedImageFilters(result)),
            )
        }
        Effect::LoadChoices(kind) => {
            let services = Arc::clone(services);
            Task::perform(
                async move {
                    let key = stash_core::cache::keys::choices(kind.key());
                    services
                        .read_cached(&key, RefreshPolicy::Auto, false, move |client| async move {
                            stash_core::adapter::choices::choice_list(&client, kind).await
                        })
                        .await
                        .map(|c| c.data)
                },
                move |result| Message::Reply(Reply::Choices { kind, result }),
            )
        }
        Effect::LoadVocabulary(rules) => {
            let services = Arc::clone(services);
            let key = stash_core::cache::keys::semantic_vocabulary(
                &stash_core::semantic::extra_rule_fields(&rules),
            );
            let rules = Arc::new(rules);
            Task::perform(
                async move {
                    services
                        .read_cached(&key, RefreshPolicy::Auto, false, move |client| {
                            let rules = Arc::clone(&rules);
                            async move {
                                stash_core::adapter::semantic::semantic_vocabulary(&client, &rules)
                                    .await
                            }
                        })
                        .await
                        .map(|c| c.data)
                },
                |result| Message::Reply(Reply::Vocabulary(result)),
            )
        }
        Effect::SearchDue {
            tab,
            generation,
            after,
        } => Task::perform(tokio::time::sleep(after), move |()| {
            Message::Reply(Reply::SearchDue { tab, generation })
        }),
        Effect::LoadSceneFiles { tab, id } => {
            let services = Arc::clone(services);
            Task::perform(
                async move {
                    let key = stash_core::cache::keys::scene_files(&id);
                    let wanted = id.clone();
                    let result = services
                        .read_cached(&key, RefreshPolicy::Auto, false, move |client| {
                            let id = wanted.clone();
                            async move {
                                stash_core::adapter::scene_page::scene_files(&client, &id).await
                            }
                        })
                        .await
                        .map(|c| c.data);
                    (id, result)
                },
                move |(id, result)| Message::Reply(Reply::SceneFiles { tab, id, result }),
            )
        }
        Effect::LoadSceneGalleries { tab, id } => {
            let services = Arc::clone(services);
            Task::perform(
                async move {
                    let key = stash_core::cache::keys::scene_galleries(&id);
                    let wanted = id.clone();
                    let result = services
                        .read_cached(&key, RefreshPolicy::Auto, false, move |client| {
                            let id = wanted.clone();
                            async move {
                                stash_core::adapter::scene_page::scene_galleries(&client, &id).await
                            }
                        })
                        .await
                        .map(|c| c.data);
                    (id, result)
                },
                move |(id, result)| Message::Reply(Reply::SceneGalleries { tab, id, result }),
            )
        }
        Effect::LoadChipDetails { kind, id } => {
            let services = Arc::clone(services);
            Task::perform(
                async move {
                    let key = stash_core::cache::keys::chip(kind.key(), &id);
                    let wanted = id.clone();
                    let result = services
                        .read_cached(&key, RefreshPolicy::Auto, false, move |client| {
                            let id = wanted.clone();
                            async move {
                                stash_core::adapter::chips::chip_details(&client, kind, &id).await
                            }
                        })
                        .await
                        .map(|c| c.data);
                    (id, result)
                },
                move |(id, result)| Message::Reply(Reply::ChipDetails { kind, id, result }),
            )
        }
        Effect::ChipRestDue {
            tab,
            generation,
            after,
        } => Task::perform(tokio::time::sleep(after), move |()| {
            Message::Reply(Reply::ChipRestDue { tab, generation })
        }),
        Effect::QueueRestDue {
            tab,
            generation,
            after,
        } => Task::perform(tokio::time::sleep(after), move |()| {
            Message::Reply(Reply::QueueRestDue { tab, generation })
        }),
        Effect::SaveTagColours(rules) => {
            let services = Arc::clone(services);
            Task::perform(
                async move { services.save_tag_colours(rules).await },
                |result| Message::Reply(Reply::TagColoursSaved(result)),
            )
        }
        Effect::LoadSceneDetails { tab, id } => {
            let services = Arc::clone(services);
            Task::perform(
                async move {
                    let key = stash_core::cache::keys::scene_details(&id);
                    let wanted = id.clone();
                    let result =
                        services
                            .read_cached(&key, RefreshPolicy::Auto, false, move |client| {
                                let id = wanted.clone();
                                async move {
                                    stash_core::adapter::scenes::scene_details(&client, &id).await
                                }
                            })
                            .await
                            .map(|cached| cached.data);
                    (id, Box::new(result))
                },
                move |(id, result)| Message::Reply(Reply::SceneDetails { tab, id, result }),
            )
        }
        Effect::LoadCover { tab, id } => {
            let services = Arc::clone(services);
            Task::perform(
                async move {
                    let cover = services
                        .scene_cover(&id)
                        .await
                        .map(iced::widget::image::Handle::from_bytes);
                    (id, cover)
                },
                move |(id, cover)| Message::Reply(Reply::Cover { tab, id, cover }),
            )
        }
        Effect::Notify { title, detail } => {
            services
                .notifications
                .post(stash_core::shell::notifications::NewNotification {
                    key: None,
                    profile_id: services.current_profile(),
                    kind: stash_core::shell::notifications::NotificationKind::Playback,
                    severity: stash_core::shell::notifications::Severity::Error,
                    title,
                    detail,
                    toast: true,
                    job: None,
                });
            Task::none()
        }
        Effect::SaveProfile { id, draft, force } => {
            let services = Arc::clone(services);
            Task::perform(
                async move { services.update_profile(id, draft, force).await },
                move |result| Message::Reply(Reply::ProfileSaved { id, result }),
            )
        }
        Effect::DeleteProfile(id) => {
            let services = Arc::clone(services);
            blocking(
                move || services.delete_profile(id),
                |result| Message::Reply(Reply::ProfileDeleted(result)),
            )
        }
        Effect::ReorderProfiles(ids) => {
            let services = Arc::clone(services);
            blocking(
                move || {
                    if let Err(e) = services.reorder_profiles(&ids) {
                        tracing::warn!(error = %e, "couldn't save the servers' order");
                    }
                    services.profiles()
                },
                |list| Message::Reply(Reply::Profiles(list)),
            )
        }
        Effect::ReadCacheSize => {
            let services = Arc::clone(services);
            blocking(
                move || {
                    services
                        .current_profile()
                        .map_or(0, |id| services.caches.size(id))
                },
                |size| Message::Reply(Reply::CacheSize(size)),
            )
        }
        Effect::ClearCache => {
            let services = Arc::clone(services);
            blocking(
                move || match services.current_profile() {
                    Some(id) => services.clear_cache(id),
                    None => Ok(0),
                },
                |result| Message::Reply(Reply::CacheCleared(result)),
            )
        }
        Effect::OpenLogFolder => {
            let logs = services.paths.logs();
            if let Err(e) = std::process::Command::new("xdg-open").arg(&logs).spawn() {
                tracing::warn!(error = %e, "couldn't open the log folder");
            }
            Task::none()
        }
        Effect::WakeBench(after) => Task::perform(tokio::time::sleep(after), |()| {
            Message::Frame(std::time::Instant::now())
        }),
        Effect::Notifications(action) => {
            match action {
                NotificationAction::MarkRead(ids) => services.notifications.mark_read(&ids),
                NotificationAction::Dismiss(id) => services.notifications.dismiss(&id),
                NotificationAction::DismissAll => services.notifications.dismiss_all(),
            }
            Task::none()
        }
        Effect::ExpireToast { id, stamp, after } => {
            Task::perform(tokio::time::sleep(after), move |()| {
                Message::Session(crate::session::Msg::Notifications(
                    crate::shell::notifications::Msg::ToastExpired { id, stamp },
                ))
            })
        }
        Effect::ScrollTo { id, y } => iced::widget::operation::scroll_to(
            iced::widget::Id::from(id),
            iced::widget::operation::AbsoluteOffset {
                x: None,
                y: Some(y),
            },
        ),
        Effect::LoadSession { profile } => {
            let path = services.paths.session();
            blocking(
                move || snapshot::load(&path, profile),
                move |saved| Message::Reply(Reply::SessionLoaded { profile, saved }),
            )
        }
        Effect::SaveSessionLater { generation } => {
            Task::perform(tokio::time::sleep(SAVE_DELAY), move |()| {
                Message::Session(crate::session::Msg::SaveDue(generation))
            })
        }
        Effect::SaveSession { profile, session } => {
            let path = services.paths.session();
            blocking(
                move || save_session(&path, profile, &session),
                |()| Message::Idle,
            )
        }
        Effect::SaveSessionNow { profile, session } => {
            save_session(&services.paths.session(), profile, &session);
            Task::none()
        }
        Effect::SetFullscreen(fullscreen) => {
            if let Some(p) = &services.player {
                p.set_fullscreen_flag(fullscreen);
            }
            match window {
                Some(id) => window::set_mode(
                    id,
                    if fullscreen {
                        window::Mode::Fullscreen
                    } else {
                        window::Mode::Windowed
                    },
                ),
                None => Task::none(),
            }
        }
        Effect::OpenScene { id, start, resume } => {
            let services = Arc::clone(services);
            Task::perform(
                async move {
                    services
                        .open_scene(&id, start, resume)
                        .await
                        .map(|(_, resumed)| resumed)
                },
                |result| Message::Reply(Reply::SceneOpened(result)),
            )
        }
        Effect::OpenTranscoded { id, reason } => {
            let services = Arc::clone(services);
            Task::perform(
                async move { services.open_transcoded(&id, &reason).await.map(|()| None) },
                |result| Message::Reply(Reply::SceneOpened(result)),
            )
        }
        Effect::SavePlayHistory { profile, report } => {
            // In memory now, so a flush (or the quit's) that follows includes it.
            services.queue_play_history(profile, report);
            flush_play_history(services, profile, None)
        }
        Effect::FlushPlayHistory { profile, deadline } => {
            flush_play_history(services, profile, deadline)
        }
        Effect::PlayHistoryUnsaved { profile, left } => {
            services
                .notifications
                .post(stash_core::shell::notifications::NewNotification {
                    key: Some(format!("play-history:{profile}")),
                    profile_id: Some(profile),
                    kind: stash_core::shell::notifications::NotificationKind::Playback,
                    severity: stash_core::shell::notifications::Severity::Warning,
                    title: "Play history couldn't be saved yet".into(),
                    detail: Some(format!(
                        "{left} {} waiting; it's sent when the server can be reached.",
                        if left == 1 {
                            "update is"
                        } else {
                            "updates are"
                        }
                    )),
                    toast: false,
                    job: None,
                });
            Task::none()
        }
        Effect::SimulateViewing { scene_id } => {
            let services = Arc::clone(services);
            Task::perform(
                async move {
                    use stash_core::playback::history::Viewing;
                    let Ok((client, _, _)) = services.active_client() else {
                        return;
                    };
                    let Some(mut viewing) = Viewing::start(&scene_id, None, true) else {
                        return;
                    };
                    // An update every quarter-second for ten minutes, as the player sends them.
                    let mut reports = Vec::new();
                    for i in 0..=(600 * 4) {
                        let t = f64::from(i) * 0.25;
                        reports.extend(viewing.observe(t, true, t, Some(3600.0)));
                    }
                    reports.extend(viewing.end(600.0));
                    for r in &reports {
                        if let Err(e) = stash_core::adapter::activity::send_report(&client, r).await
                        {
                            tracing::warn!(error = %e, "bench viewing: a report failed");
                        }
                    }
                },
                |()| Message::Idle,
            )
        }
        Effect::PreviewDue { generation, after } => {
            Task::perform(tokio::time::sleep(after), move |()| {
                Message::Reply(Reply::PreviewDue(generation))
            })
        }
        Effect::LoadPreview {
            scene_id,
            generation,
            width,
        } => {
            services.want_preview(Some(generation));
            Task::run(
                preview_frames(Arc::clone(services), scene_id, generation, width),
                move |(frames, done)| {
                    Message::Reply(Reply::Preview {
                        generation,
                        frames,
                        done,
                    })
                },
            )
        }
        Effect::StopPreview => {
            services.want_preview(None);
            Task::none()
        }
        Effect::LoadSprite { scene_id } => {
            let services = Arc::clone(services);
            Task::perform(
                async move {
                    let sheet = match services.sprite_sheet(&scene_id).await {
                        Ok(Some(s)) => Some(crate::player::view::Sprites {
                            cues: Arc::new(s.cues),
                            sheet: iced::widget::image::Handle::from_rgba(
                                s.width, s.height, s.rgba,
                            ),
                        }),
                        Ok(None) => None,
                        Err(e) => {
                            tracing::debug!(error = %e, "no seek-bar sprites");
                            None
                        }
                    };
                    (scene_id, sheet)
                },
                |(scene_id, sheet)| Message::Reply(Reply::Sprite { scene_id, sheet }),
            )
        }
        Effect::LoadPlaybackPreferences => {
            let path = services.paths.playback_preferences();
            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || crate::services::settings::load(&path))
                        .await
                        .unwrap_or_default()
                },
                |prefs| Message::Reply(Reply::PlaybackPreferences(prefs)),
            )
        }
        Effect::SavePlaybackPreferences(prefs) => {
            let path = services.paths.playback_preferences();
            Task::perform(
                async move {
                    let saved = tokio::task::spawn_blocking(move || {
                        crate::services::settings::save(&path, prefs)
                    })
                    .await;
                    if let Ok(Err(e)) = saved {
                        tracing::warn!(error = %e, "couldn't save the playback settings");
                    }
                },
                |()| Message::Idle,
            )
        }
        Effect::Player(action) => {
            if let Some(p) = &services.player {
                player_command(p, action);
            }
            Task::none()
        }
        Effect::MeasureStep { n, after } => {
            Task::perform(tokio::time::sleep(after), move |()| Message::MeasureStep(n))
        }
        Effect::Quit => {
            if let Some(p) = &services.player {
                p.close();
            }
            // The final play history report is queued; give the server up to 2 s to take what's
            // waiting, and leave the rest for next time (contracts/play-history.md).
            let services = Arc::clone(services);
            Task::perform(
                async move {
                    if let Some(profile) = services.current_profile() {
                        let deadline = std::time::Instant::now()
                            + stash_core::playback::history::FLUSH_DEADLINE;
                        let _ = services.flush_play_history(profile, Some(deadline)).await;
                    }
                },
                |()| (),
            )
            .then(|()| iced::exit())
        }
    }
}

/// How often a decoding preview hands over the frames that are ready, after the first.
const PREVIEW_BATCH: std::time::Duration = std::time::Duration::from_millis(100);

/// A card preview's frames as they decode: the first at once (the preview starts playing), then
/// the rest in batches, then `done`. Decoding stops when another preview is wanted.
fn preview_frames(
    services: Arc<Services>,
    scene_id: String,
    generation: u64,
    width: u32,
) -> impl iced::futures::Stream<Item = (crate::screens::scenes::preview::Frames, bool)> {
    iced::stream::channel(8, async move |mut out| {
        use iced::futures::SinkExt;
        let bytes = match services.preview_file(&scene_id).await {
            Ok(Some(bytes)) => bytes,
            Ok(None) => {
                let _ = out.send((Vec::new(), true)).await;
                return;
            }
            Err(e) => {
                tracing::debug!(error = %e, "no preview");
                let _ = out.send((Vec::new(), true)).await;
                return;
            }
        };
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let wanted = Arc::clone(&services);
        let decode = tokio::task::spawn_blocking(move || {
            stash_core::previews::decode_preview_each(&bytes, width, |frame| {
                wanted.preview_wanted(generation) && tx.send(frame).is_ok()
            })
        });
        let mut batch = Vec::new();
        let mut sent_first = false;
        let mut last = std::time::Instant::now();
        while let Some(f) = rx.recv().await {
            batch.push((
                iced::widget::image::Handle::from_rgba(f.width, f.height, f.rgba),
                std::time::Duration::from_millis(u64::from(f.delay_ms)),
            ));
            if !sent_first || last.elapsed() >= PREVIEW_BATCH {
                sent_first = true;
                last = std::time::Instant::now();
                if out.send((std::mem::take(&mut batch), false)).await.is_err() {
                    return;
                }
            }
        }
        if let Ok(Err(e)) = decode.await {
            tracing::debug!(error = %e, "a preview couldn't be decoded");
        }
        let _ = out.send((batch, true)).await;
    })
}

/// Send `profile`'s queued play history in the background and report back.
fn flush_play_history(
    services: &Arc<Services>,
    profile: Uuid,
    deadline: Option<std::time::Duration>,
) -> Task<Message> {
    let services = Arc::clone(services);
    Task::perform(
        async move {
            let deadline = deadline.map(|d| std::time::Instant::now() + d);
            services.flush_play_history(profile, deadline).await
        },
        move |outcome| Message::Reply(Reply::PlayHistoryFlushed { profile, outcome }),
    )
}

/// How long arriving thumbnails are gathered before they're handed over.
const THUMB_BATCH: std::time::Duration = std::time::Duration::from_millis(100);
/// Requests kept in flight (the core's `ThumbService` limits the real concurrency).
const THUMB_IN_FLIGHT: usize = 12;

/// Which cached image a thumbnail key draws on, as the core's `ThumbService` names it. Performer
/// images are keyed `performer:<id>`, and their portraits `portrait:<id>`: the same cached image,
/// decoded larger, so a portrait never downloads again. Image thumbnails are keyed `image:<id>`
/// (012). Scene thumbnails are keyed by scene id.
pub fn thumb_source(key: &ThumbKey) -> (&'static str, &str) {
    if let Some(id) = key.0.strip_prefix("image:") {
        return ("image", id);
    }
    match key
        .0
        .strip_prefix("performer:")
        .or_else(|| key.0.strip_prefix("portrait:"))
    {
        Some(id) => ("performer", id),
        None => ("scene", key.0.as_str()),
    }
}

/// One thumbnail, decoded off the UI thread at the width it's drawn, so iced has nothing to
/// decode or scale (scrolling stays smooth while rows of them appear).
async fn fetch_thumb(
    service: Option<Arc<stash_core::thumbs::ThumbService>>,
    key: ThumbKey,
    width: u32,
) -> (ThumbKey, Option<iced::widget::image::Handle>) {
    let (kind, id) = thumb_source(&key);
    let bytes = match service {
        Some(t) => t.get(kind, id, &key.1).await,
        None => None,
    };
    let Some(bytes) = bytes.filter(|b| b.as_slice() != stash_core::thumbs::PLACEHOLDER) else {
        return (key, None);
    };
    // A few at a time: decoding is CPU work, and the UI thread needs a core while rows scroll in.
    static DECODES: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(3);
    let Ok(_slot) = DECODES.acquire().await else {
        return (key, None);
    };
    let decoded =
        tokio::task::spawn_blocking(move || stash_core::thumbs::decode_rgba(&bytes, width)).await;
    let handle = match decoded {
        Ok(Ok((pixels, w, h))) => Some(iced::widget::image::Handle::from_rgba(w, h, pixels)),
        Ok(Err(reason)) => {
            tracing::debug!(%reason, "couldn't decode a thumbnail");
            None
        }
        Err(e) => {
            tracing::warn!(error = %e, "thumbnail decode failed");
            None
        }
    };
    (key, handle)
}

/// Thumbnails in page order, handed over at most every [`THUMB_BATCH`].
fn thumbnail_batches(
    services: Arc<Services>,
    keys: Vec<ThumbKey>,
    width: u32,
) -> impl iced::futures::Stream<Item = Vec<(ThumbKey, Option<iced::widget::image::Handle>)>> {
    iced::stream::channel(4, async move |mut out| {
        use iced::futures::stream::FuturesUnordered;
        use iced::futures::{SinkExt, StreamExt};
        let service = services.thumbs();
        let mut queue = keys.into_iter();
        let mut pending = FuturesUnordered::new();
        for key in queue.by_ref().take(THUMB_IN_FLIGHT) {
            pending.push(fetch_thumb(service.clone(), key, width));
        }
        let mut batch = Vec::new();
        let mut since: Option<std::time::Instant> = None;
        loop {
            let wait = since.map_or(THUMB_BATCH, |t| THUMB_BATCH.saturating_sub(t.elapsed()));
            match tokio::time::timeout(wait, pending.next()).await {
                Ok(Some(result)) => {
                    if let Some(key) = queue.next() {
                        pending.push(fetch_thumb(service.clone(), key, width));
                    }
                    batch.push(result);
                    since.get_or_insert_with(std::time::Instant::now);
                }
                Ok(None) => {
                    if !batch.is_empty() {
                        let _ = out.send(std::mem::take(&mut batch)).await;
                    }
                    break;
                }
                Err(_) => {}
            }
            if since.is_some_and(|t| t.elapsed() >= THUMB_BATCH) && !batch.is_empty() {
                if out.send(std::mem::take(&mut batch)).await.is_err() {
                    break;
                }
                since = None;
            }
        }
    })
}

/// A page of scenes through the current profile's cache (the demo's keys and policy).
async fn scenes_page(
    services: &Services,
    query: SceneQuery,
    page: u32,
    size: u32,
) -> Result<ScenePage, AppError> {
    let key = page_key(&query, page, size);
    services
        .read_cached(&key, RefreshPolicy::Auto, false, move |client| {
            let query = query.clone();
            async move {
                stash_core::adapter::scenes::find_scenes_page(&client, &query, page, size).await
            }
        })
        .await
        .map(|cached| cached.data)
}

/// One page of images through the cache (`images:q:<hash>:s:<size>:p:<page>`, 012).
async fn images_page(
    services: &Services,
    query: stash_core::images::ImageQuery,
    page: u32,
    size: u32,
) -> Result<stash_core::images::ImagePage, AppError> {
    let key = crate::screens::browse::state::page_key(
        stash_core::browse::entity::Entity::Images,
        &query,
        page,
        size,
    );
    services
        .read_cached(&key, RefreshPolicy::Auto, false, move |client| {
            let query = query.clone();
            async move {
                stash_core::adapter::images::find_images_page(&client, &query, page, size).await
            }
        })
        .await
        .map(|cached| cached.data)
}

fn save_session(path: &std::path::Path, profile: Uuid, session: &SavedSession) {
    if let Err(e) = snapshot::save(path, profile, session) {
        tracing::warn!(error = %e, "couldn't save the session");
    }
}

/// Run `work` off the UI thread and turn its result into a message.
fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
    done: impl FnOnce(T) -> Message + Send + 'static,
) -> Task<Message> {
    Task::perform(
        async move { tokio::task::spawn_blocking(work).await },
        move |result| match result {
            Ok(value) => done(value),
            Err(e) => {
                tracing::error!(error = %e, "background work failed");
                Message::Idle
            }
        },
    )
}

fn player_command(p: &player::Player, action: PlayerAction) {
    match action {
        PlayerAction::TogglePause => p.toggle_pause(),
        PlayerAction::SeekRelative(s) => p.seek_relative(s),
        PlayerAction::Seek { seconds, exact } => p.seek(seconds, exact),
        PlayerAction::SetVolume(v) => p.set_volume(v.clamp(0.0, 100.0)),
        PlayerAction::SetMuted(m) => p.set_muted(m),
        PlayerAction::SetSpeed(s) => p.set_speed(s),
        PlayerAction::FrameStep(Direction::Forward) => p.frame_step_forward(),
        PlayerAction::FrameStep(Direction::Back) => p.frame_step_back(),
        PlayerAction::Replay => p.replay(),
        PlayerAction::Close => p.close(),
    }
}
