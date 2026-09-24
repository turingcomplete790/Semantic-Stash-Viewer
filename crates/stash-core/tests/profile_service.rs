use std::sync::Mutex;

use stash_core::connection::connect::ConnectOptions;
use stash_core::connection::ConnectFailure;
use stash_core::profiles::service::{create_profile, update_profile};
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

// ---- update_profile (T033, FR-013) ----

async fn saved(server: &MockServer, store: &Mutex<ProfileStore>, key: Option<&str>) -> uuid::Uuid {
    let (profile, _) = create_profile(
        store,
        &draft(server.uri(), key, Some("Home")),
        &CancellationToken::new(),
        ConnectOptions::default(),
    )
    .await
    .expect("create");
    profile.id
}

#[tokio::test]
async fn failed_validation_without_force_keeps_old_settings() {
    let server = stash().await;
    let (_dir, store) = store();
    let id = saved(&server, &store, Some("old-key")).await;
    let before = store.lock().expect("lock").get(id).cloned();

    let err = update_profile(
        &store,
        id,
        &draft("http://127.0.0.1:1".into(), Some("new-key"), Some("Moved")),
        false,
        &CancellationToken::new(),
        ConnectOptions::default(),
    )
    .await
    .expect_err("validation fails");
    assert!(matches!(
        err,
        AppError::Connect {
            failure: ConnectFailure::Unreachable { .. }
        }
    ));
    assert_eq!(store.lock().expect("lock").get(id).cloned(), before);
}

#[tokio::test]
async fn failed_validation_with_force_saves() {
    let server = stash().await;
    let (_dir, store) = store();
    let id = saved(&server, &store, None).await;

    let updated = update_profile(
        &store,
        id,
        &draft("http://127.0.0.1:1".into(), Some("new-key"), Some("Moved")),
        true,
        &CancellationToken::new(),
        ConnectOptions::default(),
    )
    .await
    .expect("forced save");
    assert_eq!(updated.base_url.as_str(), "http://127.0.0.1:1/");
    assert_eq!(updated.api_key.as_deref(), Some("new-key"));
    assert_eq!(updated.display_name, "Moved");
    assert_eq!(store.lock().expect("lock").get(id), Some(&updated));
}

#[tokio::test]
async fn draft_key_replaces_and_blank_or_null_clears() {
    let server = stash().await;
    let (_dir, store) = store();
    let id = saved(&server, &store, Some("old-key")).await;

    for (key, expected) in [
        (Some("  new-key "), Some("new-key")),
        (Some("   "), None),
        (Some("again"), Some("again")),
        (None, None),
    ] {
        let updated = update_profile(
            &store,
            id,
            &draft(server.uri(), key, Some("Home")),
            false,
            &CancellationToken::new(),
            ConnectOptions::default(),
        )
        .await
        .expect("update");
        assert_eq!(updated.api_key.as_deref(), expected, "key {key:?}");
    }
}

#[tokio::test]
async fn update_keeps_id_and_created_at_and_rejects_duplicates() {
    let server_a = stash().await;
    let server_b = stash().await;
    let (_dir, store) = store();
    let a = saved(&server_a, &store, None).await;
    let b = saved(&server_b, &store, None).await;
    let created = store.lock().expect("lock").get(a).map(|p| p.created_at);

    // Pointing A at B's server is a duplicate.
    let err = update_profile(
        &store,
        a,
        &draft(server_b.uri(), None, None),
        false,
        &CancellationToken::new(),
        ConnectOptions::default(),
    )
    .await
    .expect_err("duplicate");
    assert_eq!(
        err,
        AppError::Connect {
            failure: ConnectFailure::DuplicateProfile { existing_id: b }
        }
    );

    // Re-saving A against its own server is fine (not a duplicate of itself).
    let updated = update_profile(
        &store,
        a,
        &draft(server_a.uri(), None, Some("Renamed")),
        false,
        &CancellationToken::new(),
        ConnectOptions::default(),
    )
    .await
    .expect("update");
    assert_eq!(updated.id, a);
    assert_eq!(Some(updated.created_at), created);
    assert_eq!(updated.display_name, "Renamed");
}

#[tokio::test]
async fn update_unknown_profile_is_not_found() {
    let (_dir, store) = store();
    let id = uuid::Uuid::new_v4();
    let err = update_profile(
        &store,
        id,
        &draft("http://127.0.0.1:1".into(), None, None),
        true,
        &CancellationToken::new(),
        ConnectOptions::default(),
    )
    .await
    .expect_err("not found");
    assert_eq!(err, AppError::ProfileNotFound { id });
}
