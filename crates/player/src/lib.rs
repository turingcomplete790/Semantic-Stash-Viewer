//! Headless mpv playback session for Semantic Stash Viewer.
//!
//! Owns the mpv core: options, commands, property observation, and the render-context wrapper.
//! This crate must never depend on Tauri or GTK (constitution Principle III); the window-system
//! code that hosts the video lives in `src-tauri/src/video_surface/`.

pub mod error;
pub mod render;
pub mod session;
pub mod snapshot;
pub mod tracks;

pub use error::PlayerError;
pub use render::{GetProcAddress, Renderer};
pub use session::{OpenRequest, Player, PlayerConfig, MAX_SPEED, MIN_SPEED};
pub use snapshot::{PlayerSnapshot, PlayerStateKind};
pub use tracks::{Track, TrackKind};
