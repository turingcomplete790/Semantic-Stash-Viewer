//! Scenes for playback, and the direct-stream guard (spec FR-001, FR-003; research R6).
//!
//! The player only ever plays Stash's direct stream, `{base}/scene/{id}/stream`, which serves
//! the original file. Every other stream Stash offers (`stream.mp4`, `.m3u8`, `.mpd`, anything
//! with `?resolution=`) is a server-side transcode. The URL is always built here from the
//! profile's base URL, never taken from Stash's `paths.stream` (which embeds the API key when
//! authentication is on).

pub mod paging;
pub mod query;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::adapter::endpoint;

/// One row of the "recently added" picker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// One card in the Scenes grid or row in the list (005 data-model "SceneCard"). Text stays on
/// one line each for title and details (research R3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SceneCard {
    pub id: String,
    /// The scene title, or the primary file's base name when the title is empty.
    pub title: String,
    /// `YYYY-MM-DD`.
    pub date: Option<String>,
    /// The primary file's duration.
    pub duration_seconds: Option<f64>,
    /// `width×height` of the primary file.
    pub resolution: Option<String>,
    pub studio: Option<String>,
    /// `ssv-thumb://localhost/scene/<id>?v=<version>`: served by the core, never carrying a key.
    pub thumb: Option<String>,
    /// A generated animated preview exists (US5; previews are images, constitution Principle V).
    pub has_preview: bool,
}

/// One page of cards with the total number of matching scenes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ScenePage {
    pub count: u32,
    /// 1-based: the page actually returned (the last page when the request was past the end).
    pub page: u32,
    pub page_size: u32,
    pub items: Vec<SceneCard>,
}

/// A thumbnail URL for the `ssv-thumb://` scheme (005 contract "URI scheme").
pub fn thumb_url(kind: &str, id: &str, version: &str) -> String {
    format!("ssv-thumb://localhost/{kind}/{id}?v={version}")
}

/// The `t` parameter of a Stash media path (it changes when the image does), or `"0"`. Only `t`
/// is kept, so nothing else from the URL (such as an `apikey`) can leak into ours.
pub fn screenshot_version(path: &str) -> String {
    Url::parse(path)
        .ok()
        .and_then(|u| {
            u.query_pairs()
                .find(|(k, _)| k == "t")
                .map(|(_, v)| v.into_owned())
        })
        .filter(|v| !v.is_empty() && v.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or_else(|| "0".to_owned())
}

/// A labelled group of scenes (the spike's test set: 4K, WMV, …).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SceneGroup {
    pub label: String,
    pub scenes: Vec<SceneListItem>,
}

/// The primary file's technical details.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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

/// What the scene view shows (007 T048). Cached per scene (`scene:details:<id>`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneDetails {
    pub id: String,
    /// The scene title, or the primary file's base name when the title is empty.
    pub title: String,
    pub code: Option<String>,
    /// `YYYY-MM-DD`.
    pub date: Option<String>,
    /// The description.
    pub details: Option<String>,
    pub director: Option<String>,
    pub studio: Option<String>,
    pub performers: Vec<String>,
    pub tags: Vec<String>,
    /// 1–100.
    pub rating100: Option<u8>,
    pub play_count: u32,
    pub o_count: u32,
    pub duration_seconds: Option<f64>,
    /// The primary file's name (not its path).
    pub file_name: Option<String>,
    pub file: Option<SceneFile>,
    /// The screenshot's version (it changes when the cover does).
    pub cover_version: String,
}

/// What the player needs to open one scene.
/// Cached per scene (`scene:<id>`, 003 research R3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayableScene {
    pub id: String,
    pub title: String,
    /// Always the direct stream (see module docs).
    pub stream_url: Url,
    pub duration_seconds: f64,
    pub file: SceneFile,
}

/// Most of a file the player caches in memory ahead of the playback position.
pub const MAX_FORWARD_CACHE_BYTES: u64 = 1 << 30;

/// Cache kept behind the playback position when the whole file doesn't fit.
pub const LARGE_FILE_BACK_CACHE_BYTES: u64 = 256 << 20;

/// Demuxer cache limits for one file (mpv `demuxer-max-bytes` / `demuxer-max-back-bytes`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeekCache {
    pub forward_bytes: u64,
    pub back_bytes: u64,
}

/// Containers that benefit from the sized cache: unindexed (FLV) or slow to seek (AVI, WMV).
const LEGACY_CONTAINERS: [&str; 3] = ["flv", "avi", "wmv"];

/// Clips shorter than this get the sized cache whatever their format (WebMs used as GIFs).
pub const SHORT_CLIP_SECONDS: f64 = 120.0;

impl PlayableScene {
    /// Cache limits that keep seeks in memory, or `None` for mpv's defaults (research R5c).
    ///
    /// For legacy containers a seek outside mpv's default cache (150 MiB ahead, 50 MiB behind)
    /// costs a disk seek plus a new HTTP range request (160–770 ms on spinning disks), and for
    /// unindexed FLVs a rescan of the file up to the target (seconds). With a cache that holds
    /// the whole file, mpv reads it in the background during playback and later seeks take
    /// ~40 ms. Short clips of any format benefit too. Modern, GPU-decoded files seek fast
    /// without it, and the background read-ahead made them slightly laggier, so they keep the
    /// defaults.
    ///
    /// Files up to 1 GiB (plus headroom) are cached whole; larger or unknown-size files get
    /// 1 GiB ahead and 256 MiB behind, so memory stays under ~1.3 GiB.
    pub fn seek_cache(&self) -> Option<SeekCache> {
        let legacy = self.file.container.as_deref().is_some_and(|c| {
            LEGACY_CONTAINERS
                .iter()
                .any(|legacy| c.eq_ignore_ascii_case(legacy))
        });
        let short = self.duration_seconds > 0.0 && self.duration_seconds < SHORT_CLIP_SECONDS;
        if !legacy && !short {
            return None;
        }
        // Headroom for demuxer overhead.
        Some(match self.file.size.map(|s| s + s / 16 + (8 << 20)) {
            Some(whole) if whole <= MAX_FORWARD_CACHE_BYTES => SeekCache {
                forward_bytes: whole,
                back_bytes: whole,
            },
            _ => SeekCache {
                forward_bytes: MAX_FORWARD_CACHE_BYTES,
                back_bytes: LARGE_FILE_BACK_CACHE_BYTES,
            },
        })
    }
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
