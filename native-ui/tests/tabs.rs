//! 007 T023: tabs and their history (spec US2; behaviour that stays B1; capability C11).

use semantic_stash_viewer_native::effects::Effect;
use semantic_stash_viewer_native::screens::{Screen, Section, SettingsPage};
use semantic_stash_viewer_native::shell::{Shell, ShellMsg, MAX_HISTORY, MAX_TABS};

fn shell() -> Shell {
    Shell::new().0
}

fn titles(s: &Shell) -> Vec<String> {
    s.tabs.iter().map(|t| t.current().title()).collect()
}

#[test]
fn a_new_shell_has_one_home_tab_and_loads_its_summary() {
    let (s, effects) = Shell::new();
    assert_eq!(titles(&s), vec!["Home"]);
    let tab = s.tabs[0].id;
    assert_eq!(effects, vec![Effect::LoadSummary { tab }]);
}

#[test]
fn opening_a_section_pushes_a_screen_and_back_returns_the_exact_state() {
    let mut s = shell();
    let _ = s.update(ShellMsg::Section(Section::Scenes));
    assert_eq!(titles(&s), vec!["Scenes"]);
    if let Screen::Scenes(state) = s.active_mut().current_mut() {
        state.page = 7;
        state.scroll = 1234.0;
    }
    let _ = s.update(ShellMsg::Section(Section::Settings));
    assert_eq!(titles(&s), vec!["Settings"]);
    let _ = s.update(ShellMsg::Back);
    let Screen::Scenes(state) = s.active().current() else {
        panic!("expected Scenes after back");
    };
    assert_eq!(
        (state.page, state.scroll),
        (7, 1234.0),
        "back returns the exact state"
    );
    let _ = s.update(ShellMsg::Forward);
    assert_eq!(titles(&s), vec!["Settings"]);
}

#[test]
fn opening_after_going_back_drops_the_forward_entries() {
    let mut s = shell();
    let _ = s.update(ShellMsg::Section(Section::Scenes));
    let _ = s.update(ShellMsg::Section(Section::Settings));
    let _ = s.update(ShellMsg::Back);
    let _ = s.update(ShellMsg::Back);
    let _ = s.update(ShellMsg::Open(Screen::settings(SettingsPage::About)));
    assert_eq!(s.active().history.len(), 2);
    assert!(s.active().cursor == 1);
    let _ = s.update(ShellMsg::Forward);
    assert_eq!(s.active().cursor, 1, "nothing to go forward to");
}

#[test]
fn returning_to_the_place_just_left_reuses_its_entry() {
    let mut s = shell();
    let _ = s.update(ShellMsg::Section(Section::Scenes));
    if let Screen::Scenes(state) = s.active_mut().current_mut() {
        state.page = 3;
    }
    let _ = s.update(ShellMsg::Open(Screen::scene("42", "A scene")));
    // Opening Scenes again from the scene goes back to the entry left, state included (005 R13).
    let _ = s.update(ShellMsg::Section(Section::Scenes));
    assert_eq!(
        s.active().history.len(),
        3,
        "kept, not pushed: forward still exists"
    );
    let Screen::Scenes(state) = s.active().current() else {
        panic!()
    };
    assert_eq!(state.page, 3);
}

#[test]
fn history_keeps_at_most_the_limit_dropping_the_oldest() {
    let mut s = shell();
    for i in 0..(MAX_HISTORY + 10) {
        let _ = s.update(ShellMsg::Open(Screen::scene(&i.to_string(), "s")));
    }
    assert_eq!(s.active().history.len(), MAX_HISTORY);
    assert_eq!(s.active().cursor, MAX_HISTORY - 1);
    assert_eq!(s.active().current().title(), "s");
}

#[test]
fn tabs_are_limited_and_the_last_one_cant_be_closed() {
    let mut s = shell();
    for _ in 0..(MAX_TABS + 5) {
        let _ = s.update(ShellMsg::NewTab);
    }
    assert_eq!(s.tabs.len(), MAX_TABS);
    let mut s = shell();
    let only = s.tabs[0].id;
    let _ = s.update(ShellMsg::Close(only));
    assert_eq!(s.tabs.len(), 1);
}

#[test]
fn closing_a_tab_selects_its_neighbour() {
    let mut s = shell();
    let _ = s.update(ShellMsg::NewTab);
    let _ = s.update(ShellMsg::NewTab);
    assert_eq!(s.selected, 2);
    let middle = s.tabs[1].id;
    let _ = s.update(ShellMsg::Select(middle));
    let _ = s.update(ShellMsg::Close(middle));
    assert_eq!(s.tabs.len(), 2);
    assert_eq!(s.selected, 1, "the tab after it takes its place");
}

#[test]
fn switching_tabs_keeps_state_and_ready_screens_dont_reload() {
    let mut s = shell();
    let first = s.tabs[0].id;
    // Make Home ready.
    let ready = semantic_stash_viewer_native::screens::home::Summary::Unreachable;
    if let Screen::Home(h) = s.active_mut().current_mut() {
        h.summary = ready;
    }
    let step = s.update(ShellMsg::NewTab);
    assert!(matches!(
        step.effects.as_slice(),
        [Effect::LoadSummary { .. }]
    ));
    let step = s.update(ShellMsg::Select(first));
    assert!(
        step.effects.is_empty(),
        "entering a screen that has its data emits no load: {:?}",
        step.effects
    );
}

#[test]
fn keyboard_tab_navigation_wraps() {
    let mut s = shell();
    let _ = s.update(ShellMsg::NewTab);
    let _ = s.update(ShellMsg::NewTab);
    let _ = s.update(ShellMsg::NextTab);
    assert_eq!(s.selected, 0, "next from the last wraps to the first");
    let _ = s.update(ShellMsg::PrevTab);
    assert_eq!(s.selected, 2);
    let _ = s.update(ShellMsg::SelectIndex(1));
    assert_eq!(s.selected, 1);
    let _ = s.update(ShellMsg::SelectIndex(9));
    assert_eq!(s.selected, 1, "no tab 10: nothing happens");
}

#[test]
fn tabs_move_left_and_right() {
    let mut s = shell();
    let _ = s.update(ShellMsg::NewTab);
    let moved = s.tabs[1].id;
    let _ = s.update(ShellMsg::MoveLeft);
    assert_eq!(s.tabs[0].id, moved);
    assert_eq!(s.selected, 0);
}

#[test]
fn open_in_new_tab_puts_the_tab_next_to_the_current_one() {
    let mut s = shell();
    let _ = s.update(ShellMsg::NewTab);
    let _ = s.update(ShellMsg::SelectIndex(0));
    let _ = s.update(ShellMsg::OpenInNewTab(Screen::scene("7", "Seven")));
    assert_eq!(titles(&s), vec!["Home", "Seven", "Home"]);
    assert_eq!(s.selected, 1);
}

#[test]
fn play_bubbles_up_from_a_screen() {
    let mut s = shell();
    let step = s.update(ShellMsg::Play("7".into()));
    assert_eq!(
        step.up,
        Some(semantic_stash_viewer_native::shell::Up::Play("7".into()))
    );
}
