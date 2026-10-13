//! The Images screen's state (012; data model "ImagesState"): the browse layer's [`List`] for
//! images (query, page, filters, search, saved filters, as Scenes), the card size, and the viewer
//! (US2), a mode of the screen driven by the core's pure machine.

use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};
use stash_core::browse::entity::Entity;
use stash_core::images::viewer::{self as machine, Command, Event};
use stash_core::images::{ImageCard, ImagePage, ImageQuery, ImageSort, Media};
use stash_core::scenes::filter::saved::SavedFilterOf;
use stash_core::AppError;

use super::viewer::{ViewerMsg, ViewerState, PAN_KEY, ZOOM_WHEEL};
use crate::effects::{Effect, PlayerAction};
use crate::library::Need;
use crate::machine::Step;
use crate::screens::browse::keyboard::GridAction;
use crate::screens::browse::layout::Layout;
use crate::screens::browse::state::{Arrival, BrowseMsg, BrowseUp, Kind, List};
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
    /// The viewer, while it's open (not saved: a restart shows the grid at the image last
    /// viewed, its card focused).
    #[serde(skip)]
    pub viewer: Option<ViewerState>,
    /// Open the viewer on this result index when its page arrives (a new tab from Ctrl+click).
    #[serde(skip)]
    pub open_on_arrival: Option<u32>,
    /// The window's geometry when last asked (closing the viewer scrolls its card into view).
    #[serde(skip)]
    pub window: Layout,
}

fn default_card_size() -> u8 {
    DEFAULT_CARD_SIZE
}

impl Default for ImagesState {
    fn default() -> Self {
        Self {
            list: List::default(),
            card_size: DEFAULT_CARD_SIZE,
            viewer: None,
            open_on_arrival: None,
            window: Layout::default(),
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
    /// The viewer's input (US2).
    Viewer(ViewerMsg),
}

impl From<BrowseMsg<ImageSort>> for ImagesMsg {
    fn from(m: BrowseMsg<ImageSort>) -> Self {
        ImagesMsg::List(m)
    }
}

/// Events for the tab.
#[derive(Debug, Clone, PartialEq)]
pub enum ImagesUp {
    /// Something from the library (a picker's list, the saved filters).
    Need(Need),
    /// Show this image in the player, owned by this tab; `looped`: it animates or is a clip.
    Show {
        id: String,
        title: String,
        looped: bool,
    },
    /// Open this Images screen (the viewer opening on arrival) in a new tab.
    OpenInNewTab(Box<ImagesState>),
    /// The viewer closed: the player stops showing this tab's image.
    Closed,
}

fn from_browse(step: Step<BrowseUp>) -> Step<ImagesUp> {
    Step {
        effects: step.effects,
        up: step.up.and_then(|up| match up {
            BrowseUp::Need(n) => Some(ImagesUp::Need(n)),
            // Images open in the viewer, below; the list's own opening isn't used.
            BrowseUp::Open { .. } => None,
        }),
    }
}

/// Whether an image's file plays on and loops (animated, or a clip).
pub fn looped(card: &ImageCard) -> bool {
    !matches!(card.media, Media::Still)
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
            ..Self::default()
        }
    }

    /// A fresh screen showing a saved filter (the server's default for new tabs).
    pub fn from_saved(filter: &SavedFilterOf<ImageSort>) -> Self {
        let mut s = Self::default();
        s.list.adopt(filter);
        s
    }

    /// Card `local` of this page as an index into the whole result.
    fn global(&self, local: usize) -> u32 {
        (self.list.page - 1) * self.list.page_size + local as u32
    }

    /// The card on this page for result index `i`.
    fn local(&self, i: u32) -> Option<usize> {
        let on_page = i.checked_sub((self.list.page - 1) * self.list.page_size)? as usize;
        (on_page < self.list.cards().len()).then_some(on_page)
    }

    /// The image the viewer shows (its id), if it's open.
    pub fn viewer_image(&self) -> Option<&str> {
        self.viewer.as_ref()?.showing.as_deref()
    }

    /// `window`: the window's geometry (the card size is applied here).
    pub fn update(&mut self, msg: ImagesMsg, tab: TabId, window: &Layout) -> Step<ImagesUp> {
        let layout = self.layout(window);
        self.list.tab = tab;
        self.window = *window;
        match msg {
            ImagesMsg::List(BrowseMsg::Open { index, new_tab })
            | ImagesMsg::List(BrowseMsg::Grid(GridAction::Open { index, new_tab })) => {
                self.open(index, new_tab)
            }
            ImagesMsg::List(m) => from_browse(self.list.update(m, tab, &layout)),
            ImagesMsg::CardSize(size) => {
                let size = size.min(CARD_SIZES.len() as u8 - 1);
                if size == self.card_size {
                    return Step::none();
                }
                // Keep the first visible card in view (as a mode change does).
                let first = layout.first_visible(self.list.scroll, self.list.mode);
                self.card_size = size;
                self.list.scroll = self.layout(window).row_top(first, self.list.mode);
                Step::effect(self.list.scroll_to(self.list.scroll))
            }
            ImagesMsg::Viewer(m) => self.viewer_msg(m, window),
        }
    }

