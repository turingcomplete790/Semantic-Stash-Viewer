//! One request per 120-card page, never one per card (005 SC-007, constitution Principle IV).

use stash_core::adapter::scenes::find_scenes_page;
use stash_core::adapter::StashClient;
use stash_core::scenes::paging::DEFAULT_PAGE_SIZE;
use stash_core::scenes::query::{SceneQuery, SceneSort, SortDirection};
use url::Url;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PAGE: &str = include_str!("fixtures/stash-v0.31.1/find-scenes-page.json");

async fn server() -> (MockServer, StashClient) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(PAGE, "application/json"))
        .mount(&server)
        .await;
    let client =
        StashClient::new(Url::parse(&server.uri()).expect("url"), false, None).expect("client");
    (server, client)
}

async fn bodies(server: &MockServer) -> Vec<serde_json::Value> {
    server
        .received_requests()
        .await
        .expect("recording")
        .iter()
        .map(|r| serde_json::from_slice(&r.body).expect("json body"))
        .collect()
}

#[tokio::test]
async fn a_page_request_asks_for_120_cards_with_the_query() {
    let (server, client) = server().await;
    let q = SceneQuery {
        search: "  beach ".into(),
        sort: SceneSort::Duration,
        direction: SortDirection::Asc,
        seed: None,
    };
    find_scenes_page(&client, &q, 7, 250).await.expect("page");
    let body = &bodies(&server).await[0];
    let f = &body["variables"]["filter"];
    assert_eq!(f["per_page"], 250);
    assert_eq!(f["page"], 7);
    assert_eq!(f["sort"], "duration");
    assert_eq!(f["direction"], "ASC");
    assert_eq!(f["q"], "beach", "search is trimmed");
    assert!(body["query"]
        .as_str()
        .expect("query")
        .contains("findScenes"));
}

#[tokio::test]
async fn random_sends_the_seed_and_no_search_sends_no_q() {
    let (server, client) = server().await;
    let q = SceneQuery {
        sort: SceneSort::Random,
        seed: Some(42),
        ..SceneQuery::default()
    };
    find_scenes_page(&client, &q, 1, DEFAULT_PAGE_SIZE)
        .await
        .expect("page");
    let f = bodies(&server).await[0]["variables"]["filter"].clone();
    assert_eq!(f["sort"], "random_42");
    assert!(f["q"].is_null(), "an empty search isn't sent");
}

#[tokio::test]
async fn each_page_shown_costs_exactly_one_request_whatever_its_size() {
    let (server, client) = server().await;
    for (page, size) in [(1, 50), (2, 50), (3, 50), (1, 1000)] {
        find_scenes_page(&client, &SceneQuery::default(), page, size)
            .await
            .expect("page");
    }
    let sent = bodies(&server).await;
    assert_eq!(sent.len(), 4, "one request per page, never per card");
    assert_eq!(sent[3]["variables"]["filter"]["per_page"], 1000);
}

#[tokio::test]
async fn sizes_other_than_the_listed_ones_are_refused_without_a_request() {
    let (server, client) = server().await;
    let r = find_scenes_page(&client, &SceneQuery::default(), 1, 55).await;
    assert!(r.is_err());
    assert!(bodies(&server).await.is_empty());
}

#[tokio::test]
async fn a_page_past_the_end_returns_the_last_page() {
    let server = MockServer::start().await;
    // Page 9 at 50 per page is past the end of 120 scenes: Stash answers with no scenes.
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_partial_json(
            serde_json::json!({ "variables": { "filter": { "page": 9 } } }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"data":{"findScenes":{"count":120,"scenes":[]}}}"#,
            "application/json",
        ))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_partial_json(
            serde_json::json!({ "variables": { "filter": { "page": 3 } } }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_raw(PAGE, "application/json"))
        .mount(&server)
        .await;
    let client =
        StashClient::new(Url::parse(&server.uri()).expect("url"), false, None).expect("client");
    let page = find_scenes_page(&client, &SceneQuery::default(), 9, 50)
        .await
        .expect("page");
    assert_eq!(page.page, 3, "the last page that exists");
    assert_eq!(page.page_size, 50);
    assert!(!page.items.is_empty());
}
