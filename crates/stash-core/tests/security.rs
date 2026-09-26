//! Security state derivation (FR-020, research R4).

use stash_core::connection::connect::{test_connection, ConnectOptions};
use stash_core::connection::security::{derive_security, SecurityState};
use stash_core::profiles::ProfileDraft;
use tokio_util::sync::CancellationToken;
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn url(s: &str) -> Url {
    Url::parse(s).expect("url")
}

#[test]
fn http_is_unencrypted_regardless_of_strict() {
    assert_eq!(
        derive_security(&url("http://stash:9999"), false),
        SecurityState::Unencrypted
    );
    assert_eq!(
        derive_security(&url("http://stash:9999"), true),
        SecurityState::Unencrypted
    );
}

#[test]
fn https_without_strict_is_unverified_even_if_the_cert_is_valid() {
    // Nothing was checked, so the viewer doesn't claim "verified".
    assert_eq!(
        derive_security(&url("https://stash.example.com"), false),
        SecurityState::EncryptedUnverified
    );
}

#[test]
fn https_with_strict_is_verified() {
    // Only reachable after a successful strict handshake.
    assert_eq!(
        derive_security(&url("https://stash.example.com"), true),
        SecurityState::EncryptedVerified
    );
}

#[test]
fn final_scheme_decides_after_a_redirect() {
    // The address was http://… but the final URL after redirects is https.
    let final_url = url("https://stash.example.com");
    assert_eq!(
        derive_security(&final_url, false),
        SecurityState::EncryptedUnverified
    );
}

#[tokio::test]
async fn test_connection_reports_security_for_http() {
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
    assert_eq!(outcome.security, SecurityState::Unencrypted);
}
