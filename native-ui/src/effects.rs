//! Effects: the only way work leaves the state machine (007 research R2).
//!
//! A transition returns [`Effect`] values (data). [`run`] turns each into an iced `Task` over the
//! service layer, where the core fetches and deserializes off the UI thread, and the typed result
//! comes back as a [`Message`] addressed to the state that asked. `update` therefore never does
//! I/O, and tests assert on effects without running anything.

use std::sync::Arc;

use iced::{window, Task};
use stash_core::profiles::{ProfileDraft, ServerProfile};
use stash_core::AppError;
use uuid::Uuid;

use crate::app::Message;
use crate::player::controls::Direction;
use crate::services::Services;

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
    Connect { profile: Uuid, launch: bool },
    /// Test a new server and save it; the result goes back to onboarding.
    CreateProfile(ProfileDraft),
    /// Re-test and save a changed server (a new API key from the key prompt).
    UpdateProfile { id: Uuid, draft: ProfileDraft },
    /// Read the saved servers (for the server menu).
    LoadProfiles,
    /// Move keyboard focus to a widget (an entry action).
    Focus(&'static str),
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
