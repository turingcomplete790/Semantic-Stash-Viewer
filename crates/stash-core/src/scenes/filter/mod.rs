//! The scene filter model: criteria, modifiers, saved-filter decoding (spec FR-002 to FR-005;
//! research R3).
//!
//! A [`Criterion`] is one condition from the web UI's filter list (or a semantic-tagging
//! placeholder, never sent). The model converts to Stash's `SceneFilterType` for the page request
//! ([`to_stash`]) and decodes the web UI's saved-filter format ([`saved`]). Criteria are saved
//! with the tab in this model's own form, so a tab restores without a request.

pub mod kinds;
pub mod saved;
pub mod search;
pub mod semantic;
pub mod to_stash;

use serde::{Deserialize, Serialize};

pub use kinds::{CriterionKind, KindInfo, ValueType};
pub use semantic::SemanticFilter;

/// A criterion's modifier (Stash's `CriterionModifier`). Each kind lists the ones the web UI
/// offers for it ([`KindInfo::modifiers`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Modifier {
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    Between,
    NotBetween,
    Includes,
    IncludesAll,
    Excludes,
    IsNull,
    NotNull,
    MatchesRegex,
    NotMatchesRegex,
}

impl Modifier {
    /// Stash's name (`INCLUDES_ALL`), as saved filters store it.
    pub fn stash_name(self) -> &'static str {
        match self {
            Modifier::Equals => "EQUALS",
            Modifier::NotEquals => "NOT_EQUALS",
            Modifier::GreaterThan => "GREATER_THAN",
            Modifier::LessThan => "LESS_THAN",
            Modifier::Between => "BETWEEN",
            Modifier::NotBetween => "NOT_BETWEEN",
            Modifier::Includes => "INCLUDES",
            Modifier::IncludesAll => "INCLUDES_ALL",
            Modifier::Excludes => "EXCLUDES",
            Modifier::IsNull => "IS_NULL",
            Modifier::NotNull => "NOT_NULL",
            Modifier::MatchesRegex => "MATCHES_REGEX",
            Modifier::NotMatchesRegex => "NOT_MATCHES_REGEX",
        }
    }

    pub fn from_stash_name(name: &str) -> Option<Self> {
        ALL_MODIFIERS
            .iter()
            .copied()
            .find(|m| m.stash_name() == name)
    }

    /// Null checks carry no value.
    pub fn is_null_check(self) -> bool {
        matches!(self, Modifier::IsNull | Modifier::NotNull)
    }

    /// Ranges carry two values.
    pub fn is_range(self) -> bool {
        matches!(self, Modifier::Between | Modifier::NotBetween)
    }

    /// The label the web UI shows.
    pub fn label(self) -> &'static str {
        match self {
            Modifier::Equals => "is",
            Modifier::NotEquals => "is not",
            Modifier::GreaterThan => "is greater than",
            Modifier::LessThan => "is less than",
            Modifier::Between => "between",
            Modifier::NotBetween => "not between",
            Modifier::Includes => "includes",
            Modifier::IncludesAll => "includes all",
            Modifier::Excludes => "excludes",
            Modifier::IsNull => "is null",
            Modifier::NotNull => "not null",
            Modifier::MatchesRegex => "matches regex",
            Modifier::NotMatchesRegex => "not matches regex",
        }
    }
}

const ALL_MODIFIERS: [Modifier; 13] = [
    Modifier::Equals,
    Modifier::NotEquals,
    Modifier::GreaterThan,
    Modifier::LessThan,
    Modifier::Between,
    Modifier::NotBetween,
    Modifier::Includes,
    Modifier::IncludesAll,
    Modifier::Excludes,
    Modifier::IsNull,
    Modifier::NotNull,
    Modifier::MatchesRegex,
    Modifier::NotMatchesRegex,
];

/// A picked performer, tag, studio, group, or gallery: the id is sent; the label is display-only
/// (from the choice list or a saved filter).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub label: String,
}

