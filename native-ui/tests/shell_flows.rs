//! 007 T026: shell flows, headless (capabilities C10, C11; behaviour that stays B4).

use iced::widget::{text, Id};
use iced_test::selector::{Bounded, Selector};
use iced_test::simulator;
use semantic_stash_viewer_native::shell::{close_tab_id, Shell, ShellMsg, NEW_TAB_ID};

/// Click `target` in the shell's chrome and apply the messages it produced.
fn click<S>(shell: &mut Shell, target: S)
where
    S: Selector + Send,
    S::Output: Bounded + Clone + Send + Sync + 'static,
{
    let messages: Vec<ShellMsg> = {
        let mut ui = simulator(shell.chrome(text("").into()));
        ui.click(target).expect("the target is on screen");
        ui.into_messages().collect()
    };
    for msg in messages {
        let _ = shell.update(msg);
    }
}

#[test]
fn the_navigation_bar_opens_each_section() {
    let (mut shell, _) = Shell::new();
    for label in ["Scenes", "Settings", "Home"] {
        click(&mut shell, label);
        assert_eq!(shell.active().current().title(), label);
    }
}

#[test]
fn the_tab_strip_adds_selects_and_closes_tabs() {
    let (mut shell, _) = Shell::new();
    let first = shell.tabs[0].id;
    click(&mut shell, Id::new(NEW_TAB_ID));
    assert_eq!(shell.tabs.len(), 2);
    assert_eq!(shell.selected, 1);
    let second = shell.tabs[1].id;
    click(&mut shell, close_tab_id(second));
    assert_eq!(shell.tabs.len(), 1);
    assert_eq!(shell.active().id, first);
}

#[test]
fn the_last_tab_has_no_working_close_button() {
    let (mut shell, _) = Shell::new();
    let only = shell.tabs[0].id;
    click(&mut shell, close_tab_id(only));
    assert_eq!(shell.tabs.len(), 1);
}

#[test]
fn back_and_forward_buttons_follow_history() {
    let (mut shell, _) = Shell::new();
    click(&mut shell, "Scenes");
    let _ = shell.update(ShellMsg::Back);
    assert_eq!(shell.active().current().title(), "Home");
    assert!(shell.active().can_go_forward());
}

mod session_level {
    use iced::keyboard::key::Named;
    use iced::keyboard::{Key, Modifiers};
    use semantic_stash_viewer_native::effects::{Effect, Reply};
    use semantic_stash_viewer_native::screens::Section;
    use semantic_stash_viewer_native::session::{Msg, Session};
    use semantic_stash_viewer_native::shell::overlays::Overlay;
    use semantic_stash_viewer_native::shell::{Shell, ShellMsg};
    use stash_core::profiles::ServerProfile;

    fn profile() -> ServerProfile {
        ServerProfile {
            id: uuid::Uuid::new_v4(),
            display_name: "Testing".into(),
            base_url: "http://localhost:9998/".parse().expect("url"),
            strict_tls: false,
            api_key: None,
            created_at: chrono::Utc::now(),
            last_used_at: None,
        }
    }

    fn key(session: &mut Session, k: Key, mods: Modifiers) -> Vec<Effect> {
        session.update(Msg::Key(k, mods)).effects
    }

    fn ch(c: &str) -> Key {
        Key::Character(c.into())
    }

    /// A session whose saved-session read came back empty.
    fn ready() -> Session {
        let p = profile();
        let (mut session, effects) = Session::enter(p.clone(), true);
        assert!(effects.contains(&Effect::LoadSession { profile: p.id }));
        let _ = session.update(Msg::Reply(Reply::SessionLoaded {
            profile: p.id,
            saved: None,
        }));
        session
    }

    #[test]
    fn tab_shortcuts_work() {
        let mut s = ready();
        let _ = key(&mut s, ch("t"), Modifiers::CTRL);
        let _ = key(&mut s, ch("t"), Modifiers::CTRL);
        assert_eq!(s.shell.tabs.len(), 3);
        let _ = key(&mut s, Key::Named(Named::Tab), Modifiers::CTRL);
        assert_eq!(s.shell.selected, 0);
        let _ = key(&mut s, ch("2"), Modifiers::CTRL);
        assert_eq!(s.shell.selected, 1);
        let _ = key(&mut s, ch("w"), Modifiers::CTRL);
        assert_eq!(s.shell.tabs.len(), 2);
        let _ = key(&mut s, ch("t"), Modifiers::empty());
        assert_eq!(s.shell.tabs.len(), 2, "plain t does nothing");
    }

    #[test]
    fn keyboard_help_opens_and_escape_closes_it() {
        let mut s = ready();
        let _ = key(&mut s, Key::Named(Named::F1), Modifiers::empty());
        assert_eq!(s.overlay, Overlay::KeyboardHelp);
        let _ = key(&mut s, ch("t"), Modifiers::CTRL);
        assert_eq!(
            s.shell.tabs.len(),
            1,
            "shortcuts wait while an overlay is open"
        );
        let _ = key(&mut s, Key::Named(Named::Escape), Modifiers::empty());
        assert_eq!(s.overlay, Overlay::None);
    }

    #[test]
    fn changes_are_saved_once_after_the_last_one() {
        let mut s = ready();
        let first = s
            .update(Msg::Shell(ShellMsg::Section(Section::Scenes)))
            .effects;
        let second = s.update(Msg::Shell(ShellMsg::NewTab)).effects;
        let generation = |effects: &[Effect]| {
            effects.iter().find_map(|e| match e {
                Effect::SaveSessionLater { generation } => Some(*generation),
                _ => None,
            })
        };
        let (g1, g2) = (
            generation(&first).expect("one"),
            generation(&second).expect("two"),
        );
        assert!(s.update(Msg::SaveDue(g1)).effects.is_empty(), "superseded");
        let due = s.update(Msg::SaveDue(g2)).effects;
        assert!(
            matches!(due.as_slice(), [Effect::SaveSession { session, .. }] if session.tabs.len() == 2)
        );
    }

    #[test]
    fn nothing_is_saved_before_the_saved_session_is_read() {
        let (mut s, _) = Session::enter(profile(), true);
        let effects = s.update(Msg::Shell(ShellMsg::NewTab)).effects;
        assert!(!effects
            .iter()
            .any(|e| matches!(e, Effect::SaveSessionLater { .. })));
        assert!(s.save_now().is_none());
    }

    #[test]
    fn a_saved_session_is_restored_when_it_arrives() {
        let p = profile();
        let (mut busy, _) = Shell::new();
        let _ = busy.update(ShellMsg::NewTab);
        let _ = busy.update(ShellMsg::Section(Section::Settings));
        let saved = busy.capture();
        let (mut s, _) = Session::enter(p.clone(), true);
        let _ = s.update(Msg::Reply(Reply::SessionLoaded {
            profile: p.id,
            saved: Some(saved),
        }));
        assert_eq!(s.shell.tabs.len(), 2);
        assert_eq!(s.shell.active().current().title(), "Settings");
        assert!(s.save_now().is_some());
    }

    #[test]
    fn the_mouse_back_and_forward_buttons_move_through_history() {
        let mut s = ready();
        let _ = s.update(Msg::Shell(ShellMsg::Section(Section::Scenes)));
        let _ = s.update(Msg::MouseHistory(true));
        assert_eq!(s.shell.active().current().title(), "Home");
        let _ = s.update(Msg::MouseHistory(false));
        assert_eq!(s.shell.active().current().title(), "Scenes");
    }
}
