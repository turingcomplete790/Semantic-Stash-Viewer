//! App shell commands (004 contracts/shell-commands.md): tabs for now; notifications, the log
//! folder, and app info arrive with US3/US4. File I/O stays in `stash-core` (Principle III).

use std::sync::Arc;

use stash_core::shell::tabs::TabSet;
use stash_core::AppError;
use tauri::State;
use uuid::Uuid;

use crate::state::AppState;

/// The saved tabs for a server profile, or `None` (the UI then starts with one Home tab).
#[tauri::command]
#[specta::specta]
pub fn shell_load_tabs(state: State<'_, AppState>, profile_id: Uuid) -> Option<TabSet> {
    state.tabs.load(profile_id)
}

/// Validate and save a profile's tabs. Returns once validated; the file is written in the
/// background so the UI never waits on disk (Principle VI).
#[tauri::command]
#[specta::specta]
pub fn shell_save_tabs(
    state: State<'_, AppState>,
    profile_id: Uuid,
    tabs: TabSet,
) -> Result<(), AppError> {
    tabs.validate()?;
    let store = Arc::clone(&state.tabs);
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = store.save(profile_id, &tabs) {
            tracing::warn!(error = %e, "could not save tabs");
        }
    });
    Ok(())
}
