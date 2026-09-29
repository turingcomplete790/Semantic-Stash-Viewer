//! App shell commands (004 contracts/shell-commands.md): tabs, notifications, the log folder,
//! and app info. File I/O stays in `stash-core` (Principle III).

use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use stash_core::shell::notifications::Notification;
use stash_core::shell::tabs::TabSet;
use stash_core::AppError;
use tauri::{AppHandle, State};
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

/// The command that opens `dir` in the system file manager (research R8). The path is always the
/// app's own directory, never something from the UI.
pub(crate) fn opener_command(dir: &Path) -> Command {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    let mut command = Command::new(program);
    command.arg(dir);
    command
}

/// Open the viewer's log folder (Settings → Troubleshooting, FR-027).
#[tauri::command]
#[specta::specta]
pub fn open_log_folder(app: AppHandle) -> Result<(), AppError> {
    let dir = crate::log_dir(&app)?;
    std::fs::create_dir_all(&dir)?;
    let mut child = opener_command(&dir)
        .spawn()
        .map_err(|e| AppError::OpenFailed {
            detail: format!("couldn't start the file manager: {e}"),
        })?;
    // Reap the opener when it exits (it hands off to the file manager right away).
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// The viewer's version, for Settings → About (FR-028).
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct AppInfo {
    pub version: String,
}

#[tauri::command]
#[specta::specta]
pub fn app_info(app: AppHandle) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_the_given_directory_with_the_platform_opener() {
        let dir = Path::new("/home/someone/.local/share/semantic-stash-viewer/logs");
        let command = opener_command(dir);
        #[cfg(target_os = "linux")]
        assert_eq!(command.get_program(), "xdg-open");
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(
            args,
            [dir.as_os_str()],
            "only the fixed directory is passed"
        );
    }
}
