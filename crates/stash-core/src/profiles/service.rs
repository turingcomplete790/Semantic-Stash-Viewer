//! Profile operations that combine validation against Stash with persistence.
//!
//! The store lives behind a `std::sync::Mutex`; it is never held across an `.await`.

use std::sync::Mutex;

use chrono::Utc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::model::{resolve_display_name, ProfileDraft, ServerProfile};
use super::store::ProfileStore;
use crate::connection::address::candidates;
use crate::connection::connect::{test_connection, ConnectOptions, ConnectOutcome};
use crate::connection::failure::ConnectFailure;
use crate::error::AppError;

fn lock(store: &Mutex<ProfileStore>) -> Result<std::sync::MutexGuard<'_, ProfileStore>, AppError> {
    store.lock().map_err(|_| AppError::Internal {
        message: "profile store lock poisoned".into(),
    })
}

/// Validate the draft against the server, then save it as a new profile (FR-008, FR-009).
///
/// Saves only on success. The **final** URL after redirects becomes `base_url`, and the
/// trimmed API key is stored on the profile. The new profile becomes the last-used one.
pub async fn create_profile(
    store: &Mutex<ProfileStore>,
    draft: &ProfileDraft,
    cancel: &CancellationToken,
    options: ConnectOptions,
) -> Result<(ServerProfile, ConnectOutcome), AppError> {
    precheck_name(draft)?;
    let outcome = test_connection(draft, cancel, options).await?;

    let mut store = lock(store)?;
    if let Some(existing) = store.find_by_base_url(&outcome.base_url) {
        return Err(ConnectFailure::DuplicateProfile {
            existing_id: existing.id,
        }
        .into());
    }

    let now = Utc::now();
    let profile = ServerProfile {
        id: Uuid::new_v4(),
        display_name: resolve_display_name(draft.display_name.as_deref(), &outcome.base_url)?,
        base_url: outcome.base_url.clone(),
        strict_tls: draft.strict_tls,
        api_key: draft.normalized_api_key(),
        created_at: now,
        last_used_at: Some(now),
    };
    store.insert(profile.clone())?;
    store.set_last_used(Some(profile.id))?;
    tracing::info!(id = %profile.id, url = %profile.base_url, "profile created");
    Ok((profile, outcome))
}

/// Update a profile from a full draft (FR-013).
///
/// The new settings are checked by connecting first. If the check fails, the old settings stay
/// unless `force` is true, in which case the draft is saved as entered (the address must still
/// be valid). The draft's `api_key` replaces the stored key; `null` or blank clears it.
pub async fn update_profile(
    store: &Mutex<ProfileStore>,
    id: Uuid,
    draft: &ProfileDraft,
    force: bool,
    cancel: &CancellationToken,
    options: ConnectOptions,
) -> Result<ServerProfile, AppError> {
    let existing = lock(store)?
        .get(id)
        .cloned()
        .ok_or(AppError::ProfileNotFound { id })?;
    precheck_name(draft)?;

    let base_url = match test_connection(draft, cancel, options).await {
        Ok(outcome) => outcome.base_url,
        Err(AppError::Cancelled) => return Err(AppError::Cancelled),
        Err(e) if !force => return Err(e),
        Err(e) => {
            tracing::info!(%id, error = %e, "saving profile despite failed check");
            candidates(&draft.address)?
                .into_iter()
                .next()
                .ok_or_else(|| AppError::Internal {
                    message: "no candidate URL".into(),
                })?
        }
    };

    let mut store = lock(store)?;
    if let Some(other) = store.find_by_base_url(&base_url) {
        if other.id != id {
            return Err(ConnectFailure::DuplicateProfile {
                existing_id: other.id,
            }
            .into());
        }
    }
    let updated = ServerProfile {
        display_name: resolve_display_name(draft.display_name.as_deref(), &base_url)?,
        base_url,
        strict_tls: draft.strict_tls,
        api_key: draft.normalized_api_key(),
        ..existing
    };
    store.replace(updated.clone())?;
    tracing::info!(%id, url = %updated.base_url, "profile updated");
    Ok(updated)
}

/// Record a successful connect: sets `last_used_at` and `last_used_profile_id` (FR-014).
pub fn mark_used(store: &Mutex<ProfileStore>, id: Uuid) -> Result<(), AppError> {
    let mut store = lock(store)?;
    let Some(profile) = store.get(id).cloned() else {
        return Ok(()); // deleted meanwhile
    };
    store.replace(ServerProfile {
        last_used_at: Some(Utc::now()),
        ..profile
    })?;
    store.set_last_used(Some(id))
}

/// Delete a profile (FR-012; the UI confirms first). Its API key goes with it, since the key is
/// stored on the profile. Clears `last_used_profile_id` if it pointed here. Disconnecting an
/// active session is the caller's job.
pub fn delete_profile(store: &Mutex<ProfileStore>, id: Uuid) -> Result<ServerProfile, AppError> {
    let removed = lock(store)?.remove(id)?;
    tracing::info!(%id, "profile deleted");
    Ok(removed)
}

/// Reorder profiles to match `ids`; unlisted profiles keep their order at the end (FR-011).
pub fn reorder_profiles(store: &Mutex<ProfileStore>, ids: &[Uuid]) -> Result<(), AppError> {
    lock(store)?.reorder(ids)
}

/// Reject an over-long display name before touching the network.
fn precheck_name(draft: &ProfileDraft) -> Result<(), AppError> {
    if let Some(name) = draft.display_name.as_deref() {
        let placeholder =
            url::Url::parse("http://placeholder").map_err(|e| AppError::Internal {
                message: e.to_string(),
            })?;
        resolve_display_name(Some(name), &placeholder)?;
    }
    Ok(())
}
