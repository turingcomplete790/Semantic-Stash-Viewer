//! Request counts per user action, on a mock server (008 research R2,
//! contracts/query-budget.md). Each action's budget is the list of named operations it may send.

use std::sync::Arc;

use stash_core::adapter::record::{RequestKind, RequestLog};
use stash_core::adapter::scenes::find_scenes_page;
use stash_core::adapter::StashClient;
use stash_core::scenes::query::SceneQuery;
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PAGE: &str = include_str!("fixtures/stash-v0.31.1/find-scenes-page.json");

/// A mock Stash answering every GraphQL request with `body`, and a client recording into a log.
async fn recorded(body: &str) -> (MockServer, StashClient, Arc<RequestLog>) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body.to_owned(), "application/json"))
        .mount(&server)
        .await;
    let log = RequestLog::new();
    let client = StashClient::new(Url::parse(&server.uri()).expect("url"), false, None)
        .expect("client")
        .with_recorder(log.clone());
    (server, client, log)
}

fn budget(log: &RequestLog, expected: &[(&str, usize)]) {
    let expected: Vec<(String, usize)> = expected
        .iter()
        .map(|(name, n)| ((*name).to_owned(), *n))
        .collect();
    assert_eq!(log.counts(), expected, "requests sent for this action");
}

#[tokio::test]
async fn showing_a_page_is_one_scenes_page_request() {
    let (_server, client, log) = recorded(PAGE).await;
    find_scenes_page(&client, &SceneQuery::default(), 1, 50)
        .await
        .expect("page");
    budget(&log, &[("ScenesPage", 1)]);
}

#[tokio::test]
async fn records_carry_sizes_and_kind() {
    let (_server, client, log) = recorded(PAGE).await;
    find_scenes_page(&client, &SceneQuery::default(), 1, 50)
        .await
        .expect("page");
    let record = &log.records()[0];
    assert_eq!(record.kind, RequestKind::GraphQl);
    assert!(record.request_bytes > 0);
    assert_eq!(record.response_bytes, PAGE.len());
}

#[tokio::test]
async fn a_failed_request_is_still_counted() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let log = RequestLog::new();
    let client = StashClient::new(Url::parse(&server.uri()).expect("url"), false, None)
        .expect("client")
        .with_recorder(log.clone());
    assert!(find_scenes_page(&client, &SceneQuery::default(), 1, 50)
        .await
        .is_err());
    budget(&log, &[("ScenesPage", 1)]);
}

#[tokio::test]
async fn reading_the_ui_settings_is_one_ui_config_request() {
    let body =
        r#"{"data":{"configuration":{"ui":{"advancedMode":false,"minimumPlayPercent":20}}}}"#;
    let (_server, client, log) = recorded(body).await;
    let settings = stash_core::adapter::filters::ui_settings(&client)
        .await
        .expect("settings");
    budget(&log, &[("UiConfig", 1)]);
    assert_eq!(settings.minimum_play_percent, 20.0);
    assert_eq!(settings.default_scene_filter, None);
    assert_eq!(
        settings.colour_rules,
        stash_core::semantic::default_colour_rules(),
        "no stored rules: the defaults"
    );
}

#[tokio::test]
async fn a_page_with_search_and_criteria_is_still_one_request() {
    use stash_core::scenes::filter::{Criterion, CriterionKind, CriterionValue, Modifier};
    let (server, client, log) = recorded(PAGE).await;
    let mut q = SceneQuery {
        search: "beach".into(),
        ..SceneQuery::default()
    };
    q.set_criterion(Criterion::new(
        CriterionKind::Organized,
        Modifier::Equals,
        CriterionValue::Bool(true),
    ));
    find_scenes_page(&client, &q, 1, 50).await.expect("page");
    budget(&log, &[("ScenesPage", 1)]);
    let requests = server.received_requests().await.expect("recording");
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).expect("json");
    assert_eq!(body["variables"]["sceneFilter"]["organized"], true);
    assert_eq!(body["variables"]["filter"]["q"], "beach");
}

#[tokio::test]
async fn saved_filters_are_one_request() {
    let body = include_str!("fixtures/saved_filters/testing-saved-filters.json");
    let (_server, client, log) = recorded(body).await;
    let filters = stash_core::adapter::filters::saved_scene_filters(&client)
        .await
        .expect("filters");
    budget(&log, &[("SavedSceneFilters", 1)]);
    assert_eq!(filters.len(), 3);
    assert!(filters.iter().all(|f| f.left_out.is_empty()));
}

#[tokio::test]
async fn each_choice_list_is_one_request() {
    use stash_core::adapter::choices::{choice_list, ChoiceKind};
    let body = r#"{"data":{"findPerformers":{"performers":[]},"findTags":{"tags":[]},
        "findStudios":{"studios":[]},"findGroups":{"groups":[]},"findGalleries":{"galleries":[]}}}"#;
    let (_server, client, log) = recorded(body).await;
    for kind in ChoiceKind::ALL {
        choice_list(&client, kind).await.expect("list");
    }
    budget(
        &log,
        &[
            ("ChoiceListPerformers", 1),
            ("ChoiceListTags", 1),
            ("ChoiceListStudios", 1),
            ("ChoiceListGroups", 1),
            ("ChoiceListGalleries", 1),
        ],
    );
}

#[tokio::test]
async fn the_tags_picker_cold_is_at_most_two_requests() {
    use stash_core::adapter::choices::{choice_list, ChoiceKind};
    let body = r#"{"data":{"findTags":{"tags":[]}}}"#;
    let (_server, client, log) = recorded(body).await;
    choice_list(&client, ChoiceKind::Tags).await.expect("tags");
    stash_core::adapter::semantic::semantic_vocabulary(
        &client,
        &stash_core::semantic::default_colour_rules(),
    )
    .await
    .expect("vocabulary");
    budget(&log, &[("ChoiceListTags", 1), ("SemanticVocabulary", 1)]);
}
