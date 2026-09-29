//! App shell commands (004 contracts/shell-commands.md): tabs and notifications; the log folder
//! and app info arrive with US4. File I/O stays in `stash-core` (Principle III).

use std::sync::Arc;

use stash_core::shell::notifications::Notification;
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

/// All notifications, newest first (updates arrive as `notifications-changed`).
#[tauri::command]
#[specta::specta]
pub fn notifications_list(state: State<'_, AppState>) -> Vec<Notification> {
    state.notifications.list()
}

/// Mark notifications read (when the centre is opened).
#[tauri::command]
#[specta::specta]
pub fn notifications_mark_read(state: State<'_, AppState>, ids: Vec<String>) {
    state.notifications.mark_read(&ids);
}

#[tauri::command]
#[specta::specta]
pub fn notification_dismiss(state: State<'_, AppState>, id: String) {
    state.notifications.dismiss(&id);
}

/// Remove every notification except jobs that are still running.
#[tauri::command]
#[specta::specta]
pub fn notifications_dismiss_all(state: State<'_, AppState>) {
    state.notifications.dismiss_all();
}
