//! Scene cards for the grid and list (005 data-model "SceneCard", "ScenePage").

use stash_core::adapter::scenes::find_scenes_page;
use stash_core::adapter::StashClient;
use stash_core::scenes::query::SceneQuery;
use stash_core::scenes::{screenshot_version, thumb_url};
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PAGE: &str = include_str!("fixtures/stash-v0.31.1/find-scenes-page.json");

async fn page() -> stash_core::scenes::ScenePage {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(PAGE, "application/json"))
        .mount(&server)
        .await;
    let client =
        StashClient::new(Url::parse(&server.uri()).expect("url"), false, None).expect("client");
    find_scenes_page(&client, &SceneQuery::default(), 1, 50)
        .await
        .expect("page")
}

#[tokio::test]
async fn cards_carry_what_the_grid_shows() {
    let p = page().await;
    assert_eq!(p.count, 36_350);
    assert_eq!(p.page, 1);
    assert_eq!(p.page_size, 50);
    assert_eq!(p.items.len(), 3);

    let a = &p.items[0];
    assert_eq!(a.id, "501");
    assert_eq!(a.title, "Scene A");
    assert_eq!(a.date.as_deref(), Some("2024-05-17"));
    assert_eq!(a.duration_seconds, Some(3725.4));
    assert_eq!(
        a.resolution.as_deref(),
        Some("1920×1080"),
        "the primary (first) file"
    );
    assert_eq!(a.studio.as_deref(), Some("Studio One"));
    assert_eq!(
        a.thumb.as_deref(),
        Some("ssv-thumb://localhost/scene/501?v=1715900000")
    );
    assert!(!a.has_preview, "filled in by US5");
}

#[tokio::test]
async fn missing_fields_stay_empty_and_the_title_falls_back_to_the_file_name() {
    let p = page().await;
    let b = &p.items[1];
    assert_eq!(b.title, "untitled-clip.mkv");
    assert_eq!(b.date, None);
    assert_eq!(b.studio, None);
    assert_eq!(b.resolution.as_deref(), Some("3840×2160"));

    let c = &p.items[2];
    assert_eq!(c.title, "No Files");
    assert_eq!(c.duration_seconds, None);
    assert_eq!(c.resolution, None);
    assert_eq!(
        c.thumb.as_deref(),
        Some("ssv-thumb://localhost/scene/503?v=0")
    );
}

#[test]
fn thumbnail_urls_carry_the_screenshot_version_and_no_credentials() {
    assert_eq!(
        screenshot_version("http://h/scene/1/screenshot?t=1715900000"),
        "1715900000"
    );
    assert_eq!(
        screenshot_version("http://h/scene/1/screenshot?apikey=SECRET&t=42"),
        "42"
    );
    assert_eq!(screenshot_version("http://h/scene/1/screenshot"), "0");
    assert_eq!(screenshot_version("not a url"), "0");
    let url = thumb_url("scene", "7", "42");
    assert_eq!(url, "ssv-thumb://localhost/scene/7?v=42");
    assert!(!url.contains("apikey"));
}
