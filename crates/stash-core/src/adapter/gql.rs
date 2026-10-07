//! Typed GraphQL through Cynic (007 research R5): the link to Stash v0.31.1's schema (registered in
//! `build.rs`), its custom scalars, the enums and input objects the adapter sends, and the shared
//! **fragments per view tier** (constitution Principle IV). Every struct here is checked against
//! the schema at build time.

#[cynic::schema("stash")]
pub mod schema {}

/// Stash's `Time` scalar: RFC 3339 text.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct Time(pub String);
cynic::impl_scalar!(Time, schema::Time);

/// Stash's `Int64` scalar (file sizes).
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(transparent)]
pub struct Int64(pub i64);
cynic::impl_scalar!(Int64, schema::Int64);

/// Stash's `Map` scalar (`custom_fields`, which the semantic plugin uses): raw JSON.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq)]
#[serde(transparent)]
pub struct JsonMap(pub serde_json::Value);
cynic::impl_scalar!(JsonMap, schema::Map);

/// Stash's `Any` scalar: raw JSON.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq)]
#[serde(transparent)]
pub struct JsonAny(pub serde_json::Value);
cynic::impl_scalar!(JsonAny, schema::Any);

#[derive(cynic::Enum, Clone, Copy, Debug, PartialEq, Eq)]
#[cynic(graphql_type = "SortDirectionEnum")]
pub enum SortDirectionEnum {
    Asc,
    Desc,
}

#[derive(cynic::Enum, Clone, Copy, Debug, PartialEq, Eq)]
#[cynic(graphql_type = "ResolutionEnum")]
pub enum ResolutionEnum {
    VeryLow,
    Low,
    #[cynic(rename = "R360P")]
    R360p,
    Standard,
    WebHd,
    StandardHd,
    FullHd,
    QuadHd,
    VrHd,
    FourK,
    FiveK,
    SixK,
    SevenK,
    EightK,
    Huge,
}

#[derive(cynic::Enum, Clone, Copy, Debug, PartialEq, Eq)]
#[cynic(graphql_type = "CriterionModifier")]
pub enum CriterionModifier {
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    IsNull,
    NotNull,
    IncludesAll,
    Includes,
    Excludes,
    MatchesRegex,
    NotMatchesRegex,
    Between,
    NotBetween,
}

/// `FindFilterType`: paging, search text, and sort.
#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(graphql_type = "FindFilterType")]
pub struct FindFilterType {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub page: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    #[cynic(rename = "per_page")]
    pub per_page: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub direction: Option<SortDirectionEnum>,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(graphql_type = "ResolutionCriterionInput")]
pub struct ResolutionCriterionInput {
    pub value: ResolutionEnum,
    pub modifier: CriterionModifier,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(graphql_type = "StringCriterionInput")]
pub struct StringCriterionInput {
    pub value: String,
    pub modifier: CriterionModifier,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(graphql_type = "IntCriterionInput")]
pub struct IntCriterionInput {
    pub value: i32,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub value2: Option<i32>,
    pub modifier: CriterionModifier,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(graphql_type = "DateCriterionInput")]
pub struct DateCriterionInput {
    pub value: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub value2: Option<String>,
    pub modifier: CriterionModifier,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(graphql_type = "TimestampCriterionInput")]
pub struct TimestampCriterionInput {
    pub value: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub value2: Option<String>,
    pub modifier: CriterionModifier,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(graphql_type = "MultiCriterionInput")]
pub struct MultiCriterionInput {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub value: Option<Vec<cynic::Id>>,
    pub modifier: CriterionModifier,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub excludes: Option<Vec<cynic::Id>>,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(graphql_type = "HierarchicalMultiCriterionInput")]
pub struct HierarchicalMultiCriterionInput {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub value: Option<Vec<cynic::Id>>,
    pub modifier: CriterionModifier,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub depth: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub excludes: Option<Vec<cynic::Id>>,
}

#[derive(cynic::Enum, Clone, Copy, Debug, PartialEq, Eq)]
#[cynic(graphql_type = "OrientationEnum")]
pub enum OrientationEnum {
    Landscape,
    Portrait,
    Square,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(graphql_type = "OrientationCriterionInput")]
pub struct OrientationCriterionInput {
    pub value: Vec<OrientationEnum>,
}

#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(graphql_type = "DuplicationCriterionInput", rename_all = "None")]
pub struct DuplicationCriterionInput {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub phash: Option<bool>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub url: Option<bool>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub stash_id: Option<bool>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub title: Option<bool>,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(graphql_type = "PhashDistanceCriterionInput")]
pub struct PhashDistanceCriterionInput {
    pub value: String,
    pub modifier: CriterionModifier,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub distance: Option<i32>,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(graphql_type = "StashIDCriterionInput", rename_all = "None")]
pub struct StashIdCriterionInput {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub stash_id: Option<String>,
    pub modifier: CriterionModifier,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(graphql_type = "CustomFieldCriterionInput")]
pub struct CustomFieldCriterionInput {
    pub field: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub value: Option<Vec<JsonAny>>,
    pub modifier: CriterionModifier,
}

