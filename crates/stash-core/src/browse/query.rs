//! A list's query: search, sort, direction, the random seed, and filter criteria (005 research R2,
//! R7; 008; generic since 012 research R1).
//!
//! Sorting happens in Stash. A random order is stable because Stash accepts a seed
//! (`random_<seed>`), so the same seed gives the same order on every page, back/forward, and
//! restart; "Reshuffle" picks a new seed.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::cache::identity::hash_hex;
use crate::error::AppError;
use crate::scenes::filter::Criterion;

/// An entity's sorts: serialized as Stash's sort names, with one meaning "random".
pub trait Sort: Copy + Eq + Default + Serialize + DeserializeOwned {
    /// The random sort (sent as `random_<seed>`).
    const RANDOM: Self;

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

/// What a list tab is showing (Scenes' `SceneQuery`, Images' `ImageQuery`). Kept in the tab's view
/// state (004) and used as the cache key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(
    rename_all = "camelCase",
    default,
    bound(deserialize = "S: Deserialize<'de> + Default")
)]
pub struct ListQuery<S> {
    /// Search text; empty means none.
    pub search: String,
    pub sort: S,
    pub direction: SortDirection,
    /// Required when `sort` is `random`; 32-bit because the bindings have no 64-bit integers
    /// (Stash accepts any u64).
    pub seed: Option<u32>,
    /// Filter criteria (008 FR-002), in the order the user added them (the chips' order). Each
    /// kind at most once, except custom fields.
    pub criteria: Vec<Criterion>,
}

impl<S: Sort> ListQuery<S> {
    /// The search trimmed, and the seed dropped unless the sort is random, so equal queries are
    /// equal (and share cache entries).
    pub fn normalized(&self) -> ListQuery<S> {
        ListQuery {
            search: self.search.trim().to_owned(),
            sort: self.sort,
            direction: self.direction,
            seed: if self.sort == S::RANDOM {
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
        if self.sort == S::RANDOM {
            let seed = self.seed.ok_or_else(|| AppError::Internal {
                message: "a random sort needs a seed".into(),
            })?;
            return Ok(format!("random_{seed}"));
        }
        Ok(self.sort.name())
    }

    /// 16 hex digits identifying the normalized query, for cache keys
    /// (`scenes:q:<hash>:p:<page>`, `images:q:…`). Fields are hashed in a fixed order.
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
