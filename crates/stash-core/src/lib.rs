//! Headless core for Semantic Stash Viewer.
//!
//! Everything that talks to Stash, and all connection and profile logic, lives here.
//! This crate must never depend on Tauri (constitution Principle III).

pub mod adapter;
pub mod connection;
pub mod error;
pub mod profiles;

pub use error::AppError;
