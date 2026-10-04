//! 007 T004: the native app keeps every file under its own app-id directories and never opens the
//! demo's `semantic-stash-viewer/` folders (spec FR-005, research R4).

use std::path::Path;

use semantic_stash_viewer_native::services::{Paths, Services, APP_ID};

#[test]
fn every_file_lives_under_the_app_id() {
    assert_eq!(APP_ID, "dev.semantic-stash-viewer");
    let p = Paths::under(Path::new("/c"), Path::new("/d"), Path::new("/k"));
    assert_eq!(
        p.profiles(),
        Path::new("/c/dev.semantic-stash-viewer/profiles.json")
    );
    assert_eq!(
        p.session(),
        Path::new("/d/dev.semantic-stash-viewer/session.json")
    );
    assert_eq!(
        p.notifications(),
        Path::new("/d/dev.semantic-stash-viewer/notifications.json")
    );
    assert_eq!(p.logs(), Path::new("/d/dev.semantic-stash-viewer/logs"));
    assert_eq!(p.cache_root(), Path::new("/k/dev.semantic-stash-viewer"));
}

#[test]
fn the_demos_files_are_never_opened_or_created() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (config, data, cache) = (
        dir.path().join("c"),
        dir.path().join("d"),
        dir.path().join("k"),
    );
    // The demo's folders, with a profile in them.
    let demo = config.join("semantic-stash-viewer");
    std::fs::create_dir_all(&demo).expect("demo dir");
    let demo_profiles = r#"{"version":1,"lastUsedProfileId":null,"profiles":[]}"#;
    std::fs::write(demo.join("profiles.json"), demo_profiles).expect("demo profiles");

    let p = Paths::under(&config, &data, &cache);
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let services = Services::open(p, runtime.handle().clone()).expect("open");
    // A fresh start: the demo's profile file isn't read.
    assert!(services.profiles().is_empty());
    drop(services);

    assert_eq!(
        std::fs::read_to_string(demo.join("profiles.json")).expect("read"),
        demo_profiles
    );
    for root in [&data, &cache] {
        assert!(!root.join("semantic-stash-viewer").exists());
    }
}
