//! Tauri shell for Semantic Stash Viewer: a thin command/event layer over `stash-core`.
//! No domain logic lives here (constitution Principle III).

mod commands;
mod events;
mod logging;
#[cfg(debug_assertions)]
mod measure;
mod player_commands;
mod shell_commands;
mod state;
#[cfg(target_os = "linux")]
mod video_surface;

use std::path::PathBuf;
use std::sync::Arc;

use player::{Player, PlayerConfig};

use tauri::Manager;
use tauri_specta::{collect_commands, collect_events, Builder};

use state::AppState;

/// Directory name under the platform config/data dirs (research R11).
const APP_DIR: &str = "semantic-stash-viewer";

/// The viewer's log directory (`~/.local/share/semantic-stash-viewer/logs/` on Linux).
pub(crate) fn log_dir(app: &tauri::AppHandle) -> Result<PathBuf, stash_core::AppError> {
    app.path()
        .local_data_dir()
        .map(|dir| dir.join(APP_DIR).join("logs"))
        .map_err(|e| stash_core::AppError::Internal {
            message: format!("no data directory: {e}"),
        })
}

/// Where the generated TypeScript bindings live.
pub const BINDINGS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../ui/src/bindings.ts");

/// Every command and event exposed to the UI. Shared by the app and the `export-bindings` bin.
pub fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::list_profiles,
            commands::test_connection,
            commands::cancel_request,
            commands::create_profile,
            commands::update_profile,
            commands::delete_profile,
            commands::reorder_profiles,
            commands::connect,
            commands::disconnect,
            commands::get_connection_snapshot,
            player_commands::player_snapshot,
            player_commands::list_recent_scenes,
            player_commands::list_test_scenes,
            player_commands::player_open,
            player_commands::player_close,
            player_commands::player_toggle_pause,
            player_commands::player_set_paused,
            player_commands::player_seek,
            player_commands::player_seek_relative,
            player_commands::player_set_speed,
            player_commands::player_set_volume,
            player_commands::player_set_muted,
            player_commands::player_frame_step,
            player_commands::player_replay,
            player_commands::player_set_fullscreen,
            player_commands::player_set_viewport,
            player_commands::player_set_video_visible,
            player_commands::scene_screenshot_url,
            player_commands::debug_open_scene,
            player_commands::debug_bench_enabled,
            player_commands::debug_report,
            shell_commands::shell_load_tabs,
            shell_commands::shell_save_tabs,
            shell_commands::notifications_list,
            shell_commands::notifications_mark_read,
            shell_commands::notification_dismiss,
            shell_commands::notifications_dismiss_all,
            shell_commands::open_log_folder,
            shell_commands::app_info,
            player_commands::player_stats,
        ])
        .events(collect_events![
            events::ConnectionStateEvent,
            events::ProfilesChangedEvent,
            player_commands::PlayerStateEvent,
            events::NotificationsChangedEvent
        ])
}

/// Write `ui/src/bindings.ts`.
pub fn export_bindings(builder: &Builder<tauri::Wry>) -> Result<(), String> {
    builder
        .export(
            specta_typescript::Typescript::default().header("// @ts-nocheck\n"),
            BINDINGS_PATH,
        )
        .map_err(|e| e.to_string())
}

pub fn run() {
    let builder = specta_builder();

    #[cfg(debug_assertions)]
    if let Err(e) = export_bindings(&builder) {
        eprintln!("failed to export TypeScript bindings: {e}");
    }

    tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            let data_dir = app.path().local_data_dir()?.join(APP_DIR);
            let log_guard = logging::init(&data_dir.join("logs"));
            app.manage(log_guard);

            let config_dir: PathBuf = app.path().config_dir()?.join(APP_DIR);
            let runtime = tauri::async_runtime::handle().inner().clone();
            let player = start_player(app);
            let state = AppState::open(
                &config_dir.join("profiles.json"),
                &data_dir.join("shell").join("tabs.json"),
                &data_dir.join("shell").join("notifications.json"),
                runtime,
                player,
            )?;
            events::forward_connection_state(
                app.handle().clone(),
                &state.manager,
                std::sync::Arc::clone(&state.profiles),
            );
            events::forward_notifications(app.handle().clone(), &state.notifications);
            auto_connect(&state);
            app.manage(state);
            #[cfg(debug_assertions)]
            measure::start_if_requested(app.handle());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Semantic Stash Viewer");
}

/// Reconnect to the last-used profile at launch without blocking window creation (FR-014).
fn auto_connect(state: &AppState) {
    let last = state.profiles.lock().ok().and_then(|store| {
        let id = store.last_used_profile_id()?;
        store.get(id).cloned()
    });
    if let Some(profile) = last {
        tracing::info!(id = %profile.id, "auto-connecting to last-used profile");
        commands::start_session(state, &profile, true);
    }
}

/// Start mpv (render mode) and host its video under the webview. Playback is optional: if
/// libmpv or the video surface fails, the app keeps working without it.
fn start_player(app: &tauri::App) -> Option<Arc<Player>> {
    let player = match Player::new(PlayerConfig::render()) {
        Ok(player) => Arc::new(player),
        Err(e) => {
            tracing::error!(error = %e, "video player unavailable");
            return None;
        }
    };
    player_commands::forward_player_state(app.handle().clone(), &player);

    match app.get_webview_window("main") {
        Some(window) => {
            player_commands::sync_fullscreen_flag(&window, &player);
            #[cfg(target_os = "linux")]
            if let Err(e) = video_surface::install(&window, Arc::clone(&player)) {
                tracing::error!(error = %e, "could not install the video surface");
            }
        }
        None => tracing::error!("main window not found; no video surface"),
    }

    Some(player)
}
