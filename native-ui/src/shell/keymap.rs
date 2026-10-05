//! The keymap (007 T030, research R6): one table of `(chord, level, action, label)` that serves
//! dispatch, the keyboard help overlay, and Settings → Keyboard. Key events a focused field
//! captured never get here (the app's key subscription keeps only uncaptured ones). The rest go
//! to the innermost active level that binds the chord; unbound keys bubble up to the shell.

use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use iced::widget::{column, row, scrollable, text};
use iced::{Element, Length};

use crate::widgets::theme;

/// Which level of the state machine a binding belongs to, innermost first when dispatching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Level {
    Player,
    Scenes,
    Shell,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Level::Player => "Player",
            Level::Scenes => "Scenes",
            Level::Shell => "Everywhere",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChordKey {
    Named(Named),
    /// Matched case-insensitively.
    Char(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Chord {
    pub key: ChordKey,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Chord {
    const fn plain(key: ChordKey) -> Self {
        Self {
            key,
            ctrl: false,
            shift: false,
            alt: false,
        }
    }

    const fn ctrl(key: ChordKey) -> Self {
        Self {
            key,
            ctrl: true,
            shift: false,
            alt: false,
        }
    }

    const fn ctrl_shift(key: ChordKey) -> Self {
        Self {
            key,
            ctrl: true,
            shift: true,
            alt: false,
        }
    }

    const fn alt(key: ChordKey) -> Self {
        Self {
            key,
            ctrl: false,
            shift: false,
            alt: true,
        }
    }

    /// Whether a key press matches. Shift is ignored for characters, which already carry it
    /// (`?` is Shift+/).
    pub fn matches(&self, key: &Key, mods: Modifiers) -> bool {
        if self.ctrl != mods.control() || self.alt != mods.alt() || mods.logo() {
            return false;
        }
        match (&self.key, key) {
            (ChordKey::Named(want), Key::Named(got)) => want == got && self.shift == mods.shift(),
            (ChordKey::Char(want), Key::Character(got)) => want.eq_ignore_ascii_case(got.as_str()),
            _ => false,
        }
    }

    /// How the chord is written in help: `Ctrl+T`, `Space`, `←`.
    pub fn describe(&self) -> String {
        let key = match &self.key {
            ChordKey::Named(n) => match n {
                Named::Space => "Space".to_owned(),
                Named::ArrowLeft => "←".to_owned(),
                Named::ArrowRight => "→".to_owned(),
                Named::ArrowUp => "↑".to_owned(),
                Named::ArrowDown => "↓".to_owned(),
                Named::Escape => "Esc".to_owned(),
                Named::PageUp => "Page Up".to_owned(),
                Named::PageDown => "Page Down".to_owned(),
                other => format!("{other:?}"),
            },
            ChordKey::Char(c) => c.to_uppercase(),
        };
        let mut out = String::new();
        if self.ctrl {
            out.push_str("Ctrl+");
        }
        if self.alt {
            out.push_str("Alt+");
        }
        if self.shift {
            out.push_str("Shift+");
        }
        out.push_str(&key);
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    /// By position, from 0.
    SelectTab(usize),
    MoveTabLeft,
    MoveTabRight,
    Back,
    Forward,
    KeyboardHelp,
    FocusNext,
    FocusPrevious,
    /// Handled by the player's own controls (002 FR-009; frame steps only while paused).
    Player,
    /// Handled by the Scenes grid (`screens::scenes::keyboard`; 005 FR-006).
    Scenes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub level: Level,
    pub chord: Chord,
    pub action: Action,
    pub label: &'static str,
}

const DIGITS: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];

/// Every binding.
pub fn bindings() -> Vec<Binding> {
    use ChordKey::{Char, Named as N};
    let b = |level, chord, action, label| Binding {
        level,
        chord,
        action,
        label,
    };
    let mut all = vec![
        b(
            Level::Shell,
            Chord::ctrl(Char("t")),
            Action::NewTab,
            "New tab",
        ),
        b(
            Level::Shell,
            Chord::ctrl(Char("w")),
            Action::CloseTab,
            "Close tab",
        ),
        b(
            Level::Shell,
            Chord::ctrl(N(Named::Tab)),
            Action::NextTab,
            "Next tab",
        ),
        b(
            Level::Shell,
            Chord::ctrl_shift(N(Named::Tab)),
            Action::PrevTab,
            "Previous tab",
        ),
        b(
            Level::Shell,
            Chord::ctrl(N(Named::PageDown)),
            Action::NextTab,
            "Next tab",
        ),
        b(
            Level::Shell,
            Chord::ctrl(N(Named::PageUp)),
            Action::PrevTab,
            "Previous tab",
        ),
        b(
            Level::Shell,
            Chord::ctrl_shift(N(Named::PageUp)),
            Action::MoveTabLeft,
            "Move tab left",
        ),
        b(
            Level::Shell,
            Chord::ctrl_shift(N(Named::PageDown)),
            Action::MoveTabRight,
            "Move tab right",
        ),
        b(
            Level::Shell,
            Chord::alt(N(Named::ArrowLeft)),
            Action::Back,
            "Back",
        ),
        b(
            Level::Shell,
            Chord::alt(N(Named::ArrowRight)),
            Action::Forward,
            "Forward",
        ),
        b(
            Level::Shell,
            Chord::plain(N(Named::F1)),
            Action::KeyboardHelp,
            "Keyboard shortcuts",
        ),
        b(
            Level::Shell,
            Chord::plain(Char("?")),
            Action::KeyboardHelp,
            "Keyboard shortcuts",
        ),
        b(
            Level::Shell,
            Chord::plain(N(Named::Tab)),
            Action::FocusNext,
            "Next control",
        ),
        b(
            Level::Shell,
            Chord {
                shift: true,
                ..Chord::plain(N(Named::Tab))
            },
            Action::FocusPrevious,
            "Previous control",
        ),
    ];
    for (i, d) in DIGITS.iter().enumerate() {
        all.push(b(
            Level::Shell,
            Chord::ctrl(Char(d)),
            Action::SelectTab(i),
            "Go to tab",
        ));
    }
    // The grid's keys (005 FR-006), listed here; the grid applies them.
    for (chord, label) in [
        (
            Chord::plain(N(Named::ArrowRight)),
            "Move between scenes (arrows)",
        ),
        (Chord::plain(N(Named::Home)), "First scene on the page"),
        (Chord::plain(N(Named::End)), "Last scene on the page"),
        (Chord::plain(Char("]")), "Next page"),
        (Chord::plain(Char("[")), "Previous page"),
        (Chord::plain(N(Named::Enter)), "Open the scene"),
        (Chord::ctrl(N(Named::Enter)), "Open in a new tab"),
    ] {
        all.push(b(Level::Scenes, chord, Action::Scenes, label));
    }
    // 002 FR-009, exactly.
    for (key, label) in [
        (N(Named::Space), "Play or pause"),
        (N(Named::ArrowLeft), "Back 10 seconds"),
        (N(Named::ArrowRight), "Forward 10 seconds"),
        (N(Named::ArrowUp), "Volume up"),
        (N(Named::ArrowDown), "Volume down"),
        (Char("["), "Slower"),
        (Char("]"), "Faster"),
        (Char("\\"), "Normal speed"),
        (Char("."), "Next frame (paused)"),
        (Char(","), "Previous frame (paused)"),
        (Char("f"), "Fullscreen"),
        (Char("m"), "Mute"),
        (N(Named::Escape), "Leave fullscreen, or close the player"),
    ] {
        all.push(b(Level::Player, Chord::plain(key), Action::Player, label));
    }
    all
}

/// The action for a key press, looking through the active levels innermost first.
pub fn resolve(key: &Key, mods: Modifiers, active: &[Level]) -> Option<Action> {
    let all = bindings();
    active.iter().find_map(|level| {
        all.iter()
            .find(|b| b.level == *level && b.chord.matches(key, mods))
            .map(|b| b.action)
    })
}

/// The table for people: keyboard help and Settings → Keyboard. The nine "go to tab" bindings
/// show as one row.
pub fn help_list<'a, M: 'a>() -> Element<'a, M> {
    let mut list = column![].spacing(4);
    let mut level = None;
    for b in bindings() {
        if matches!(b.action, Action::SelectTab(i) if i > 0) {
            continue;
        }
        if level != Some(b.level) {
            level = Some(b.level);
            list = list.push(text(b.level.label()).size(15).color(theme::FROSTED_BLUE));
        }
        let keys = match b.action {
            Action::SelectTab(_) => "Ctrl+1…9".to_owned(),
            Action::Scenes if b.chord.key == ChordKey::Named(Named::ArrowRight) => {
                "← ↑ → ↓".to_owned()
            }
            _ => b.chord.describe(),
        };
        let label = match b.action {
            Action::SelectTab(_) => "Go to tab 1–9",
            _ => b.label,
        };
        list = list.push(
            row![
                text(keys).size(13).width(Length::Fixed(150.0)),
                text(label).size(13)
            ]
            .spacing(12),
        );
    }
    scrollable(list).height(Length::Fill).into()
}
