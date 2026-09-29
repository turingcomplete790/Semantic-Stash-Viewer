//! Tauri commands (contracts/tauri-commands.md). Each is a thin call into `stash-core`.

use std::time::Duration;

use stash_core::connection::connect::{self, ConnectOptions, TestResult};
use stash_core::connection::manager::{ConnectRequest, Target};
use stash_core::connection::snapshot::{ConnectionSnapshot, SessionState};
use stash_core::profiles::{service, ProfileDraft, ProfileSummary, ServerProfile};
use stash_core::AppError;
use tauri::{AppHandle, State};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::events::emit_profiles_changed;
use crate::state::AppState;

fn lock_err<T>(_: T) -> AppError {
    AppError::Internal {
        message: "state lock poisoned".into(),
    }
}

/// Saved profiles, in the user's order.
#[tauri::command]
#[specta::specta]
pub fn list_profiles(state: State<'_, AppState>) -> Result<Vec<ProfileSummary>, AppError> {
    let store = state.profiles.lock().map_err(lock_err)?;
    Ok(store.list().iter().map(ProfileSummary::from).collect())
}

/// Check a draft against the server without saving it (US1). Cancellable via `cancel_request`
/// with the same `request_id`; times out after 15 s (FR-006).
#[tauri::command]
#[specta::specta]
pub async fn test_connection(
    state: State<'_, AppState>,
    draft: ProfileDraft,
    request_id: String,
) -> Result<TestResult, AppError> {
    let token = register(&state, &request_id)?;
    let result = connect::test_connection(&draft, &token, ConnectOptions::default()).await;
    unregister(&state, &request_id);
    result.map(|outcome| TestResult::from(&outcome))
}

/// Cancel an in-flight `test_connection` or `connect`. Unknown ids are ignored.
#[tauri::command]
#[specta::specta]
pub fn cancel_request(state: State<'_, AppState>, request_id: String) -> Result<(), AppError> {
    if let Some(token) = state.requests.lock().map_err(lock_err)?.remove(&request_id) {
        token.cancel();
    }
    Ok(())
}

/// Re-check the draft and save it as a new profile (FR-008). Fails with `duplicateProfile` if
/// the server is already saved.
#[tauri::command]
#[specta::specta]
pub async fn create_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    draft: ProfileDraft,
) -> Result<ProfileSummary, AppError> {
    let (profile, _outcome) = service::create_profile(
        &state.profiles,
        &draft,
        &CancellationToken::new(),
        ConnectOptions::default(),
    )
    .await?;
    emit_profiles_changed(&app, &state);
    Ok(ProfileSummary::from(&profile))
}

/// Make a profile active and connect to it. Resolves once the attempt settles (connected,
/// offline, auth failed, or failed); progress also arrives as `connection-state` events.
/// Cancellable via `cancel_request`, which returns the session to Idle.
#[tauri::command]
#[specta::specta]
pub async fn connect(
    state: State<'_, AppState>,
    profile_id: Uuid,
    request_id: String,
) -> Result<ConnectionSnapshot, AppError> {
    let profile = get_profile(&state, profile_id)?;
    let mut rx = state.manager.subscribe();
    let token = register(&state, &request_id)?;
    start_session(&state, &profile, false);

    // Generous upper bound; the probe itself times out after 15 s.
    let deadline = tokio::time::sleep(Duration::from_secs(30));
    tokio::pin!(deadline);
    let result = loop {
        tokio::select! {
            () = token.cancelled() => {
                state.manager.disconnect();
                break Err(AppError::Cancelled);
            }
            () = &mut deadline => break Ok(state.manager.snapshot()),
            received = rx.recv() => match received {
                Ok(s) if s.profile_id == Some(profile_id)
                    && !matches!(s.state, SessionState::Connecting { .. }) => break Ok(s),
                Ok(_) => {}
                Err(_) => break Ok(state.manager.snapshot()),
            },
        }
    };
    unregister(&state, &request_id);
    result
}

/// End the session (Idle).
#[tauri::command]
#[specta::specta]
pub fn disconnect(state: State<'_, AppState>) {
    stop_playback(&state);
    state.manager.disconnect();
}

/// Current connection state, for UI hydration on window load.
#[tauri::command]
#[specta::specta]
pub fn get_connection_snapshot(state: State<'_, AppState>) -> ConnectionSnapshot {
    state.manager.snapshot()
}

/// Update a profile from a full draft (FR-013). The new settings are checked first; with
/// `force`, they are saved even if the check fails. If the profile is active, the session
/// reconnects with the new settings.
#[tauri::command]
#[specta::specta]
pub async fn update_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_id: Uuid,
    draft: ProfileDraft,
    force: bool,
) -> Result<ProfileSummary, AppError> {
    let updated = service::update_profile(
        &state.profiles,
        profile_id,
        &draft,
        force,
        &CancellationToken::new(),
        ConnectOptions::default(),
    )
    .await?;
    if state.manager.active_profile_id() == Some(profile_id) {
        start_session(&state, &updated, false);
    }
    emit_profiles_changed(&app, &state);
    Ok(ProfileSummary::from(&updated))
}

/// Delete a profile (FR-012; the UI confirms first). If it was active, the session goes Idle.
#[tauri::command]
#[specta::specta]
pub fn delete_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_id: Uuid,
) -> Result<(), AppError> {
    service::delete_profile(&state.profiles, profile_id)?;
    if let Err(e) = state.tabs.delete(profile_id) {
        tracing::warn!(error = %e, "could not delete the profile's saved tabs");
    }
    if state.manager.active_profile_id() == Some(profile_id) {
        stop_playback(&state);
        state.manager.disconnect();
    }
    emit_profiles_changed(&app, &state);
    Ok(())
}

/// Reorder profiles to match `profile_ids` (FR-011).
#[tauri::command]
#[specta::specta]
pub fn reorder_profiles(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_ids: Vec<Uuid>,
) -> Result<(), AppError> {
    service::reorder_profiles(&state.profiles, &profile_ids)?;
    emit_profiles_changed(&app, &state);
    Ok(())
}

/// Start a session for a profile (also used for auto-connect at launch).
pub fn start_session(state: &AppState, profile: &ServerProfile, is_launch: bool) {
    stop_playback(state);
    state.manager.connect(ConnectRequest {
        target: Target::from(profile),
        is_launch,
        has_connected_before: profile.last_used_at.is_some(),
    });
}

/// Stop any playback when the server changes or goes away (FR-007).
fn stop_playback(state: &AppState) {
    if let Some(player) = &state.player {
        player.close();
    }
}

fn get_profile(state: &AppState, id: Uuid) -> Result<ServerProfile, AppError> {
    state
        .profiles
        .lock()
        .map_err(lock_err)?
        .get(id)
        .cloned()
        .ok_or(AppError::ProfileNotFound { id })
}

fn register(state: &AppState, request_id: &str) -> Result<CancellationToken, AppError> {
    let token = CancellationToken::new();
    state
        .requests
        .lock()
        .map_err(lock_err)?
        .insert(request_id.to_owned(), token.clone());
    Ok(token)
}

fn unregister(state: &AppState, request_id: &str) {
    if let Ok(mut requests) = state.requests.lock() {
        requests.remove(request_id);
    }
}
