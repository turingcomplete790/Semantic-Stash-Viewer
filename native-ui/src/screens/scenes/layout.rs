//! The grid's geometry (007 T038): columns, card and row sizes from the window's size. The view
//! and the keyboard and scroll logic use the same numbers, so "the first visible scene" and
//! "scroll the focused card into view" match what's drawn.

use crate::screens::Mode;

/// Cards are at least this wide; columns are as many as fit.
pub const MIN_CARD: f32 = 240.0;
pub const GAP: f32 = 12.0;
/// Space around the grid.
pub const PAD: f32 = 16.0;
/// Room the vertical scrollbar takes.
pub const SCROLLBAR: f32 = 12.0;
/// The title and details lines under a card's thumbnail.
pub const CARD_TEXT: f32 = 52.0;
/// A row in the list.
pub const LIST_ROW: f32 = 76.0;
/// The navigation bar, the tab strip, and the page controls above the grid.
pub const ABOVE: f32 = 88.0 + 52.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    /// The window's logical size.
    pub width: f32,
    pub height: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Self::new(1600.0, 1000.0)
    }
}

impl Layout {
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    fn content_width(&self) -> f32 {
        (self.width - 2.0 * PAD - SCROLLBAR).max(MIN_CARD)
    }

    pub fn cols(&self, mode: Mode) -> usize {
        match mode {
            Mode::List => 1,
            Mode::Grid => {
                (((self.content_width() + GAP) / (MIN_CARD + GAP)).floor() as usize).max(1)
            }
        }
    }

    /// A card's width in the grid.
    pub fn card_width(&self) -> f32 {
        let cols = self.cols(Mode::Grid) as f32;
        (self.content_width() - GAP * (cols - 1.0)) / cols
    }

    /// A 16:9 thumbnail on a card.
    pub fn thumb_height(&self) -> f32 {
        (self.card_width() * 9.0 / 16.0).floor()
    }

    /// A row's height including the gap below it.
    pub fn row_height(&self, mode: Mode) -> f32 {
        match mode {
            Mode::Grid => self.thumb_height() + CARD_TEXT + GAP,
            Mode::List => LIST_ROW + GAP / 2.0,
        }
    }

    /// The scrolled area's visible height.
    pub fn viewport(&self) -> f32 {
        (self.height - ABOVE).max(100.0)
    }

    /// The first card at least partly visible at `scroll`.
    pub fn first_visible(&self, scroll: f32, mode: Mode) -> usize {
        let row = (scroll.max(0.0) / self.row_height(mode)).floor() as usize;
        row * self.cols(mode)
    }

    /// The top of card `index`'s row, in the scrolled content.
    pub fn row_top(&self, index: usize, mode: Mode) -> f32 {
        (index / self.cols(mode)) as f32 * self.row_height(mode)
    }

    /// Where to scroll so card `index` is fully visible from `scroll`, if it isn't.
    pub fn reveal(&self, index: usize, scroll: f32, mode: Mode) -> Option<f32> {
        let top = self.row_top(index, mode);
        let bottom = top + self.row_height(mode);
        if top < scroll {
            Some(top)
        } else if bottom > scroll + self.viewport() {
            Some((bottom - self.viewport()).max(0.0))
        } else {
            None
        }
    }

    /// The rows to build at `scroll` (T041): those in view plus one each side. Layout runs on
    /// every scroll frame, so the fewer rows built the better. `(first, end)`.
    pub fn window(&self, scroll: f32, mode: Mode, cards: usize) -> (usize, usize) {
        let rows = cards.div_ceil(self.cols(mode));
        let row_h = self.row_height(mode);
        let visible = (scroll.max(0.0) / row_h).floor() as usize;
        let in_view = (self.viewport() / row_h).ceil() as usize + 1;
        let first = visible.saturating_sub(1);
        let end = (visible + in_view + 1).min(rows);
        (first.min(end), end)
    }
}
