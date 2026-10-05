//! 007 T047: the Scene screen's transitions (spec US4; capability C15).

use semantic_stash_viewer_native::effects::Effect;
use semantic_stash_viewer_native::screens::scene::{Details, SceneState};
use semantic_stash_viewer_native::screens::Screen;
use semantic_stash_viewer_native::shell::{Shell, ShellMsg, Up};
use stash_core::scenes::SceneDetails;
use stash_core::AppError;

const TAB: u64 = 1;

fn details(id: &str) -> SceneDetails {
    SceneDetails {
        id: id.into(),
        title: "Seven".into(),
        code: None,
        date: Some("2026-01-02".into()),
        details: Some("A description.".into()),
        director: None,
        studio: Some("A studio".into()),
        performers: vec!["Someone".into()],
        tags: vec!["A tag".into()],
        rating100: Some(80),
        play_count: 3,
        o_count: 0,
        duration_seconds: Some(125.0),
        file_name: Some("seven.mp4".into()),
        file: None,
        cover_version: "1".into(),
    }
}

#[test]
fn entering_loads_the_details_and_cover_once() {
    let mut s = SceneState::new("7", "Seven");
    let effects = s.enter(TAB);
    assert!(effects.contains(&Effect::LoadSceneDetails {
        tab: TAB,
        id: "7".into()
    }));
    assert!(effects.contains(&Effect::LoadCover {
        tab: TAB,
        id: "7".into()
    }));
    assert!(s.enter(TAB).is_empty(), "already on their way");
    s.details_loaded(Ok(details("7")));
    assert!(matches!(s.details, Details::Ready(_)));
    assert!(s.enter(TAB).is_empty(), "here already");
}

#[test]
fn a_restored_tab_shows_its_title_before_the_details_arrive() {
    let json = r#"{"Scene":{"scene_id":"7","title":"Seven"}}"#;
    let screen: Screen = serde_json::from_str(json).expect("parse");
    assert_eq!(screen.title(), "Seven");
    let Screen::Scene(s) = screen else { panic!() };
    assert!(matches!(s.details, Details::Loading));
}

#[test]
fn the_title_follows_the_details() {
    let mut s = SceneState::new("7", "an old title");
    let _ = s.enter(TAB);
    s.details_loaded(Ok(details("7")));
    assert_eq!(s.title, "Seven");
}

#[test]
fn a_details_failure_shows_its_message() {
    let mut s = SceneState::new("7", "Seven");
    let _ = s.enter(TAB);
    s.details_loaded(Err(AppError::SceneNotFound { id: "7".into() }));
    let Details::Failed(message) = &s.details else {
        panic!("expected a failure")
    };
    assert!(!message.is_empty());
    // Entering again retries.
    assert!(s
        .enter(TAB)
        .iter()
        .any(|e| matches!(e, Effect::LoadSceneDetails { .. })));
}

#[test]
fn play_bubbles_up_with_the_scene() {
    let (mut shell, _) = Shell::new();
    let _ = shell.update(ShellMsg::Open(Screen::scene("7", "Seven")));
    let step = shell.update(ShellMsg::Play("7".into()));
    assert_eq!(step.up, Some(Up::Play("7".into())));
}

#[test]
fn results_land_on_the_screen_that_asked() {
    let (mut shell, _) = Shell::new();
    let tab = shell.active().id;
    let _ = shell.update(ShellMsg::Open(Screen::scene("7", "Seven")));
    shell.scene_details(tab, "7", Ok(details("7")));
    let Screen::Scene(s) = shell.active().current() else {
        panic!()
    };
    assert!(matches!(s.details, Details::Ready(_)));
    // Another scene's result changes nothing.
    shell.scene_details(tab, "8", Err(AppError::SceneNotFound { id: "8".into() }));
    let Screen::Scene(s) = shell.active().current() else {
        panic!()
    };
    assert!(matches!(s.details, Details::Ready(_)));
}
