//! 006 T009: the native build finds the web build's files, keeps its own shell state, and never
//! rewrites the saved profiles.

use std::path::Path;

use semantic_stash_viewer_native::services::{Paths, Services};

#[test]
fn paths_match_the_web_builds_directories() {
    let p = Paths::under(Path::new("/c"), Path::new("/d"), Path::new("/k"));
    // src-tauri/src/lib.rs: config_dir()/APP_DIR/profiles.json, cache_dir()/APP_DIR.
    assert_eq!(
        p.profiles(),
        Path::new("/c/semantic-stash-viewer/profiles.json")
    );
    assert_eq!(p.cache_root(), Path::new("/k/semantic-stash-viewer"));
    assert_eq!(p.logs(), Path::new("/d/semantic-stash-viewer/logs"));
}

#[test]
fn shell_state_is_kept_apart_from_the_web_builds() {
    let p = Paths::under(Path::new("/c"), Path::new("/d"), Path::new("/k"));
    let web_tabs = Path::new("/d/semantic-stash-viewer/shell/tabs.json");
    let web_notifications = Path::new("/d/semantic-stash-viewer/shell/notifications.json");
    assert_ne!(p.tabs(), web_tabs);
    assert_ne!(p.notifications(), web_notifications);
    assert_eq!(p.tabs().parent(), web_tabs.parent());
}

#[test]
fn opening_never_rewrites_the_saved_profiles() {
    let dir = tempfile::tempdir().expect("temp dir");
    let p = Paths::under(
        &dir.path().join("c"),
        &dir.path().join("d"),
        &dir.path().join("k"),
    );
    std::fs::create_dir_all(&p.config).expect("config dir");
    let profiles = r#"{"version":1,"lastUsedProfileId":null,"profiles":[]}"#;
    std::fs::write(p.profiles(), profiles).expect("write profiles");
    let before = std::fs::read(p.profiles()).expect("read");

    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let services = Services::open(p.clone(), runtime.handle().clone()).expect("open");
    assert!(services.profiles().is_empty());
    assert!(services.last_used().is_none());
    drop(services);

    assert_eq!(std::fs::read(p.profiles()).expect("read"), before);
}
