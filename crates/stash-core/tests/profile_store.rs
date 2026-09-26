use chrono::Utc;
use stash_core::profiles::store::PROFILES_FILE_VERSION;
use stash_core::profiles::{ProfileStore, ServerProfile, StoreNotice};
use stash_core::AppError;
use url::Url;
use uuid::Uuid;

fn profile(url: &str, key: Option<&str>) -> ServerProfile {
    ServerProfile {
        id: Uuid::new_v4(),
        display_name: "Home".into(),
        base_url: Url::parse(url).expect("url"),
        strict_tls: true,
        api_key: key.map(str::to_owned),
        created_at: Utc::now(),
        last_used_at: None,
    }
}

#[test]
fn round_trip_keeps_key_and_settings() {
    // SC-004: a saved profile comes back with its key, so relaunch never asks again.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("profiles.json");
    let p = profile("http://localhost:9999", Some("my-api-key"));

    let mut store = ProfileStore::open(&path).expect("open");
    store.insert(p.clone()).expect("insert");
    store.set_last_used(Some(p.id)).expect("last used");

    let reopened = ProfileStore::open(&path).expect("reopen");
    assert_eq!(reopened.list(), std::slice::from_ref(&p));
    assert_eq!(
        reopened.get(p.id).and_then(|x| x.api_key.clone()),
        Some("my-api-key".into())
    );
    assert_eq!(reopened.last_used_profile_id(), Some(p.id));
}

#[test]
fn missing_file_is_empty_store() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = ProfileStore::open(dir.path().join("nested/profiles.json")).expect("open");
    assert!(store.list().is_empty());
}

#[test]
fn atomic_write_leaves_no_temp_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("profiles.json");
    let mut store = ProfileStore::open(&path).expect("open");
    store.insert(profile("http://a:1", None)).expect("insert");
    store.insert(profile("http://b:2", None)).expect("insert");

    let names: Vec<String> = std::fs::read_dir(dir.path())
        .expect("read dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["profiles.json".to_string()]);
}

#[test]
fn future_version_is_refused_and_not_overwritten() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("profiles.json");
    let future = format!(
        r#"{{"version": {}, "profiles": []}}"#,
        PROFILES_FILE_VERSION + 1
    );
    std::fs::write(&path, &future).expect("write");

    let err = ProfileStore::open(&path).expect_err("must refuse");
    assert_eq!(
        err,
        AppError::UnsupportedProfilesVersion {
            found: PROFILES_FILE_VERSION + 1
        }
    );
    assert_eq!(std::fs::read_to_string(&path).expect("read"), future);
}

#[test]
fn corrupt_file_is_moved_to_bak() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("profiles.json");
    std::fs::write(&path, "{ not json").expect("write");

    let mut store = ProfileStore::open(&path).expect("open");
    assert!(store.list().is_empty());
    let backup = dir.path().join("profiles.json.bak");
    assert_eq!(
        store.take_notice(),
        Some(StoreNotice::CorruptFileMovedAside {
            backup: backup.clone()
        })
    );
    assert_eq!(
        std::fs::read_to_string(backup).expect("backup"),
        "{ not json"
    );
    assert!(!path.exists());
}

#[test]
fn find_by_base_url_ignores_case_and_trailing_slash() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut store = ProfileStore::open(dir.path().join("profiles.json")).expect("open");
    let p = profile("http://localhost:9999", None);
    store.insert(p.clone()).expect("insert");

    let probe = Url::parse("HTTP://LocalHost:9999/").expect("url");
    assert_eq!(store.find_by_base_url(&probe).map(|x| x.id), Some(p.id));
    let other = Url::parse("http://localhost:9998").expect("url");
    assert!(store.find_by_base_url(&other).is_none());
}

#[test]
fn remove_clears_last_used_and_reorder_persists() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("profiles.json");
    let (a, b, c) = (
        profile("http://a:1", None),
        profile("http://b:2", None),
        profile("http://c:3", None),
    );
    let mut store = ProfileStore::open(&path).expect("open");
    for p in [&a, &b, &c] {
        store.insert(p.clone()).expect("insert");
    }
    store.set_last_used(Some(b.id)).expect("last used");
    store.reorder(&[c.id, a.id]).expect("reorder");
    store.remove(b.id).expect("remove");

    let reopened = ProfileStore::open(&path).expect("reopen");
    let ids: Vec<Uuid> = reopened.list().iter().map(|p| p.id).collect();
    assert_eq!(ids, vec![c.id, a.id]);
    assert_eq!(reopened.last_used_profile_id(), None);
}
