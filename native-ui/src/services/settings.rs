//! The app's own playback preferences (008 FR-012, FR-017): one small file in the config folder,
//! the same for every server. Written atomically; a missing or damaged file gives the defaults.

use serde::{Deserialize, Serialize};
use stash_core::shell::store::{JsonStore, Versioned};
use stash_core::AppError;

/// Settings → Playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackPreferences {
    /// "Resume where you left off": Play starts at the scene's resume point.
    pub resume: bool,
    /// "Save play history": resume points, playing time, and play counts go to Stash.
    pub save_history: bool,
    /// The image slideshow's seconds per image (012 FR-014), 1–60; files from before 012 read 5.
    #[serde(default = "default_delay")]
    pub slideshow_delay_s: u32,
}

/// The slideshow's default delay.
pub const DEFAULT_SLIDESHOW_DELAY_S: u32 = 5;

fn default_delay() -> u32 {
    DEFAULT_SLIDESHOW_DELAY_S
}

impl Default for PlaybackPreferences {
    fn default() -> Self {
        Self {
            resume: true,
            save_history: true,
            slideshow_delay_s: DEFAULT_SLIDESHOW_DELAY_S,
        }
    }
}

/// `playback.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct PreferencesFile {
    version: u32,
    #[serde(default)]
    playback: PlaybackPreferences,
}

impl Default for PreferencesFile {
    fn default() -> Self {
        Self {
            version: Self::VERSION,
            playback: PlaybackPreferences::default(),
        }
    }
}

impl Versioned for PreferencesFile {
    const VERSION: u32 = 1;
}

pub fn load(path: &std::path::Path) -> PlaybackPreferences {
    JsonStore::<PreferencesFile>::new(path).load().playback
}

pub fn save(path: &std::path::Path, playback: PlaybackPreferences) -> Result<(), AppError> {
    JsonStore::<PreferencesFile>::new(path).save(&PreferencesFile {
        version: PreferencesFile::VERSION,
        playback,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_from_before_the_slideshow_reads_the_default_delay() {
        let old: PlaybackPreferences =
            serde_json::from_str(r#"{"resume":false,"save_history":true}"#).expect("parse");
        assert!(!old.resume);
        assert_eq!(old.slideshow_delay_s, 5);
    }

    #[test]
    fn both_switches_default_on_and_round_trip() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("playback.json");
        assert_eq!(load(&path), PlaybackPreferences::default());
        let off = PlaybackPreferences {
            resume: false,
            save_history: false,
            slideshow_delay_s: 12,
        };
        save(&path, off).expect("save");
        assert_eq!(load(&path), off);
        std::fs::write(&path, "not json").expect("damage");
        assert_eq!(load(&path), PlaybackPreferences::default());
    }
}
