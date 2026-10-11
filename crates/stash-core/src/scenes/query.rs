//! What a Scenes tab is showing: search, sort, direction, and the random seed (005 research R2,
//! R7; data-model "SceneQuery").
//!
//! Sorting happens in Stash. A random order is stable because Stash accepts a seed
//! (`random_<seed>`), so the same seed gives the same order on every page, back/forward, and
//! restart; "Reshuffle" picks a new seed. Scene filter criteria join the query with US2.

use serde::{Deserialize, Serialize};

use crate::cache::identity::hash_hex;
use crate::error::AppError;
use crate::scenes::filter::Criterion;

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

    /// Stash's sort name (the serde name).
    fn name(self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Asc,
    #[default]
    Desc,
}

/// What a Scenes tab is showing. Kept in the tab's view state (004) and used as the cache key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SceneQuery {
    /// Search text; empty means none.
    pub search: String,
    pub sort: SceneSort,
    pub direction: SortDirection,
    /// Required when `sort` is `random`; 32-bit because the bindings have no 64-bit integers
    /// (Stash accepts any u64).
    pub seed: Option<u32>,
    /// Filter criteria (008 FR-002), in the order the user added them (the chips' order). Each
    /// kind at most once, except custom fields.
    pub criteria: Vec<Criterion>,
}

impl SceneQuery {
    /// The search trimmed, and the seed dropped unless the sort is random, so equal queries are
    /// equal (and share cache entries).
    pub fn normalized(&self) -> SceneQuery {
        SceneQuery {
            search: self.search.trim().to_owned(),
            sort: self.sort,
            direction: self.direction,
            seed: if self.sort == SceneSort::Random {
                self.seed
            } else {
                None
            },
            criteria: {
                let mut criteria = self.criteria.clone();
                criteria.sort_by_key(Criterion::sort_key);
                criteria
            },
        }
    }

    /// Add `criterion`, replacing one of the same kind (custom fields: the same field) so each
    /// kind appears at most once; a new kind goes at the end, a replaced one keeps its place.
    pub fn set_criterion(&mut self, criterion: Criterion) {
        let key = criterion.sort_key();
        match self.criteria.iter_mut().find(|c| c.sort_key() == key) {
            Some(existing) => *existing = criterion,
            None => self.criteria.push(criterion),
        }
    }

    /// Remove the criterion with `sort_key`.
    pub fn remove_criterion(&mut self, sort_key: &str) {
        self.criteria.retain(|c| c.sort_key() != sort_key);
    }

    /// The sort Stash receives: its name, or `random_<seed>`.
    pub fn stash_sort(&self) -> Result<String, AppError> {
        if self.sort == SceneSort::Random {
            let seed = self.seed.ok_or_else(|| AppError::Internal {
                message: "a random sort needs a seed".into(),
            })?;
            return Ok(format!("random_{seed}"));
        }
        Ok(self.sort.name())
    }

    /// 16 hex digits identifying the normalized query, for cache keys
    /// (`scenes:q:<hash>:p:<page>`). Fields are hashed in a fixed order.
    pub fn cache_hash(&self) -> String {
        let n = self.normalized();
        let key = format!(
            "v1\0{}\0{}\0{}\0{}",
            n.search,
            n.sort.name(),
            match n.direction {
                SortDirection::Asc => "asc",
                SortDirection::Desc => "desc",
            },
            n.seed.map_or_else(String::new, |s| s.to_string()),
        );
        // Without criteria the key is 007's, so existing cache entries stay valid.
        let key = if n.criteria.is_empty() {
            key
        } else {
            format!(
                "{key}\0{}",
                serde_json::to_string(&n.criteria).unwrap_or_default()
            )
        };
        hash_hex(key.as_bytes())
    }
}
