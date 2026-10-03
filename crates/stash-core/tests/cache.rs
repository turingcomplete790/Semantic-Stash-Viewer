//! The view cache (003 US1; data-model "Cache file").

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use stash_core::cache::{delete_profile_cache, limit_for_free_space, Limit, ViewCache};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Item {
    name: String,
    blob: String,
}

fn item(name: &str, size: usize) -> Item {
    Item {
        name: name.into(),
        blob: "x".repeat(size),
    }
}

const MB: u64 = 1024 * 1024;
const GB: u64 = 1024 * MB;

fn open(dir: &tempfile::TempDir, limit: Limit) -> ViewCache {
    ViewCache::open_with(dir.path().join("p/cache.sqlite3"), limit).expect("open")
}

#[test]
fn put_then_get_returns_the_value_and_when_it_was_fetched() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut c = open(&dir, Limit::Fixed(10 * MB));
    assert!(c.get::<Item>("scene:1").is_none());
    c.put("scene:1", &item("one", 10)).expect("put");
    let got = c.get::<Item>("scene:1").expect("hit");
    assert_eq!(got.data, item("one", 10));
    assert!(got.from_cache);
    assert!(chrono::DateTime::parse_from_rfc3339(&got.fetched_at).is_ok());
    assert!(c.size_bytes() > 10);
}

#[test]
fn survives_reopening() {
    let dir = tempfile::tempdir().expect("tempdir");
    open(&dir, Limit::Fixed(10 * MB))
        .put("scenes:recent", &item("list", 5))
        .expect("put");
    let c = open(&dir, Limit::Fixed(10 * MB));
    assert_eq!(
        c.get::<Item>("scenes:recent").expect("hit").data.name,
        "list"
    );
}

#[test]
fn evicts_least_recently_used_entries_past_the_limit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut c = open(&dir, Limit::Fixed(3500));
    c.put("a", &item("a", 1000)).expect("a");
    c.put("b", &item("b", 1000)).expect("b");
    c.put("c", &item("c", 1000)).expect("c");
    // Use "a" so "b" becomes the least recently used.
    std::thread::sleep(std::time::Duration::from_millis(5));
    assert!(c.get::<Item>("a").is_some());
    c.put("d", &item("d", 1000)).expect("d");
    assert!(c.size_bytes() <= 3500);
    assert!(
        c.get::<Item>("b").is_none(),
        "least recently used goes first"
    );
    assert!(c.get::<Item>("a").is_some());
    assert!(c.get::<Item>("d").is_some());
}

#[test]
fn the_limit_is_five_percent_of_free_space_clamped() {
    assert_eq!(limit_for_free_space(100 * GB), 5 * GB);
    assert_eq!(limit_for_free_space(500 * MB), 64 * MB);
    assert_eq!(limit_for_free_space(1024 * GB), 10 * GB);
    assert_eq!(limit_for_free_space(0), 64 * MB);
}

#[test]
fn free_space_is_re_read_every_50_writes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let calls = Arc::new(AtomicU32::new(0));
    let counter = Arc::clone(&calls);
    let mut c = open(
        &dir,
        Limit::FreeSpace(Box::new(move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
            Some(100 * GB)
        })),
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1, "read on open");
    for i in 0..49 {
        c.put(&format!("k{i}"), &item("x", 1)).expect("put");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    c.put("k49", &item("x", 1)).expect("put");
    assert_eq!(calls.load(Ordering::SeqCst), 2, "re-read after 50 writes");
}

#[test]
fn invalidation_removes_exactly_the_affected_entries() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut c = open(&dir, Limit::Fixed(10 * MB));
    for key in ["scene:1", "scene:2", "scenes:recent", "server:info"] {
        c.put(key, &item(key, 5)).expect("put");
    }
    assert_eq!(c.invalidate(&["scene:1"]).expect("invalidate"), 1);
    assert!(c.get::<Item>("scene:1").is_none());
    for key in ["scene:2", "scenes:recent", "server:info"] {
        assert!(c.get::<Item>(key).is_some(), "{key} must be untouched");
    }
    assert_eq!(c.invalidate_prefix("scenes:").expect("prefix"), 1);
    assert!(c.get::<Item>("scenes:recent").is_none());
    for key in ["scene:2", "server:info"] {
        assert!(c.get::<Item>(key).is_some(), "{key} must be untouched");
    }
}

#[test]
fn replace_overwrites() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut c = open(&dir, Limit::Fixed(10 * MB));
    c.put("scene:1", &item("old", 1)).expect("put");
    c.replace("scene:1", &item("new", 1)).expect("replace");
    assert_eq!(c.get::<Item>("scene:1").expect("hit").data.name, "new");
}

