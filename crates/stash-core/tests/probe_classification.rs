//! Probe classification against recorded Stash v0.31.1 responses (contracts/stash-http.md).

use std::time::Duration;

use stash_core::adapter::probe::probe;
use stash_core::adapter::StashClient;
use stash_core::connection::connect::{test_connection, ConnectOptions};
use stash_core::connection::version::VersionStatus;
use stash_core::connection::ConnectFailure;
use stash_core::profiles::ProfileDraft;
use stash_core::AppError;
use tokio_util::sync::CancellationToken;
use url::Url;
use wiremock::matchers::{header, header_exists, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stash-v0.31.1");

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{FIXTURES}/{name}")).expect("fixture")
}

fn stash_json(name: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(fixture(name), "application/json")
}

fn form_based_401() -> ResponseTemplate {
    // Matches probe-401-formbased.http, captured from a live server.
    assert!(fixture("probe-401-formbased.http").contains("Www-Authenticate: FormBased"));
    ResponseTemplate::new(401).insert_header("Www-Authenticate", "FormBased")
}

async fn healthz(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/healthz"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("healthz-ok.txt")))
        .mount(server)
        .await;
}

fn client(server: &MockServer, key: Option<&str>) -> StashClient {
    let base = Url::parse(&server.uri()).expect("url");
    StashClient::new(base, false, key.map(str::to_owned)).expect("client")
}

#[tokio::test]
async fn ok_response_returns_version_status_and_counts() {
    let server = MockServer::start().await;
    healthz(&server).await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(stash_json("probe-ok.json"))
        .mount(&server)
        .await;

    let data = probe(&client(&server, None)).await.expect("probe");
    assert_eq!(data.version.as_deref(), Some("v0.31.1"));
    assert_eq!(data.app_schema, 85);
    assert_eq!(data.status, "OK");
    assert_eq!(
        (
            data.counts.scenes,
            data.counts.images,
            data.counts.galleries,
            data.counts.performers
        ),
        (27552, 25632, 739, 575)
    );
}

#[tokio::test]
async fn api_key_is_sent_as_header() {
    let server = MockServer::start().await;
    healthz(&server).await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(header("ApiKey", "good-key"))
        .respond_with(stash_json("probe-ok.json"))
        .expect(1)
        .mount(&server)
        .await;

    probe(&client(&server, Some("good-key")))
        .await
        .expect("probe");
}

#[tokio::test]
async fn form_based_401_without_key_is_api_key_required() {
    let server = MockServer::start().await;
    healthz(&server).await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(form_based_401())
        .mount(&server)
        .await;

    assert_eq!(
        probe(&client(&server, None)).await.err(),
        Some(ConnectFailure::ApiKeyRequired)
    );
}

#[tokio::test]
async fn wrong_key_on_open_server_is_invalid_but_not_required() {
    // Observed on v0.31.1: an invalid key gets 401 even when Stash has no auth configured.
    let server = MockServer::start().await;
    healthz(&server).await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(header_exists("ApiKey"))
        .respond_with(form_based_401())
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(stash_json("probe-ok.json"))
        .mount(&server)
        .await;

    assert_eq!(
        probe(&client(&server, Some("bogus"))).await.err(),
        Some(ConnectFailure::ApiKeyInvalidButNotRequired)
    );
}

#[tokio::test]
async fn wrong_key_on_auth_server_is_rejected() {
    let server = MockServer::start().await;
    healthz(&server).await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(form_based_401())
        .mount(&server)
        .await;

    assert_eq!(
        probe(&client(&server, Some("bogus"))).await.err(),
        Some(ConnectFailure::ApiKeyRejected)
    );
}

#[tokio::test]
async fn html_404_is_not_stash() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(404).set_body_raw("<html>nope</html>", "text/html"))
        .mount(&server)
        .await;

    assert_eq!(
        probe(&client(&server, None)).await.err(),
        Some(ConnectFailure::NotStash { status: Some(404) })
    );
}

#[tokio::test]
async fn non_stash_json_and_non_json_are_not_stash() {
    for body in [
        "hello",
        r#"{"data":{"somethingElse":1}}"#,
        r#"{"errors":[]}"#,
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
        assert_eq!(
            probe(&client(&server, None)).await.err(),
            Some(ConnectFailure::NotStash { status: Some(200) }),
            "body {body}"
        );
    }
}

#[tokio::test]
async fn plain_401_without_form_based_is_not_stash() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(401).insert_header("Www-Authenticate", "Basic"))
        .mount(&server)
        .await;

    assert_eq!(
        probe(&client(&server, None)).await.err(),
        Some(ConnectFailure::NotStash { status: Some(401) })
    );
}

