//! "Show cached, then refresh" (003 research R4–R5).

use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use stash_core::cache::refresh::{RefreshPolicy, Refresher};
use stash_core::cache::{Limit, ViewCache};
use stash_core::shell::notifications::{NotificationCenter, Severity};
use stash_core::AppError;
use uuid::Uuid;

struct Rig {
    _dir: tempfile::TempDir,
    refresher: Arc<Refresher>,
    center: Arc<NotificationCenter>,
    now: Arc<AtomicI64>,
    changes: Arc<Mutex<Vec<String>>>,
    profile: Uuid,
}

fn rig() -> Rig {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache =
        ViewCache::open_with(dir.path().join("c.sqlite3"), Limit::Fixed(10 << 20)).expect("cache");
    let center = Arc::new(NotificationCenter::open(dir.path().join("n.json")));
    let now = Arc::new(AtomicI64::new(1_000_000));
    let clock_now = Arc::clone(&now);
    let changes: Arc<Mutex<Vec<String>>> = Arc::default();
    let seen = Arc::clone(&changes);
    let profile = Uuid::new_v4();
    let refresher = Refresher::with_clock(
        profile,
        Arc::new(Mutex::new(cache)),
        Arc::clone(&center),
        Arc::new(move |key: &str| seen.lock().expect("lock").push(key.to_owned())),
        Arc::new(move || clock_now.load(Ordering::SeqCst)),
    );
    Rig {
        _dir: dir,
        refresher,
        center,
        now,
        changes,
        profile,
    }
}

type Fetched =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<String>, AppError>> + Send>>;

/// A fetch that returns `values[call]` (repeating the last) and counts calls.
fn fetcher(
    values: Vec<Result<Vec<String>, AppError>>,
) -> (
    Arc<AtomicU32>,
    impl Fn() -> Fetched + Send + Sync + Clone + 'static,
) {
    let calls = Arc::new(AtomicU32::new(0));
    let counter = Arc::clone(&calls);
    let values = Arc::new(values);
    let f = move || {
        let n = counter.fetch_add(1, Ordering::SeqCst) as usize;
        let v = values[n.min(values.len() - 1)].clone();
        Box::pin(async move { v }) as Fetched
    };
    (calls, f)
}

async fn settle() {
    tokio::time::sleep(Duration::from_millis(100)).await;
}

