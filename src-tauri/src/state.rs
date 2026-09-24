//! Shared app state managed by Tauri.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use stash_core::profiles::ProfileStore;
use stash_core::AppError;
use tokio_util::sync::CancellationToken;

pub struct AppState {
    pub profiles: Mutex<ProfileStore>,
    /// In-flight cancellable requests, keyed by the UI's request id.
    pub requests: Mutex<HashMap<String, CancellationToken>>,
}

impl AppState {
    pub fn open(profiles_path: &Path) -> Result<Self, AppError> {
        let mut store = ProfileStore::open(profiles_path)?;
        if let Some(notice) = store.take_notice() {
            tracing::warn!(?notice, "profile store notice");
        }
        Ok(Self {
            profiles: Mutex::new(store),
            requests: Mutex::new(HashMap::new()),
        })
    }
}
