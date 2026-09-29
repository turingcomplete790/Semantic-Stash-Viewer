//! Per-profile tab sets (004 US2, data-model "TabSet").

use serde_json::json;
use stash_core::shell::tabs::{HistoryEntry, Tab, TabSet, TabsStore};
use stash_core::shell::{MAX_HISTORY, MAX_TABS, MAX_VIEW_STATE_BYTES};
use stash_core::AppError;
use uuid::Uuid;

fn entry(kind: &str) -> HistoryEntry {
    HistoryEntry {
        route: json!({ "kind": kind }),
        view_state: None,
    }
}

fn tab(id: &str, kinds: &[&str]) -> Tab {
    Tab {
        id: id.into(),
        history: kinds.iter().map(|k| entry(k)).collect(),
        index: u32::try_from(kinds.len() - 1).expect("small"),
    }
}

fn set(tabs: Vec<Tab>, selected: &str) -> TabSet {
    TabSet {
        tabs,
        selected_tab_id: selected.into(),
    }
}

fn store() -> (tempfile::TempDir, TabsStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = TabsStore::new(dir.path().join("shell/tabs.json"));
    (dir, store)
}

fn invalid(result: Result<(), AppError>) -> bool {
    matches!(result, Err(AppError::TabSetInvalid { .. }))
}

#[test]
fn saves_and_loads_per_profile() {
    let (_dir, store) = store();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let set_a = set(
        vec![tab("1", &["home", "scenes"]), tab("2", &["settings"])],
        "2",
    );
    let set_b = set(vec![tab("x", &["home"])], "x");
    store.save(a, &set_a).expect("save a");
    store.save(b, &set_b).expect("save b");
    assert_eq!(store.load(a), Some(set_a));
    assert_eq!(store.load(b), Some(set_b));
    assert_eq!(store.load(Uuid::new_v4()), None);
}

#[test]
fn keeps_view_state_as_opaque_json() {
    let (_dir, store) = store();
    let p = Uuid::new_v4();
    let mut t = tab("1", &["scenes"]);
    t.history[0].view_state = Some(json!({ "scrollTop": 420, "filters": { "q": "beach" } }));
    let s = set(vec![t], "1");
    store.save(p, &s).expect("save");
    assert_eq!(store.load(p), Some(s));
}

#[test]
fn rejects_invalid_tab_sets() {
    let (_dir, store) = store();
    let p = Uuid::new_v4();
    // No tabs.
    assert!(invalid(store.save(p, &set(vec![], "1"))));
    // Selected tab missing.
    assert!(invalid(
        store.save(p, &set(vec![tab("1", &["home"])], "nope"))
    ));
    // Too many tabs.
    let many: Vec<Tab> = (0..=MAX_TABS)
        .map(|i| tab(&i.to_string(), &["home"]))
        .collect();
    assert!(invalid(store.save(p, &set(many, "0"))));
    // Too much history.
    let long: Vec<&str> = std::iter::repeat_n("home", MAX_HISTORY + 1).collect();
    assert!(invalid(store.save(p, &set(vec![tab("1", &long)], "1"))));
    // Empty history.
    let mut empty = tab("1", &["home"]);
    empty.history.clear();
    empty.index = 0;
    assert!(invalid(store.save(p, &set(vec![empty], "1"))));
    // Index out of range.
    let mut out = tab("1", &["home"]);
    out.index = 1;
    assert!(invalid(store.save(p, &set(vec![out], "1"))));
    // View state too large.
    let mut big = tab("1", &["home"]);
    big.history[0].view_state = Some(json!({ "blob": "x".repeat(MAX_VIEW_STATE_BYTES) }));
    assert!(invalid(store.save(p, &set(vec![big], "1"))));
    // Nothing was written by the rejected saves.
    assert_eq!(store.load(p), None);
}

#[test]
fn delete_removes_only_that_profile() {
    let (_dir, store) = store();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    store
        .save(a, &set(vec![tab("1", &["home"])], "1"))
        .expect("a");
    store
        .save(b, &set(vec![tab("2", &["home"])], "2"))
        .expect("b");
    store.delete(a).expect("delete");
    assert_eq!(store.load(a), None);
    assert!(store.load(b).is_some());
}

#[test]
fn prune_drops_profiles_that_no_longer_exist() {
    let (_dir, store) = store();
    let keep = Uuid::new_v4();
    let gone = Uuid::new_v4();
    store
        .save(keep, &set(vec![tab("1", &["home"])], "1"))
        .expect("keep");
    store
        .save(gone, &set(vec![tab("2", &["home"])], "2"))
        .expect("gone");
    store.prune(&[keep]).expect("prune");
    assert!(store.load(keep).is_some());
    assert_eq!(store.load(gone), None);
}

#[test]
fn damaged_file_is_moved_aside() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("tabs.json");
    std::fs::write(&path, b"{ broken").expect("write");
    let store = TabsStore::new(&path);
    assert_eq!(store.load(Uuid::new_v4()), None);
    assert!(dir.path().join("tabs.json.bak").exists());
}
