//! The player's controls (006 T011, T018): the logic matches the web player (002 FR-008–FR-010;
//! `ui/src/player/keyboard.ts`, `speeds.ts`, `Controls.tsx`), and the view draws them over the
//! video.

use std::time::{Duration, Instant};

use iced::keyboard::key::Named;
use iced::keyboard::Key;

/// The speed list (web `SPEEDS`).
pub const SPEEDS: [f64; 9] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0];
/// Volume change per arrow key.
pub const VOLUME_STEP: f64 = 5.0;
/// Skip buttons and arrow keys.
pub const SKIP_SECONDS: f64 = 10.0;
const IDLE: Duration = Duration::from_millis(3000);
/// A seek target is shown until mpv is within this of it…
const ARRIVED_WITHIN: f64 = 1.0;
/// …or until this long has passed.
const TARGET_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    TogglePause,
    SeekRelative(f64),
    /// Change the volume by this much (clamped to 0–100 by the caller).
    Volume(f64),
    Speed(Direction),
    NormalSpeed,
    FrameStep(Direction),
    ToggleFullscreen,
    ToggleMute,
    /// Leave fullscreen if fullscreen, otherwise close the player.
    Escape,
}

/// The 002 FR-009 key map. Frame stepping only while paused.
pub fn key_action(key: &Key, paused: bool) -> Option<Action> {
    match key {
        Key::Named(Named::Space) => Some(Action::TogglePause),
        Key::Named(Named::ArrowLeft) => Some(Action::SeekRelative(-SKIP_SECONDS)),
        Key::Named(Named::ArrowRight) => Some(Action::SeekRelative(SKIP_SECONDS)),
        Key::Named(Named::ArrowUp) => Some(Action::Volume(VOLUME_STEP)),
        Key::Named(Named::ArrowDown) => Some(Action::Volume(-VOLUME_STEP)),
        Key::Named(Named::Escape) => Some(Action::Escape),
        Key::Character(c) => match c.as_str() {
            "[" => Some(Action::Speed(Direction::Back)),
            "]" => Some(Action::Speed(Direction::Forward)),
            "\\" => Some(Action::NormalSpeed),
            "." if paused => Some(Action::FrameStep(Direction::Forward)),
            "," if paused => Some(Action::FrameStep(Direction::Back)),
            "f" | "F" => Some(Action::ToggleFullscreen),
            "m" | "M" => Some(Action::ToggleMute),
            _ => None,
        },
        _ => None,
    }
}

/// The next speed in the list in `direction`, staying at the ends (web `nextSpeed`).
pub fn next_speed(current: f64, direction: Direction) -> f64 {
    match direction {
        Direction::Forward => SPEEDS
            .iter()
            .copied()
            .find(|s| *s > current + 1e-9)
            .unwrap_or(SPEEDS[SPEEDS.len() - 1]),
        Direction::Back => SPEEDS
            .iter()
            .rev()
            .copied()
            .find(|s| *s < current - 1e-9)
            .unwrap_or(SPEEDS[0]),
    }
}

/// Controls and the cursor hide after 3 s without movement while playing (FR-010).
#[derive(Debug, Clone, Copy)]
pub struct AutoHide {
    last_activity: Instant,
}

impl AutoHide {
    pub fn new(now: Instant) -> Self {
        Self { last_activity: now }
    }

    /// Mouse movement or a key press.
    pub fn activity(&mut self, now: Instant) {
        self.last_activity = now;
    }

    pub fn visible(&self, now: Instant, playing: bool) -> bool {
        !playing || now.duration_since(self.last_activity) < IDLE
    }

    /// When the controls will hide, if they're showing during playback.
    pub fn hides_at(&self) -> Instant {
        self.last_activity + IDLE
    }
}

/// The time under the pointer at `x` on a seek bar `width` wide.
pub fn hover_time(x: f32, width: f32, duration: f64) -> f64 {
    if width <= 0.0 || duration <= 0.0 {
        return 0.0;
    }
    (f64::from(x) / f64::from(width)).clamp(0.0, 1.0) * duration
}

/// What the seek bar shows: a seek target until mpv gets there (or gives up), then mpv's position.
#[derive(Debug, Clone, Copy, Default)]
pub struct SeekBar {
    target: Option<(f64, Instant)>,
}

impl SeekBar {
    pub fn seek_to(&mut self, seconds: f64, now: Instant) {
        self.target = Some((seconds, now));
    }

    pub fn shown(&mut self, position: f64, now: Instant) -> f64 {
        match self.target {
            Some((target, since))
                if (position - target).abs() >= ARRIVED_WITHIN
                    && now.duration_since(since) < TARGET_TIMEOUT =>
            {
                target
            }
            _ => {
                self.target = None;
                position
            }
        }
    }

    /// The position to show without updating (for drawing).
    pub fn peek(&self, position: f64, now: Instant) -> f64 {
        match self.target {
            Some((target, since))
                if (position - target).abs() >= ARRIVED_WITHIN
                    && now.duration_since(since) < TARGET_TIMEOUT =>
            {
                target
            }
            _ => position,
        }
    }
}
