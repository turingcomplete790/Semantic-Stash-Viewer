use std::sync::Mutex;

use stash_core::connection::connect::ConnectOptions;
use stash_core::connection::ConnectFailure;
use stash_core::profiles::service::create_profile;
use stash_core::profiles::{ProfileDraft, ProfileStore};
use stash_core::AppError;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PROBE_OK: &str = include_str!("fixtures/stash-v0.31.1/probe-ok.json");

async fn stash() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/healthz"))
        .respond_with(ResponseTemplate::new(200).set_body_string("."))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(PROBE_OK, "application/json"))
        .mount(&server)
        .await;
    server
}

fn store() -> (tempfile::TempDir, Mutex<ProfileStore>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = ProfileStore::open(dir.path().join("profiles.json")).expect("open");
    (dir, Mutex::new(store))
}

fn draft(address: String, key: Option<&str>, name: Option<&str>) -> ProfileDraft {
    ProfileDraft {
        address,
        api_key: key.map(str::to_owned),
        display_name: name.map(str::to_owned),
        strict_tls: false,
    }
}

#[tokio::test]
async fn create_saves_final_url_trimmed_key_and_last_used() {
    let server = stash().await;
    let (_dir, store) = store();

    let (profile, outcome) = create_profile(
        &store,
        &draft(format!("{}/graphql", server.uri()), Some("  key-1  "), None),
        &CancellationToken::new(),
        ConnectOptions::default(),
    )
    .await
    .expect("create");

    assert_eq!(
        profile.base_url.as_str().trim_end_matches('/'),
        server.uri()
    );
    assert_eq!(profile.api_key.as_deref(), Some("key-1"));
    assert_eq!(
        profile.display_name,
        server.uri().trim_start_matches("http://")
    );
    assert_eq!(outcome.server.counts.scenes, 27552);

    let s = store.lock().expect("lock");
    assert_eq!(s.list().len(), 1);
    assert_eq!(s.last_used_profile_id(), Some(profile.id));
}

#[tokio::test]
async fn duplicate_server_is_rejected_with_existing_id() {
    let server = stash().await;
    let (_dir, store) = store();
    let (first, _) = create_profile(
        &store,
        &draft(server.uri(), None, Some("Home")),
        &CancellationToken::new(),
        ConnectOptions::default(),
    )
    .await
    .expect("create");

    let err = create_profile(
        &store,
        &draft(format!("{}/", server.uri()), None, None),
        &CancellationToken::new(),
        ConnectOptions::default(),
    )
    .await
    .expect_err("duplicate");
    assert_eq!(
        err,
        AppError::Connect {
            failure: ConnectFailure::DuplicateProfile {
                existing_id: first.id
            }
        }
    );
    assert_eq!(store.lock().expect("lock").list().len(), 1);
}

#[tokio::test]
async fn failed_check_saves_nothing() {
    let (_dir, store) = store();
    let err = create_profile(
        &store,
        &draft("http://127.0.0.1:1".into(), None, None),
        &CancellationToken::new(),
        ConnectOptions::default(),
    )
    .await
    .expect_err("unreachable");
    assert!(matches!(
        err,
        AppError::Connect {
            failure: ConnectFailure::Unreachable { .. }
        }
    ));
    assert!(store.lock().expect("lock").list().is_empty());
}

#[tokio::test]
async fn too_long_name_is_rejected_before_network() {
    let (_dir, store) = store();
    let name = "x".repeat(65);
    let err = create_profile(
        &store,
        &draft("http://127.0.0.1:1".into(), None, Some(&name)),
        &CancellationToken::new(),
        ConnectOptions::default(),
    )
    .await
    .expect_err("invalid name");
    assert!(matches!(err, AppError::InvalidDisplayName { .. }));
}
