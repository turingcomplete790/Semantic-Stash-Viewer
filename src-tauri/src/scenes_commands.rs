//! Scene browsing (005 P1; contracts/scenes-browse.md): pages of scene cards through the view
//! cache, and the sort menu. Read-only.

use serde::Serialize;
use stash_core::adapter::scenes::find_scenes_page;
use stash_core::cache::refresh::RefreshPolicy;
use stash_core::cache::Cached;
use stash_core::scenes::paging::validate_page_size;
use stash_core::scenes::query::{SceneQuery, SceneSort};
use stash_core::scenes::ScenePage;
use stash_core::AppError;
use tauri::State;

use crate::cache_commands::read_cached;
use crate::state::AppState;

/// One page of scenes for a query at a page size (20…1000; default 50): the cached copy at once
/// when there is one, refreshed quietly after 5 s (003). One request per page (constitution IV).
#[tauri::command]
#[specta::specta]
pub async fn scenes_page(
    state: State<'_, AppState>,
    query: SceneQuery,
    page: u32,
    page_size: u32,
) -> Result<Cached<ScenePage>, AppError> {
    validate_page_size(page_size)?;
    let key = format!("scenes:q:{}:s:{page_size}:p:{page}", query.cache_hash());
    let query = query.normalized();
    read_cached(&state, &key, RefreshPolicy::Auto, false, move |client| {
        let query = query.clone();
        async move { find_scenes_page(&client, &query, page, page_size).await }
    })
    .await
}

/// A sort the user can pick, with the web UI's label.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct SortOption {
    pub value: SceneSort,
    pub label: String,
}

/// The sort menu, in the web UI's order.
#[tauri::command]
#[specta::specta]
pub fn scene_sorts() -> Vec<SortOption> {
    SceneSort::all()
        .iter()
        .map(|&value| SortOption {
            value,
            label: value.label().to_owned(),
        })
        .collect()
}
