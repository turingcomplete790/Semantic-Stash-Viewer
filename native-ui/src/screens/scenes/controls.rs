//! The Scenes controls (007 T039): the browse layer's bar and pager (012) for scenes.

use iced::Element;

use super::state::{shell_msg, ScenesState};
use crate::screens::browse::controls;
use crate::shell::ShellMsg;

pub use crate::screens::browse::controls::GO_TO_ID;

/// Position, first/previous/next/last, and go to page.
pub fn pager(state: &ScenesState) -> Element<'_, ShellMsg> {
    controls::pager(&state.list, shell_msg)
}

/// The bar above the grid.
pub fn top(state: &ScenesState) -> Element<'_, ShellMsg> {
    controls::top(&state.list, shell_msg)
}
