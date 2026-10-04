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

/// The parts of `SceneFilterType` the adapter uses today.
#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(graphql_type = "SceneFilterType")]
pub struct SceneFilterType {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<ResolutionCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    #[cynic(rename = "video_codec")]
    pub video_codec: Option<StringCriterionInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub path: Option<StringCriterionInput>,
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

/// **Player tier**: what playback needs (the stream URL is built by the core, never read from
/// Stash, whose `paths.stream` embeds the API key).
#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "Scene")]
pub struct PlayableSceneFields {
    pub id: cynic::Id,
    pub title: Option<String>,
    pub files: Vec<PlayableFileFields>,
}
