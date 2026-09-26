//! Player commands and the `player-state` event (contracts/player-commands.md).
//!
//! The UI passes only a scene ID; the core builds and checks the direct stream URL
//! (FR-003) and never writes to Stash (FR-013).

use std::sync::Arc;

use player::{
    CacheLimits, FrameDirection, OpenRequest, Player, PlayerCommand, PlayerSnapshot,
    PlayerStateKind, PlayerStats,
};
use serde::Serialize;
use stash_core::adapter::scenes::{playable_scene, recent_scenes, test_scenes};
use stash_core::adapter::StashClient;
use stash_core::scenes::{is_direct_stream, SceneGroup, SceneListItem};
use stash_core::AppError;
use tauri::{AppHandle, Manager, State, WebviewWindow};
use tauri_specta::Event;

use crate::state::AppState;

/// Emitted on every player state change; position updates are throttled to ~4/s.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "player-state")]
pub struct PlayerStateEvent(pub PlayerSnapshot);

/// Forward the player's snapshots to the UI, and leave fullscreen whenever playback ends.
///
/// Fullscreen is the app window's, so closing the player by any path (✕, Esc, disconnect,
/// switching servers) must restore the window; otherwise it stays fullscreen with no video.
pub fn forward_player_state(app: AppHandle, player: &Arc<Player>) {
    let mut rx = player.subscribe();
    let player = Arc::clone(player);
    tauri::async_runtime::spawn(async move {
        let mut was_idle = true;
        while rx.changed().await.is_ok() {
            let snapshot = rx.borrow_and_update().clone();
            let idle = snapshot.state == PlayerStateKind::Idle;
            if idle && !was_idle {
                leave_fullscreen(&app, &player);
            }
            was_idle = idle;
            if let Err(e) = PlayerStateEvent(snapshot).emit(&app) {
                tracing::warn!(error = %e, "failed to emit player-state");
            }
        }
    });
}

fn leave_fullscreen(app: &AppHandle, player: &Player) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if window.is_fullscreen().unwrap_or(false) {
        if let Err(e) = window.set_fullscreen(false) {
            tracing::warn!(error = %e, "could not leave fullscreen");
        }
    }
    player.set_fullscreen_flag(false);
}

/// Keep the snapshot's fullscreen flag true to the window, including when the desktop takes
/// the window out of fullscreen (e.g. the Meta key on KDE).
pub fn sync_fullscreen_flag(window: &WebviewWindow, player: &Arc<Player>) {
    let player = Arc::clone(player);
    let observed = window.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Resized(_)) {
            let fullscreen = observed.is_fullscreen().unwrap_or(false);
            if player.snapshot().fullscreen != fullscreen {
                player.set_fullscreen_flag(fullscreen);
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

/// The spike's test set: a few random 4K, WMV, VP9, AV1, MPEG-4, and FLV scenes, in one
/// request (research R7). Each call reshuffles.
#[tauri::command]
#[specta::specta]
pub async fn list_test_scenes(state: State<'_, AppState>) -> Result<Vec<SceneGroup>, AppError> {
    let (client, _, _) = active_client(&state)?;
    test_scenes(&client).await
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
    let cache = scene.seek_cache().map(|c| CacheLimits {
        forward_bytes: c.forward_bytes,
        back_bytes: c.back_bytes,
    });
    tracing::info!(scene = %scene.id, sized_cache = cache.is_some(), "opening scene");
    player.open(OpenRequest {
        source: scene.stream_url.to_string(),
        scene_id: Some(scene.id),
        title: Some(scene.title),
        api_key,
        strict_tls,
        cache,
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

fn send(state: &AppState, command: PlayerCommand) -> Result<(), AppError> {
    player(state)?.dispatch(command);
    Ok(())
}

/// Play/pause (FR-008). All control commands return immediately; results arrive as
/// `player-state` events (contract invariant 1).
#[tauri::command]
#[specta::specta]
pub fn player_toggle_pause(state: State<'_, AppState>) -> Result<(), AppError> {
    send(&state, PlayerCommand::TogglePause)
}

#[tauri::command]
#[specta::specta]
pub fn player_set_paused(state: State<'_, AppState>, paused: bool) -> Result<(), AppError> {
    send(&state, PlayerCommand::SetPaused(paused))
}

/// Absolute seek: keyframe-fast while dragging, `exact` on release.
#[tauri::command]
#[specta::specta]
pub fn player_seek(
    state: State<'_, AppState>,
    position_seconds: f64,
    exact: bool,
) -> Result<(), AppError> {
    send(
        &state,
        PlayerCommand::Seek {
            position_seconds,
            exact,
        },
    )
}

/// Relative seek (±10 s for skip buttons and arrow keys).
#[tauri::command]
#[specta::specta]
pub fn player_seek_relative(state: State<'_, AppState>, seconds: f64) -> Result<(), AppError> {
    send(&state, PlayerCommand::SeekRelative(seconds))
}

/// Clamped to 0.25–4.0 by the player.
#[tauri::command]
#[specta::specta]
pub fn player_set_speed(state: State<'_, AppState>, speed: f64) -> Result<(), AppError> {
    send(&state, PlayerCommand::SetSpeed(speed))
}

/// Clamped to 0–100 by the player.
#[tauri::command]
#[specta::specta]
pub fn player_set_volume(state: State<'_, AppState>, volume: f64) -> Result<(), AppError> {
    send(&state, PlayerCommand::SetVolume(volume))
}

#[tauri::command]
#[specta::specta]
pub fn player_set_muted(state: State<'_, AppState>, muted: bool) -> Result<(), AppError> {
    send(&state, PlayerCommand::SetMuted(muted))
}

/// One frame forward or back; only while paused.
#[tauri::command]
#[specta::specta]
pub fn player_frame_step(
    state: State<'_, AppState>,
    direction: FrameDirection,
) -> Result<(), AppError> {
    send(&state, PlayerCommand::FrameStep(direction))
}

/// From the ended state: back to the start and play (FR-015).
#[tauri::command]
#[specta::specta]
pub fn player_replay(state: State<'_, AppState>) -> Result<(), AppError> {
    send(&state, PlayerCommand::Replay)
}

/// Enter or leave fullscreen (the window), mirrored into the player snapshot.
#[tauri::command]
#[specta::specta]
pub fn player_set_fullscreen(
    window: WebviewWindow,
    state: State<'_, AppState>,
    fullscreen: bool,
) -> Result<(), AppError> {
    window
        .set_fullscreen(fullscreen)
        .map_err(|e| AppError::Internal {
            message: format!("could not change fullscreen: {e}"),
        })?;
    player(&state)?.set_fullscreen_flag(fullscreen);
    Ok(())
}

/// Measurements for the decision record. Debug builds only.
#[tauri::command]
#[specta::specta]
pub fn player_stats(state: State<'_, AppState>) -> Result<PlayerStats, AppError> {
    if !cfg!(debug_assertions) {
        return Err(AppError::Internal {
            message: "player_stats is only available in debug builds".into(),
        });
    }
    Ok(player(&state)?.stats())
}
