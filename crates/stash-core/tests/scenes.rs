//! Scene queries and the direct-stream guard (spec FR-001, FR-003; research R6).

use stash_core::adapter::scenes::{playable_scene, recent_scenes, test_scenes};
use stash_core::adapter::StashClient;
use stash_core::scenes::{
    direct_stream_url, is_direct_stream, PlayableScene, SceneFile, SeekCache,
    LARGE_FILE_BACK_CACHE_BYTES, MAX_FORWARD_CACHE_BYTES,
};
use stash_core::AppError;
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const RECENT: &str = include_str!("fixtures/stash-v0.31.1/recent-scenes.json");
const PLAYABLE: &str = include_str!("fixtures/stash-v0.31.1/playable-scene.json");
const NOT_FOUND: &str = include_str!("fixtures/stash-v0.31.1/scene-not-found.json");
const TEST_SET: &str = include_str!("fixtures/stash-v0.31.1/test-scenes.json");

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

#[tokio::test]
async fn test_scenes_come_back_as_labelled_groups_in_order() {
    let server = stash_answering(TEST_SET).await;
    let groups = test_scenes(&client(&server)).await.expect("test set");

    let labels: Vec<&str> = groups.iter().map(|g| g.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "4K H.264",
            "4K HEVC",
            "Above 4K",
            "WMV above 720p",
            "VP9 WebM above 720p",
            "AV1",
            "MPEG-4 Part 2 (AVI/DivX)",
            "FLV",
        ]
    );
    let wmv = &groups[3];
    assert!(!wmv.scenes.is_empty());
    assert!(
        wmv.scenes
            .iter()
            .all(|s| s.video_codec.as_deref() == Some("wmv3")
                && s.container.as_deref() == Some("wmv"))
    );
    assert!(groups[0]
        .scenes
        .iter()
        .all(|s| s.resolution.as_deref() == Some("3840×2160")));
}

#[tokio::test]
async fn empty_test_groups_are_dropped() {
    let body = r#"{"data":{
        "fourKH264":{"scenes":[]},"fourKHevc":{"scenes":[]},"aboveFourK":{"scenes":[]},
        "wmvHd":{"scenes":[{"id":"9","title":"","files":[{"basename":"x.wmv","duration":60.0,"width":1920,"height":1080,"video_codec":"wmv3","format":"wmv"}]}]},
        "vp9Hd":{"scenes":[]},"av1":{"scenes":[]},"mpeg4":{"scenes":[]},"flv":{"scenes":[]}}}"#;
    let server = stash_answering(body).await;
    let groups = test_scenes(&client(&server)).await.expect("test set");
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].label, "WMV above 720p");
    assert_eq!(groups[0].scenes[0].title, "x.wmv");
}

#[test]
fn legacy_formats_and_short_clips_get_a_sized_cache() {
    const MIB: u64 = 1 << 20;
    let scene = |container: &str, duration_seconds: f64, size: Option<u64>| PlayableScene {
        id: "1".into(),
        title: "t".into(),
        stream_url: Url::parse("http://stash.local/scene/1/stream").expect("url"),
        duration_seconds,
        file: SceneFile {
            container: Some(container.into()),
            size,
            ..SceneFile::default()
        },
    };
    // Modern containers keep mpv's defaults.
    assert_eq!(scene("mp4", 1800.0, Some(900 * MIB)).seek_cache(), None);
    assert_eq!(scene("matroska", 600.0, Some(200 * MIB)).seek_cache(), None);
    // Short clips of any format are cached whole.
    let clip = scene("webm", 45.0, Some(20 * MIB))
        .seek_cache()
        .expect("cache");
    assert!(clip.forward_bytes > 20 * MIB && clip.forward_bytes < 40 * MIB);
    assert_eq!(clip.back_bytes, clip.forward_bytes);
    // Legacy containers: whole file up to 1 GiB, then capped.
    for container in ["flv", "AVI", "wmv"] {
        let c = scene(container, 1800.0, Some(230 * MIB))
            .seek_cache()
            .expect(container);
        assert!(c.forward_bytes > 230 * MIB && c.forward_bytes < 260 * MIB);
    }
    let capped = Some(SeekCache {
        forward_bytes: MAX_FORWARD_CACHE_BYTES,
        back_bytes: LARGE_FILE_BACK_CACHE_BYTES,
    });
    assert_eq!(scene("avi", 7250.0, Some(1400 * MIB)).seek_cache(), capped);
    assert_eq!(scene("flv", 1800.0, None).seek_cache(), capped);
}

#[tokio::test]
async fn fetches_a_scene_screenshot_as_a_data_url_with_the_api_key() {
    use stash_core::adapter::scenes::scene_screenshot;
    use wiremock::matchers::header;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/scene/5/screenshot"))
        .and(header("ApiKey", "secret"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "image/jpeg")
                .set_body_bytes(vec![0xFF, 0xD8, 0xFF]),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/scene/6/screenshot"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/scene/7/screenshot"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "image/jpeg")
                .set_body_bytes(vec![0u8; 3 * 1024 * 1024]),
        )
        .mount(&server)
        .await;
    let client =
        StashClient::new(url(&server.uri()), false, Some("secret".into())).expect("client");

    let shot = scene_screenshot(&client, "5").await.expect("fetch");
    assert_eq!(shot.as_deref(), Some("data:image/jpeg;base64,/9j/"));
    // Missing screenshot: nothing to show, not an error.
    assert_eq!(scene_screenshot(&client, "6").await.expect("fetch"), None);
    // Oversized: skipped rather than pushed through the UI bridge.
    assert_eq!(scene_screenshot(&client, "7").await.expect("fetch"), None);
}