/// Stash's resolution buckets, by the web UI's names (`1080p`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Resolution {
    #[serde(rename = "144p")]
    VeryLow,
    #[serde(rename = "240p")]
    Low,
    #[serde(rename = "360p")]
    R360p,
    #[serde(rename = "480p")]
    Standard,
    #[serde(rename = "540p")]
    WebHd,
    #[serde(rename = "720p")]
    StandardHd,
    #[serde(rename = "1080p")]
    FullHd,
    #[serde(rename = "1440p")]
    QuadHd,
    #[serde(rename = "4k")]
    FourK,
    #[serde(rename = "5k")]
    FiveK,
    #[serde(rename = "6k")]
    SixK,
    #[serde(rename = "7k")]
    SevenK,
    #[serde(rename = "8k")]
    EightK,
    #[serde(rename = "Huge")]
    Huge,
}

impl Resolution {
    pub const ALL: [Resolution; 14] = [
        Resolution::VeryLow,
        Resolution::Low,
        Resolution::R360p,
        Resolution::Standard,
        Resolution::WebHd,
        Resolution::StandardHd,
        Resolution::FullHd,
        Resolution::QuadHd,
        Resolution::FourK,
        Resolution::FiveK,
        Resolution::SixK,
        Resolution::SevenK,
        Resolution::EightK,
        Resolution::Huge,
    ];

    /// The web UI's name (`1080p`).
    pub fn name(self) -> &'static str {
        match self {
            Resolution::VeryLow => "144p",
            Resolution::Low => "240p",
            Resolution::R360p => "360p",
            Resolution::Standard => "480p",
            Resolution::WebHd => "540p",
            Resolution::StandardHd => "720p",
            Resolution::FullHd => "1080p",
            Resolution::QuadHd => "1440p",
            Resolution::FourK => "4k",
            Resolution::FiveK => "5k",
            Resolution::SixK => "6k",
            Resolution::SevenK => "7k",
            Resolution::EightK => "8k",
            Resolution::Huge => "Huge",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|r| r.name().eq_ignore_ascii_case(name))
    }
}

/// Picture orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Orientation {
    Landscape,
    Portrait,
    Square,
}

impl Orientation {
    pub const ALL: [Orientation; 3] = [
        Orientation::Landscape,
        Orientation::Portrait,
        Orientation::Square,
    ];

    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "landscape" => Some(Orientation::Landscape),
            "portrait" => Some(Orientation::Portrait),
            "square" => Some(Orientation::Square),
            _ => None,
        }
    }
}

/// Which kinds of duplicate a "duplicated" criterion asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Duplication {
    pub phash: bool,
    pub url: bool,
    pub stash_id: bool,
    pub title: bool,
}

/// A criterion's value. Which variant a kind takes is its [`ValueType`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CriterionValue {
    /// For null checks.
    None,
    Text(String),
    /// Numbers and durations (seconds); `Between` uses both.
    Int(i64, Option<i64>),
    /// `YYYY-MM-DD` (dates) or `YYYY-MM-DD HH:MM` (timestamps); `Between` uses both.
    Date(String, Option<String>),
    Bool(bool),
    Resolution(Resolution),
    Orientation(Vec<Orientation>),
    Items {
        items: Vec<Item>,
        excluded: Vec<Item>,
        /// Sub-tags/sub-studios/sub-groups: `Some(-1)` all levels, `Some(n)` n levels.
        depth: Option<i32>,
    },
    StashId {
        endpoint: String,
        id: String,
    },
    Phash {
        hash: String,
        distance: Option<i32>,
    },
    Duplicated(Duplication),
    CustomField {
        field: String,
        values: Vec<serde_json::Value>,
    },
}

/// One condition: the kind, its modifier, and its value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Criterion {
    pub kind: CriterionKind,
    pub modifier: Modifier,
    pub value: CriterionValue,
}

