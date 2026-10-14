//! What a Scenes tab is showing: search, sort, direction, and the random seed (005 research R2,
//! R7; data-model "SceneQuery").
//!
//! Sorting happens in Stash. A random order is stable because Stash accepts a seed
//! (`random_<seed>`), so the same seed gives the same order on every page, back/forward, and
//! restart; "Reshuffle" picks a new seed. Scene filter criteria join the query with US2.

use serde::{Deserialize, Serialize};

pub use crate::browse::query::{ListQuery, Sort, SortDirection};

/// Every scene sort the Stash web UI offers (`ui/v2.5/src/models/list-filter/scenes.ts` plus the
/// media and common options), serialized as Stash's sort names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SceneSort {
    Title,
    Path,
    Rating,
    FileModTime,
    TagCount,
    PerformerCount,
    Random,
    Organized,
    #[default]
    Date,
    ProductionDate,
    FileCount,
    Filesize,
    Duration,
    Framerate,
    Resolution,
    Bitrate,
    LastPlayedAt,
    ResumeTime,
    PlayDuration,
    PlayCount,
    Interactive,
    InteractiveSpeed,
    PerceptualSimilarity,
    PerformerAge,
    Studio,
    CreatedAt,
    UpdatedAt,
}

impl SceneSort {
    /// In the web UI's menu order.
    pub fn all() -> &'static [SceneSort] {
        use SceneSort::*;
        &[
            Bitrate,
            CreatedAt,
            Date,
            Duration,
            FileCount,
            FileModTime,
            Filesize,
            Framerate,
            Interactive,
            InteractiveSpeed,
            LastPlayedAt,
            Organized,
            Path,
            PerceptualSimilarity,
            PerformerAge,
            PerformerCount,
            PlayCount,
            PlayDuration,
            ProductionDate,
            Random,
            Rating,
            Resolution,
            ResumeTime,
            Studio,
            TagCount,
            Title,
            UpdatedAt,
        ]
    }

    /// The web UI's label (Stash's default en-GB locale).
    pub fn label(self) -> &'static str {
        use SceneSort::*;
        match self {
            Title => "Title",
            Path => "Path",
            Rating => "Rating",
            FileModTime => "File Modification Time",
            TagCount => "Tag Count",
            PerformerCount => "Performer Count",
            Random => "Random",
            Organized => "Organised",
            Date => "Date",
            ProductionDate => "Production Date",
            FileCount => "File Count",
            Filesize => "File Size",
            Duration => "Duration",
            Framerate => "Frame Rate",
            Resolution => "Resolution",
            Bitrate => "Bit Rate",
            LastPlayedAt => "Last Played At",
            ResumeTime => "Resume Time",
            PlayDuration => "Play Duration",
            PlayCount => "Play Count",
            Interactive => "Interactive",
            InteractiveSpeed => "Interactive Speed",
            PerceptualSimilarity => "Perceptual Similarity (pHash)",
            PerformerAge => "Performer Age",
            Studio => "Studio",
            CreatedAt => "Created At",
            UpdatedAt => "Updated At",
        }
    }
}

impl Sort for SceneSort {
    const RANDOM: SceneSort = SceneSort::Random;
}

/// What a Scenes tab is showing. Kept in the tab's view state (004) and used as the cache key.
pub type SceneQuery = ListQuery<SceneSort>;
