//! Scene queries and the direct-stream guard (spec FR-001, FR-003; research R6).

use stash_core::adapter::scenes::{playable_scene, recent_scenes};
use stash_core::adapter::StashClient;
use stash_core::scenes::{direct_stream_url, is_direct_stream};
use stash_core::AppError;
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const RECENT: &str = include_str!("fixtures/stash-v0.31.1/recent-scenes.json");
const PLAYABLE: &str = include_str!("fixtures/stash-v0.31.1/playable-scene.json");
const NOT_FOUND: &str = include_str!("fixtures/stash-v0.31.1/scene-not-found.json");

fn url(s: &str) -> Url {
    Url::parse(s).expect("url")
}

#[test]
fn direct_stream_url_is_built_from_the_base() {
    assert_eq!(
        direct_stream_url(&url("http://h:9999"), "42").as_str(),
        "http://h:9999/scene/42/stream"
    );
    assert_eq!(
        direct_stream_url(&url("https://h/stash"), "42").as_str(),
        "https://h/stash/scene/42/stream"
    );
}

#[test]
fn guard_accepts_only_the_direct_stream() {
    assert!(is_direct_stream(&url("http://h:9999/scene/42/stream")));
    assert!(is_direct_stream(&url("https://h/stash/scene/42/stream")));
    for transcode in [
        "http://h:9999/scene/42/stream.mp4",
        "http://h:9999/scene/42/stream.mkv",
        "http://h:9999/scene/42/stream.webm",
        "http://h:9999/scene/42/stream.m3u8",
        "http://h:9999/scene/42/stream.mpd",
        "http://h:9999/scene/42/stream?resolution=STANDARD",
        "http://h:9999/scene/42/stream.mp4?resolution=LOW",
        "http://h:9999/scene/42/screenshot",
        "http://h:9999/scene//stream",
    ] {
        assert!(
            !is_direct_stream(&url(transcode)),
            "{transcode} must be rejected"
        );
    }
}

async fn stash_answering(body: &'static str) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
    server
}

fn client(server: &MockServer) -> StashClient {
    StashClient::new(url(&server.uri()), false, None).expect("client")
}

#[tokio::test]
async fn recent_scenes_parse_with_title_fallback_and_resolution() {
    let server = stash_answering(RECENT).await;
    let items = recent_scenes(&client(&server)).await.expect("recent");

    assert_eq!(items.len(), 3);
    assert_eq!(items[0].title, "Scene A");
    // Empty title falls back to the primary file's basename.
    assert_eq!(items[1].title, "scene-b.mp4");
    // Whitespace is trimmed.
    assert_eq!(items[2].title, "Scene C");
    assert_eq!(items[0].resolution.as_deref(), Some("1280×720"));
    assert_eq!(items[1].resolution.as_deref(), Some("960×640"));
    assert!(items[0].duration_seconds > 4000.0);
    assert_eq!(items[0].video_codec.as_deref(), Some("h264"));
    assert_eq!(items[0].container.as_deref(), Some("mp4"));
}

#[tokio::test]
async fn playable_scene_builds_the_stream_url_itself() {
    let server = stash_answering(PLAYABLE).await;
    let scene = playable_scene(&client(&server), "1").await.expect("scene");

    assert_eq!(scene.id, "1");
    assert_eq!(scene.title, "scene-a.mp4");
    assert_eq!(
        scene.stream_url.as_str(),
        format!("{}/scene/1/stream", server.uri())
    );
    assert!(is_direct_stream(&scene.stream_url));
    assert_eq!(scene.file.video_codec.as_deref(), Some("h264"));
    assert_eq!(scene.file.audio_codec.as_deref(), Some("aac"));
    assert_eq!(
        (scene.file.width, scene.file.height),
        (Some(960), Some(640))
    );
}

#[tokio::test]
async fn missing_scene_is_scene_not_found() {
    let server = stash_answering(NOT_FOUND).await;
    let err = playable_scene(&client(&server), "999")
        .await
        .expect_err("not found");
    assert_eq!(err, AppError::SceneNotFound { id: "999".into() });
}

#[tokio::test]
async fn scene_without_files_is_no_playable_file() {
    let server =
        stash_answering(r#"{"data":{"findScene":{"id":"7","title":"x","files":[]}}}"#).await;
    let err = playable_scene(&client(&server), "7")
        .await
        .expect_err("no file");
    assert_eq!(err, AppError::NoPlayableFile { id: "7".into() });
}