/// Why a criterion can't be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invalid {
    /// The kind doesn't offer this modifier.
    Modifier,
    /// The value doesn't fit the kind or modifier (missing, empty, or a range without its second
    /// value).
    Value,
}

impl Criterion {
    pub fn new(kind: CriterionKind, modifier: Modifier, value: CriterionValue) -> Self {
        Self {
            kind,
            modifier,
            value,
        }
    }

    pub fn info(&self) -> KindInfo {
        self.kind.info()
    }

    /// A stable sort key: the kind's key, and a custom field's name (several may coexist).
    pub fn sort_key(&self) -> String {
        match &self.value {
            CriterionValue::CustomField { field, .. } => format!("{}:{field}", self.info().key),
            _ => self.info().key.to_owned(),
        }
    }

    /// Check the criterion: the modifier is one the kind allows; `Between` has two values;
    /// `IsNull`/`NotNull` carry no value; `Items` are non-empty unless the modifier is a null
    /// check.
    pub fn validate(&self) -> Result<(), Invalid> {
        self.validate_for(crate::browse::entity::Entity::Scenes)
    }

    /// As [`Self::validate`], for `entity`'s list (images have their own is-missing choices).
    pub fn validate_for(&self, entity: crate::browse::entity::Entity) -> Result<(), Invalid> {
        let info = self.kind.info_for(entity);
        let modifiers = info.modifiers();
        if modifiers.is_empty() {
            // Kinds without modifiers (booleans, choices) always use `Equals`.
            if self.modifier != Modifier::Equals {
                return Err(Invalid::Modifier);
            }
        } else if !modifiers.contains(&self.modifier) {
            return Err(Invalid::Modifier);
        }
        if self.modifier.is_null_check() {
            return match &self.value {
                CriterionValue::None => Ok(()),
                CriterionValue::CustomField { field, values }
                    if !field.is_empty() && values.is_empty() =>
                {
                    Ok(())
                }
                _ => Err(Invalid::Value),
            };
        }
        let fits = match (info.value_type, &self.value) {
            (ValueType::Text { .. } | ValueType::Captions, CriterionValue::Text(t)) => {
                !t.trim().is_empty()
            }
            (
                ValueType::Number { .. } | ValueType::Duration { .. },
                CriterionValue::Int(_, second),
            ) => !self.modifier.is_range() || second.is_some(),
            (
                ValueType::Date | ValueType::Timestamp { .. },
                CriterionValue::Date(first, second),
            ) => {
                !first.trim().is_empty()
                    && (!self.modifier.is_range()
                        || second.as_ref().is_some_and(|s| !s.trim().is_empty()))
            }
            (ValueType::Bool | ValueType::StringBool, CriterionValue::Bool(_)) => true,
            (ValueType::Choice(options), CriterionValue::Text(t)) => options.contains(&t.as_str()),
            (ValueType::Resolution, CriterionValue::Resolution(_)) => true,
            (ValueType::Orientation, CriterionValue::Orientation(o)) => !o.is_empty(),
            (
                ValueType::Items { .. },
                CriterionValue::Items {
                    items, excluded, ..
                },
            ) => !items.is_empty() || !excluded.is_empty(),
            (ValueType::StashId, CriterionValue::StashId { id, .. }) => !id.trim().is_empty(),
            (ValueType::Phash, CriterionValue::Phash { hash, .. }) => !hash.trim().is_empty(),
            (ValueType::Duplicated, CriterionValue::Duplicated(d)) => {
                d.phash || d.url || d.stash_id || d.title
            }
            (ValueType::CustomField, CriterionValue::CustomField { field, values }) => {
                !field.is_empty()
                    && (!values.is_empty())
                    && (!self.modifier.is_range() || values.len() == 2)
            }
            (ValueType::Semantic, _) => true,
            _ => false,
        };
        if fits {
            Ok(())
        } else {
            Err(Invalid::Value)
        }
    }
}
