//! Effects: the only way work leaves the state machine (007 research R2).
//!
//! A transition returns [`Effect`] values (data). [`run`] turns each into an iced `Task` over the
//! service layer, where the core fetches and deserializes off the UI thread, and the typed result
//! comes back as a [`Message`] addressed to the state that asked. `update` therefore never does
//! I/O, and tests assert on effects without running anything.

use std::sync::Arc;

use iced::{window, Task};
use stash_core::connection::ServerInfo;
use stash_core::profiles::{ProfileDraft, ServerProfile};
use stash_core::AppError;
use uuid::Uuid;

use crate::app::Message;
use crate::player::controls::Direction;
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
