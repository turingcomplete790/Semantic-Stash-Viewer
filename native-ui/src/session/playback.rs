//! The session's playback region (007 data model "Session"; T049): the 006 player around mpv. One
//! player for the app; it outlives tab switches. The tab that started it owns it: the player
//! shows in that tab's scene, and a now-playing bar shows everywhere else. Every command leaves
//! as an effect.
//!
//! It also follows each viewing (008 US3): the core's [`Viewing`] turns the player's snapshots
//! into play history reports, sent as effects at each checkpoint and whenever a viewing ends.

use std::sync::OnceLock;
use std::time::Instant;

use player::{PlayerSnapshot, PlayerStateKind};
use stash_core::playback::history::{PlayHistoryReport, Viewing};
use uuid::Uuid;

use crate::effects::{Effect, PlayerAction};
use crate::player::controls::{key_action, Action};
use crate::player::view::{Msg as ControlsMsg, PlayerScreen};
use crate::shell::TabId;

pub struct Playback {
    pub screen: PlayerScreen,
    /// The tab whose scene is playing.
    pub owner: Option<TabId>,
    /// The scene asked for (the snapshot carries it once mpv has it).
    pub scene_id: Option<String>,
    /// The current failure has been posted to the notification centre.
    failure_posted: bool,
    /// The server play history goes to.
    pub profile: Option<Uuid>,
    /// Settings → Playback (both on by default).
    pub resume: bool,
    pub save_history: bool,
    /// The server's `minimumPlayPercent` (UI settings; 0 when unset).
    pub minimum_percent: Option<f64>,
    /// The viewing in progress, while play history is on.
    viewing: Option<Viewing>,
    /// The scene that just reached its end (not a failure), for the session to take: the queue
    /// view's Play next (010 R4).
    ended: Option<String>,
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            screen: PlayerScreen::default(),
            owner: None,
            scene_id: None,
            failure_posted: false,
            profile: None,
            resume: true,
            save_history: true,
            minimum_percent: None,
            viewing: None,
            ended: None,
        }
    }
}

