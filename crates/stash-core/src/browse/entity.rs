//! What a list shows (012 research R1): each entity's criteria, saved-filter mode, default filter,
//! cache-key prefix, and thumbnail kind. Each entity's sorts are its own type ([`super::query::Sort`]).

use serde::{Deserialize, Serialize};

use crate::scenes::filter::kinds::{CriterionKind, IMAGE_IS_MISSING_CHOICES, IS_MISSING_CHOICES};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Entity {
    #[default]
    Scenes,
    Images,
}

impl Entity {
    /// The criterion kinds this entity's filter menu offers, in menu order.
    pub fn criteria(self) -> impl Iterator<Item = CriterionKind> {
        CriterionKind::ALL
            .iter()
            .copied()
            .filter(move |k| k.applies_to(self))
    }

    /// Stash's `FilterMode` for this entity's saved filters.
    pub fn filter_mode(self) -> &'static str {
        match self {
            Entity::Scenes => "SCENES",
            Entity::Images => "IMAGES",
        }
    }

    /// The key of this entity's default filter in the web UI's configuration
    /// (`ui.defaultFilters.<key>`).
    pub fn default_filter_key(self) -> &'static str {
        match self {
            Entity::Scenes => "scenes",
            Entity::Images => "images",
        }
    }

    /// The first part of this entity's page cache keys (`scenes:q:…`, `images:q:…`).
    pub fn cache_prefix(self) -> &'static str {
        match self {
            Entity::Scenes => "scenes",
            Entity::Images => "images",
        }
    }

    /// The kind in this entity's thumbnail URLs (`ssv-thumb://localhost/<kind>/<id>`).
    pub fn thumb_kind(self) -> &'static str {
        match self {
            Entity::Scenes => "scene",
            Entity::Images => "image",
        }
    }

    /// The is-missing choices the web UI offers for this entity.
    pub fn is_missing_choices(self) -> &'static [&'static str] {
        match self {
            Entity::Scenes => IS_MISSING_CHOICES,
            Entity::Images => IMAGE_IS_MISSING_CHOICES,
        }
    }
}
