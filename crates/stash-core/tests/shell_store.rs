//! The shared versioned JSON store behind tabs and notifications (004 research R3).

use serde::{Deserialize, Serialize};
use stash_core::shell::store::{JsonStore, Versioned};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Sample {
    version: u32,
    #[serde(default)]
    items: Vec<String>,
}

impl Default for Sample {
    fn default() -> Self {
        Self {
            version: Self::VERSION,
            items: Vec::new(),
        }
    }
}

impl Versioned for Sample {
    const VERSION: u32 = 1;
}

fn sample(items: &[&str]) -> Sample {
    Sample {
        items: items.iter().map(|s| (*s).to_owned()).collect(),
        ..Sample::default()
    }
}

#[test]
fn missing_file_loads_as_empty() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = JsonStore::<Sample>::new(dir.path().join("shell/sample.json"));
    assert_eq!(store.load(), Sample::default());
}

#[test]
fn save_then_load_round_trips() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("shell/sample.json");
    let store = JsonStore::<Sample>::new(&path);
    store.save(&sample(&["a", "b"])).expect("save");
    assert_eq!(store.load(), sample(&["a", "b"]));
    // A second store on the same path sees the same data.
    assert_eq!(JsonStore::<Sample>::new(&path).load(), sample(&["a", "b"]));
}

#[test]
fn saves_are_atomic_and_leave_no_temp_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("sample.json");
    let store = JsonStore::<Sample>::new(&path);
    store.save(&sample(&["first"])).expect("save");
    // A crash between writing the temp file and renaming it leaves the old file intact.
    std::fs::write(
        dir.path().join(".sample.json.crashed.tmp"),
        b"{\"version\":1,\"it",
    )
    .expect("write");
    assert_eq!(store.load(), sample(&["first"]));
    store.save(&sample(&["second"])).expect("save");
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .expect("read_dir")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp") && !n.contains("crashed"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "temp files left behind: {leftovers:?}"
    );
    assert_eq!(store.load(), sample(&["second"]));
}

#[test]
fn damaged_file_is_moved_aside_and_loads_as_empty() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("sample.json");
    std::fs::write(&path, b"not json at all").expect("write");
    let store = JsonStore::<Sample>::new(&path);
    assert_eq!(store.load(), Sample::default());
    assert!(!path.exists());
    let backup = dir.path().join("sample.json.bak");
    assert_eq!(std::fs::read(&backup).expect("backup"), b"not json at all");
}

#[test]
fn newer_version_is_moved_aside_and_loads_as_empty() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("sample.json");
    std::fs::write(&path, br#"{"version":99,"items":["future"]}"#).expect("write");
    let store = JsonStore::<Sample>::new(&path);
    assert_eq!(store.load(), Sample::default());
    assert!(dir.path().join("sample.json.bak").exists());
}
