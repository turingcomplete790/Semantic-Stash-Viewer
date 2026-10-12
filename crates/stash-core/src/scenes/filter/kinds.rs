//! Every scene criterion the web UI offers for the pinned Stash v0.31.1 schema, with its label,
//! group, value type, and modifiers (research R3; the web UI's `models/list-filter`).
//!
//! The key is Stash's `SceneFilterType` / `ImageFilterType` field name, which the web UI's saved
//! filters also use. Each kind names the entities it applies to (012: images use the web UI's
//! v0.31.1 `images.ts` list).

use serde::{Deserialize, Serialize};

use super::semantic::SemanticFilter;
use super::Modifier;
use crate::browse::entity::Entity;
use Modifier::*;

/// One criterion kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CriterionKind {
    Title,
    Code,
    Details,
    Director,
    /// Images only.
    Photographer,
    Path,
    /// Images only: the file's folder, sent as `files_filter.parent_folder`.
    Folder,
    Oshash,
    Checksum,
    PhashDistance,
    Duplicated,
    Organized,
    Rating,
    OCounter,
    Resolution,
    Orientation,
    Framerate,
    Bitrate,
    VideoCodec,
    AudioCodec,
    Duration,
    ResumeTime,
    PlayDuration,
    PlayCount,
    LastPlayedAt,
    HasMarkers,
    IsMissing,
    Tags,
    TagCount,
    PerformerTags,
    Performers,
    PerformerCount,
    PerformerAge,
    PerformerFavorite,
    Studios,
    Groups,
    Galleries,
    Url,
    StashId,
    StashIdCount,
    Interactive,
    InteractiveSpeed,
    Captions,
    FileCount,
    Date,
    CreatedAt,
    UpdatedAt,
    /// May appear more than once (one per field).
    CustomField,
    /// A semantic-tagging plugin filter: shown, kept, never sent (spec FR-003b).
    Semantic(SemanticFilter),
}

/// Where a kind sits in the filter menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Group {
    Text,
    File,
    Activity,
    People,
    Tags,
    Relations,
    Dates,
    Other,
    Semantic,
}

impl Group {
    pub const ALL: [Group; 9] = [
        Group::Text,
        Group::People,
        Group::Tags,
        Group::Relations,
        Group::File,
        Group::Activity,
        Group::Dates,
        Group::Other,
        Group::Semantic,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Group::Text => "Text",
            Group::File => "File",
            Group::Activity => "Activity",
            Group::People => "Performers",
            Group::Tags => "Tags",
            Group::Relations => "Studio, groups, galleries",
            Group::Dates => "Dates",
            Group::Other => "Other",
            Group::Semantic => "Semantic",
        }
    }
}

/// The is-missing choices for scenes (the web UI's list, minus fields v0.31.1 lacks).
pub const IS_MISSING_CHOICES: &[&str] = &[
    "title",
    "code",
    "details",
    "director",
    "url",
    "date",
    "rating",
    "cover",
    "galleries",
    "studio",
    "group",
    "performers",
    "tags",
    "stash_id",
];

/// The is-missing choices for images (the web UI's `ImageIsMissingCriterionOption`).
pub const IMAGE_IS_MISSING_CHOICES: &[&str] = &[
    "title",
    "details",
    "photographer",
    "url",
    "date",
    "code",
    "rating",
    "galleries",
    "studio",
    "performers",
    "tags",
];

/// What a kind's value looks like, and so which modifiers it offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    Text {
        nullable: bool,
    },
    Number {
        nullable: bool,
    },
    /// Seconds, shown as a duration.
    Duration {
        nullable: bool,
    },
    Date,
    Timestamp {
        nullable: bool,
    },
    /// Sent as a boolean.
    Bool,
    /// Sent as `"true"` / `"false"` (`has_markers`).
    StringBool,
    /// One of a fixed list of strings (`is_missing`).
    Choice(&'static [&'static str]),
    Resolution,
    Orientation,
    Items {
        hierarchical: bool,
        modifiers: &'static [Modifier],
    },
    /// Caption language codes.
    Captions,
    StashId,
    Phash,
    Duplicated,
    CustomField,
    Semantic,
}

