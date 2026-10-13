//! The Images screen's state (012 US1; data model "ImagesState"): the browse layer's [`List`] for
//! images (query, page, filters, search, saved filters, as Scenes) and the card size. The viewer
//! (US2) joins it.

use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};
use stash_core::browse::entity::Entity;
use stash_core::images::{ImageCard, ImagePage, ImageQuery, ImageSort};
use stash_core::scenes::filter::saved::SavedFilterOf;

use crate::effects::Effect;
use crate::library::Need;
use crate::machine::Step;
use crate::screens::browse::layout::Layout;
use crate::screens::browse::state::{BrowseMsg, BrowseUp, Kind, List};
use crate::screens::Mode;
use crate::shell::TabId;

/// Images as a browse list.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImageKind;

impl Kind for ImageKind {
    type Sort = ImageSort;
    type Card = ImageCard;
    type Page = ImagePage;
    const ENTITY: Entity = Entity::Images;

    fn card_id(card: &ImageCard) -> &str {
        &card.id
    }

    fn card_title(card: &ImageCard) -> &str {
        &card.title
    }

    fn card_thumb(card: &ImageCard) -> Option<&str> {
        card.thumb.as_deref()
    }

    fn card_view(
        card: &ImageCard,
        index: usize,
        focused: bool,
        _marked: bool,
        mode: Mode,
        layout: &Layout,
        pictures: crate::screens::browse::grid::Pictures<'_>,
        msg: crate::screens::browse::controls::Map<Self>,
    ) -> iced::Element<'static, crate::shell::ShellMsg> {
        super::grid::card_view(card, index, focused, mode, layout, pictures, msg)
    }

    fn page_parts(page: ImagePage) -> (u32, u32, Vec<ImageCard>) {
        (page.count, page.page, page.items)
    }

    fn load(tab: TabId, generation: u64, query: ImageQuery, page: u32, size: u32) -> Effect {
        Effect::LoadImagesPage {
            tab,
            generation,
            query,
            page,
            size,
        }
    }

    fn prefetch(query: ImageQuery, pages: Vec<u32>, size: u32, warm: Option<u32>) -> Effect {
        Effect::PrefetchImages {
            query,
            pages,
            size,
            warm,
        }
    }

    fn saved_filters() -> Need {
        Need::SavedImageFilters
    }

    fn sorts() -> &'static [ImageSort] {
        ImageSort::all()
    }

    fn sort_label(sort: ImageSort) -> &'static str {
        sort.label()
    }

    fn modes() -> &'static [(Mode, &'static str)] {
        &[(Mode::Grid, "Grid"), (Mode::Wall, "Wall")]
    }

    fn saved(
        library: &crate::library::Library,
    ) -> &crate::library::Loadable<Vec<SavedFilterOf<ImageSort>>> {
        &library.saved_images
    }

    const NOUN: &'static str = "images";
    const ITEM: &'static str = "image";
    const SEARCH_PLACEHOLDER: &'static str = "Search images";

    fn default_query() -> ImageQuery {
        ImageQuery::images_default()
    }
}

/// The card size control's widths (the web UI's zoom slider, 012 FR-004): a grid card is at
/// least this wide.
pub const CARD_SIZES: [f32; 4] = [160.0, 240.0, 320.0, 480.0];

/// A new tab's card size (240 px, as Scenes' cards).
pub const DEFAULT_CARD_SIZE: u8 = 1;

/// The shared views' list messages, as the shell's Images messages.
pub fn shell_msg(m: BrowseMsg<ImageSort>) -> crate::shell::ShellMsg {
    crate::shell::ShellMsg::Images(ImagesMsg::List(m))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImagesState {
    /// The list: query, page, size, mode, scroll, focus, applied filter (saved), and its
    /// transient state.
    #[serde(flatten)]
    pub list: List<ImageKind>,
    /// Index into [`CARD_SIZES`].
    #[serde(default = "default_card_size")]
    pub card_size: u8,
}

fn default_card_size() -> u8 {
    DEFAULT_CARD_SIZE
}

impl Default for ImagesState {
    fn default() -> Self {
        Self {
            list: List::default(),
            card_size: DEFAULT_CARD_SIZE,
        }
    }
}

impl Deref for ImagesState {
    type Target = List<ImageKind>;

    fn deref(&self) -> &List<ImageKind> {
        &self.list
    }
}

impl DerefMut for ImagesState {
    fn deref_mut(&mut self) -> &mut List<ImageKind> {
        &mut self.list
    }
}

#[derive(Debug, Clone)]
pub enum ImagesMsg {
    /// The list's own messages (paging, sort, search, filters, the keyboard's card).
    List(BrowseMsg<ImageSort>),
    /// The card size control moved (an index into [`CARD_SIZES`]).
    CardSize(u8),
}

impl From<BrowseMsg<ImageSort>> for ImagesMsg {
    fn from(m: BrowseMsg<ImageSort>) -> Self {
        ImagesMsg::List(m)
    }
}

impl ImagesState {
    /// The window's geometry for this list: cards at the chosen size, square pictures.
    pub fn layout(&self, window: &Layout) -> Layout {
        let size = CARD_SIZES[(self.card_size as usize).min(CARD_SIZES.len() - 1)];
        window.with_cards(size, 1.0)
    }

    /// Only the saved fields.
    pub fn persistent(&self) -> Self {
        Self {
            list: self.list.persistent(),
            card_size: self.card_size,
        }
    }

    /// A fresh screen showing a saved filter (the server's default for new tabs).
    pub fn from_saved(filter: &SavedFilterOf<ImageSort>) -> Self {
        let mut s = Self::default();
        s.list.adopt(filter);
        s
    }

    /// `window`: the window's geometry (the card size is applied here).
    pub fn update(&mut self, msg: ImagesMsg, tab: TabId, window: &Layout) -> Step<BrowseUp> {
        let layout = self.layout(window);
        match msg {
            ImagesMsg::List(m) => self.list.update(m, tab, &layout),
            ImagesMsg::CardSize(size) => {
                let size = size.min(CARD_SIZES.len() as u8 - 1);
                if size == self.card_size {
                    return Step::none();
                }
                self.list.tab = tab;
                // Keep the first visible card in view (as a mode change does).
                let first = layout.first_visible(self.list.scroll, self.list.mode);
                self.card_size = size;
                self.list.scroll = self.layout(window).row_top(first, self.list.mode);
                Step::effect(self.list.scroll_to(self.list.scroll))
            }
        }
    }
}
