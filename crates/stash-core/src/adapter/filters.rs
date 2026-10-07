//! Saved scene filters and the server's UI settings (spec FR-004, research R3;
//! contracts/query-budget.md `UiConfig`).

use cynic::QueryBuilder;
use serde::{Deserialize, Serialize};

use super::gql::{schema, JsonMap};
use super::StashClient;
use crate::error::AppError;
use crate::scenes::filter::saved::{decode_saved_filter, SavedFilter};
use crate::semantic::{parse_colour_rules, ColourRule, TAG_COLOURS_KEY};

// ---- SavedSceneFilters ------------------------------------------------------------------------

#[derive(cynic::Enum, Clone, Copy, Debug)]
#[cynic(graphql_type = "FilterMode")]
pub(crate) enum FilterMode {
    Scenes,
    Performers,
    Studios,
    Galleries,
    SceneMarkers,
    Movies,
    Groups,
    Tags,
    Images,
}

#[derive(cynic::QueryVariables, Debug)]
pub(crate) struct SavedFiltersVariables {
    pub(crate) mode: Option<FilterMode>,
}

#[derive(cynic::QueryFragment, Debug, serde::Serialize)]
#[cynic(graphql_type = "SavedFindFilterType")]
struct SavedFind {
    q: Option<String>,
    page: Option<i32>,
    #[cynic(rename = "per_page")]
    per_page: Option<i32>,
    sort: Option<String>,
    direction: Option<super::gql::SortDirectionEnum>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "SavedFilter")]
struct SavedFilterFields {
    id: cynic::Id,
    name: String,
    #[cynic(rename = "find_filter")]
    find_filter: Option<SavedFind>,
    #[cynic(rename = "object_filter")]
    object_filter: Option<JsonMap>,
    #[cynic(rename = "ui_options")]
    ui_options: Option<JsonMap>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "SavedFiltersVariables")]
pub(crate) struct SavedSceneFilters {
    #[arguments(mode: $mode)]
    find_saved_filters: Vec<SavedFilterFields>,
}

/// One `SavedSceneFilters` request: every saved scene filter, decoded (cached per server as
/// `saved-filters:scenes`).
pub async fn saved_scene_filters(client: &StashClient) -> Result<Vec<SavedFilter>, AppError> {
    let data = client
        .graphql(&SavedSceneFilters::build(SavedFiltersVariables {
            mode: Some(FilterMode::Scenes),
        }))
        .await?;
    Ok(data
        .find_saved_filters
        .into_iter()
        .map(|f| {
            let find = f
                .find_filter
                .as_ref()
                .and_then(|find| serde_json::to_value(find).ok());
            decode_saved_filter(
                f.id.inner(),
                &f.name,
                find.as_ref(),
                f.object_filter.as_ref().map(|m| &m.0),
                f.ui_options.as_ref().map(|m| &m.0),
            )
        })
        .collect())
}

// ---- UiConfig: configuration { ui } ---------------------------------------------------------

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "ConfigResult")]
pub(crate) struct UiConfigResult {
    ui: JsonMap,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
pub(crate) struct UiConfig {
    configuration: UiConfigResult,
}

/// What the viewer reads from the server's UI configuration, once per session (cached as
/// `ui:settings` and quietly refreshed).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiSettings {
    /// The web UI's default scene filter (`defaultFilters.scenes`), in Stash's saved-filter
    /// format, decoded by the filter model.
    pub default_scene_filter: Option<serde_json::Value>,
    /// The share watched at which a viewing counts as a play (`minimumPlayPercent`; 0 when unset).
    pub minimum_play_percent: f64,
    /// The web UI's own "track activity" setting (`trackActivity`); read, not used for history.
    pub web_track_activity: bool,
    /// The tag colour rules (`semanticStashViewer.tagColours`), or the defaults.
    pub colour_rules: Vec<ColourRule>,
}

/// Read the UI settings from `configuration { ui }`'s map. Missing keys give the defaults.
pub fn parse_ui_settings(ui: &serde_json::Value) -> UiSettings {
    let colour_value = TAG_COLOURS_KEY
        .split('.')
        .try_fold(ui, |node, key| node.get(key));
    let (colour_rules, skipped) = parse_colour_rules(colour_value);
    for field in skipped {
        tracing::warn!(field = %field, "skipped an invalid tag colour rule");
    }
    UiSettings {
        default_scene_filter: ui
            .get("defaultFilters")
            .and_then(|f| f.get("scenes"))
            .filter(|v| v.is_object())
            .cloned(),
        minimum_play_percent: ui
            .get("minimumPlayPercent")
            .and_then(serde_json::Value::as_f64)
            .filter(|p| p.is_finite() && *p >= 0.0)
            .unwrap_or(0.0),
        // The web UI's default is on.
        web_track_activity: ui
            .get("trackActivity")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
        colour_rules,
    }
}

/// One `UiConfig` request.
pub async fn ui_settings(client: &StashClient) -> Result<UiSettings, AppError> {
    let data = client.graphql(&UiConfig::build(())).await?;
    Ok(parse_ui_settings(&data.configuration.ui.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic::default_colour_rules;
    use serde_json::json;

    #[test]
    fn missing_keys_give_the_defaults() {
        let s = parse_ui_settings(&json!({ "advancedMode": false }));
        assert_eq!(s.default_scene_filter, None);
        assert_eq!(s.minimum_play_percent, 0.0);
        assert!(s.web_track_activity);
        assert_eq!(s.colour_rules, default_colour_rules());
    }

    #[test]
    fn reads_the_default_filter_play_settings_and_colour_rules() {
        let s = parse_ui_settings(&json!({
            "defaultFilters": { "scenes": { "mode": "SCENES", "find_filter": { "q": "x" } } },
            "minimumPlayPercent": 25,
            "trackActivity": false,
            "semanticStashViewer": { "tagColours": { "version": 1, "rules": [
                { "field": "isRole", "colour": "#8840c0" }
            ]}}
        }));
        assert_eq!(
            s.default_scene_filter.as_ref().and_then(|f| f.get("mode")),
            Some(&json!("SCENES"))
        );
        assert_eq!(s.minimum_play_percent, 25.0);
        assert!(!s.web_track_activity);
        assert_eq!(s.colour_rules.len(), 1);
        assert_eq!(s.colour_rules[0].field, "isRole");
    }

    #[test]
    fn a_negative_or_odd_play_percent_means_zero() {
        assert_eq!(
            parse_ui_settings(&json!({ "minimumPlayPercent": -5 })).minimum_play_percent,
            0.0
        );
        assert_eq!(
            parse_ui_settings(&json!({ "minimumPlayPercent": "20" })).minimum_play_percent,
            0.0
        );
    }

    #[test]
    fn settings_round_trip_through_the_cache_format() {
        let s = parse_ui_settings(&json!({}));
        let json = serde_json::to_string(&s).expect("encode");
        let back: UiSettings = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, s);
    }
}
