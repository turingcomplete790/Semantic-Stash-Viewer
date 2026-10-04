//! A tab (007 data model "Tab"): a history of whole screen states and a cursor into it. Inactive
//! screens keep their state, which is how a tab never resets (B1).

use crate::effects::Effect;
use crate::screens::{Screen, Section};

/// The most screens a tab remembers; the oldest go first.
pub const MAX_HISTORY: usize = 50;

/// A tab's id: stable while the app runs (tabs move and close), never saved.
pub type TabId = u64;

#[derive(Debug, Clone, PartialEq)]
pub struct Tab {
    pub id: TabId,
    /// At least one entry; the active screen is `history[cursor]`.
    pub history: Vec<Screen>,
    pub cursor: usize,
}

impl Tab {
    pub fn new(id: TabId, screen: Screen) -> Self {
        Self {
            id,
            history: vec![screen],
            cursor: 0,
        }
    }

    /// A tab from saved history; `None` if it's empty. The cursor is kept in range and the
    /// history to the limit.
    pub fn restore(id: TabId, mut history: Vec<Screen>, cursor: usize) -> Option<Self> {
        if history.is_empty() {
            return None;
        }
        let mut cursor = cursor.min(history.len() - 1);
        if history.len() > MAX_HISTORY {
            let drop = history.len() - MAX_HISTORY;
            history.drain(..drop);
            cursor = cursor.saturating_sub(drop);
        }
        Some(Self {
            id,
            history,
            cursor,
        })
    }

    pub fn current(&self) -> &Screen {
        &self.history[self.cursor]
    }

    pub fn current_mut(&mut self) -> &mut Screen {
        &mut self.history[self.cursor]
    }

    pub fn can_go_back(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.cursor + 1 < self.history.len()
    }

    /// The active screen's entry actions.
    pub fn enter(&self) -> Vec<Effect> {
        self.current().enter(self.id)
    }

    /// Open a screen: entries after the cursor are dropped, and the oldest beyond the limit.
    pub fn open(&mut self, screen: Screen) -> Vec<Effect> {
        self.history.truncate(self.cursor + 1);
        self.history.push(screen);
        if self.history.len() > MAX_HISTORY {
            self.history.remove(0);
        }
        self.cursor = self.history.len() - 1;
        self.enter()
    }

    /// Open a navigation bar section. Already there: nothing. The place just left is that
    /// section: go back to it, state and all (005 R13). Otherwise a fresh screen.
    pub fn open_section(&mut self, section: Section) -> Vec<Effect> {
        if self.current().section() == Some(section) {
            return Vec::new();
        }
        if self.cursor > 0 && self.history[self.cursor - 1].section() == Some(section) {
            return self.back();
        }
        self.open(Screen::for_section(section))
    }

    pub fn back(&mut self) -> Vec<Effect> {
        if !self.can_go_back() {
            return Vec::new();
        }
        self.cursor -= 1;
        self.enter()
    }

    pub fn forward(&mut self) -> Vec<Effect> {
        if !self.can_go_forward() {
            return Vec::new();
        }
        self.cursor += 1;
        self.enter()
    }
}
