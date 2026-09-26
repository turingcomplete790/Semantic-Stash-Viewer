//! Scenes for playback, and the direct-stream guard (spec FR-001, FR-003; research R6).
//!
//! The player only ever plays Stash's direct stream, `{base}/scene/{id}/stream`, which serves
//! the original file. Every other stream Stash offers (`stream.mp4`, `.m3u8`, `.mpd`, anything
//! with `?resolution=`) is a server-side transcode. The URL is always built here from the
//! profile's base URL, never taken from Stash's `paths.stream` (which embeds the API key when
//! authentication is on).

use serde::Serialize;
use url::Url;

use crate::adapter::endpoint;

/// One row of the "recently added" picker.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SceneListItem {
    pub id: String,
    /// The scene title, or the primary file's basename when the title is empty.
    pub title: String,
    pub duration_seconds: f64,
    /// `width×height`, e.g. `1920×1080`.
    pub resolution: Option<String>,
    pub video_codec: Option<String>,
    pub container: Option<String>,
}

/// The primary file's technical details.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SceneFile {
    pub container: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub frame_rate: Option<f64>,
    pub bit_rate: Option<u64>,
    pub size: Option<u64>,
}

/// What the player needs to open one scene.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayableScene {
    pub id: String,
    pub title: String,
    /// Always the direct stream (see module docs).
    pub stream_url: Url,
    pub duration_seconds: f64,
    pub file: SceneFile,
}

/// `{base}/scene/{id}/stream`, keeping any reverse-proxy sub-path in `base`.
pub fn direct_stream_url(base: &Url, id: &str) -> Url {
    let mut url = endpoint(&endpoint(base, "scene"), id);
    if let Ok(mut segments) = url.path_segments_mut() {
        segments.push("stream");
    }
    url
}

/// True only for `…/scene/{id}/stream` with no query and no suffix.
pub fn is_direct_stream(url: &Url) -> bool {
    if url.query().is_some() || url.fragment().is_some() {
        return false;
    }
    let segments: Vec<&str> = url
        .path_segments()
        .map(Iterator::collect)
        .unwrap_or_default();
    matches!(
        segments.as_slice(),
        [.., "scene", id, "stream"] if !id.is_empty()
    )
}

/// Title shown to the user: the trimmed title, or the file's basename when it's empty.
pub(crate) fn display_title(title: Option<&str>, basename: Option<&str>) -> String {
    match title.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => t.to_owned(),
        None => basename.unwrap_or("Untitled scene").to_owned(),
    }
}

/// `width×height` when both are known and non-zero.
pub(crate) fn resolution(width: Option<i64>, height: Option<i64>) -> Option<String> {
    match (width, height) {
        (Some(w), Some(h)) if w > 0 && h > 0 => Some(format!("{w}×{h}")),
        _ => None,
    }
}
