//! Tauri shell for Semantic Stash Viewer: a thin command/event layer over `stash-core`.
//! No domain logic lives here (constitution Principle III).

mod commands;
mod events;
mod logging;
mod state;

use std::path::PathBuf;

use tauri::Manager;
use tauri_specta::{collect_commands, collect_events, Builder};

use state::AppState;

/// Directory name under the platform config/data dirs (research R11).
const APP_DIR: &str = "semantic-stash-viewer";

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
        ])
        .events(collect_events![
            events::ConnectionStateEvent,
            events::ProfilesChangedEvent
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
            let state = AppState::open(&config_dir.join("profiles.json"), runtime)?;
            events::forward_connection_state(app.handle().clone(), &state.manager);
            auto_connect(&state);
            app.manage(state);
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
