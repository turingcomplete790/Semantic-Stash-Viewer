//! 008 T072: previews and seek-bar sprites in the core (research R8, R9).

use stash_core::adapter::StashClient;
use stash_core::previews::{decode_preview, preview_bytes};
use stash_core::sprites::{lookup, parse_vtt, sprite_sources, Rect};
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PREVIEW: &[u8] = include_bytes!("fixtures/preview-3-frames.webp");

#[test]
fn an_animated_webp_decodes_to_frames_with_their_delays() {
    let frames = decode_preview(PREVIEW, 640).expect("frames");
    assert_eq!(frames.len(), 3);
    assert_eq!(
        frames.iter().map(|f| f.delay_ms).collect::<Vec<_>>(),
        vec![100, 200, 300]
    );
    assert_eq!(
        (frames[0].width, frames[0].height),
        (64, 36),
        "never enlarged"
    );
    assert_eq!(frames[0].rgba.len(), 64 * 36 * 4);
    // The first frame is red.
    assert!(frames[0].rgba[0] > 150 && frames[0].rgba[1] < 100);
}

#[test]
fn frames_are_scaled_down_to_the_card_width() {
    let frames = decode_preview(PREVIEW, 32).expect("frames");
    assert_eq!((frames[0].width, frames[0].height), (32, 18));
    assert_eq!(frames[0].rgba.len(), 32 * 18 * 4);
}

#[test]
fn something_that_isnt_a_webp_is_an_error_not_a_panic() {
    assert!(decode_preview(b"not a webp", 100).is_err());
}

const VTT: &str = "WEBVTT

00:00:00.000 --> 00:00:12.679
abc_sprite.jpg#xywh=0,0,160,90

00:00:12.679 --> 00:00:25.358
abc_sprite.jpg#xywh=160,0,160,90

00:25.358 --> 00:38.038
abc_sprite.jpg#xywh=320,0,160,90

garbage --> line
abc_sprite.jpg#xywh=1,2
";

#[test]
fn the_vtt_index_maps_times_to_sheet_regions() {
    let cues = parse_vtt(VTT);
    assert_eq!(cues.len(), 3, "malformed cues are skipped");
    assert_eq!(
        lookup(&cues, 0.0),
        Some(Rect {
            x: 0,
            y: 0,
            w: 160,
            h: 90
        })
    );
    assert_eq!(lookup(&cues, 13.0).map(|r| r.x), Some(160));
    assert_eq!(lookup(&cues, 30.0).map(|r| r.x), Some(320), "mm:ss times");
    assert_eq!(
        lookup(&cues, 999.0).map(|r| r.x),
        Some(320),
        "past the end: the last"
    );
    assert_eq!(lookup(&[], 1.0), None);
}

async fn server_404() -> (MockServer, StashClient) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404).set_body_string("404 page not found"))
        .mount(&server)
        .await;
    let client =
        StashClient::new(Url::parse(&server.uri()).expect("url"), false, None).expect("client");
    (server, client)
}

#[tokio::test]
async fn a_missing_preview_or_sprite_is_a_quiet_none() {
    let (_s, client) = server_404().await;
    assert_eq!(preview_bytes(&client, "181").await.expect("no error"), None);
    assert_eq!(
        sprite_sources(&client, "181").await.expect("no error"),
        None
    );
}

#[tokio::test]
async fn the_preview_is_fetched_from_the_scenes_webp_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/scene/185/webp"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(PREVIEW.to_vec(), "image/webp"))
        .mount(&server)
        .await;
    let client =
        StashClient::new(Url::parse(&server.uri()).expect("url"), false, None).expect("client");
    let bytes = preview_bytes(&client, "185")
        .await
        .expect("ok")
        .expect("some");
    assert_eq!(bytes, PREVIEW);
}

#[test]
fn decoding_frame_by_frame_stops_when_asked() {
    let mut seen = 0;
    let decoded = stash_core::previews::decode_preview_each(PREVIEW, 64, |_| {
        seen += 1;
        seen < 2
    })
    .expect("decoded");
    assert_eq!((decoded, seen), (2, 2), "stopped after the second frame");
}
