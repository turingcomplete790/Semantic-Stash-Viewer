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
use crate::screens::scenes::thumbs::{thumb_key, ThumbKey, WARM_LIMIT};
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
    /// Scenes' thumbnails (the core's, through its disk cache), delivered in batches so arriving
    /// thumbnails don't rebuild the view on every frame.
    LoadThumbnails {
        keys: Vec<ThumbKey>,
        width: u32,
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
    /// Look up a scene and start playing it.
    OpenScene(String),
    /// A playback control.
    Player(PlayerAction),
    /// Stop playback and the video path, then leave the app.
    Quit,
}

/// The result of an effect that reports back.
#[derive(Debug, Clone)]
pub enum Reply {
    ProfileCreated(Result<ServerProfile, AppError>),
    ProfileUpdated(Result<ServerProfile, AppError>),
    Profiles(Vec<ServerProfile>),
    SceneOpened(Result<(), AppError>),
    Summary {
        tab: TabId,
        info: Option<ServerInfo>,
    },
    ScenesPage {
        tab: TabId,
        generation: u64,
        result: Result<ScenePage, AppError>,
    },
    /// Thumbnails that arrived, decoded (`None`: the core had nothing for that scene).
    Thumbnails(Vec<(ThumbKey, Option<iced::widget::image::Handle>)>),
    /// Thumbnails worth having before they're shown (the next page's).
    Warm(Vec<ThumbKey>),
    SessionLoaded {
        profile: Uuid,
        saved: Option<SavedSession>,
    },
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
        Effect::LoadThumbnails { keys, width } => {
            let services = Arc::clone(services);
            Task::run(thumbnail_batches(services, keys, width), |batch| {
                Message::Reply(Reply::Thumbnails(batch))
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
        Effect::OpenScene(id) => {
            let services = Arc::clone(services);
            Task::perform(
                async move { services.open_scene(&id).await.map(|_| ()) },
                |result| Message::Reply(Reply::SceneOpened(result)),
            )
        }
        Effect::Player(action) => {
            if let Some(p) = &services.player {
                player_command(p, action);
            }
            Task::none()
        }
        Effect::Quit => {
            if let Some(p) = &services.player {
                p.close();
            }
            iced::exit()
        }
    }
}

/// How long arriving thumbnails are gathered before they're handed over.
const THUMB_BATCH: std::time::Duration = std::time::Duration::from_millis(100);
/// Requests kept in flight (the core's `ThumbService` limits the real concurrency).
const THUMB_IN_FLIGHT: usize = 12;

/// One thumbnail, decoded off the UI thread at the width it's drawn, so iced has nothing to
/// decode or scale (scrolling stays smooth while rows of them appear).
async fn fetch_thumb(
    service: Option<Arc<stash_core::thumbs::ThumbService>>,
    key: ThumbKey,
    width: u32,
) -> (ThumbKey, Option<iced::widget::image::Handle>) {
    let bytes = match service {
        Some(t) => t.get("scene", &key.0, &key.1).await,
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