    /// Open card `index` of this page in the viewer, here or in a new tab.
    fn open(&mut self, index: usize, new_tab: bool) -> Step<ImagesUp> {
        if index >= self.list.cards().len() {
            return Step::none();
        }
        let at = self.global(index);
        self.list.focused = Some(index);
        if new_tab {
            let mut other = self.persistent();
            other.open_on_arrival = Some(at);
            return Step::up(ImagesUp::OpenInNewTab(Box::new(other)));
        }
        let count = self.list.count().unwrap_or(0);
        self.viewer = Some(ViewerState::default());
        self.event(Event::Open { at, count })
    }

    /// Step the machine and carry out its command.
    fn event(&mut self, event: Event) -> Step<ImagesUp> {
        let Some(v) = &mut self.viewer else {
            return Step::none();
        };
        let (next, command) = machine::step(&v.machine, event);
        v.machine = next;
        self.apply(command)
    }

    fn apply(&mut self, command: Command) -> Step<ImagesUp> {
        match command {
            Command::Show { at, prefetch } => self.show(at, prefetch),
            Command::SetView(view) => Step::effect(Effect::Player(PlayerAction::View(view))),
            Command::Prefetch(indices) => self.prefetch(&indices),
            Command::Close { at } => self.close(at),
            Command::Nothing => Step::none(),
        }
    }

    /// The files of these result indices, fetched ahead: from this page's cards, or another
    /// page's through the cache (a step across the page edge stays local, 012 R4).
    fn prefetch(&self, indices: &[u32]) -> Step<ImagesUp> {
        let size = self.list.page_size.max(1);
        let mut ids = Vec::new();
        let mut elsewhere: Vec<(u32, Vec<usize>)> = Vec::new();
        for &i in indices {
            match self.local(i).and_then(|l| self.list.cards().get(l)) {
                Some(card) => ids.push(card.id.clone()),
                None => {
                    let (page, position) = (i / size + 1, (i % size) as usize);
                    match elsewhere.iter_mut().find(|(p, _)| *p == page) {
                        Some((_, positions)) => positions.push(position),
                        None => elsewhere.push((page, vec![position])),
                    }
                }
            }
        }
        let mut effects = Vec::new();
        if !ids.is_empty() {
            effects.push(Effect::PrefetchImageFiles(ids));
        }
        effects.extend(elsewhere.into_iter().map(|(page, positions)| {
            Effect::PrefetchPageImageFiles {
                query: self.list.query.normalized(),
                page,
                size,
                positions,
            }
        }));
        Step::effects(effects)
    }

    /// Show result index `at`: its card if it's on this page, else load the page that holds it
    /// (shown when it arrives; the grid's page follows, unseen).
    fn show(&mut self, at: u32, prefetch: Vec<u32>) -> Step<ImagesUp> {
        let Some(local) = self.local(at) else {
            let size = self.list.page_size.max(1);
            let effects = self.list.go_to_page(at / size + 1);
            if let Some(v) = &mut self.viewer {
                v.pending = Some(at);
            }
            return Step::effects(effects);
        };
        let card = self.list.cards()[local].clone();
        self.list.focused = Some(local);
        if let Some(v) = &mut self.viewer {
            v.pending = None;
            v.showing = Some(card.id.clone());
            v.title = card.title.clone();
            v.error = None;
            v.picture = (card.width > 0 && card.height > 0).then_some((card.width, card.height));
        }
        let mut step = self.prefetch(&prefetch);
        step.up = Some(ImagesUp::Show {
            looped: looped(&card),
            id: card.id,
            title: card.title,
        });
        step
    }

    /// Back to the grid on the page of the last image viewed, its card focused and in view.
    fn close(&mut self, at: u32) -> Step<ImagesUp> {
        self.viewer = None;
        let mut effects = Vec::new();
        if let Some(local) = self.local(at) {
            self.list.focused = Some(local);
            let layout = self.layout(&self.window);
            if let Some(y) = layout.reveal(local, self.list.scroll, self.list.mode) {
                self.list.scroll = y;
            }
            effects.push(self.list.scroll_to(self.list.scroll));
        }
        Step {
            effects,
            up: Some(ImagesUp::Closed),
        }
    }

    /// Close the viewer (Escape, Back, the close button), if it's open.
    pub fn close_viewer(&mut self) -> Step<ImagesUp> {
        if self.viewer.is_none() {
            return Step::none();
        }
        self.event(Event::Close)
    }

