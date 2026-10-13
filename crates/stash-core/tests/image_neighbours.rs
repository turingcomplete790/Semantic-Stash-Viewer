//! 012 T031: the viewer's neighbour files (research R4): fetched with the key in a header, kept on
//! disk within their limits, a deleted image is `Missing`.

use stash_core::adapter::StashClient;
use stash_core::images::neighbours::{Neighbours, State};
use url::Url;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn server_with(id: &str, bytes: Vec<u8>, content_type: &str) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/image/{id}/image")))
        .and(header("ApiKey", "secret"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", content_type)
                .set_body_bytes(bytes),
        )
        .mount(&server)
        .await;
    server
}

fn client(server: &MockServer) -> StashClient {
    StashClient::new(
        Url::parse(&server.uri()).expect("url"),
        false,
        Some("secret".into()),
    )
    .expect("client")
}

#[tokio::test]
async fn a_file_is_fetched_with_the_key_in_a_header_and_is_a_local_path() {
    let server = server_with("12", vec![1, 2, 3, 4], "image/jpeg").await;
    let dir = tempfile::tempdir().expect("dir");
    let store = Neighbours::open(dir.path().join("images"));
    let path = store
        .fetch(&client(&server), "12")
        .await
        .expect("fetched")
        .expect("a file");
    assert_eq!(path, dir.path().join("images").join("12.jpg"));
    assert_eq!(std::fs::read(&path).expect("read"), vec![1, 2, 3, 4]);
    assert_eq!(store.ready("12"), Some(path));
    assert_eq!(store.size_bytes(), 4);
    // The URL never carries the key.
    let requests = server.received_requests().await.expect("recorded");
    assert!(requests.iter().all(|r| !r.url.as_str().contains("secret")));
    // Here already: no second request.
    store.fetch(&client(&server), "12").await.expect("again");
    assert_eq!(server.received_requests().await.expect("recorded").len(), 1);
}

#[tokio::test]
async fn without_the_key_the_server_refuses_and_its_failed() {
    let server = server_with("12", vec![1], "image/jpeg").await;
    // Stash refuses a request without the key.
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(401))
        .with_priority(10)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().expect("dir");
    let store = Neighbours::open(dir.path().join("images"));
    let keyless =
        StashClient::new(Url::parse(&server.uri()).expect("url"), false, None).expect("client");
    assert!(store.fetch(&keyless, "12").await.is_err());
    assert_eq!(store.state("12"), Some(State::Failed));
    // A failure can be tried again.
    assert!(store
        .fetch(&client(&server), "12")
        .await
        .expect("ok")
        .is_some());
}

#[tokio::test]
async fn a_404_is_missing() {
    let server = MockServer::start().await;
    let dir = tempfile::tempdir().expect("dir");
    let store = Neighbours::open(dir.path().join("images"));
    let got = store.fetch(&client(&server), "99").await.expect("no error");
    assert_eq!(got, None);
    assert_eq!(store.state("99"), Some(State::Missing));
    assert_eq!(store.ready("99"), None);
}

#[tokio::test]
async fn at_most_the_file_limit_least_recently_used_evicted() {
    let server = MockServer::start().await;
    for id in 1..=4 {
        Mock::given(method("GET"))
            .and(path(format!("/image/{id}/image")))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "image/png")
                    .set_body_bytes(vec![0u8; 10]),
            )
            .mount(&server)
            .await;
    }
    let dir = tempfile::tempdir().expect("dir");
    let store = Neighbours::with_limits(dir.path().join("images"), 3, 1_000);
    let c = client(&server);
    for id in ["1", "2", "3"] {
        store.fetch(&c, id).await.expect("fetched");
    }
    // 1 used again: 2 is now the oldest.
    assert!(store.ready("1").is_some());
    store.fetch(&c, "4").await.expect("fetched");
    assert_eq!(store.state("2"), None, "evicted");
    assert!(!dir.path().join("images").join("2.png").exists());
    for id in ["1", "3", "4"] {
        assert!(store.ready(id).is_some(), "{id} kept");
    }
    assert_eq!(store.size_bytes(), 30);
}

#[tokio::test]
async fn at_most_the_byte_limit_and_no_bigger_file() {
    let server = MockServer::start().await;
    for (id, size) in [("1", 40usize), ("2", 40), ("3", 200)] {
        Mock::given(method("GET"))
            .and(path(format!("/image/{id}/image")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![0u8; size]))
            .mount(&server)
            .await;
    }
    let dir = tempfile::tempdir().expect("dir");
    let store = Neighbours::with_limits(dir.path().join("images"), 8, 100);
    let c = client(&server);
    store.fetch(&c, "1").await.expect("fetched");
    store.fetch(&c, "2").await.expect("fetched");
    assert_eq!(store.size_bytes(), 80);
    assert!(store.fetch(&c, "3").await.is_err(), "larger than the store");
    assert_eq!(store.state("3"), Some(State::Failed));
    assert!(!dir.path().join("images").join("3.part").exists());
    assert_eq!(store.size_bytes(), 80, "nothing evicted for it");
}

#[tokio::test]
async fn clear_and_a_new_session_start_empty() {
    let server = server_with("5", vec![7; 16], "image/webp").await;
    let dir = tempfile::tempdir().expect("dir");
    let store = Neighbours::open(dir.path().join("images"));
    store.fetch(&client(&server), "5").await.expect("fetched");
    assert_eq!(store.clear(), 16);
    assert_eq!(store.size_bytes(), 0);
    store.fetch(&client(&server), "5").await.expect("fetched");
    let again = Neighbours::open(dir.path().join("images"));
    assert_eq!(again.size_bytes(), 0);
    assert!(!dir.path().join("images").join("5.webp").exists());
}

#[tokio::test]
async fn a_login_page_is_never_kept() {
    let server = server_with(
        "8",
        b"<html>login</html>".to_vec(),
        "text/html; charset=utf-8",
    )
    .await;
    let dir = tempfile::tempdir().expect("dir");
    let store = Neighbours::open(dir.path().join("images"));
    assert!(store.fetch(&client(&server), "8").await.is_err());
    assert_eq!(store.state("8"), Some(State::Failed));
    assert_eq!(store.size_bytes(), 0);
}
