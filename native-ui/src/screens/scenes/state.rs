//! The Scenes screen's state and transitions (007 T036; data model "Screen"; 005's paging rules,
//! behaviour that stays B3). One request per page through the core's cache; when a page arrives,
//! its neighbours are prefetched and the next page's thumbnails warmed, so moving there is instant.

use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use stash_core::scenes::paging::{last_page, page_of, PAGE_SIZES};
use stash_core::scenes::query::{SceneQuery, SceneSort, SortDirection};
use stash_core::scenes::{SceneCard, ScenePage};
use stash_core::AppError;

use super::keyboard::{GridAction, PageDirection};
use super::layout::Layout;
use crate::effects::Effect;
use crate::machine::Step;
use crate::screens::Mode;
use crate::shell::TabId;
use crate::widgets::scroll_watch::{AutoScroll, AutoScrolled};

/// Request generations, unique across every screen so a result can only land where it was asked
/// for.
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

fn next_generation() -> u64 {
    NEXT_GENERATION.fetch_add(1, Ordering::Relaxed)
}

/// The cache key for a page (the demo's, so caches compare): `scenes:q:<hash>:s:<size>:p:<page>`.
pub fn page_key(query: &SceneQuery, page: u32, size: u32) -> String {
    format!("scenes:q:{}:s:{size}:p:{page}", query.cache_hash())
}

/// The scrollable's widget id in tab `tab`.
pub fn scroll_id(tab: TabId) -> String {
    format!("scenes-{tab}")
}

/// What the screen has to show (transient).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Data {
    #[default]
    Loading,
    Ready {
        /// Every matching scene.
        count: u32,
        cards: Vec<SceneCard>,
    },
    /// Nothing cached and the server can't be reached.
    Unreachable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenesState {
    pub query: SceneQuery,
    /// From 1: the page showing.
    pub page: u32,
    pub page_size: u32,
    pub mode: Mode,
    /// Logical pixels from the top.
    pub scroll: f32,
    /// The keyboard's card on the page.
    pub focused: Option<usize>,
    #[serde(skip)]
    pub data: Data,
    /// The request in flight.
    #[serde(skip)]
    pending: Option<u64>,
    /// The request in flight refreshes the page in place (cards stay, scroll stays).
    #[serde(skip)]
    refreshing: bool,
    /// What's typed in "go to page".
    #[serde(skip)]
    pub go_to: String,
    #[serde(skip)]
    tab: TabId,
    /// The request whose page is showing (the grid rebuilds only when it changes).
    #[serde(skip)]
    pub loaded: u64,
    /// A bench scroll to run (the UI bench only).
    #[serde(skip)]
    pub auto_scroll: Option<AutoScroll>,
}