/// `SceneFilterType`: every criterion the filter model sends (spec FR-002; research R3).
#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(graphql_type = "SceneFilterType", rename_all = "None")]
pub struct SceneFilterType {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub title: Option<StringCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub code: Option<StringCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub details: Option<StringCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub director: Option<StringCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub oshash: Option<StringCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<StringCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub phash_distance: Option<PhashDistanceCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub path: Option<StringCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub file_count: Option<IntCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub rating100: Option<IntCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub organized: Option<bool>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub o_counter: Option<IntCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub duplicated: Option<DuplicationCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<ResolutionCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub orientation: Option<OrientationCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub framerate: Option<IntCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<IntCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub video_codec: Option<StringCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub audio_codec: Option<StringCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub duration: Option<IntCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub has_markers: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub is_missing: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub studios: Option<HierarchicalMultiCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub groups: Option<HierarchicalMultiCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub galleries: Option<MultiCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub tags: Option<HierarchicalMultiCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub tag_count: Option<IntCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub performer_tags: Option<HierarchicalMultiCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub performer_favorite: Option<bool>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub performer_age: Option<IntCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub performers: Option<MultiCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub performer_count: Option<IntCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub stash_id_endpoint: Option<StashIdCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub stash_id_count: Option<IntCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub url: Option<StringCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub interactive: Option<bool>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub interactive_speed: Option<IntCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub captions: Option<StringCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub resume_time: Option<IntCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub play_count: Option<IntCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub play_duration: Option<IntCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub last_played_at: Option<TimestampCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub date: Option<DateCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<TimestampCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<TimestampCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub custom_fields: Option<Vec<CustomFieldCriterionInput>>,
}

// ---- Fragments per view tier ------------------------------------------------------------------

/// The primary file as a list row or card needs it.
#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "VideoFile")]
pub struct ListFileFields {
    pub basename: String,
    pub duration: f64,
    pub width: i32,
    pub height: i32,
}

/// The primary file with its codec and container, for list rows that show them.
#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "VideoFile")]
pub struct CodecFileFields {
    pub basename: String,
    pub duration: f64,
    pub width: i32,
    pub height: i32,
    #[cynic(rename = "video_codec")]
    pub video_codec: String,
    pub format: String,
}

/// A scene as a row in a picker or test list.
#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "Scene")]
pub struct SceneRowFields {
    pub id: cynic::Id,
    pub title: Option<String>,
    pub files: Vec<CodecFileFields>,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "Studio")]
pub struct StudioName {
    pub name: String,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "ScenePathsType")]
pub struct ScreenshotPath {
    pub screenshot: Option<String>,
}

/// **Card tier**: what a grid card or list row shows (005 data model "SceneCard").
#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "Scene")]
pub struct SceneCardFields {
    pub id: cynic::Id,
    pub title: Option<String>,
    pub date: Option<String>,
    pub files: Vec<ListFileFields>,
    pub studio: Option<StudioName>,
    pub paths: ScreenshotPath,
}

/// The primary file as the player needs it.
#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "VideoFile")]
pub struct PlayableFileFields {
    pub basename: String,
    pub duration: f64,
    pub width: i32,
    pub height: i32,
    #[cynic(rename = "video_codec")]
    pub video_codec: String,
    #[cynic(rename = "audio_codec")]
    pub audio_codec: String,
    pub format: String,
    #[cynic(rename = "frame_rate")]
    pub frame_rate: f64,
    #[cynic(rename = "bit_rate")]
    pub bit_rate: i32,
    pub size: Int64,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "Performer")]
pub struct PerformerName {
    pub id: cynic::Id,
    pub name: String,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "Tag")]
pub struct TagName {
    pub id: cynic::Id,
    pub name: String,
}

/// A studio with its id (the scene page's studio leads to filtered Scenes, 008 FR-006).
#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "Studio")]
pub struct StudioRef {
    pub id: cynic::Id,
    pub name: String,
}

/// **Detail tier**: what the scene view shows (007 T048).
#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "Scene")]
pub struct SceneDetailFields {
    pub id: cynic::Id,
    pub title: Option<String>,
    pub code: Option<String>,
    pub details: Option<String>,
    pub director: Option<String>,
    pub date: Option<String>,
    pub rating100: Option<i32>,
    #[cynic(rename = "play_count")]
    pub play_count: Option<i32>,
    #[cynic(rename = "o_counter")]
    pub o_counter: Option<i32>,
    pub files: Vec<PlayableFileFields>,
    pub studio: Option<StudioRef>,
    pub performers: Vec<PerformerName>,
    pub tags: Vec<TagName>,
    pub paths: ScreenshotPath,
}

/// **Player tier**: what playback needs (the stream URL is built by the core, never read from
/// Stash, whose `paths.stream` embeds the API key).
#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "Scene")]
pub struct PlayableSceneFields {
    pub id: cynic::Id,
    pub title: Option<String>,
    pub files: Vec<PlayableFileFields>,
}