    fn viewer_msg(&mut self, msg: ViewerMsg, window: &Layout) -> Step<ImagesUp> {
        let Some(v) = &mut self.viewer else {
            return Step::none();
        };
        match msg {
            ViewerMsg::Event(Event::Zoom { delta, .. }) => {
                let anchor = (0.0, 0.0);
                self.event(Event::Zoom { delta, anchor })
            }
            ViewerMsg::Event(e) => self.event(e),
            ViewerMsg::Wheel { y, ctrl: true } => {
                let anchor = v.anchor(window);
                self.event(Event::Zoom {
                    delta: y * ZOOM_WHEEL,
                    anchor,
                })
            }
            ViewerMsg::Wheel { y, ctrl: false } => {
                if y > 0.0 {
                    self.event(Event::Previous)
                } else if y < 0.0 {
                    self.event(Event::Next)
                } else {
                    Step::none()
                }
            }
            ViewerMsg::Pointer(p) => {
                v.activity = Some(std::time::Instant::now());
                // Dragging pans by the movement since the last position.
                let pan = v
                    .drag
                    .and_then(|from| v.pan_by(window, p.x - from.x, p.y - from.y));
                if v.drag.is_some() {
                    v.drag = Some(p);
                }
                v.pointer = Some(p);
                match pan {
                    Some((dx, dy)) => self.event(Event::Pan { dx, dy }),
                    None => Step::none(),
                }
            }
            ViewerMsg::Press => {
                v.drag = v.pointer;
                Step::none()
            }
            ViewerMsg::Release => {
                v.drag = None;
                Step::none()
            }
            ViewerMsg::DoubleClick => {
                v.drag = None;
                self.event(Event::ToggleActual)
            }
            ViewerMsg::Close => self.event(Event::Close),
        }
    }

    /// An arrow key: a pan while the picture is zoomed, otherwise a step.
    pub fn arrow(&mut self, dx: i32, dy: i32) -> Step<ImagesUp> {
        let Some(v) = &self.viewer else {
            return Step::none();
        };
        let view = v.machine.view;
        if view.zoom > 0.0 || view.actual {
            return self.event(Event::Pan {
                dx: dx as f32 * PAN_KEY,
                dy: dy as f32 * PAN_KEY,
            });
        }
        match (dx, dy) {
            (1, 0) => self.event(Event::Next),
            (-1, 0) => self.event(Event::Previous),
            _ => Step::none(),
        }
    }

    /// A viewer key's event (the session's keymap resolves the key).
    pub fn viewer_event(&mut self, event: Event) -> Step<ImagesUp> {
        self.event(event)
    }

    /// The image in the player is on screen; `picture`: its size, when the player knows it.
    pub fn image_shown(&mut self, id: &str, picture: Option<(u32, u32)>) {
        let Some(v) = &mut self.viewer else {
            return;
        };
        if v.showing.as_deref() != Some(id) {
            return;
        }
        if picture.is_some() {
            v.picture = picture;
        }
        let looping = self
            .list
            .cards()
            .iter()
            .find(|c| c.id == id)
            .is_some_and(looped);
        let at = v.machine.at;
        let _ = self.event(Event::Shown { at, looping });
    }

    /// The image in the player couldn't be shown.
    pub fn image_failed(&mut self, id: &str, reason: String) {
        let Some(v) = &mut self.viewer else {
            return;
        };
        if v.showing.as_deref() != Some(id) {
            return;
        }
        v.error = Some(reason);
        let at = v.machine.at;
        let _ = self.event(Event::Failed(at));
    }

    /// A page arrived: the list shows it; the viewer shows the image it was waiting for, and
    /// follows a result that changed size.
    pub fn page_loaded(
        &mut self,
        generation: u64,
        result: Result<ImagePage, AppError>,
        waiting: bool,
    ) -> Step<ImagesUp> {
        let refreshing = match self.list.arrive(generation, result, waiting) {
            Arrival::Arrived { refreshing } => refreshing,
            Arrival::NotOurs => return Step::none(),
            Arrival::Failed => {
                // The page the viewer stepped to couldn't load: it stays where it was.
                if let Some(v) = &mut self.viewer {
                    v.pending = None;
                }
                return Step::none();
            }
        };
        let mut effects = self.list.after_arrival(refreshing);
        let count = self.list.count().unwrap_or(0);
        if let Some(at) = self.open_on_arrival.take() {
            self.viewer = Some(ViewerState::default());
            return self.event(Event::Open { at, count }).with_all(effects);
        }
        let Some(v) = &self.viewer else {
            return Step::effects(effects);
        };
        let step = if v.machine.count != count {
            self.event(Event::ResultsChanged { count })
        } else {
            Step::none()
        };
        let pending = self.viewer.as_ref().and_then(|v| v.pending);
        let step = match (step.up, pending) {
            (None, Some(at)) => {
                let prefetch = self
                    .viewer
                    .as_ref()
                    .map(|v| {
                        let neighbours = [at.checked_add(1), at.checked_sub(1)];
                        neighbours
                            .into_iter()
                            .flatten()
                            .filter(|i| *i < v.machine.count)
                            .collect()
                    })
                    .unwrap_or_default();
                self.show(at, prefetch).with_all(step.effects)
            }
            (up, _) => Step {
                effects: step.effects,
                up,
            },
        };
        effects.extend(step.effects);
        Step {
            effects,
            up: step.up,
        }
    }
}
