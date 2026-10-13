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
    // Loaded data isn't saved, so compare the saved forms.
    assert_eq!(restored.capture(), shell.capture());
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
fn a_version_1_file_from_007_starts_fresh() {
    assert_eq!(VERSION, 2, "008 saves Scenes tabs in version 2");
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("session.json");
    let profile = Uuid::new_v4();
    let v1 = format!(r#"{{"version":1,"sessions":{{"{profile}":{{"selected":0,"tabs":[]}}}}}}"#);
    std::fs::write(&path, v1).expect("write");
    assert!(
        snapshot::load(&path, profile).is_none(),
        "no migration from 007"
    );
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

#[test]
fn a_filtered_scenes_tab_round_trips_without_a_request() {
    use semantic_stash_viewer_native::screens::scenes::{Applied, ScenesState};
    use stash_core::scenes::filter::kinds::CriterionKind;
    use stash_core::scenes::filter::{Criterion, CriterionValue, Item, Modifier, SemanticFilter};
    use stash_core::scenes::query::{SceneSort, SortDirection};

    let mut state = ScenesState::at(4, 120);
    state.query.search = "beach".into();
    state.query.sort = SceneSort::Title;
    state.query.direction = SortDirection::Asc;
    state.query.criteria = vec![
        Criterion::new(
            CriterionKind::Tags,
            Modifier::IncludesAll,
            CriterionValue::Items {
                items: vec![Item {
                    id: "7".into(),
                    label: "A tag".into(),
                }],
                excluded: vec![Item {
                    id: "9".into(),
                    label: "Another".into(),
                }],
                depth: Some(-1),
            },
        ),
        Criterion::new(
            CriterionKind::Semantic(SemanticFilter::Role),
            Modifier::Equals,
            CriterionValue::None,
        ),
    ];
    state.applied = Some(Applied {
        id: "2".into(),
        name: "Favourites".into(),
    });
    state.scroll = 640.0;

    let (mut shell, _) = Shell::new();
    let _ = shell.update(ShellMsg::Open(Screen::Scenes(Box::new(state.clone()))));
    let saved = snapshot::capture(&shell);

    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("session.json");
    let profile = Uuid::new_v4();
    snapshot::save(&path, profile, &saved).expect("save");
    let loaded = snapshot::load(&path, profile).expect("load");
    assert_eq!(loaded, saved);

    let (restored, _) = Shell::restore(loaded).expect("restore");
    let Screen::Scenes(back) = restored.active().current() else {
        panic!("Scenes restored")
    };
    assert_eq!(
        back.query, state.query,
        "search, criteria (labels kept), sort"
    );
    assert_eq!(back.applied, state.applied);
    assert_eq!((back.page, back.page_size), (4, 120));
    assert_eq!(back.scroll, 640.0);
}

/// 010 T007, 011 T010: a session from 008 (no queue fields) loads with Autoplay on, Repeat Off,
/// Shuffle off, and no selection; a 010 session's Play next becomes Autoplay; a Queue tab
/// round-trips its mode, selection, and playlist controls.
#[test]
fn queue_fields_are_additive() {
    use semantic_stash_viewer_native::screens::ScenesState;
    use stash_core::playback::playlist::Repeat;
    let mut value = serde_json::to_value(ScenesState::default()).expect("serialise");
    let map = value.as_object_mut().expect("an object");
    for field in ["queue_selected", "autoplay", "repeat", "shuffle"] {
        assert!(map.remove(field).is_some(), "{field} is saved");
    }
    let old: ScenesState = serde_json::from_value(value.clone()).expect("a 008 tab loads");
    assert!(old.autoplay, "Autoplay defaults to on");
    assert_eq!(old.repeat, Repeat::Off);
    assert!(!old.shuffle);
    assert_eq!(old.queue_selected, None);

    // A 010 tab with Play next off keeps it as Autoplay off.
    let map = value.as_object_mut().expect("an object");
    map.insert("play_next".into(), serde_json::Value::Bool(false));
    let from_010: ScenesState = serde_json::from_value(value).expect("a 010 tab loads");
    assert!(!from_010.autoplay, "Play next off becomes Autoplay off");

    let mut queue = ScenesState::default();
    queue.mode = Mode::Queue;
    queue.queue_selected = Some("42".into());
    queue.autoplay = false;
    queue.repeat = Repeat::One;
    queue.shuffle = true;
    let json = serde_json::to_string(&queue.persistent()).expect("serialise");
    assert!(!json.contains("play_next") && !json.contains("loop_queue"));
    let back: ScenesState = serde_json::from_str(&json).expect("parse");
    assert_eq!(
        (back.mode, back.queue_selected.as_deref(), back.autoplay),
        (Mode::Queue, Some("42"), false)
    );
    assert_eq!((back.repeat, back.shuffle), (Repeat::One, true));
    assert_eq!(VERSION, 2, "no version bump");
}

/// 012 T026: an Images tab's query, page, page size, card size, mode, scroll, focus, and applied
/// filter round-trip in version 2 and restore without a request until it's shown.
#[test]
fn an_images_tab_round_trips() {
    use semantic_stash_viewer_native::screens::browse::state::Applied;
    use semantic_stash_viewer_native::screens::ImagesState;
    use stash_core::browse::query::SortDirection;
    use stash_core::images::ImageSort;
    use stash_core::scenes::filter::kinds::CriterionKind;
    use stash_core::scenes::filter::{Criterion, CriterionValue, Item, Modifier};

    let mut state = ImagesState::default();
    state.query.search = "shore".into();
    state.query.sort = ImageSort::Date;
    state.query.direction = SortDirection::Desc;
    state.query.criteria = vec![Criterion::new(
        CriterionKind::Folder,
        Modifier::Includes,
        CriterionValue::Items {
            items: vec![Item {
                id: "6".into(),
                label: "ssv-prod".into(),
            }],
            excluded: vec![],
            depth: Some(-1),
        },
    )];
    state.page = 3;
    state.page_size = 60;
    state.card_size = 3;
    state.mode = Mode::Wall;
    state.scroll = 512.0;
    state.focused = Some(7);
    state.applied = Some(Applied {
        id: "4".into(),
        name: "SSV images performer A".into(),
    });

    let (mut shell, _) = Shell::new();
    let _ = shell.update(ShellMsg::Open(Screen::Images(Box::new(state.clone()))));
    let saved = snapshot::capture(&shell);
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("session.json");
    let profile = Uuid::new_v4();
    snapshot::save(&path, profile, &saved).expect("save");
    let loaded = snapshot::load(&path, profile).expect("load");
    assert_eq!(loaded, saved);

    let (restored, _) = Shell::restore(loaded).expect("restore");
    let Screen::Images(back) = restored.active().current() else {
        panic!("Images restored")
    };
    assert_eq!(back.query, state.query);
    assert_eq!(
        (back.page, back.page_size, back.card_size, back.mode),
        (3, 60, 3, Mode::Wall)
    );
    assert_eq!((back.scroll, back.focused), (512.0, Some(7)));
    assert_eq!(back.applied, state.applied);
    assert_eq!(restored.capture(), shell.capture());
    assert_eq!(VERSION, 2, "Images tabs are additive");
}

/// 012 T026: a version 2 session saved before Images (no Images screens, no card size) loads as
/// it was, and an Images tab without a card size gets the default.
#[test]
fn sessions_from_before_images_load_unchanged() {
    use semantic_stash_viewer_native::screens::ImagesState;
    let shell = busy_shell();
    let saved = snapshot::capture(&shell);
    let json = serde_json::to_string(&saved).expect("serialise");
    assert!(!json.contains("Images") && !json.contains("card_size"));
    let back: SavedSession = serde_json::from_str(&json).expect("parse");
    assert_eq!(back, saved);

    let mut value = serde_json::to_value(ImagesState::default()).expect("serialise");
    assert!(value
        .as_object_mut()
        .expect("an object")
        .remove("card_size")
        .is_some());
    let old: ImagesState = serde_json::from_value(value).expect("loads");
    assert_eq!(old.card_size, ImagesState::default().card_size);
}