impl Default for ScenesState {
    fn default() -> Self {
        Self {
            query: SceneQuery::default(),
            page: 1,
            page_size: stash_core::scenes::paging::DEFAULT_PAGE_SIZE,
            mode: Mode::Grid,
            scroll: 0.0,
            focused: None,
            data: Data::Loading,
            pending: None,
            refreshing: false,
            go_to: String::new(),
            tab: 0,
            loaded: 0,
            auto_scroll: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ScenesMsg {
    /// Go to a page (first, previous, next, last, or typed); clamped to the pages that exist.
    Page(u32),
    GoToInput(String),
    GoToSubmit,
    PageSize(u32),
    Sort(SceneSort),
    ToggleDirection,
    /// A new random order.
    Reshuffle,
    Mode(Mode),
    Scrolled {
        y: f32,
    },
    Focus(usize),
    Grid(GridAction),
    /// A card was clicked (Ctrl turns it into a new tab, above) or middle-clicked.
    Open {
        index: usize,
        new_tab: bool,
    },
    Retry,
    /// The UI bench: scroll automatically, then report the frame timings.
    AutoScroll(Option<AutoScroll>),
    AutoScrolled(AutoScrolled),
}

/// Events for the tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScenesUp {
    Open {
        id: String,
        title: String,
        new_tab: bool,
    },
}

fn new_seed() -> u32 {
    let bytes = uuid::Uuid::new_v4().into_bytes();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

impl ScenesState {
    /// A fresh screen at `page` with `page_size` per page.
    pub fn at(page: u32, page_size: u32) -> Self {
        Self {
            page,
            page_size,
            ..Self::default()
        }
    }

    pub fn cards(&self) -> &[SceneCard] {
        match &self.data {
            Data::Ready { cards, .. } => cards,
            _ => &[],
        }
    }

    pub fn count(&self) -> Option<u32> {
        match &self.data {
            Data::Ready { count, .. } => Some(*count),
            _ => None,
        }
    }

    /// The last page, once the count is known.
    pub fn last_page(&self) -> Option<u32> {
        self.count().map(|c| last_page(c, self.page_size))
    }

    pub fn ready(&self) -> bool {
        matches!(self.data, Data::Ready { .. })
    }

    /// Nothing showing and nothing on its way.
    pub fn needs_load(&self) -> bool {
        !self.ready() && self.pending.is_none()
    }

    /// Whether request `generation` is this screen's.
    pub fn awaits(&self, generation: u64) -> bool {
        self.pending == Some(generation)
    }

    pub fn cache_key(&self) -> String {
        page_key(&self.query, self.page, self.page_size)
    }

    /// Only the saved fields.
    pub fn persistent(&self) -> Self {
        Self {
            query: self.query.clone(),
            page: self.page,
            page_size: self.page_size,
            mode: self.mode,
            scroll: self.scroll,
            focused: self.focused,
            ..Self::default()
        }
    }

    fn scroll_to(&self, y: f32) -> Effect {
        Effect::ScrollTo {
            id: scroll_id(self.tab),
            y,
        }
    }

    /// Entry: a page that's here comes back where it was left; otherwise load it.
    pub fn enter(&mut self, tab: TabId) -> Vec<Effect> {
        self.tab = tab;
        if self.ready() {
            vec![self.scroll_to(self.scroll)]
        } else if self.pending.is_some() {
            Vec::new()
        } else {
            self.load()
        }
    }

    fn load(&mut self) -> Vec<Effect> {
        let generation = next_generation();
        self.pending = Some(generation);
        if !self.refreshing {
            self.data = Data::Loading;
        }
        vec![Effect::LoadScenesPage {
            tab: self.tab,
            generation,
            query: self.query.normalized(),
            page: self.page,
            size: self.page_size,
        }]
    }

    fn go_to_page(&mut self, page: u32) -> Vec<Effect> {
        let page = match self.last_page() {
            Some(last) => page.clamp(1, last),
            None => page.max(1),
        };
        if page == self.page && !self.needs_load() {
            return Vec::new();
        }
        self.page = page;
        self.scroll = 0.0;
        self.focused = None;
        self.refreshing = false;
        self.load()
    }

    /// The query changed: back to page 1.
    fn requery(&mut self) -> Vec<Effect> {
        self.page = 1;
        self.scroll = 0.0;
        self.focused = None;
        self.refreshing = false;
        self.load()
    }

    /// The first card at least partly visible, as an index into the whole result.
    fn first_visible(&self, layout: &Layout) -> u32 {
        let on_page = layout.first_visible(self.scroll, self.mode);
        let on_page = on_page.min(self.cards().len().saturating_sub(1));
        (self.page - 1) * self.page_size + on_page as u32
    }

    pub fn update(&mut self, msg: ScenesMsg, tab: TabId, layout: &Layout) -> Step<ScenesUp> {
        self.tab = tab;
        match msg {
            ScenesMsg::Page(page) => Step::effects(self.go_to_page(page)),
            ScenesMsg::GoToInput(text) => {
                self.go_to = text.chars().filter(char::is_ascii_digit).take(7).collect();
                Step::none()
            }
            ScenesMsg::GoToSubmit => {
                let target = self.go_to.parse::<u32>().ok();
                self.go_to.clear();
                match target {
                    Some(page) => Step::effects(self.go_to_page(page)),
                    None => Step::none(),
                }
            }
            ScenesMsg::PageSize(size) => {
                if !PAGE_SIZES.contains(&size) || size == self.page_size {
                    return Step::none();
                }
                // Keep the first visible scene on screen (005).
                let first = self.first_visible(layout);
                self.page_size = size;
                self.page = page_of(first, size);
                let on_page = (first - (self.page - 1) * size) as usize;
                self.scroll = layout.row_top(on_page, self.mode);
                self.focused = None;
                self.refreshing = false;
                Step::effects(self.load())
            }
            ScenesMsg::Sort(sort) => {
                if sort == self.query.sort {
                    return Step::none();
                }
                self.query.sort = sort;
                if sort == SceneSort::Random && self.query.seed.is_none() {
                    self.query.seed = Some(new_seed());
                }
                Step::effects(self.requery())
            }
            ScenesMsg::ToggleDirection => {
                self.query.direction = match self.query.direction {
                    SortDirection::Asc => SortDirection::Desc,
                    SortDirection::Desc => SortDirection::Asc,
                };
                Step::effects(self.requery())
            }
            ScenesMsg::Reshuffle => {
                let old = self.query.seed;
                while self.query.seed == old {
                    self.query.seed = Some(new_seed());
                }
                Step::effects(self.requery())
            }
            ScenesMsg::Mode(mode) => {
                if mode == self.mode {
                    return Step::none();
                }
                let first = layout.first_visible(self.scroll, self.mode);
                self.mode = mode;
                self.scroll = layout.row_top(first, mode);
                Step::effect(self.scroll_to(self.scroll))
            }
            ScenesMsg::Scrolled { y } => {
                self.scroll = y;
                Step::none()
            }
            ScenesMsg::Focus(index) => Step::effects(self.focus(index, layout)),
            ScenesMsg::Grid(action) => self.grid(action, layout),
            ScenesMsg::Open { index, new_tab } => {
                let Some(card) = self.cards().get(index) else {
                    return Step::none();
                };
                let up = ScenesUp::Open {
                    id: card.id.clone(),
                    title: card.title.clone(),
                    new_tab,
                };
                self.focused = Some(index);
                Step::up(up)
            }
            ScenesMsg::AutoScroll(auto) => {
                self.auto_scroll = auto;
                Step::none()
            }
            ScenesMsg::AutoScrolled(_) => {
                self.auto_scroll = None;
                Step::none()
            }
            ScenesMsg::Retry => {
                self.refreshing = false;
                Step::effects(self.load())
            }
        }
    }

    fn focus(&mut self, index: usize, layout: &Layout) -> Vec<Effect> {
        let count = self.cards().len();
        if count == 0 {
            return Vec::new();
        }
        let index = index.min(count - 1);
        self.focused = Some(index);
        match layout.reveal(index, self.scroll, self.mode) {
            Some(y) => {
                self.scroll = y;
                vec![self.scroll_to(y)]
            }
            None => Vec::new(),
        }
    }

    fn grid(&mut self, action: GridAction, layout: &Layout) -> Step<ScenesUp> {
        let last = self.last_page().unwrap_or(self.page);
        match action {
            GridAction::Move(index) => Step::effects(self.focus(index, layout)),
            GridAction::Edge(PageDirection::Next) if self.page < last => {
                let effects = self.go_to_page(self.page + 1);
                self.focused = Some(0);
                Step::effects(effects)
            }
            GridAction::Edge(PageDirection::Previous) if self.page > 1 => {
                let effects = self.go_to_page(self.page - 1);
                self.focused = Some(self.page_size as usize - 1);
                Step::effects(effects)
            }
            GridAction::Edge(_) => Step::none(),
            GridAction::Page(PageDirection::Next) => Step::effects(self.go_to_page(self.page + 1)),
            GridAction::Page(PageDirection::Previous) => {
                Step::effects(self.go_to_page(self.page.saturating_sub(1)))
            }
            GridAction::Open { index, new_tab } => {
                self.update(ScenesMsg::Open { index, new_tab }, self.tab, layout)
            }
        }
    }

    /// A page arrived. `waiting`: the session is still connecting, so a failure isn't final.
    pub fn page_loaded(
        &mut self,
        generation: u64,
        result: Result<ScenePage, AppError>,
        waiting: bool,
    ) -> Vec<Effect> {
        if self.pending != Some(generation) {
            return Vec::new();
        }
        self.pending = None;
        let refreshing = std::mem::take(&mut self.refreshing);
        match result {
            Ok(page) => {
                self.page = page.page;
                let last = last_page(page.count, self.page_size);
                if let Some(f) = self.focused {
                    self.focused = (!page.items.is_empty()).then(|| f.min(page.items.len() - 1));
                }
                self.data = Data::Ready {
                    count: page.count,
                    cards: page.items,
                };
                self.loaded = generation;
                let mut effects = Vec::new();
                if !refreshing {
                    effects.push(self.scroll_to(self.scroll));
                }
                let next = (self.page < last).then_some(self.page + 1);
                let pages: Vec<u32> = next
                    .into_iter()
                    .chain((self.page > 1).then(|| self.page - 1))
                    .collect();
                if !pages.is_empty() {
                    effects.push(Effect::PrefetchScenes {
                        query: self.query.normalized(),
                        pages,
                        size: self.page_size,
                        warm: next,
                    });
                }
                effects
            }
            Err(e) => {
                tracing::debug!(error = %e, page = self.page, "scenes page unavailable");
                if !refreshing {
                    self.data = if waiting {
                        Data::Loading
                    } else {
                        Data::Unreachable
                    };
                }
                Vec::new()
            }
        }
    }

    /// Cached data changed: re-read the page in place if it's this one (`*`: everything).
    pub fn cache_changed(&mut self, key: &str, tab: TabId) -> Vec<Effect> {
        if !self.ready() || (key != "*" && key != self.cache_key()) {
            return Vec::new();
        }
        self.tab = tab;
        self.refreshing = true;
        self.load()
    }
}
