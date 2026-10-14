//! The grid's geometry (007 T038): columns, card and row sizes from the window's size. The view
//! and the keyboard and scroll logic use the same numbers, so "the first visible scene" and
//! "scroll the focused card into view" match what's drawn.

use crate::screens::Mode;

/// Cards are at least this wide; columns are as many as fit.
pub const MIN_CARD: f32 = 240.0;
pub const GAP: f32 = 12.0;
/// The wall's tiles (008 FR-020): dense stills at least this wide, this far apart.
pub const MIN_WALL: f32 = 160.0;
pub const WALL_GAP: f32 = 4.0;
/// Space around the grid.
pub const PAD: f32 = 16.0;
/// Room the vertical scrollbar takes.
pub const SCROLLBAR: f32 = 12.0;
/// The title and details lines under a card's thumbnail.
pub const CARD_TEXT: f32 = 52.0;
/// A row in the list.
pub const LIST_ROW: f32 = 76.0;
/// A row in the queue's list (010): a small still, title, and details line.
pub const QUEUE_ROW: f32 = 60.0;
/// The queue's list is a third of the window, at least this wide (010 research R7).
pub const QUEUE_LIST_MIN: f32 = 300.0;
/// The queue's panel is at least this wide with its details.
pub const QUEUE_PANEL_MIN: f32 = 480.0;
/// Below this, the panel has no room even for the player or cover.
pub const QUEUE_MEDIA_MIN: f32 = 240.0;
/// The playlist strip above the queue's list (011: Shuffle, Repeat, Autoplay).
pub const QUEUE_STRIP: f32 = 36.0;

/// What fits of the queue view in the window (010 R7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueFit {
    /// The list and the whole panel.
    Both,
    /// The list and the panel's player or cover, no details.
    MediaOnly,
    /// The list alone: choosing a scene opens its page.
    ListOnly,
}

/// The navigation bar, the tab strip, and the page controls above the grid.
pub const ABOVE: f32 = 88.0 + 52.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    /// The window's logical size.
    pub width: f32,
    pub height: f32,
    /// Physical pixels per logical pixel (2 on a high-density display): images are decoded at
    /// their drawn size times this, so they stay sharp.
    pub scale: f32,
    /// Grid cards are at least this wide (Images' card size, 012 FR-004); the wall's tiles scale
    /// with it.
    pub min_card: f32,
    /// A card's picture height over its width: 16:9 for scenes, square for images.
    pub aspect: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Self::new(1600.0, 1000.0)
    }
}