fn list(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

fn failure() -> AppError {
    AppError::Internal {
        message: "the server did not respond in time".into(),
    }
}

#[tokio::test]
async fn a_miss_fetches_and_stores() {
    let r = rig();
    let (calls, fetch) = fetcher(vec![Ok(list(&["a"]))]);
    let got = r
        .refresher
        .get_or_fetch("scenes:recent", RefreshPolicy::Auto, fetch)
        .await
        .expect("fetch");
    assert_eq!(got.data, list(&["a"]));
    assert!(!got.from_cache);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_fresh_hit_does_not_refetch_but_a_stale_one_refreshes_in_the_background() {
    let r = rig();
    let (calls, fetch) = fetcher(vec![Ok(list(&["a"])), Ok(list(&["a", "b"]))]);
    r.refresher
        .get_or_fetch("scenes:recent", RefreshPolicy::Auto, fetch.clone())
        .await
        .expect("miss");

    r.now.fetch_add(3_000, Ordering::SeqCst); // 3 s later: fresh
    let hit = r
        .refresher
        .get_or_fetch("scenes:recent", RefreshPolicy::Auto, fetch.clone())
        .await
        .expect("hit");
    assert!(hit.from_cache);
    settle().await;
    assert_eq!(calls.load(Ordering::SeqCst), 1, "no refresh under 5 s");

    r.now.fetch_add(3_000, Ordering::SeqCst); // 6 s after the fetch: stale
    let stale = r
        .refresher
        .get_or_fetch("scenes:recent", RefreshPolicy::Auto, fetch.clone())
        .await
        .expect("stale hit");
    assert_eq!(stale.data, list(&["a"]), "cached copy returned at once");
    settle().await;
    assert_eq!(calls.load(Ordering::SeqCst), 2, "background refresh ran");
    assert_eq!(
        *r.changes.lock().expect("lock"),
        ["scenes:recent"],
        "data differed"
    );

    let fresh = r
        .refresher
        .get_or_fetch("scenes:recent", RefreshPolicy::Auto, fetch)
        .await
        .expect("fresh");
    assert_eq!(fresh.data, list(&["a", "b"]));
}

#[tokio::test]
async fn an_unchanged_refresh_raises_no_change() {
    let r = rig();
    let (calls, fetch) = fetcher(vec![Ok(list(&["a"]))]);
    r.refresher
        .get_or_fetch("scenes:recent", RefreshPolicy::Auto, fetch.clone())
        .await
        .expect("miss");
    r.now.fetch_add(10_000, Ordering::SeqCst);
    r.refresher
        .get_or_fetch("scenes:recent", RefreshPolicy::Auto, fetch)
        .await
        .expect("hit");
    settle().await;
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert!(r.changes.lock().expect("lock").is_empty());
}

#[tokio::test]
async fn offline_serves_the_cache_and_does_not_fetch() {
    let r = rig();
    let (calls, fetch) = fetcher(vec![Ok(list(&["a"]))]);
    r.refresher
        .get_or_fetch("scenes:recent", RefreshPolicy::Auto, fetch.clone())
        .await
        .expect("miss");
    r.refresher.set_online(false);
    r.now.fetch_add(60_000, Ordering::SeqCst);
    let hit = r
        .refresher
        .get_or_fetch("scenes:recent", RefreshPolicy::Auto, fetch.clone())
        .await
        .expect("cached");
    assert!(hit.from_cache);
    settle().await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let miss = r
        .refresher
        .get_or_fetch("scene:9", RefreshPolicy::Auto, fetch)
        .await;
    assert!(matches!(miss, Err(AppError::NotConnected)));
}

#[tokio::test]
async fn manual_keys_are_never_refreshed_in_the_background() {
    let r = rig();
    let (calls, fetch) = fetcher(vec![Ok(list(&["a"])), Ok(list(&["b"]))]);
    r.refresher
        .get_or_fetch("scenes:test-set", RefreshPolicy::Manual, fetch.clone())
        .await
        .expect("miss");
    r.now.fetch_add(60_000, Ordering::SeqCst);
    r.refresher
        .get_or_fetch("scenes:test-set", RefreshPolicy::Manual, fetch)
        .await
        .expect("hit");
    settle().await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn repeated_failures_become_one_notification_then_recover() {
    let r = rig();
    let (_calls, first) = fetcher(vec![Ok(list(&["a"]))]);
    r.refresher
        .get_or_fetch("scenes:recent", RefreshPolicy::Auto, first)
        .await
        .expect("miss");
    let key = format!("background:{}:scenes:recent", r.profile);
    let (_c, failing) = fetcher(vec![Err(failure())]);
    for attempt in 1..=3 {
        r.now.fetch_add(10_000, Ordering::SeqCst);
        r.refresher
            .get_or_fetch("scenes:recent", RefreshPolicy::Auto, failing.clone())
            .await
            .expect("cached copy still served");
        settle().await;
        let n = r.center.find(&key);
        if attempt < 3 {
            assert!(n.is_none(), "no notification after {attempt} failures");
        } else {
            let n = n.expect("posted after 3 failures");
            assert!(!n.toast, "failing for 20 s: no toast yet");
            assert!(n.title.starts_with("Couldn't refresh"));
        }
    }
    // Still failing 70 s after the first failure: now it toasts.
    r.now.fetch_add(50_000, Ordering::SeqCst);
    r.refresher
        .get_or_fetch("scenes:recent", RefreshPolicy::Auto, failing)
        .await
        .expect("cached");
    settle().await;
    assert!(r.center.find(&key).expect("still there").toast);
    assert_eq!(r.center.list().len(), 1, "one entry, updated in place");

    let (_c2, working) = fetcher(vec![Ok(list(&["a"]))]);
    r.now.fetch_add(10_000, Ordering::SeqCst);
    r.refresher
        .get_or_fetch("scenes:recent", RefreshPolicy::Auto, working)
        .await
        .expect("cached");
    settle().await;
    let n = r.center.find(&key).expect("updated");
    assert_eq!(n.title, "Refreshed");
    assert_eq!(n.severity, Severity::Info);
}
