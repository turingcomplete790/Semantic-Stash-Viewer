//! Events from the core to the UI (contracts/tauri-commands.md "Events").

use serde::Serialize;
use stash_core::connection::snapshot::ConnectionSnapshot;
use stash_core::profiles::ProfileSummary;
use tauri::AppHandle;
use tauri_specta::Event;
use tokio::sync::broadcast::error::RecvError;

use crate::state::{AppState, Manager};

/// Emitted on every connection state transition.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "connection-state")]
pub struct ConnectionStateEvent(pub ConnectionSnapshot);

/// Forward every manager snapshot to the UI as a `connection-state` event.
pub fn forward_connection_state(app: AppHandle, manager: &Manager) {
    let mut rx = manager.subscribe();
    let manager = manager.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let snapshot = match rx.recv().await {
                Ok(snapshot) => snapshot,
                // Missed some transitions: send the current state so the UI catches up.
                Err(RecvError::Lagged(_)) => manager.snapshot(),
                Err(RecvError::Closed) => break,
            };
            if let Err(e) = ConnectionStateEvent(snapshot).emit(&app) {
                tracing::warn!(error = %e, "failed to emit connection-state");
            }
        }
    });
}

/// Emitted after every profile create, update, delete, or reorder, with the full list.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "profiles-changed")]
pub struct ProfilesChangedEvent(pub Vec<ProfileSummary>);

/// Send the current profile list to the UI.
pub fn emit_profiles_changed(app: &AppHandle, state: &AppState) {
    let profiles = match state.profiles.lock() {
        Ok(store) => store.list().iter().map(ProfileSummary::from).collect(),
        Err(_) => return,
    };
    if let Err(e) = ProfilesChangedEvent(profiles).emit(app) {
        tracing::warn!(error = %e, "failed to emit profiles-changed");
    }
}
