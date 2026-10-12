//! The Scenes screen (007 US3; spec FR-008): the library a page at a time, as a grid or a list,
//! with thumbnails, sorting, page sizes, the keyboard, and return to place.

pub mod controls;
pub mod filters;
pub mod grid;
pub mod queue;
pub mod state;

/// Moved to the browse layer (012); kept here for existing paths.
pub use crate::screens::browse::{editor, keyboard, layout, picker, preview, thumbs};

pub use grid::view;
pub use state::{
    page_key, scroll_id, Advance, Applied, Data, Panel, ScenesMsg, ScenesState, ScenesUp,
};
pub use thumbs::Thumbs;
