//! Why playback failed; each maps to a plain-language message in the UI (FR-006).

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PlayerError {
    #[error("not connected to a server")]
    NotConnected,
    #[error("scene not found")]
    SceneNotFound,
    #[error("the scene has no playable file")]
    NoPlayableFile,
    /// The stream couldn't be opened (network error, 401/403, 404).
    #[error("the video stream couldn't be opened")]
    StreamUnreachable,
    /// mpv couldn't decode the file.
    #[error("the video format isn't supported")]
    UnsupportedFormat,
    #[error("playback failed: {detail}")]
    PlaybackFailed { detail: String },
}

// Error codes from mpv's client.h.
const MPV_ERROR_LOADING_FAILED: i32 = -13;
const MPV_ERROR_NOTHING_TO_PLAY: i32 = -16;
const MPV_ERROR_UNKNOWN_FORMAT: i32 = -17;
const MPV_ERROR_UNSUPPORTED: i32 = -18;

impl PlayerError {
    /// Map an mpv error (usually from an `end-file` event) to a user-facing error.
    pub(crate) fn from_mpv(error: &libmpv2::Error) -> Self {
        match error {
            libmpv2::Error::Raw(code) => match *code {
                MPV_ERROR_LOADING_FAILED => Self::StreamUnreachable,
                MPV_ERROR_UNKNOWN_FORMAT | MPV_ERROR_UNSUPPORTED | MPV_ERROR_NOTHING_TO_PLAY => {
                    Self::UnsupportedFormat
                }
                other => Self::PlaybackFailed {
                    detail: format!("mpv error {other}"),
                },
            },
            other => Self::PlaybackFailed {
                detail: other.to_string(),
            },
        }
    }
}
