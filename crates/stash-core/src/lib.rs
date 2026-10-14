//! Headless core for Semantic Stash Viewer.
//!
//! Everything that talks to Stash, and all connection and profile logic, lives here.
//! This crate must never depend on a UI toolkit (constitution Principle III).

// No unsafe code in the core (constitution C4).
#![forbid(unsafe_code)]

pub mod adapter;
pub mod browse;
pub mod cache;
pub mod connection;
pub mod error;
pub mod images;
pub mod jobs;
pub mod playback;
pub mod previews;
pub mod profiles;
pub mod scenes;
pub mod semantic;
pub mod shell;
pub mod sprites;
pub mod thumbs;

pub use error::AppError;
