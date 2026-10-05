//! 007 T033: the grid's keyboard rules (005 research R8, FR-006; the web build's
//! `scenes/keyboard.ts`), as a pure function.

use iced::keyboard::key::Named;
use iced::keyboard::Key;
use semantic_stash_viewer_native::screens::scenes::keyboard::PageDirection::{Next, Previous};
use semantic_stash_viewer_native::screens::scenes::keyboard::{grid_key, GridAction, GridKeys};

fn at(index: usize) -> GridKeys {
    GridKeys {
        index: Some(index),
        count: 10,
        cols: 4,
        ctrl: false,
    }
}

fn named(n: Named) -> Key {
    Key::Named(n)
}

fn ch(c: &str) -> Key {
    Key::Character(c.into())
}

#[test]
fn arrows_move_within_the_page() {
    assert_eq!(
        grid_key(&named(Named::ArrowRight), at(1)),
        Some(GridAction::Move(2))
    );
    assert_eq!(
        grid_key(&named(Named::ArrowLeft), at(1)),
        Some(GridAction::Move(0))
    );
    assert_eq!(
        grid_key(&named(Named::ArrowDown), at(1)),
        Some(GridAction::Move(5))
    );
    assert_eq!(
        grid_key(&named(Named::ArrowUp), at(5)),
        Some(GridAction::Move(1))
    );
}

#[test]
fn past_an_edge_crosses_to_the_neighbouring_page() {
    assert_eq!(
        grid_key(&named(Named::ArrowRight), at(9)),
        Some(GridAction::Edge(Next))
    );
    assert_eq!(
        grid_key(&named(Named::ArrowDown), at(7)),
        Some(GridAction::Edge(Next))
    );
    assert_eq!(
        grid_key(&named(Named::ArrowLeft), at(0)),
        Some(GridAction::Edge(Previous))
    );
    assert_eq!(
        grid_key(&named(Named::ArrowUp), at(3)),
        Some(GridAction::Edge(Previous))
    );
}

#[test]
fn home_end_and_brackets() {
    assert_eq!(
        grid_key(&named(Named::Home), at(6)),
        Some(GridAction::Move(0))
    );
    assert_eq!(
        grid_key(&named(Named::End), at(2)),
        Some(GridAction::Move(9))
    );
    assert_eq!(grid_key(&ch("]"), at(2)), Some(GridAction::Page(Next)));
    assert_eq!(grid_key(&ch("["), at(2)), Some(GridAction::Page(Previous)));
}

#[test]
fn enter_opens_and_ctrl_enter_opens_in_a_new_tab() {
    assert_eq!(
        grid_key(&named(Named::Enter), at(3)),
        Some(GridAction::Open {
            index: 3,
            new_tab: false
        })
    );
    let ctrl = GridKeys {
        ctrl: true,
        ..at(3)
    };
    assert_eq!(
        grid_key(&named(Named::Enter), ctrl),
        Some(GridAction::Open {
            index: 3,
            new_tab: true
        })
    );
}

#[test]
fn list_mode_is_one_column() {
    let list = GridKeys { cols: 1, ..at(4) };
    assert_eq!(
        grid_key(&named(Named::ArrowDown), list),
        Some(GridAction::Move(5))
    );
    assert_eq!(
        grid_key(&named(Named::ArrowUp), list),
        Some(GridAction::Move(3))
    );
}

#[test]
fn without_focus_an_arrow_focuses_the_first_card_and_enter_does_nothing() {
    let none = GridKeys {
        index: None,
        ..at(0)
    };
    assert_eq!(
        grid_key(&named(Named::ArrowDown), none),
        Some(GridAction::Move(0))
    );
    assert_eq!(grid_key(&named(Named::Enter), none), None);
}

#[test]
fn other_keys_are_not_the_grids() {
    assert_eq!(grid_key(&ch("t"), at(0)), None);
    let empty = GridKeys {
        index: None,
        count: 0,
        cols: 4,
        ctrl: false,
    };
    assert_eq!(grid_key(&named(Named::ArrowDown), empty), None);
}
