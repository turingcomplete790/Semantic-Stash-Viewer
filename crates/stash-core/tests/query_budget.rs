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
