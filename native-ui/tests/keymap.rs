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

/// 012 T028: Images has the Scenes grid's keys, listed under "Images".
#[test]
fn images_has_the_scenes_grid_keys() {
    let images = [Level::Images];
    let none = Modifiers::empty();
    assert_eq!(resolve(&ch("/"), none, &images), Some(Action::FocusSearch));
    assert_eq!(resolve(&ch("f"), none, &images), Some(Action::FilterMenu));
    assert_eq!(resolve(&ch("s"), none, &images), Some(Action::SavedFilters));
    for (key, mods) in [
        (Key::Named(Named::ArrowRight), none),
        (Key::Named(Named::Home), none),
        (Key::Named(Named::End), none),
        (ch("]"), none),
        (ch("["), none),
        (Key::Named(Named::Enter), none),
        (Key::Named(Named::Enter), Modifiers::CTRL),
    ] {
        assert_eq!(
            resolve(&key, mods, &images),
            Some(Action::Images),
            "{key:?}"
        );
    }
    let scenes: Vec<_> = bindings()
        .into_iter()
        .filter(|b| b.level == Level::Scenes)
        .map(|b| b.chord)
        .collect();
    let images: Vec<_> = bindings()
        .into_iter()
        .filter(|b| b.level == Level::Images)
        .map(|b| b.chord)
        .collect();
    assert_eq!(images, scenes, "the same chords, in the same order");
}

/// 012 T045: the viewer's keys are listed under "Image viewer", and no chord means two things
/// within Shell, Images, or Viewer, nor does a viewer or Images key take a shell shortcut.
#[test]
fn the_image_levels_are_listed_and_never_clash() {
    use semantic_stash_viewer_native::shell::keymap::ViewerKey;
    let all = bindings();
    for level in [Level::Shell, Level::Images, Level::Viewer] {
        let mut seen = std::collections::HashMap::new();
        for b in all.iter().filter(|b| b.level == level) {
            if let Some(other) = seen.insert(b.chord.clone(), b.action) {
                // One action may have two chords; one chord never two actions.
                assert_eq!(other, b.action, "{:?} clashes at {level:?}", b.chord);
            }
        }
    }
    let shell: Vec<_> = all
        .iter()
        .filter(|b| b.level == Level::Shell)
        .map(|b| b.chord.clone())
        .collect();
    for b in all
        .iter()
        .filter(|b| matches!(b.level, Level::Images | Level::Viewer))
    {
        assert!(
            !shell.contains(&b.chord),
            "{:?} takes a shell shortcut",
            b.chord
        );
    }
    let viewer: Vec<_> = all.iter().filter(|b| b.level == Level::Viewer).collect();
    for key in [
        ViewerKey::Next,
        ViewerKey::Previous,
        ViewerKey::Back,
        ViewerKey::First,
        ViewerKey::Last,
        ViewerKey::Up,
        ViewerKey::Down,
        ViewerKey::ZoomIn,
        ViewerKey::ZoomOut,
        ViewerKey::Fit,
        ViewerKey::ActualSize,
        ViewerKey::Rotate,
        ViewerKey::Slideshow,
        ViewerKey::DelayDown,
        ViewerKey::DelayUp,
        ViewerKey::Details,
        ViewerKey::Fullscreen,
        ViewerKey::Close,
    ] {
        assert!(
            viewer.iter().any(|b| b.action == Action::Viewer(key)),
            "{key:?} has a key"
        );
    }
}