const TEXT: &[Modifier] = &[
    Equals,
    NotEquals,
    Includes,
    Excludes,
    MatchesRegex,
    NotMatchesRegex,
];
const TEXT_NULLABLE: &[Modifier] = &[
    Equals,
    NotEquals,
    Includes,
    Excludes,
    IsNull,
    NotNull,
    MatchesRegex,
    NotMatchesRegex,
];
const NUMBER: &[Modifier] = &[
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    Between,
    NotBetween,
];
const NUMBER_NULLABLE: &[Modifier] = &[
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    Between,
    NotBetween,
    IsNull,
    NotNull,
];
const RESOLUTION: &[Modifier] = &[Equals, NotEquals, GreaterThan, LessThan];
const CAPTIONS: &[Modifier] = &[Includes, Excludes, IsNull, NotNull];
const EQUALS_OR_NULL: &[Modifier] = &[Equals, NotEquals, IsNull, NotNull];
const CUSTOM_FIELD: &[Modifier] = &[
    Equals,
    NotEquals,
    Includes,
    Excludes,
    GreaterThan,
    LessThan,
    Between,
    NotBetween,
    IsNull,
    NotNull,
];
const TAGS: &[Modifier] = &[IncludesAll, Includes, Equals, IsNull, NotNull];
const PERFORMER_TAGS: &[Modifier] = &[IncludesAll, Includes, IsNull, NotNull];
const PERFORMERS: &[Modifier] = &[IncludesAll, Includes, Equals, IsNull, NotNull];
const STUDIOS: &[Modifier] = &[Includes, IsNull, NotNull];
const GROUPS: &[Modifier] = &[Includes, Excludes, IsNull, NotNull];
const GALLERIES: &[Modifier] = &[IncludesAll, Includes, Excludes, IsNull, NotNull];
const FOLDER: &[Modifier] = &[Includes];

/// A kind's description.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KindInfo {
    pub kind: CriterionKind,
    /// Stash's field name (and the saved-filter key).
    pub key: &'static str,
    pub label: &'static str,
    pub group: Group,
    pub value_type: ValueType,
}

impl KindInfo {
    /// The modifiers the web UI offers for this kind; empty for kinds that only take a value
    /// (booleans, choices, orientation, duplicates), which use `Equals`.
    pub fn modifiers(&self) -> &'static [Modifier] {
        match self.value_type {
            ValueType::Text { nullable: false } => TEXT,
            ValueType::Text { nullable: true } => TEXT_NULLABLE,
            ValueType::Number { nullable: false }
            | ValueType::Duration { nullable: false }
            | ValueType::Timestamp { nullable: false } => NUMBER,
            ValueType::Number { nullable: true }
            | ValueType::Duration { nullable: true }
            | ValueType::Timestamp { nullable: true }
            | ValueType::Date => NUMBER_NULLABLE,
            ValueType::Resolution => RESOLUTION,
            ValueType::Captions => CAPTIONS,
            ValueType::StashId | ValueType::Phash => EQUALS_OR_NULL,
            ValueType::CustomField => CUSTOM_FIELD,
            ValueType::Items { modifiers, .. } => modifiers,
            ValueType::Bool
            | ValueType::StringBool
            | ValueType::Choice(_)
            | ValueType::Orientation
            | ValueType::Duplicated
            | ValueType::Semantic => &[],
        }
    }

    /// The modifier a new criterion of this kind starts with.
    pub fn default_modifier(&self) -> Modifier {
        self.modifiers().first().copied().unwrap_or(Equals)
    }

    /// Whether a query may hold more than one criterion of this kind.
    pub fn repeatable(&self) -> bool {
        matches!(self.kind, CriterionKind::CustomField)
    }

    /// Placeholders are shown and kept but never sent (spec FR-003b).
    pub fn is_placeholder(&self) -> bool {
        matches!(self.kind, CriterionKind::Semantic(_))
    }
}