#[test]
fn a_different_server_wipes_the_cache() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut c = open(&dir, Limit::Fixed(10 * MB));
    assert!(
        !c.check_identity("aaaaaaaaaaaaaaaa").expect("first"),
        "first identity just recorded"
    );
    c.put("scene:1", &item("one", 1)).expect("put");
    assert!(!c.check_identity("aaaaaaaaaaaaaaaa").expect("same"));
    assert!(c.get::<Item>("scene:1").is_some());
    assert!(
        c.check_identity("bbbbbbbbbbbbbbbb").expect("different"),
        "wiped"
    );
    assert!(c.get::<Item>("scene:1").is_none());
    // The new identity sticks across reopening.
    let mut again = open(&dir, Limit::Fixed(10 * MB));
    assert!(!again
        .check_identity("bbbbbbbbbbbbbbbb")
        .expect("same after reopen"));
}

#[test]
fn a_damaged_file_is_recreated_empty() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("p/cache.sqlite3");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
    std::fs::write(&path, b"this is not a database at all").expect("write");
    let mut c = ViewCache::open_with(&path, Limit::Fixed(10 * MB)).expect("recreated");
    assert!(c.get::<Item>("anything").is_none());
    c.put("scene:1", &item("one", 1)).expect("usable");
}

#[test]
fn a_newer_schema_is_recreated() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("p/cache.sqlite3");
    {
        let mut c = ViewCache::open_with(&path, Limit::Fixed(10 * MB)).expect("open");
        c.put("scene:1", &item("one", 1)).expect("put");
    }
    {
        let conn = rusqlite::Connection::open(&path).expect("raw");
        conn.execute(
            "UPDATE meta SET value = '99' WHERE key = 'schema_version'",
            [],
        )
        .expect("bump");
    }
    let c = ViewCache::open_with(&path, Limit::Fixed(10 * MB)).expect("recreated");
    assert!(c.get::<Item>("scene:1").is_none());
}

#[test]
fn clear_reports_the_bytes_freed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut c = open(&dir, Limit::Fixed(10 * MB));
    c.put("a", &item("a", 2000)).expect("a");
    c.put("b", &item("b", 3000)).expect("b");
    let before = c.size_bytes();
    assert_eq!(c.clear().expect("clear"), before);
    assert_eq!(c.size_bytes(), 0);
    assert!(c.get::<Item>("a").is_none());
}

#[test]
fn deleting_a_profile_removes_its_cache_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let profile_dir = dir.path().join("p");
    {
        let mut c = open(&dir, Limit::Fixed(10 * MB));
        c.put("a", &item("a", 1)).expect("a");
    }
    delete_profile_cache(&profile_dir);
    assert!(!profile_dir.exists());
}

// ---- Blob entries (005 T004: thumbnails) ----

#[test]
fn blobs_round_trip_and_count_toward_the_size() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut c = open(&dir, Limit::Fixed(10 * MB));
    assert!(c.get_bytes("thumb:scene:1:7").is_none());
    let bytes: Vec<u8> = (0..4000u32).map(|i| (i % 251) as u8).collect();
    c.put_bytes("thumb:scene:1:7", &bytes).expect("put");
    assert_eq!(c.get_bytes("thumb:scene:1:7").expect("hit"), bytes);
    assert!(c.size_bytes() >= 4000);
    // A blob isn't JSON: reading it as typed data is a miss, not a crash.
    assert!(c.get::<Item>("thumb:scene:1:7").is_none());
}

#[test]
fn blobs_are_evicted_least_recently_used_with_everything_else() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut c = open(&dir, Limit::Fixed(3500));
    c.put_bytes("thumb:scene:a:1", &[1u8; 1000]).expect("a");
    c.put("scenes:q:x:p:1", &item("page", 1000)).expect("page");
    c.put_bytes("thumb:scene:c:1", &[3u8; 1000]).expect("c");
    std::thread::sleep(std::time::Duration::from_millis(5));
    assert!(c.get_bytes("thumb:scene:a:1").is_some());
    c.put_bytes("thumb:scene:d:1", &[4u8; 1000]).expect("d");
    assert!(c.size_bytes() <= 3500);
    assert!(
        c.get::<Item>("scenes:q:x:p:1").is_none(),
        "least recently used goes first"
    );
    assert!(c.get_bytes("thumb:scene:a:1").is_some());
    assert!(c.get_bytes("thumb:scene:d:1").is_some());
}

#[test]
fn clearing_or_a_different_server_removes_blobs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut c = open(&dir, Limit::Fixed(10 * MB));
    c.check_identity("aaaaaaaaaaaaaaaa").expect("first");
    c.put_bytes("thumb:scene:1:1", &[9u8; 100]).expect("put");
    c.clear().expect("clear");
    assert!(c.get_bytes("thumb:scene:1:1").is_none());
    c.put_bytes("thumb:scene:1:1", &[9u8; 100])
        .expect("put again");
    assert!(c.check_identity("bbbbbbbbbbbbbbbb").expect("different"));
    assert!(c.get_bytes("thumb:scene:1:1").is_none());
}
