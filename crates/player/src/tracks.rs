//! Embedded tracks from mpv's `track-list` (FR-014).

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum TrackKind {
    Video,
    Audio,
    Subtitle,
}

impl TrackKind {
    /// mpv's `track-list/N/type` value (`video`, `audio`, `sub`).
    pub fn from_mpv(kind: &str) -> Option<Self> {
        match kind {
            "video" => Some(Self::Video),
            "audio" => Some(Self::Audio),
            "sub" => Some(Self::Subtitle),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Track {
    /// mpv's per-kind track id (`track-list/N/id`); small, so `i32` (TypeScript-safe).
    pub id: i32,
    pub kind: TrackKind,
    pub title: Option<String>,
    pub language: Option<String>,
    pub codec: Option<String>,
    pub default: bool,
    pub external: bool,
}

/// Read the full track list via per-track sub-properties (`track-list/N/…`).
pub(crate) fn read_tracks(mpv: &libmpv2::Mpv) -> Vec<Track> {
    let count = mpv.get_property::<i64>("track-list/count").unwrap_or(0);
    (0..count)
        .filter_map(|i| {
            let prop = |name: &str| format!("track-list/{i}/{name}");
            let text = |name: &str| {
                mpv.get_property::<String>(&prop(name))
                    .ok()
                    .filter(|s| !s.is_empty())
            };
            let flag = |name: &str| mpv.get_property::<bool>(&prop(name)).unwrap_or(false);
            Some(Track {
                id: i32::try_from(mpv.get_property::<i64>(&prop("id")).ok()?).ok()?,
                kind: TrackKind::from_mpv(&text("type")?)?,
                title: text("title"),
                language: text("lang"),
                codec: text("codec"),
                default: flag("default"),
                external: flag("external"),
            })
        })
        .collect()
}
