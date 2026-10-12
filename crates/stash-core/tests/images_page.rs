//! 012 T013: Images' page and details requests (research R2; constitution Principle IV): one
//! request per page with the filter and sort, cards mapped from the visual file, details on demand.

use serde_json::json;
use stash_core::adapter::images::{find_images_page, image_details};
use stash_core::adapter::StashClient;
use stash_core::images::{ImageQuery, ImageSort, Media};
use stash_core::scenes::filter::{Criterion, CriterionKind, CriterionValue, Modifier};
use stash_core::scenes::query::SortDirection;
use stash_core::AppError;
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn page_body() -> serde_json::Value {
    json!({"data": {"findImages": {"count": 3, "images": [
        {"id": "11", "title": "", "date": "2024-05-06",
         "paths": {"thumbnail": "http://stash/image/11/thumbnail?t=1700000001"},
         "visual_files": [{"__typename": "ImageFile", "basename": "beach.jpg", "width": 4000, "height": 3000, "format": "jpeg"}]},
        {"id": "12", "title": "Loop", "date": null,
         "paths": {"thumbnail": "http://stash/image/12/thumbnail?t=1700000002"},
         "visual_files": [{"__typename": "ImageFile", "basename": "loop.gif", "width": 480, "height": 360, "format": "gif"}]},
        {"id": "13", "title": "Clip", "date": null,
         "paths": {"thumbnail": null},
         "visual_files": [{"__typename": "VideoFile", "basename": "clip.mp4", "width": 640, "height": 360, "duration": 5.0}]}
    ]}}})
}

async fn server(body: serde_json::Value) -> (MockServer, StashClient) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
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
async fn a_page_request_sends_the_query_and_maps_the_cards() {
    let (server, client) = server(page_body()).await;
    let mut q = ImageQuery {
        search: " sea ".into(),
        sort: ImageSort::OCounter,
        direction: SortDirection::Asc,
        ..ImageQuery::default()
    };
    q.set_criterion(Criterion::new(
        CriterionKind::Photographer,
        Modifier::Includes,
        CriterionValue::Text("Ann".into()),
    ));
    let page = find_images_page(&client, &q, 2, 40).await.expect("page");

    let sent = bodies(&server).await;
    assert_eq!(sent.len(), 1, "one request per page");
    let body = &sent[0];
    assert!(body["query"]
        .as_str()
        .expect("query")
        .contains("findImages"));
    let f = &body["variables"]["filter"];
    assert_eq!(
        (f["page"].clone(), f["per_page"].clone()),
        (json!(2), json!(40))
    );
    assert_eq!(f["sort"], "o_counter");
    assert_eq!(f["direction"], "ASC");
    assert_eq!(f["q"], "sea");
    assert_eq!(
        body["variables"]["imageFilter"]["photographer"],
        json!({"value": "Ann", "modifier": "INCLUDES"})
    );

    assert_eq!(page.count, 3);
    let [a, b, c] = &page.items[..] else {
        panic!("three cards: {page:?}")
    };
    assert_eq!(a.title, "beach.jpg", "no title: the file's name");
    assert_eq!((a.width, a.height, a.media), (4000, 3000, Media::Still));
    assert_eq!(
        a.thumb.as_deref(),
        Some("ssv-thumb://localhost/image/11?v=1700000001")
    );
    assert_eq!(a.date.as_deref(), Some("2024-05-06"));
    assert_eq!((b.title.as_str(), b.media), ("Loop", Media::MayAnimate));
    assert_eq!(c.media, Media::Clip { duration: 5.0 });
    assert_eq!(
        c.thumb.as_deref(),
        Some("ssv-thumb://localhost/image/13?v=0")
    );
}

#[tokio::test]
async fn random_sends_its_seed() {
    let (server, client) = server(page_body()).await;
    let q = ImageQuery {
        sort: ImageSort::Random,
        seed: Some(7),
        ..ImageQuery::default()
    };
    find_images_page(&client, &q, 1, 50).await.expect("page");
    assert_eq!(
        bodies(&server).await[0]["variables"]["filter"]["sort"],
        "random_7"
    );
}

#[tokio::test]
async fn details_map_every_field() {
    let body = json!({"data": {"findImage": {
        "id": "11", "title": "", "code": "C-1", "date": "2024-05-06", "photographer": "Ann",
        "details": "  ", "rating100": 80, "organized": true, "o_counter": 2,
        "urls": ["https://example.org/a", " "],
        "studio": {"id": "4", "name": "Studio"},
        "performers": [{"id": "2", "name": "A", "image_path": "http://stash/performer/2/image?t=5"}],
        "tags": [{"id": "6", "name": "Tag"}],
        "galleries": [{"id": "9", "title": "Trip", "image_count": 12}],
        "visual_files": [{"__typename": "ImageFile", "basename": "beach.jpg", "path": "/lib/beach.jpg",
                          "size": 123456, "format": "jpeg", "width": 4000, "height": 3000}]
    }}});
    let (server, client) = server(body).await;
    let d = image_details(&client, "11").await.expect("details");
    assert_eq!(d.title, "beach.jpg");
    assert_eq!(d.code.as_deref(), Some("C-1"));
    assert_eq!(d.photographer.as_deref(), Some("Ann"));
    assert_eq!(d.details, None, "blank text is none");
    assert_eq!((d.rating100, d.organized, d.o_count), (Some(80), true, 2));
    assert_eq!(d.urls, vec!["https://example.org/a".to_owned()]);
    assert_eq!(d.studio.as_ref().map(|s| s.name.as_str()), Some("Studio"));
    assert_eq!(d.performers[0].image.as_deref(), Some("5"));
    assert_eq!(d.tags[0].id, "6");
    assert_eq!(d.galleries[0].image_count, 12);
    let file = d.file.expect("file");
    assert_eq!(
        (file.size, file.width, file.format.as_str()),
        (123456, 4000, "jpeg")
    );
    let sent = bodies(&server).await;
    assert_eq!(sent.len(), 1);
    assert!(!sent[0]["query"]
        .as_str()
        .expect("q")
        .contains("custom_fields"));
}

#[tokio::test]
async fn a_missing_image_is_image_not_found() {
    let (_server, client) = server(json!({"data": {"findImage": null}})).await;
    match image_details(&client, "99").await {
        Err(AppError::ImageNotFound { id }) => assert_eq!(id, "99"),
        other => panic!("{other:?}"),
    }
}