macro_rules! kinds {
    ($($kind:ident, $key:literal, $label:literal, $group:ident, $value:expr;)*) => {
        impl CriterionKind {
            /// Every sendable kind, in menu order (semantic placeholders come from
            /// [`SemanticFilter::ALL`]).
            pub const ALL: &'static [CriterionKind] = &[$(CriterionKind::$kind,)*];

            pub fn info(self) -> KindInfo {
                match self {
                    $(CriterionKind::$kind => KindInfo {
                        kind: self,
                        key: $key,
                        label: $label,
                        group: Group::$group,
                        value_type: $value,
                    },)*
                    CriterionKind::Semantic(s) => KindInfo {
                        kind: self,
                        key: s.key(),
                        label: s.label(),
                        group: Group::Semantic,
                        value_type: ValueType::Semantic,
                    },
                }
            }

            /// The kind a saved filter's key names (`performer_tags`), if the viewer knows it.
            pub fn from_key(key: &str) -> Option<Self> {
                match key {
                    $($key => Some(CriterionKind::$kind),)*
                    _ => SemanticFilter::from_key(key).map(CriterionKind::Semantic),
                }
            }
        }
    };
}

kinds! {
    Title, "title", "Title", Text, ValueType::Text { nullable: true };
    Code, "code", "Studio code", Text, ValueType::Text { nullable: true };
    Details, "details", "Details", Text, ValueType::Text { nullable: true };
    Director, "director", "Director", Text, ValueType::Text { nullable: true };
    Photographer, "photographer", "Photographer", Text, ValueType::Text { nullable: true };
    Url, "url", "URL", Text, ValueType::Text { nullable: true };
    Path, "path", "Path", File, ValueType::Text { nullable: false };
    Folder, "folder", "Folder", File, ValueType::Items { hierarchical: true, modifiers: FOLDER };
    Performers, "performers", "Performers", People,
        ValueType::Items { hierarchical: false, modifiers: PERFORMERS };
    PerformerCount, "performer_count", "Performer count", People, ValueType::Number { nullable: false };
    PerformerAge, "performer_age", "Performer age", People, ValueType::Number { nullable: false };
    PerformerFavorite, "performer_favorite", "Favourite performer", People, ValueType::Bool;
    PerformerTags, "performer_tags", "Performer tags", People,
        ValueType::Items { hierarchical: true, modifiers: PERFORMER_TAGS };
    Tags, "tags", "Tags", Tags, ValueType::Items { hierarchical: true, modifiers: TAGS };
    TagCount, "tag_count", "Tag count", Tags, ValueType::Number { nullable: false };
    Studios, "studios", "Studios", Relations,
        ValueType::Items { hierarchical: true, modifiers: STUDIOS };
    Groups, "groups", "Groups", Relations, ValueType::Items { hierarchical: true, modifiers: GROUPS };
    Galleries, "galleries", "Galleries", Relations,
        ValueType::Items { hierarchical: false, modifiers: GALLERIES };
    HasMarkers, "has_markers", "Has markers", Relations, ValueType::StringBool;
    Resolution, "resolution", "Resolution", File, ValueType::Resolution;
    Orientation, "orientation", "Orientation", File, ValueType::Orientation;
    Duration, "duration", "Duration", File, ValueType::Duration { nullable: true };
    Framerate, "framerate", "Frame rate", File, ValueType::Number { nullable: false };
    Bitrate, "bitrate", "Bit rate", File, ValueType::Number { nullable: false };
    VideoCodec, "video_codec", "Video codec", File, ValueType::Text { nullable: true };
    AudioCodec, "audio_codec", "Audio codec", File, ValueType::Text { nullable: true };
    FileCount, "file_count", "File count", File, ValueType::Number { nullable: false };
    Oshash, "oshash", "Oshash", File, ValueType::Text { nullable: false };
    Checksum, "checksum", "Checksum", File, ValueType::Text { nullable: true };
    PhashDistance, "phash_distance", "Phash", File, ValueType::Phash;
    Duplicated, "duplicated", "Duplicated", File, ValueType::Duplicated;
    Captions, "captions", "Captions", File, ValueType::Captions;
    Rating, "rating100", "Rating", Activity, ValueType::Number { nullable: true };
    Organized, "organized", "Organized", Activity, ValueType::Bool;
    OCounter, "o_counter", "O-counter", Activity, ValueType::Number { nullable: false };
    PlayCount, "play_count", "Play count", Activity, ValueType::Number { nullable: false };
    PlayDuration, "play_duration", "Play duration", Activity, ValueType::Duration { nullable: false };
    ResumeTime, "resume_time", "Resume time", Activity, ValueType::Duration { nullable: false };
    LastPlayedAt, "last_played_at", "Last played", Activity, ValueType::Timestamp { nullable: false };
    Interactive, "interactive", "Interactive", Activity, ValueType::Bool;
    InteractiveSpeed, "interactive_speed", "Interactive speed", Activity, ValueType::Number { nullable: false };
    Date, "date", "Date", Dates, ValueType::Date;
    CreatedAt, "created_at", "Created", Dates, ValueType::Timestamp { nullable: false };
    UpdatedAt, "updated_at", "Updated", Dates, ValueType::Timestamp { nullable: false };
    StashId, "stash_id_endpoint", "StashID", Other, ValueType::StashId;
    StashIdCount, "stash_id_count", "StashID count", Other, ValueType::Number { nullable: false };
    IsMissing, "is_missing", "Is missing", Other, ValueType::Choice(IS_MISSING_CHOICES);
    CustomField, "custom_fields", "Custom field", Other, ValueType::CustomField;
}