impl Layout {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            scale: 1.0,
            min_card: MIN_CARD,
            aspect: 9.0 / 16.0,
        }
    }

    /// The same window with cards at least `min_card` wide and pictures `aspect` high.
    pub fn with_cards(self, min_card: f32, aspect: f32) -> Self {
        Self {
            min_card,
            aspect,
            ..self
        }
    }

    fn min_wall(&self) -> f32 {
        MIN_WALL * self.min_card / MIN_CARD
    }

    /// The same window at this display scale.
    pub fn scaled(self, scale: f32) -> Self {
        Self {
            scale: if scale.is_finite() && scale > 0.0 {
                scale
            } else {
                1.0
            },
            ..self
        }
    }

    /// Physical pixels for an image drawn `logical` pixels wide.
    pub fn physical(&self, logical: f32) -> u32 {
        (logical * self.scale).ceil().max(1.0) as u32
    }

    fn content_width(&self) -> f32 {
        (self.width - 2.0 * PAD - SCROLLBAR).max(self.min_card)
    }

    /// How much of the queue view fits.
    pub fn queue_fit(&self) -> QueueFit {
        if self.width >= QUEUE_LIST_MIN + QUEUE_PANEL_MIN {
            QueueFit::Both
        } else if self.width >= QUEUE_LIST_MIN + QUEUE_MEDIA_MIN {
            QueueFit::MediaOnly
        } else {
            QueueFit::ListOnly
        }
    }

    /// The queue's list width: a third of the window, at least its minimum; all of it when the
    /// panel doesn't fit.
    pub fn queue_list_width(&self) -> f32 {
        match self.queue_fit() {
            QueueFit::Both => (self.width / 3.0).max(QUEUE_LIST_MIN),
            QueueFit::MediaOnly => QUEUE_LIST_MIN,
            QueueFit::ListOnly => self.width,
        }
    }

    /// The queue's panel width (0 when it doesn't show).
    pub fn queue_panel_width(&self) -> f32 {
        (self.width - self.queue_list_width()).max(0.0)
    }

    pub fn cols(&self, mode: Mode) -> usize {
        match mode {
            Mode::List | Mode::Queue => 1,
            Mode::Grid => {
                (((self.content_width() + GAP) / (self.min_card + GAP)).floor() as usize).max(1)
            }
            Mode::Wall => (((self.content_width() + WALL_GAP) / (self.min_wall() + WALL_GAP))
                .floor() as usize)
                .max(1),
        }
    }

    /// Space between cards in a row.
    pub fn gap(&self, mode: Mode) -> f32 {
        match mode {
            Mode::Wall => WALL_GAP,
            Mode::Grid | Mode::List | Mode::Queue => GAP,
        }
    }

    /// A wall tile's width.
    pub fn wall_width(&self) -> f32 {
        let cols = self.cols(Mode::Wall) as f32;
        (self.content_width() - WALL_GAP * (cols - 1.0)) / cols
    }

    /// A wall tile's height (16:9 for scenes).
    pub fn wall_height(&self) -> f32 {
        (self.wall_width() * self.aspect).floor()
    }

    /// A card's width in the grid.
    pub fn card_width(&self) -> f32 {
        let cols = self.cols(Mode::Grid) as f32;
        (self.content_width() - GAP * (cols - 1.0)) / cols
    }

    /// A card's thumbnail height (16:9 for scenes).
    pub fn thumb_height(&self) -> f32 {
        (self.card_width() * self.aspect).floor()
    }

    /// A row's height including the gap below it.
    pub fn row_height(&self, mode: Mode) -> f32 {
        match mode {
            Mode::Grid => self.thumb_height() + CARD_TEXT + GAP,
            Mode::List => LIST_ROW + GAP / 2.0,
            Mode::Queue => QUEUE_ROW + GAP / 2.0,
            Mode::Wall => self.wall_height() + WALL_GAP,
        }
    }

    /// The scrolled area's visible height.
    pub fn viewport(&self) -> f32 {
        (self.height - ABOVE).max(100.0)
    }

    /// The scrolled list's visible height in `mode`: the queue's list sits below its playlist
    /// strip.
    pub fn list_viewport(&self, mode: Mode) -> f32 {
        match mode {
            Mode::Queue => (self.viewport() - QUEUE_STRIP).max(100.0),
            Mode::Grid | Mode::List | Mode::Wall => self.viewport(),
        }
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
        } else if bottom > scroll + self.list_viewport(mode) {
            Some((bottom - self.list_viewport(mode)).max(0.0))
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
        let in_view = (self.list_viewport(mode) / row_h).ceil() as usize + 1;
        let first = visible.saturating_sub(1);
        let end = (visible + in_view + 1).min(rows);
        (first.min(end), end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_queue_list_is_a_third_with_minimums() {
        let wide = Layout::new(1800.0, 1000.0);
        assert_eq!(wide.queue_fit(), QueueFit::Both);
        assert_eq!(wide.queue_list_width(), 600.0);
        assert_eq!(wide.queue_panel_width(), 1200.0);
        // A third would be under the list's minimum: it keeps the minimum.
        let small = Layout::new(800.0, 700.0);
        assert_eq!(small.queue_fit(), QueueFit::Both);
        assert_eq!(small.queue_list_width(), QUEUE_LIST_MIN);
        assert!(small.queue_panel_width() >= QUEUE_PANEL_MIN);
    }

    #[test]
    fn a_narrow_window_drops_the_details_then_the_panel() {
        let narrow = Layout::new(640.0, 700.0);
        assert_eq!(narrow.queue_fit(), QueueFit::MediaOnly);
        assert_eq!(narrow.queue_list_width(), QUEUE_LIST_MIN);
        let tiny = Layout::new(500.0, 700.0);
        assert_eq!(tiny.queue_fit(), QueueFit::ListOnly);
        assert_eq!(tiny.queue_list_width(), 500.0);
        assert_eq!(tiny.queue_panel_width(), 0.0);
    }
}
