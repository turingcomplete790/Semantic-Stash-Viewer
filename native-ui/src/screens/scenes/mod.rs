//! The Scenes screen (007 US3; spec FR-008): the library a page at a time, as a grid or a list,
//! with thumbnails, sorting, page sizes, the keyboard, and return to place.

pub mod controls;
pub mod grid;
pub mod keyboard;
pub mod layout;
pub mod state;
pub mod thumbs;

pub use grid::view;
pub use state::{page_key, scroll_id, Data, ScenesMsg, ScenesState, ScenesUp};
pub use thumbs::Thumbs;
