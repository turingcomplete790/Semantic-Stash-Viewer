//! Player commands and the `player-state` event (contracts/player-commands.md).
//!
//! The UI passes only a scene ID; the core builds and checks the direct stream URL
//! (FR-003) and never writes to Stash (FR-013).

use std::sync::Arc;

use player::{OpenRequest, Player, PlayerSnapshot};
use serde::Serialize;
use stash_core::adapter::scenes::{playable_scene, recent_scenes};
use stash_core::adapter::StashClient;
use stash_core::scenes::{is_direct_stream, SceneListItem};
use stash_core::AppError;
use tauri::{AppHandle, State};
use tauri_specta::Event;

use crate::state::AppState;

/// Emitted on every player state change; position updates are throttled to ~4/s.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "player-state")]
pub struct PlayerStateEvent(pub PlayerSnapshot);

/// Forward the player's snapshots to the UI.
pub fn forward_player_state(app: AppHandle, player: &Arc<Player>) {
    let mut rx = player.subscribe();
    tauri::async_runtime::spawn(async move {
        while rx.changed().await.is_ok() {
            let snapshot = rx.borrow_and_update().clone();
            if let Err(e) = PlayerStateEvent(snapshot).emit(&app) {
                tracing::warn!(error = %e, "failed to emit player-state");
            }
        }
    });
}

fn player(state: &AppState) -> Result<&Arc<Player>, AppError> {
    state.player.as_ref().ok_or_else(|| AppError::Internal {
        message: "the video player isn't available (libmpv failed to start; see the log)".into(),
    })
}

/// A client for the active profile. Fails if no server is active.
fn active_client(state: &AppState) -> Result<(StashClient, Option<String>, bool), AppError> {
    let id = state
        .manager
        .active_profile_id()
        .ok_or_else(|| AppError::Internal {
            message: "not connected to a server".into(),
        })?;
    let profile = state
        .profiles
        .lock()
        .map_err(|_| AppError::Internal {
            message: "state lock poisoned".into(),
        })?
        .get(id)
        .cloned()
        .ok_or(AppError::ProfileNotFound { id })?;
    let client = StashClient::new(
        profile.base_url.clone(),
        profile.strict_tls,
        profile.api_key.clone(),
    )?;
    Ok((client, profile.api_key, profile.strict_tls))
}

/// Current player state, for UI hydration.
#[tauri::command]
#[specta::specta]
pub fn player_snapshot(state: State<'_, AppState>) -> Result<PlayerSnapshot, AppError> {
    Ok(player(&state)?.snapshot())
}

/// The 20 most recently added scenes on the active server (FR-001).
#[tauri::command]
#[specta::specta]
pub async fn list_recent_scenes(
    state: State<'_, AppState>,
) -> Result<Vec<SceneListItem>, AppError> {
    let (client, _, _) = active_client(&state)?;
    recent_scenes(&client).await
}

/// Look up a scene and start playing its direct stream (FR-001, FR-003, FR-004).
#[tauri::command]
#[specta::specta]
pub async fn player_open(
    state: State<'_, AppState>,
    scene_id: String,
) -> Result<PlayerSnapshot, AppError> {
    let player = Arc::clone(player(&state)?);
    let (client, api_key, strict_tls) = active_client(&state)?;
    let scene = playable_scene(&client, scene_id.trim()).await?;
    if !is_direct_stream(&scene.stream_url) {
        // Can't happen (the URL is built by stash-core), but never hand mpv a transcode URL.
        return Err(AppError::Internal {
            message: "refusing to play a non-direct stream".into(),
        });
    }
    tracing::info!(scene = %scene.id, "opening scene");
    player.open(OpenRequest {
        source: scene.stream_url.to_string(),
        scene_id: Some(scene.id),
        title: Some(scene.title),
        api_key,
        strict_tls,
    });
    Ok(player.snapshot())
}

/// Stop playback and release audio/video (FR-007).
#[tauri::command]
#[specta::specta]
pub fn player_close(state: State<'_, AppState>) -> Result<(), AppError> {
    player(&state)?.close();
    Ok(())
}
