//! Server identity (003 research R2): tells servers apart without storing their paths.

use stash_core::cache::identity::server_identity;
use stash_core::connection::connect::{test_connection, ConnectOptions};
use stash_core::profiles::ProfileDraft;
use tokio_util::sync::CancellationToken;
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn url(s: &str) -> Url {
    Url::parse(s).expect("url")
}

#[test]
fn is_16_hex_digits_and_stable() {
    let a = server_identity(
        &url("http://localhost:9999"),
        Some("/data/a.sqlite"),
        Some("/c.yml"),
    );
    assert_eq!(a.len(), 16);
    assert!(a
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert_eq!(
        a,
        server_identity(
            &url("http://localhost:9999"),
            Some("/data/a.sqlite"),
            Some("/c.yml")
        )
    );
}

#[test]
fn changes_when_any_input_changes() {
    let base = server_identity(&url("http://localhost:9999"), Some("/db"), Some("/cfg"));
    assert_ne!(
        base,
        server_identity(&url("http://localhost:9998"), Some("/db"), Some("/cfg"))
    );
    assert_ne!(
        base,
        server_identity(&url("http://localhost:9999"), Some("/db2"), Some("/cfg"))
    );
    assert_ne!(
        base,
        server_identity(&url("http://localhost:9999"), Some("/db"), Some("/cfg2"))
    );
    assert_ne!(
        base,
        server_identity(&url("http://localhost:9999"), None, Some("/cfg"))
    );
}

#[test]
fn ignores_trailing_slashes_and_host_case() {
    let a = server_identity(&url("http://Stash.LAN:9999/"), Some("/db"), None);
    let b = server_identity(&url("http://stash.lan:9999"), Some("/db"), None);
    assert_eq!(a, b);
}

#[tokio::test]
async fn a_probed_server_carries_its_identity() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/healthz"))
        .respond_with(ResponseTemplate::new(200).set_body_string("."))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            include_str!("fixtures/stash-v0.31.1/probe-ok.json"),
            "application/json",
        ))
        .mount(&server)
        .await;
    let draft = ProfileDraft {
        address: server.uri(),
        ..Default::default()
    };
    let outcome = test_connection(&draft, &CancellationToken::new(), ConnectOptions::default())
        .await
        .expect("connect");
    assert_eq!(
        outcome.server.identity,
        server_identity(
            &outcome.base_url,
            Some("/data/stash-go.sqlite"),
            Some("/data/config.yml")
        )
    );
}
