//! The semantic-tagging plugin's filters as first-class placeholders (spec FR-003b): shown and
//! described in the filter menu, kept in tabs and saved filters, never sent to Stash. Phase 4
//! makes them work through the semantic engine (constitution Principle XII).

use serde::{Deserialize, Serialize};

/// One semantic filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SemanticFilter {
    Trait,
    TraitCategory,
    Label,
    Role,
    RoleCategory,
    Pluggable,
    ActionVariation,
    MiseEnScene,
    TimelineRelation,
    Archetype,
    /// A performer's Variation (the design documents' rename of "Look").
    Variation,
}

impl SemanticFilter {
    pub const ALL: [SemanticFilter; 11] = [
        SemanticFilter::Trait,
        SemanticFilter::TraitCategory,
        SemanticFilter::Label,
        SemanticFilter::Role,
        SemanticFilter::RoleCategory,
        SemanticFilter::Pluggable,
        SemanticFilter::ActionVariation,
        SemanticFilter::MiseEnScene,
        SemanticFilter::TimelineRelation,
        SemanticFilter::Archetype,
        SemanticFilter::Variation,
    ];

    /// The key a saved filter or tab stores it under (prefixed so it never meets a Stash field).
    pub fn key(self) -> &'static str {
        match self {
            SemanticFilter::Trait => "semantic_trait",
            SemanticFilter::TraitCategory => "semantic_trait_category",
            SemanticFilter::Label => "semantic_label",
            SemanticFilter::Role => "semantic_role",
            SemanticFilter::RoleCategory => "semantic_role_category",
            SemanticFilter::Pluggable => "semantic_pluggable",
            SemanticFilter::ActionVariation => "semantic_action",
            SemanticFilter::MiseEnScene => "semantic_mise_en_scene",
            SemanticFilter::TimelineRelation => "semantic_timeline",
            SemanticFilter::Archetype => "semantic_archetype",
            SemanticFilter::Variation => "semantic_variation",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|s| s.key() == key)
    }

    pub fn label(self) -> &'static str {
        match self {
            SemanticFilter::Trait => "Trait",
            SemanticFilter::TraitCategory => "Trait category",
            SemanticFilter::Label => "Label",
            SemanticFilter::Role => "Role",
            SemanticFilter::RoleCategory => "Role category",
            SemanticFilter::Pluggable => "Pluggable",
            SemanticFilter::ActionVariation => "Action and variation",
            SemanticFilter::MiseEnScene => "Mise-en-scène and location",
            SemanticFilter::TimelineRelation => "Timeline relation",
            SemanticFilter::Archetype => "Archetype",
            SemanticFilter::Variation => "Variation",
        }
    }

    /// One line for the filter menu.
    pub fn description(self) -> &'static str {
        match self {
            SemanticFilter::Trait => "Scenes whose performers have a trait",
            SemanticFilter::TraitCategory => "Scenes whose performers have a trait in a category",
            SemanticFilter::Label => "Scenes whose performers carry a label",
            SemanticFilter::Role => "Scenes where a performer plays a role",
            SemanticFilter::RoleCategory => "Scenes with a role from a category",
            SemanticFilter::Pluggable => "Scenes where a performer has a pluggable tag",
            SemanticFilter::ActionVariation => {
                "Scenes with an action, optionally a variation of it"
            }
            SemanticFilter::MiseEnScene => "Scenes set in a mise-en-scène or location",
            SemanticFilter::TimelineRelation => {
                "Scenes where things happen at the same time, before, or after"
            }
            SemanticFilter::Archetype => "Scenes with performers of an archetype",
            SemanticFilter::Variation => "Scenes featuring a performer's variation",
        }
    }
}
