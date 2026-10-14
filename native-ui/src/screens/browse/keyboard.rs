//! The grid's keyboard rules (007 T033; 005 research R8, FR-006), as the web build had them: arrows
//! move within the page and cross to the neighbouring page at its edges, Home/End, `[` `]` change
//! page, Enter opens and Ctrl+Enter opens in a new tab. The list is one column.

use iced::keyboard::key::Named;
use iced::keyboard::Key;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageDirection {
    Next,
    Previous,
}

/// What the grid knows when a key arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridKeys {
    /// The focused card on the page, if any.
    pub index: Option<usize>,
    /// Cards on the page.
    pub count: usize,
    pub cols: usize,
    pub ctrl: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridAction {
    Move(usize),
    /// Moved past the page's edge.
    Edge(PageDirection),
    Page(PageDirection),
    Open {
        index: usize,
        new_tab: bool,
    },
}

pub fn grid_key(key: &Key, c: GridKeys) -> Option<GridAction> {
    if c.count == 0 {
        return None;
    }
    let last = c.count - 1;
    let cols = c.cols.max(1) as isize;
    let step = |delta: isize| -> GridAction {
        let Some(index) = c.index else {
            return GridAction::Move(0);
        };
        let target = index as isize + delta;
        if target > last as isize {
            GridAction::Edge(PageDirection::Next)
        } else if target < 0 {
            GridAction::Edge(PageDirection::Previous)
        } else {
            GridAction::Move(target as usize)
        }
    };
    match key {
        Key::Named(Named::ArrowRight) => Some(step(1)),
        Key::Named(Named::ArrowLeft) => Some(step(-1)),
        Key::Named(Named::ArrowDown) => Some(step(cols)),
        Key::Named(Named::ArrowUp) => Some(step(-cols)),
        Key::Named(Named::Home) => Some(GridAction::Move(0)),
        Key::Named(Named::End) => Some(GridAction::Move(last)),
        Key::Named(Named::Enter) => c.index.map(|index| GridAction::Open {
            index,
            new_tab: c.ctrl,
        }),
        Key::Character(s) if s.as_str() == "]" => Some(GridAction::Page(PageDirection::Next)),
        Key::Character(s) if s.as_str() == "[" => Some(GridAction::Page(PageDirection::Previous)),
        _ => None,
    }
}
