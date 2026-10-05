//! The session's playback region (007 data model "Session"; T049): the 006 player around mpv. One
//! player for the app; it outlives tab switches. The tab that started it owns it: the player
//! shows in that tab's scene, and a now-playing bar shows everywhere else. Every command leaves
//! as an effect.

use player::{PlayerSnapshot, PlayerStateKind};

use crate::effects::{Effect, PlayerAction};
use crate::player::controls::{key_action, Action};
use crate::player::view::{Msg as ControlsMsg, PlayerScreen};
use crate::shell::TabId;

#[derive(Default)]
pub struct Playback {
    pub screen: PlayerScreen,
    /// The tab whose scene is playing.
    pub owner: Option<TabId>,
    /// The scene asked for (the snapshot carries it once mpv has it).
    pub scene_id: Option<String>,
    /// The current failure has been posted to the notification centre.
    failure_posted: bool,
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

    /// Start `scene_id` for tab `owner`.
    pub fn open(&mut self, scene_id: String, owner: TabId) -> Vec<Effect> {
        self.owner = Some(owner);
        self.scene_id = Some(scene_id.clone());
        self.failure_posted = false;
        vec![Effect::OpenScene(scene_id)]
    }

    /// The scene playing (or opening).
    pub fn playing_scene(&self) -> Option<&str> {
        self.snapshot()
            .scene_id
            .as_deref()
            .or(self.scene_id.as_deref())
    }

    pub fn update(&mut self, msg: Msg) -> Vec<Effect> {
        match msg {
            Msg::Snapshot(s) => {
                let was_active = self.active();
                let fullscreen = s.fullscreen;
                let failure = (s.state == PlayerStateKind::Error)
                    .then(|| (s.title.clone(), s.error.as_ref().map(ToString::to_string)));
                self.screen.update(ControlsMsg::Snapshot(s));
                let mut effects = Vec::new();
                // A failure is shown on the player and posted once (US5's centre lists it).
                match failure {
                    Some((title, detail)) if !self.failure_posted => {
                        self.failure_posted = true;
                        effects.push(Effect::Notify {
                            title: match title {
                                Some(t) => format!("Couldn't play {t}"),
                                None => "Couldn't play the scene".into(),
                            },
                            detail,
                        });
                    }
                    Some(_) => {}
                    None => self.failure_posted = false,
                }
                if was_active && !self.active() {
                    self.owner = None;
                    self.scene_id = None;
                    // Playback stopped by any path: leave fullscreen (002 FR-007).
                    if fullscreen {
                        effects.push(Effect::SetFullscreen(false));
                    }
                }
                effects
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
