//! 007 T024: the saved session (contracts/session-snapshot.md; behaviour that stays B2).

use semantic_stash_viewer_native::screens::{Mode, Screen, Section, SettingsPage};
use semantic_stash_viewer_native::session::snapshot::{self, SavedSession, VERSION};
use semantic_stash_viewer_native::shell::{Shell, ShellMsg};
use uuid::Uuid;

fn busy_shell() -> Shell {
    let (mut s, _) = Shell::new();
    let _ = s.update(ShellMsg::Section(Section::Scenes));
    if let Screen::Scenes(state) = s.active_mut().current_mut() {
        state.page = 12;
        state.page_size = 120;
        state.mode = Mode::List;
        state.scroll = 845.5;
    }
    let _ = s.update(ShellMsg::Open(Screen::scene("42", "A scene")));
    let _ = s.update(ShellMsg::NewTab);
    let _ = s.update(ShellMsg::Section(Section::Settings));
    let _ = s.update(ShellMsg::Open(Screen::settings(SettingsPage::About)));
    let first = s.tabs[0].id;
    let _ = s.update(ShellMsg::Select(first));
    s
}

#[test]
fn a_session_round_trips_exactly() {
    let shell = busy_shell();
    let saved = snapshot::capture(&shell);
    let json = serde_json::to_string(&saved).expect("serialise");
    let back: SavedSession = serde_json::from_str(&json).expect("parse");
    assert_eq!(back, saved);
    let (restored, _) = Shell::restore(back).expect("valid");
    assert_eq!(restored.selected, shell.selected);
    assert_eq!(restored.tabs.len(), shell.tabs.len());
    for (a, b) in restored.tabs.iter().zip(&shell.tabs) {
        assert_eq!(a.history, b.history);
        assert_eq!(a.cursor, b.cursor);
    }
}

#[test]
fn loaded_data_is_not_written() {
    let (mut s, _) = Shell::new();
    if let Screen::Home(h) = s.active_mut().current_mut() {
        h.summary = semantic_stash_viewer_native::screens::home::Summary::Unreachable;
    }
    let json = serde_json::to_string(&snapshot::capture(&s)).expect("serialise");
    assert!(!json.contains("Unreachable"), "{json}");
}

#[test]
fn files_round_trip_per_profile_and_writes_are_atomic() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("session.json");
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    let saved = snapshot::capture(&busy_shell());
    snapshot::save(&path, a, &saved).expect("save a");
    snapshot::save(&path, b, &snapshot::capture(&Shell::new().0)).expect("save b");
    assert_eq!(snapshot::load(&path, a), Some(saved));
    assert!(snapshot::load(&path, b).is_some());
    assert!(snapshot::load(&path, Uuid::new_v4()).is_none());
    // No temporary file is left behind.
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .expect("list")
        .map(|e| e.expect("entry").file_name().into_string().expect("utf-8"))
        .collect();
    assert_eq!(names, vec!["session.json"]);
}

#[test]
fn a_broken_or_old_file_is_set_aside_and_the_app_starts_fresh() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("session.json");
    std::fs::write(&path, "{ not json").expect("write");
    assert!(snapshot::load(&path, Uuid::new_v4()).is_none());
    assert!(dir.path().join("session.json.bad").exists());
    assert!(!path.exists());

    let old = format!(r#"{{"version":{},"sessions":{{}}}}"#, VERSION + 1);
    std::fs::write(&path, old).expect("write");
    assert!(snapshot::load(&path, Uuid::new_v4()).is_none());
    assert!(dir.path().join("session.json.bad").exists());
}

#[test]
fn limits_apply_when_saving() {
    let (mut s, _) = Shell::new();
    for _ in 0..60 {
        let _ = s.update(ShellMsg::NewTab);
    }
    let saved = snapshot::capture(&s);
    assert!(saved.tabs.len() <= 50);
    assert!(saved.selected < saved.tabs.len());
}

#[test]
fn an_invalid_saved_session_is_refused() {
    let saved = SavedSession {
        selected: 3,
        tabs: vec![],
    };
    assert!(Shell::restore(saved).is_none(), "no tabs");
}
