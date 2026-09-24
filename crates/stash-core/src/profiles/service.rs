//! Profile operations that combine validation against Stash with persistence.
//!
//! The store lives behind a `std::sync::Mutex`; it is never held across an `.await`.

use std::sync::Mutex;

use chrono::Utc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::model::{resolve_display_name, ProfileDraft, ServerProfile};
use super::store::ProfileStore;
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
    // Reject an over-long name before touching the network.
    if let Some(name) = draft.display_name.as_deref() {
        let placeholder =
            url::Url::parse("http://placeholder").map_err(|e| AppError::Internal {
                message: e.to_string(),
            })?;
        resolve_display_name(Some(name), &placeholder)?;
    }

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
