//! Tauri commands (contracts/tauri-commands.md). Each is a thin call into `stash-core`.

use stash_core::connection::connect::{self, ConnectOptions, TestResult};
use stash_core::profiles::{service, ProfileDraft, ProfileSummary};
use stash_core::AppError;
use tauri::State;
use tokio_util::sync::CancellationToken;

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

/// Cancel an in-flight `test_connection` (or `connect`, later). Unknown ids are ignored.
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
    Ok(ProfileSummary::from(&profile))
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
