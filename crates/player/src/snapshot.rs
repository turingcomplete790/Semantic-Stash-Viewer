//! `PlayerSnapshot`: the player state sent to the UI (contracts/player-commands.md).

use serde::Serialize;

use crate::error::PlayerError;
use crate::tracks::Track;

/// Player state (data-model.md "PlayerState and transitions").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum PlayerStateKind {
    #[default]
    Idle,
    Loading,
    Playing,
    Paused,
    /// End of file reached; the last frame stays up with a replay option (FR-015).
    Ended,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PlayerSnapshot {
    pub scene_id: Option<String>,
    pub title: Option<String>,
    pub state: PlayerStateKind,
    pub position_seconds: f64,
    pub duration_seconds: Option<f64>,
    pub paused: bool,
    /// 0.25–4.0.
    pub speed: f64,
    /// 0–100.
    pub volume: f64,
    pub muted: bool,
    pub fullscreen: bool,
    /// mpv `hwdec-current`, e.g. `vaapi`, or `no` for software decoding.
    pub hwdec: Option<String>,
    /// Embedded tracks (FR-014); read-only in the spike.
    pub tracks: Vec<Track>,
    pub error: Option<PlayerError>,
}

impl Default for PlayerSnapshot {
    fn default() -> Self {
        Self {
            scene_id: None,
            title: None,
            state: PlayerStateKind::Idle,
            position_seconds: 0.0,
            duration_seconds: None,
            paused: false,
            speed: 1.0,
            volume: 100.0,
            muted: false,
            fullscreen: false,
            hwdec: None,
            tracks: Vec::new(),
            error: None,
        }
    }
}

/// Playback measurements for the spike's decision record (debug builds only).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PlayerStats {
    /// From opening a scene to its first rendered frame.
    pub open_to_first_frame_ms: Option<f64>,
    /// From the last seek to the next rendered frame.
    pub last_seek_to_frame_ms: Option<f64>,
    /// mpv `frame-drop-count` + `decoder-frame-drop-count`; `i32` keeps it TypeScript-safe.
    pub dropped_frames: i32,
    pub hwdec: Option<String>,
}
