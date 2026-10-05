//! Headless mpv playback session for Semantic Stash Viewer.
//!
//! Owns the mpv core: options, commands, property observation, and the render-context wrapper.
//! This crate must never depend on a UI toolkit (constitution Principle III); the window-system
//! code that hosts the video lives in `native-ui/src/player/video/`.

// The unsafe-code gate (constitution C4): every unsafe block documented, one unsafe operation per
// block, and unsafe functions' bodies checked like any other code.
#![deny(
    unsafe_op_in_unsafe_fn,
    clippy::undocumented_unsafe_blocks,
    clippy::multiple_unsafe_ops_per_block
)]

pub mod commands;
pub mod error;
pub mod render;
pub mod session;
pub mod snapshot;
mod timing;
pub mod tracks;

pub use commands::{FrameDirection, PlayerCommand};
pub use error::PlayerError;
pub use render::{GetProcAddress, RenderOwner, Renderer};
pub use session::{CacheLimits, OpenRequest, Player, PlayerConfig, MAX_SPEED, MIN_SPEED};
pub use snapshot::{PlayerSnapshot, PlayerStateKind, PlayerStats};
pub use tracks::{Track, TrackKind};
