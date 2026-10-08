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

fn scene_with_performers(n: usize) -> String {
    let performers: Vec<serde_json::Value> = (0..n)
        .map(|i| {
            serde_json::json!({"id": i.to_string(), "name": format!("P{i}"),
                "image_path": format!("http://h/performer/{i}/image?default=true")})
        })
        .collect();
    serde_json::json!({"data": {"findScene": {
        "id": "7", "title": "Seven", "code": null, "details": null, "director": null,
        "date": "2026-01-02", "rating100": 80, "organized": true, "o_counter": 1,
        "play_count": 2, "resume_time": 30.5, "play_duration": 120.0, "urls": ["https://example.org"],
        "studio": {"id": "4", "name": "S"}, "performers": performers,
        "tags": [{"id": "6", "name": "T"}],
        "groups": [{"group": {"id": "1", "name": "G"}, "scene_index": 2}],
        "scene_markers": [
            {"id": "2", "title": "Later", "seconds": 60.0, "end_seconds": null, "primary_tag": {"name": "T"}},
            {"id": "1", "title": "First", "seconds": 10.0, "end_seconds": 20.0, "primary_tag": {"name": "T"}}
        ],
        "files": [{"basename": "seven.mp4", "duration": 125.0, "width": 1920, "height": 1080,
            "video_codec": "h264", "audio_codec": "aac", "format": "mp4", "frame_rate": 30.0,
            "bit_rate": 4000000, "size": 123456}],
        "paths": {"screenshot": "http://h/scene/7/screenshot?t=99",
            "vtt": "http://h/scene/abc_thumbs.vtt?apikey=k", "sprite": "http://h/scene/abc_sprite.jpg?apikey=k"}
    }}})
    .to_string()
}

#[tokio::test]
async fn a_scene_page_is_one_request_even_with_40_performers() {
    let (_server, client, log) = recorded(&scene_with_performers(40)).await;
    let d = stash_core::adapter::scene_page::scene_details(&client, "7")
        .await
        .expect("details");
    budget(&log, &[("SceneDetails", 1)]);
    assert_eq!(d.performers.len(), 40);
    assert_eq!(
        d.performers[0].image, None,
        "Stash's default picture means no image"
    );
    assert_eq!(d.markers[0].title, "First", "markers in time order");
    assert_eq!(d.groups[0].scene_index, Some(2));
    assert_eq!(
        d.sprite.as_deref(),
        Some("abc_sprite.jpg"),
        "no address or key kept"
    );
    assert_eq!(d.resume_time, Some(30.5));
    assert!(d.organized);
}

#[tokio::test]
async fn files_and_galleries_are_one_request_each() {
    let body = r#"{"data":{"findScene":{"files":[{"id":"9","basename":"a.mp4","path":"/x/a.mp4",
        "size":1,"duration":1.0,"width":1,"height":1,"video_codec":"h264","audio_codec":"aac",
        "format":"mp4","frame_rate":30.0,"bit_rate":1,"fingerprints":[{"type":"oshash","value":"ab"}]}],
        "galleries":[{"id":"3","title":null,"image_count":12}]}}}"#;
    let (_server, client, log) = recorded(body).await;
    let files = stash_core::adapter::scene_page::scene_files(&client, "7")
        .await
        .expect("files");
    let galleries = stash_core::adapter::scene_page::scene_galleries(&client, "7")
        .await
        .expect("galleries");
    budget(&log, &[("SceneFiles", 1), ("SceneGalleries", 1)]);
    assert_eq!(files[0].fingerprints, vec![("oshash".into(), "ab".into())]);
    assert_eq!(galleries[0].title, "Gallery 3");
}

#[tokio::test]
async fn each_chip_card_is_one_request() {
    use stash_core::adapter::chips::{chip_details, ChipDetails, ChipKind};
    let body = r#"{"data":{
        "findPerformer":{"id":"2","name":"P","disambiguation":"","gender":"FEMALE","birthdate":"1990-01-01","country":"NZ","favorite":true,"scene_count":5},
        "findTag":{"id":"2","name":"T","description":"A tag","scene_count":9},
        "findStudio":{"id":"2","name":"S","parent_studio":{"name":"Parent"},"scene_count":3}}}"#;
    let (_server, client, log) = recorded(body).await;
    let p = chip_details(&client, ChipKind::Performer, "2")
        .await
        .expect("performer");
    let _ = chip_details(&client, ChipKind::Tag, "2")
        .await
        .expect("tag");
    let s = chip_details(&client, ChipKind::Studio, "2")
        .await
        .expect("studio");
    budget(
        &log,
        &[("PerformerCard", 1), ("TagCard", 1), ("StudioCard", 1)],
    );
    assert!(
        matches!(p, ChipDetails::Performer { gender: Some(ref g), disambiguation: None, .. } if g == "Female")
    );
    assert!(matches!(s, ChipDetails::Studio { parent: Some(ref p), .. } if p == "Parent"));
}

#[tokio::test]
async fn saving_tag_colours_is_one_write_of_that_key_only() {
    use stash_core::semantic::{ColourRule, Rgb};
    let body = r##"{"data":{"configureUISetting":{"semanticStashViewer":{"tagColours":{"version":1,
        "rules":[{"field":"isRole","colour":"#112233"}]}},"advancedMode":true}}}"##;
    let (server, client, log) = recorded(body).await;
    let rules = vec![ColourRule {
        field: "isRole".into(),
        colour: Rgb(0x11, 0x22, 0x33),
    }];
    let settings = stash_core::adapter::filters::save_tag_colours(&client, &rules)
        .await
        .expect("saved");
    budget(&log, &[("SaveTagColours", 1)]);
    assert_eq!(
        settings.colour_rules, rules,
        "the settings Stash now holds come back"
    );
    let requests = server.received_requests().await.expect("recording");
    let sent: serde_json::Value = serde_json::from_slice(&requests[0].body).expect("json");
    assert_eq!(sent["variables"]["key"], "semanticStashViewer.tagColours");
    assert_eq!(
        sent["variables"]["value"],
        serde_json::json!({"version": 1, "rules": [{"field": "isRole", "colour": "#112233"}]}),
        "the whole value, colours in lower case"
    );
    assert!(sent["query"]
        .as_str()
        .expect("query")
        .contains("configureUISetting"));
}