/// Seconds on a monotonic clock (a viewing measures playing time with it).
fn clock() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
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

    /// Start `scene_id` for tab `owner`: at its resume point when "Resume where you left off" is
    /// on, otherwise from the beginning.
    pub fn open(&mut self, scene_id: String, owner: TabId) -> Vec<Effect> {
        self.open_at(scene_id, owner, None)
    }

    /// Start `scene_id` for tab `owner` at `start` seconds (a marker), or as [`Self::open`].
    pub fn open_at(&mut self, scene_id: String, owner: TabId, start: Option<f64>) -> Vec<Effect> {
        // Another scene ends the current viewing.
        let mut effects = self.finish();
        self.owner = Some(owner);
        self.scene_id = Some(scene_id.clone());
        self.failure_posted = false;
        self.screen.chapters.clear();
        self.screen.restart_offer = None;
        self.screen.sprites = None;
        self.screen.sprites_requested = false;
        self.screen.direct_failure = None;
        self.viewing = Viewing::start(&scene_id, self.minimum_percent, self.save_history);
        self.ended = None;
        effects.push(Effect::OpenScene {
            id: scene_id,
            start,
            resume: self.resume && start.is_none(),
        });
        effects
    }

    /// Play the scene again from the beginning as a new viewing (Repeat one, 011 R4): the
    /// player's own replay (a seek to 0 and unpause, which copes with mpv holding the file open at
    /// its end), not a reopen of the same file.
    pub fn replay(&mut self) -> Vec<Effect> {
        let Some(id) = self.playing_scene().map(str::to_owned) else {
            return Vec::new();
        };
        let mut effects = self.finish();
        self.viewing = Viewing::start(&id, self.minimum_percent, self.save_history);
        self.ended = None;
        effects.push(Effect::Player(PlayerAction::Replay));
        effects
    }

    /// Play the server's transcode of the scene that couldn't be decoded (008 FR-023): only when
    /// offered, never retried after a transcode fails.
    fn open_transcoded(&mut self) -> Vec<Effect> {
        if !self.screen.offers_transcode() {
            return Vec::new();
        }
        let Some(id) = self.playing_scene().map(str::to_owned) else {
            return Vec::new();
        };
        let reason = self
            .snapshot()
            .error
            .as_ref()
            .map_or_else(|| "unknown".to_owned(), ToString::to_string);
        self.screen.direct_failure = Some(reason.clone());
        self.failure_posted = false;
        self.viewing = Viewing::start(&id, self.minimum_percent, self.save_history);
        vec![Effect::OpenTranscoded { id, reason }]
    }

    /// The viewing ends now (quit, server switch, leaving the session): its final report.
    pub fn finish(&mut self) -> Vec<Effect> {
        let report = self.viewing.take().and_then(|mut v| v.end(clock()));
        self.report(report)
    }

    /// The scene opened at its resume point: offer "Start from the beginning" for a while.
    pub fn resumed(&mut self, at: Option<f64>) {
        if at.is_some() {
            self.screen.offer_restart(Instant::now());
        }
    }

    /// Seek-bar sprites arrived for `scene_id` (ignored if another scene plays now).
    pub fn sprites_loaded(&mut self, scene_id: &str, sheet: Option<crate::player::view::Sprites>) {
        if self.playing_scene() == Some(scene_id) {
            self.screen.sprites = sheet;
        }
    }

    /// The playing scene's markers, as chapters on the seek bar (from its scene page).
    pub fn set_chapters(&mut self, chapters: Vec<(f64, String)>) {
        if self.screen.chapters != chapters {
            self.screen.chapters = chapters;
        }
    }

    fn report(&self, report: Option<PlayHistoryReport>) -> Vec<Effect> {
        match (report, self.profile) {
            (Some(report), Some(profile)) => vec![Effect::SavePlayHistory { profile, report }],
            _ => Vec::new(),
        }
    }

    /// The scene that reached its end since the last call, once (010 R4).
    pub fn take_ended(&mut self) -> Option<String> {
        self.ended.take()
    }

    /// The scene playing (or opening): the one asked for, which the snapshot reports once mpv
    /// has it (until then it may still report the scene before, as Play next opens the next).
    pub fn playing_scene(&self) -> Option<&str> {
        self.scene_id
            .as_deref()
            .or(self.snapshot().scene_id.as_deref())
    }

    pub fn update(&mut self, msg: Msg) -> Vec<Effect> {
        match msg {
            Msg::Snapshot(s) => {
                let was_active = self.active();
                let was_ended = self.snapshot().state == PlayerStateKind::Ended;
                let fullscreen = s.fullscreen;
                let failure = (s.state == PlayerStateKind::Error)
                    .then(|| (s.title.clone(), s.error.as_ref().map(ToString::to_string)));
                let state = s.state;
                let observed = (s.scene_id.clone(), s.position_seconds, s.duration_seconds);
                self.screen.update(ControlsMsg::Snapshot(s));
                let mut effects = Vec::new();
                // The viewing follows the player: a checkpoint each minute of playing.
                let checkpoint = match self.viewing.as_mut() {
                    Some(v) if observed.0.as_deref() == Some(v.scene_id.as_str()) => v.observe(
                        clock(),
                        state == PlayerStateKind::Playing,
                        observed.1,
                        observed.2,
                    ),
                    _ => None,
                };
                effects.extend(self.report(checkpoint));
                // Reached its end (an Error is never an end): reported once.
                if state == PlayerStateKind::Ended && !was_ended {
                    self.ended = self.playing_scene().map(str::to_owned);
                }
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
                // Stopped by any path, or failed: the viewing ends.
                if (was_active && !self.active()) || state == PlayerStateKind::Error {
                    effects.extend(self.finish());
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
            Msg::Controls(ControlsMsg::PlayTranscoded) => self.open_transcoded(),
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
            Action::Chapter(d) => ControlsMsg::Chapter(d),
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
