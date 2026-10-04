//! The session's playback region (007 data model "Session"): the 006 player around mpv. One
//! player for the app; it outlives tab switches. Every command leaves as an effect.

use player::{PlayerSnapshot, PlayerStateKind};

use crate::effects::{Effect, PlayerAction};
use crate::player::controls::{key_action, Action};
use crate::player::view::{Msg as ControlsMsg, PlayerScreen};

#[derive(Default)]
pub struct Playback {
    pub screen: PlayerScreen,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Snapshot(PlayerSnapshot),
    Controls(ControlsMsg),
    Close,
    ToggleFullscreen,
}

impl Playback {
    pub fn snapshot(&self) -> &PlayerSnapshot {
        &self.screen.snapshot
    }

    /// Something is open (loading, playing, paused, ended, or failed).
    pub fn active(&self) -> bool {
        self.snapshot().state != PlayerStateKind::Idle
    }

    pub fn update(&mut self, msg: Msg) -> Vec<Effect> {
        match msg {
            Msg::Snapshot(s) => {
                let was_active = self.active();
                let fullscreen = s.fullscreen;
                self.screen.update(ControlsMsg::Snapshot(s));
                // Playback stopped by any path: leave fullscreen (002 FR-007).
                if was_active && !self.active() && fullscreen {
                    return vec![Effect::SetFullscreen(false)];
                }
                Vec::new()
            }
            Msg::Controls(m) => self.screen.update(m),
            Msg::Close => {
                let mut effects = vec![Effect::Player(PlayerAction::Close)];
                if self.snapshot().fullscreen {
                    effects.push(Effect::SetFullscreen(false));
                }
                effects
            }
            Msg::ToggleFullscreen => {
                let _ = self.screen.update(ControlsMsg::Activity);
                vec![Effect::SetFullscreen(!self.snapshot().fullscreen)]
            }
        }
    }

    /// A key while the player is showing (002 FR-009). `None` if it isn't a player key.
    pub fn key(&mut self, key: &iced::keyboard::Key) -> Option<Vec<Effect>> {
        let action = key_action(key, self.snapshot().paused)?;
        let msg = match action {
            Action::TogglePause => ControlsMsg::TogglePause,
            Action::SeekRelative(s) => ControlsMsg::Skip(s),
            Action::Volume(d) => ControlsMsg::VolumeBy(d),
            Action::Speed(d) => ControlsMsg::SpeedStep(d),
            Action::NormalSpeed => ControlsMsg::Speed(1.0),
            Action::FrameStep(d) => ControlsMsg::Frame(d),
            Action::ToggleMute => ControlsMsg::ToggleMute,
            Action::ToggleFullscreen => return Some(self.update(Msg::ToggleFullscreen)),
            Action::Escape => {
                let _ = self.screen.update(ControlsMsg::Activity);
                return Some(if self.snapshot().fullscreen {
                    vec![Effect::SetFullscreen(false)]
                } else {
                    self.update(Msg::Close)
                });
            }
        };
        Some(self.screen.update(msg))
    }
}
