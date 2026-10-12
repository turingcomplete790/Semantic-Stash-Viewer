//! 007 T025: the keymap (behaviour that stays B4).

use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use semantic_stash_viewer_native::shell::keymap::{bindings, resolve, Action, Level};

fn ch(c: &str) -> Key {
    Key::Character(c.into())
}

#[test]
fn shell_shortcuts_resolve() {
    let shell = [Level::Shell];
    assert_eq!(
        resolve(&ch("t"), Modifiers::CTRL, &shell),
        Some(Action::NewTab)
    );
    assert_eq!(
        resolve(&ch("w"), Modifiers::CTRL, &shell),
        Some(Action::CloseTab)
    );
    assert_eq!(
        resolve(&Key::Named(Named::Tab), Modifiers::CTRL, &shell),
        Some(Action::NextTab)
    );
    assert_eq!(
        resolve(
            &Key::Named(Named::Tab),
            Modifiers::CTRL | Modifiers::SHIFT,
            &shell
        ),
        Some(Action::PrevTab)
    );
    assert_eq!(
        resolve(&ch("3"), Modifiers::CTRL, &shell),
        Some(Action::SelectTab(2))
    );
    assert_eq!(
        resolve(&Key::Named(Named::ArrowLeft), Modifiers::ALT, &shell),
        Some(Action::Back)
    );
    assert_eq!(
        resolve(&Key::Named(Named::F1), Modifiers::empty(), &shell),
        Some(Action::KeyboardHelp)
    );
}

#[test]
fn unbound_keys_resolve_to_nothing() {
    assert_eq!(
        resolve(&ch("t"), Modifiers::empty(), &[Level::Shell]),
        None,
        "plain t isn't a shortcut"
    );
    assert_eq!(resolve(&ch("q"), Modifiers::CTRL, &[Level::Shell]), None);
}

#[test]
fn the_innermost_level_wins() {
    // A Scenes binding (added in US3) would shadow a shell one for the same chord; with only the
    // shell level active the shell's applies.
    assert_eq!(
        resolve(&ch("t"), Modifiers::CTRL, &[Level::Scenes, Level::Shell]),
        Some(Action::NewTab)
    );
}

#[test]
fn every_binding_appears_once_with_a_label() {
    let all = bindings();
    assert!(all.iter().all(|b| !b.label.is_empty()));
    let mut seen = std::collections::HashSet::new();
    for b in &all {
        assert!(
            seen.insert((b.level, b.chord.clone())),
            "duplicate {:?}",
            b.chord
        );
    }
}

#[test]
fn the_player_keeps_002s_shortcuts_exactly() {
    let player: Vec<String> = bindings()
        .iter()
        .filter(|b| b.level == Level::Player)
        .map(|b| b.chord.describe())
        .collect();
    let expected = [
        "Space", "←", "→", "↑", "↓", "[", "]", "\\", ".", ",", "F", "M", "Esc",
    ];
    assert_eq!(&player[..expected.len()], expected);
    // 008 FR-011 adds the chapter keys after them (mpv's convention).
    assert_eq!(player.len(), expected.len() + 2, "{player:?}");
}

#[test]
fn characters_ignore_shift_and_case() {
    // `?` is Shift+/ on most layouts.
    assert_eq!(
        resolve(&ch("?"), Modifiers::SHIFT, &[Level::Shell]),
        Some(Action::KeyboardHelp)
    );
    assert_eq!(
        resolve(
            &ch("T"),
            Modifiers::CTRL | Modifiers::SHIFT,
            &[Level::Shell]
        ),
        Some(Action::NewTab)
    );
    assert_eq!(
        resolve(&ch("f"), Modifiers::empty(), &[Level::Player]),
        Some(Action::Player)
    );
}

/// 010 T034: the queue view's keys are listed, and none clashes with the player's.
#[test]
fn the_queue_views_keys_are_listed() {
    let queue: Vec<String> = bindings()
        .iter()
        .filter(|b| b.level == Level::Queue)
        .map(|b| b.chord.describe())
        .collect();
    for key in [
        "↑",
        "↓",
        "Home",
        "End",
        "[",
        "]",
        "Enter",
        "Ctrl+Enter",
        "F6",
        "N",
        "P",
        "A",
        "X",
        "R",
        "O",
        "Ctrl+O",
    ] {
        assert!(queue.iter().any(|k| k == key), "{key} in {queue:?}");
    }
    // 011: Loop's `L` is gone; the playlist controls are Autoplay, Shuffle, Repeat.
    assert!(!queue.iter().any(|k| k == "L"), "{queue:?}");
    for (c, action) in [
        ("a", Action::QueueAutoplay),
        ("x", Action::QueueShuffle),
        ("r", Action::QueueRepeat),
    ] {
        assert_eq!(
            resolve(&ch(c), Modifiers::empty(), &[Level::Queue]),
            Some(action)
        );
    }
    // Next, Previous, the playlist controls, focus, and open aren't the player's or the Scenes
    // screen's keys, so they reach the queue while something plays.
    for c in ["n", "p", "a", "x", "r", "o"] {
        assert_eq!(resolve(&ch(c), Modifiers::empty(), &[Level::Player]), None);
    }
    for c in ["x", "r"] {
        assert_eq!(resolve(&ch(c), Modifiers::empty(), &[Level::Scenes]), None);
    }
    assert_eq!(
        resolve(
            &Key::Named(iced::keyboard::key::Named::F6),
            Modifiers::empty(),
            &[Level::Player]
        ),
        None
    );
}