#[tokio::test]
async fn closed_port_is_unreachable() {
    let base = Url::parse("http://127.0.0.1:1").expect("url");
    let c = StashClient::new(base, false, None).expect("client");
    assert!(matches!(
        probe(&c).await.err(),
        Some(ConnectFailure::Unreachable { .. })
    ));
}

#[tokio::test]
async fn slow_response_is_timeout() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(stash_json("probe-ok.json").set_delay(Duration::from_secs(1)))
        .mount(&server)
        .await;
    let base = Url::parse(&server.uri()).expect("url");
    let c =
        StashClient::with_timeout(base, false, None, Duration::from_millis(200)).expect("client");

    assert_eq!(probe(&c).await.err(), Some(ConnectFailure::Timeout));
}

#[tokio::test]
async fn redirect_is_followed_and_final_base_reported() {
    // A 301 turns POST into GET, so the probe resolves redirects via GET /healthz first.
    let server = MockServer::start().await;
    let moved = format!("{}/stash/healthz", server.uri());
    Mock::given(method("GET"))
        .and(path("/healthz"))
        .respond_with(ResponseTemplate::new(301).insert_header("Location", moved.as_str()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/stash/healthz"))
        .respond_with(ResponseTemplate::new(200).set_body_string("."))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/stash/graphql"))
        .respond_with(stash_json("probe-ok.json"))
        .mount(&server)
        .await;

    let data = probe(&client(&server, None)).await.expect("probe");
    assert_eq!(
        data.final_base_url.as_str(),
        format!("{}/stash", server.uri())
    );
}

// ---- test_connection: candidates, version gate, cancellation ----

fn draft(address: &str, key: Option<&str>) -> ProfileDraft {
    ProfileDraft {
        address: address.into(),
        api_key: key.map(str::to_owned),
        ..Default::default()
    }
}

#[tokio::test]
async fn scheme_less_address_falls_back_from_https_to_http() {
    let server = MockServer::start().await;
    healthz(&server).await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(stash_json("probe-ok.json"))
        .mount(&server)
        .await;
    let host_port = server.uri().trim_start_matches("http://").to_owned();

    let outcome = test_connection(
        &draft(&host_port, None),
        &CancellationToken::new(),
        ConnectOptions::default(),
    )
    .await
    .expect("connect");
    assert_eq!(
        outcome.base_url.as_str().trim_end_matches('/'),
        server.uri()
    );
    assert_eq!(outcome.server.version_status, VersionStatus::Supported);
    assert_eq!(outcome.server.counts.scenes, 27552);
}

#[tokio::test]
async fn version_gate_is_applied() {
    for (fixture_name, expected) in [
        (
            "probe-old-version.json",
            Err(ConnectFailure::UnsupportedVersion {
                found: "v0.30.1".into(),
                minimum: "v0.31.1".into(),
            }),
        ),
        (
            "probe-needs-migration.json",
            Err(ConnectFailure::ServerNotReady {
                status: "NEEDS_MIGRATION".into(),
            }),
        ),
        (
            "probe-dev-version.json",
            Ok(VersionStatus::DevelopmentBuild),
        ),
    ] {
        let server = MockServer::start().await;
        healthz(&server).await;
        Mock::given(method("POST"))
            .respond_with(stash_json(fixture_name))
            .mount(&server)
            .await;
        let result = test_connection(
            &draft(&server.uri(), None),
            &CancellationToken::new(),
            ConnectOptions::default(),
        )
        .await
        .map(|o| o.server.version_status)
        .map_err(|e| match e {
            AppError::Connect { failure } => failure,
            other => panic!("unexpected {other:?}"),
        });
        assert_eq!(result, expected, "fixture {fixture_name}");
    }
}

#[tokio::test]
async fn cancellation_returns_cancelled() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(1)))
        .mount(&server)
        .await;
    let token = CancellationToken::new();
    let cancel = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        cancel.cancel();
    });

    let result = test_connection(
        &draft(&server.uri(), None),
        &token,
        ConnectOptions::default(),
    )
    .await;
    assert_eq!(result.err(), Some(AppError::Cancelled));
}

#[tokio::test]
async fn overall_budget_times_out() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(1)))
        .mount(&server)
        .await;
    let options = ConnectOptions {
        timeout: Duration::from_millis(200),
    };

    let result = test_connection(
        &draft(&server.uri(), None),
        &CancellationToken::new(),
        options,
    )
    .await;
    assert_eq!(
        result.err(),
        Some(AppError::Connect {
            failure: ConnectFailure::Timeout
        })
    );
}
