//! The Scenes filter bar, menu, editor, and saved filters: the browse layer's (012) for scenes.

use iced::Element;

use super::state::{shell_msg, ScenesState};
use super::thumbs::Thumbs;
use crate::library::Library;
use crate::screens::browse::filters;
use crate::shell::ShellMsg;

pub use crate::screens::browse::filters::MenuRow;

/// The active criteria as chips, the applied saved filter, and what it left out.
pub fn bar<'a>(state: &'a ScenesState, library: &'a Library) -> Option<Element<'a, ShellMsg>> {
    filters::bar(&state.list, library, shell_msg)
}

/// The filter menu's rows.
pub fn menu_rows(state: &ScenesState) -> Vec<MenuRow> {
    filters::menu_rows(&state.list)
}

/// The actionable row the highlight is on (clamped).
pub fn menu_target(state: &ScenesState, highlight: usize) -> Option<MenuRow> {
    filters::menu_target(&state.list, highlight)
}

/// The open panel, if any, with a close button.
pub fn panel<'a>(
    state: &'a ScenesState,
    library: &'a Library,
    thumbs: &'a Thumbs,
) -> Option<Element<'a, ShellMsg>> {
    filters::panel(&state.list, library, thumbs, shell_msg)
}