impl CriterionKind {
    /// The entities whose lists offer this kind.
    pub fn entities(self) -> &'static [Entity] {
        use CriterionKind::*;
        const BOTH: &[Entity] = &[Entity::Scenes, Entity::Images];
        match self {
            Photographer | Folder => &[Entity::Images],
            Title | Code | Details | Url | Path | Checksum | PhashDistance | Galleries
            | Organized | OCounter | Resolution | Orientation | IsMissing | Tags | Rating
            | TagCount | PerformerTags | Performers | PerformerCount | PerformerAge
            | PerformerFavorite | Studios | Date | FileCount | CreatedAt | UpdatedAt
            | CustomField | Semantic(_) => BOTH,
            Director | Oshash | Duplicated | Duration | Framerate | Bitrate | VideoCodec
            | AudioCodec | Captions | PlayCount | PlayDuration | ResumeTime | LastPlayedAt
            | Interactive | InteractiveSpeed | HasMarkers | Groups | StashId | StashIdCount => {
                &[Entity::Scenes]
            }
        }
    }

    pub fn applies_to(self, entity: Entity) -> bool {
        self.entities().contains(&entity)
    }

    /// The kind's description as `entity`'s list shows it (images have their own is-missing
    /// choices).
    pub fn info_for(self, entity: Entity) -> KindInfo {
        let info = self.info();
        match info.value_type {
            ValueType::Choice(_) if self == CriterionKind::IsMissing => KindInfo {
                value_type: ValueType::Choice(entity.is_missing_choices()),
                ..info
            },
            _ => info,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip_and_are_unique() {
        let mut keys = std::collections::HashSet::new();
        for kind in CriterionKind::ALL {
            let info = kind.info();
            assert!(keys.insert(info.key), "duplicate key {}", info.key);
            assert_eq!(CriterionKind::from_key(info.key), Some(*kind));
        }
        assert_eq!(CriterionKind::ALL.len(), 48);
    }

    #[test]
    fn every_kind_has_a_usable_default_modifier() {
        for kind in CriterionKind::ALL {
            let info = kind.info();
            let m = info.default_modifier();
            assert!(info.modifiers().is_empty() || info.modifiers().contains(&m));
        }
    }
}
