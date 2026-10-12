//! A list section's state and transitions (012 research R1; moved from Scenes, 007 T036, 005's
//! paging rules, 008's filters): the query, the page showing, the filter panels, the search, saved
//! filters, and the keyboard's card. One request per page through the core's cache; when a page
//! arrives its neighbours are prefetched. Scenes and Images are instances ([`Kind`]); what is only
//! theirs (Scenes' queue view, Images' viewer) wraps this.

use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use stash_core::browse::entity::Entity;
use stash_core::browse::paging::{last_page, page_of, DEFAULT_PAGE_SIZE, PAGE_SIZES};
use stash_core::browse::query::{ListQuery, Sort, SortDirection};
use stash_core::scenes::filter::kinds::CriterionKind;
use stash_core::scenes::filter::saved::{DisplayMode, SavedFilterOf};
use stash_core::scenes::filter::Criterion;
use stash_core::AppError;

use super::editor::{Editor, EditorMsg};
use super::keyboard::{GridAction, PageDirection};
use super::layout::Layout;
use crate::effects::Effect;
use crate::library::Need;
use crate::machine::Step;
use crate::screens::Mode;
use crate::shell::TabId;
use crate::widgets::scroll_watch::{AutoScroll, AutoScrolled};

/// Request generations, unique across every screen so a result can only land where it was asked
/// for.
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

pub fn next_generation() -> u64 {
    NEXT_GENERATION.fetch_add(1, Ordering::Relaxed)
}

/// How long after the last keystroke a search runs (008 data model "Search").
pub const SEARCH_DELAY: std::time::Duration = std::time::Duration::from_millis(300);

/// The search box's widget id (one list shows at a time, so every list shares it).
pub const SEARCH_ID: &str = "scenes-search";

/// The picker's search field's widget id.
pub const PICKER_ID: &str = "scenes-picker";

/// What a list shows: its sorts, cards, and pages, and the effects that load them.
pub trait Kind: Clone + std::fmt::Debug + PartialEq + Default + 'static {
    type Sort: Sort + std::fmt::Debug + Send + Sync + 'static;
    type Card: Clone + std::fmt::Debug + PartialEq + Send + Sync + 'static;
    type Page;
    const ENTITY: Entity;

    fn card_id(card: &Self::Card) -> &str;
    fn card_title(card: &Self::Card) -> &str;
    /// The card's `ssv-thumb://` URL, if it has a picture.
    fn card_thumb(card: &Self::Card) -> Option<&str>;
    /// Card `index` drawn for `mode`; `marked`: it carries the list's mark (the scene playing).
    #[allow(clippy::too_many_arguments)]
    fn card_view(
        card: &Self::Card,
        index: usize,
        focused: bool,
        marked: bool,
        mode: Mode,
        layout: &Layout,
        pictures: super::grid::Pictures<'_>,
        msg: super::controls::Map<Self>,
    ) -> iced::Element<'static, crate::shell::ShellMsg>;
    /// `(count, page, cards)`.
    fn page_parts(page: Self::Page) -> (u32, u32, Vec<Self::Card>);
    /// Load `page` of `query` for tab `tab`, answered as request `generation`.
    fn load(
        tab: TabId,
        generation: u64,
        query: ListQuery<Self::Sort>,
        page: u32,
        size: u32,
    ) -> Effect;
    /// Prefetch `pages` of `query` (and warm `warm`'s thumbnails).
    fn prefetch(
        query: ListQuery<Self::Sort>,
        pages: Vec<u32>,
        size: u32,
        warm: Option<u32>,
    ) -> Effect;
    /// The library's saved filters for this list.
    fn saved_filters() -> Need;
    /// Every sort the web UI offers, in its menu order.
    fn sorts() -> &'static [Self::Sort];
    /// A sort's label in the web UI.
    fn sort_label(sort: Self::Sort) -> &'static str;
    /// The display modes offered, with their labels, in the bar's order.
    fn modes() -> &'static [(Mode, &'static str)];
    /// The library's saved filters for this list, as loaded.
    fn saved(
        library: &crate::library::Library,
    ) -> &crate::library::Loadable<Vec<SavedFilterOf<Self::Sort>>>;
    /// What the list holds, in running text ("scenes").
    const NOUN: &'static str;
    /// One of them ("scene").
    const ITEM: &'static str;
    /// The search box's placeholder ("Search scenes").
    const SEARCH_PLACEHOLDER: &'static str;
    /// A new list's query.
    fn default_query() -> ListQuery<Self::Sort> {
        ListQuery::default()
    }
}

/// The cache key for a page (the demo's, so caches compare): `scenes:q:<hash>:s:<size>:p:<page>`.
pub fn page_key<S: Sort>(entity: Entity, query: &ListQuery<S>, page: u32, size: u32) -> String {
    format!(
        "{}:q:{}:s:{size}:p:{page}",
        entity.cache_prefix(),
        query.cache_hash()
    )
}

/// The scrollable's widget id in tab `tab` (`scenes-<tab>`, `images-<tab>`).
pub fn scroll_id_for(entity: Entity, tab: TabId) -> String {
    format!("{}-{tab}", entity.cache_prefix())
}

/// A saved filter applied to this tab (shown above the chips until the criteria change).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Applied {
    pub id: String,
    pub name: String,
}

/// What opens above the grid (transient).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Panel {
    #[default]
    None,
    /// The filter menu: the active filters, then every kind by group; `highlight` is the
    /// keyboard's row.
    Menu { highlight: usize },
    /// One criterion being added or changed.
    Editor(Box<Editor>),
    /// The saved filters, by name.
    Saved { highlight: usize },
}

/// What the list has to show (transient).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Data<C> {
    #[default]
    Loading,
    Ready {
        /// Every match.
        count: u32,
        cards: Vec<C>,
    },
    /// Nothing cached and the server can't be reached.
    Unreachable,
}

/// The cards in `data` (none until a page is here).
pub fn cards_of<C>(data: &Data<C>) -> &[C] {
    match data {
        Data::Ready { cards, .. } => cards,
        _ => &[],
    }
}

/// A list's own messages (Scenes and Images each wrap them in theirs).
#[derive(Debug, Clone)]
pub enum BrowseMsg<S> {
    /// Go to a page (first, previous, next, last, or typed); clamped to the pages that exist.
    Page(u32),
    GoToInput(String),
    GoToSubmit,
    PageSize(u32),
    Sort(S),
    ToggleDirection,
    /// A new random order.
    Reshuffle,
    Mode(Mode),
    Scrolled {
        y: f32,
    },
    Focus(usize),
    Grid(GridAction),
    /// A card was clicked (Ctrl turns it into a new tab) or middle-clicked.
    Open {
        index: usize,
        new_tab: bool,
    },
    /// The pointer moved onto card `index` (its preview may start, 008 FR-019).
    Hover(usize),
    /// The pointer left card `index`.
    HoverEnd(usize),
    Retry,
    /// The search box changed.
    SearchInput(String),
    /// The search box's Enter: run it now.
    SearchSubmit,
    /// Open the filter menu (`true`) or close whatever is open.
    FilterMenu(bool),
    /// Move the filter menu's highlight.
    MenuMove(i32),
    /// Start a new criterion of this kind.
    Add(CriterionKind),
    /// Change an active criterion (by sort key).
    Edit(String),
    /// Remove an active criterion (by sort key).
    Remove(String),
    ClearFilters,
    Editor(EditorMsg),
    /// Open the saved filters (`true`) or close them.
    SavedMenu(bool),
    ApplySaved(Box<SavedFilterOf<S>>),
    DismissLeftOut,
    /// Show items matching exactly this criterion (from a performer, tag, or studio chip).
    FilterTo(Criterion),
    /// The UI bench: scroll automatically, then report the frame timings.
    AutoScroll(Option<AutoScroll>),
    AutoScrolled(AutoScrolled),
}

/// Events for the tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowseUp {
    Open {
        id: String,
        title: String,
        new_tab: bool,
    },
    /// Something from the library (a picker's list, the vocabulary, the saved filters).
    Need(Need),
}

pub fn new_seed() -> u32 {
    let bytes = uuid::Uuid::new_v4().into_bytes();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

/// How a page request ended, for the section's own work on arrival.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrival {
    /// Not this list's request (an older one, or another screen's).
    NotOurs,
    /// The page couldn't be read.
    Failed,
    /// The page is showing; `refreshing`: it replaced the same page in place.
    Arrived { refreshing: bool },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound(serialize = "", deserialize = ""))]
pub struct List<K: Kind> {
    pub query: ListQuery<K::Sort>,
    /// From 1: the page showing.
    pub page: u32,
    pub page_size: u32,
    pub mode: Mode,
    /// Logical pixels from the top.
    pub scroll: f32,
    /// The keyboard's card on the page.
    pub focused: Option<usize>,
    /// The saved filter applied to this tab, if the criteria haven't changed since.
    #[serde(default)]
    pub applied: Option<Applied>,
    #[serde(skip)]
    pub data: Data<K::Card>,
    /// What's typed in the search box, before the debounce runs it (`None`: the query's search).
    #[serde(skip)]
    pub search_draft: Option<String>,
    /// The debounce waiting to run the search.
    #[serde(skip)]
    pub(crate) search_pending: Option<u64>,
    #[serde(skip)]
    pub panel: Panel,
    /// Parts of the last applied saved filter the viewer couldn't apply (FR-005).
    #[serde(skip)]
    pub left_out: Vec<String>,
    /// The request in flight.
    #[serde(skip)]
    pub(crate) pending: Option<u64>,
    /// The request in flight refreshes the page in place (cards stay, scroll stays).
    #[serde(skip)]
    pub(crate) refreshing: bool,
    /// What's typed in "go to page".
    #[serde(skip)]
    pub go_to: String,
    #[serde(skip)]
    pub(crate) tab: TabId,
    /// The request whose page is showing (the grid rebuilds only when it changes).
    #[serde(skip)]
    pub loaded: u64,
    /// A bench scroll to run (the UI bench only).
    #[serde(skip)]
    pub auto_scroll: Option<AutoScroll>,
    /// The results changed (search, filter, sort, page size, or page) since the section last
    /// looked ([`List::take_changed`]).
    #[serde(skip)]
    pub(crate) changed: bool,
    /// The query changed: the arriving page is a new result.
    #[serde(skip)]
    pub(crate) new_query: bool,
}

impl<K: Kind> Default for List<K> {
    fn default() -> Self {
        Self {
            query: K::default_query(),
            page: 1,
            page_size: DEFAULT_PAGE_SIZE,
            mode: Mode::Grid,
            scroll: 0.0,
            focused: None,
            applied: None,
            data: Data::Loading,
            search_draft: None,
            search_pending: None,
            panel: Panel::None,
            left_out: Vec::new(),
            pending: None,
            refreshing: false,
            go_to: String::new(),
            tab: 0,
            loaded: 0,
            auto_scroll: None,
            changed: false,
            new_query: false,
        }
    }
}

impl<K: Kind> List<K> {
    pub fn cards(&self) -> &[K::Card] {
        cards_of(&self.data)
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

    /// Whether request `generation` is this list's.
    pub fn awaits(&self, generation: u64) -> bool {
        self.pending == Some(generation)
    }

    pub fn cache_key(&self) -> String {
        page_key(K::ENTITY, &self.query, self.page, self.page_size)
    }

    /// The tab this list belongs to (set on every entry and message).
    pub fn tab(&self) -> TabId {
        self.tab
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
            applied: self.applied.clone(),
            ..Self::default()
        }
    }

    /// Whether the results changed since the last call (the section resets what depends on them).
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    /// Whether the query changed since the last call (the arriving page is a new result).
    pub fn take_new_query(&mut self) -> bool {
        std::mem::take(&mut self.new_query)
    }

    /// Take a saved filter's search, criteria, sort, page size, and mode.
    pub fn adopt(&mut self, filter: &SavedFilterOf<K::Sort>) {
        self.query = filter.query.clone();
        if self.query.sort == K::Sort::RANDOM && self.query.seed.is_none() {
            self.query.seed = Some(new_seed());
        }
        if let Some(size) = filter.page_size {
            // The nearest size the viewer offers.
            self.page_size = PAGE_SIZES
                .iter()
                .copied()
                .min_by_key(|s| s.abs_diff(size))
                .unwrap_or(self.page_size);
        }
        match filter.mode {
            // A Stash saved filter has no Queue mode: a tab in Queue stays there (010 R6).
            _ if self.mode == Mode::Queue => {}
            Some(DisplayMode::Grid) => self.mode = Mode::Grid,
            Some(DisplayMode::List) if K::ENTITY == Entity::Scenes => self.mode = Mode::List,
            Some(DisplayMode::Wall) => self.mode = Mode::Wall,
            // The tagger is Phase 3; images have no list mode.
            Some(DisplayMode::List | DisplayMode::Tagger) | None => {}
        }
        self.applied = (filter.id != "default").then(|| Applied {
            id: filter.id.clone(),
            name: filter.name.clone(),
        });
        self.left_out = filter.left_out.clone();
        self.search_draft = None;
        self.search_pending = None;
    }

    /// What the search box shows.
    pub fn search_text(&self) -> &str {
        self.search_draft.as_deref().unwrap_or(&self.query.search)
    }

    /// The criteria changed by the user: the applied saved filter no longer describes the tab.
    fn criteria_changed(&mut self) -> Vec<Effect> {
        self.applied = None;
        self.left_out.clear();
        self.requery()
    }

    /// A debounced search came due.
    pub fn search_due(&mut self, generation: u64) -> Vec<Effect> {
        if self.search_pending != Some(generation) {
            return Vec::new();
        }
        self.run_search()
    }

    /// Whether this list waits for search debounce `generation`.
    pub fn awaits_search(&self, generation: u64) -> bool {
        self.search_pending == Some(generation)
    }

    fn run_search(&mut self) -> Vec<Effect> {
        self.search_pending = None;
        let Some(draft) = self.search_draft.take() else {
            return Vec::new();
        };
        if draft.trim() == self.query.search.trim() {
            return Vec::new();
        }
        self.query.search = draft.trim().to_owned();
        self.requery()
    }

    /// Open the editor for a new criterion of `kind`, or the existing one of that kind.
    fn open_editor(&mut self, editor: Editor) -> Step<BrowseUp> {
        let need = editor.picker().map(Need::Choices);
        let picker = editor.picker().is_some();
        self.panel = Panel::Editor(Box::new(editor));
        let mut step = match need {
            Some(n) => Step::up(BrowseUp::Need(n)),
            None => Step::none(),
        };
        if picker {
            step.effects.push(Effect::Focus(PICKER_ID));
        }
        step
    }

    fn editor_msg(&mut self, msg: EditorMsg) -> Step<BrowseUp> {
        let Panel::Editor(editor) = &mut self.panel else {
            return Step::none();
        };
        match msg {
            EditorMsg::Cancel => {
                self.panel = Panel::None;
                Step::none()
            }
            EditorMsg::Apply => match editor.build() {
                Ok(criterion) => {
                    if let Some(old) = editor.replacing.clone() {
                        if old != criterion.sort_key() {
                            self.query.remove_criterion(&old);
                        }
                    }
                    self.query.set_criterion(criterion);
                    self.panel = Panel::None;
                    Step::effects(self.criteria_changed())
                }
                Err(e) => {
                    editor.error = Some(e);
                    Step::none()
                }
            },
            other => {
                editor.update(other);
                Step::none()
            }
        }
    }

    pub fn scroll_to(&self, y: f32) -> Effect {
        Effect::ScrollTo {
            id: scroll_id_for(K::ENTITY, self.tab),
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

    pub(crate) fn load(&mut self) -> Vec<Effect> {
        let generation = next_generation();
        self.pending = Some(generation);
        if !self.refreshing {
            self.data = Data::Loading;
        }
        vec![K::load(
            self.tab,
            generation,
            self.query.normalized(),
            self.page,
            self.page_size,
        )]
    }

    pub fn go_to_page(&mut self, page: u32) -> Vec<Effect> {
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
        self.changed = true;
        self.load()
    }

    /// The query changed: back to page 1.
    fn requery(&mut self) -> Vec<Effect> {
        self.new_query = true;
        self.page = 1;
        self.scroll = 0.0;
        self.focused = None;
        self.refreshing = false;
        self.changed = true;
        self.load()
    }

    /// The first card at least partly visible, as an index into the whole result.
    fn first_visible(&self, layout: &Layout) -> u32 {
        let on_page = layout.first_visible(self.scroll, self.mode);
        let on_page = on_page.min(self.cards().len().saturating_sub(1));
        (self.page - 1) * self.page_size + on_page as u32
    }

    /// Show the list in `mode`, keeping the first visible card in view.
    pub fn set_mode(&mut self, mode: Mode, layout: &Layout) -> Effect {
        let first = layout.first_visible(self.scroll, self.mode);
        self.mode = mode;
        self.scroll = layout.row_top(first, mode);
        self.scroll_to(self.scroll)
    }

    /// Move the keyboard's card to `index`, keeping it in view.
    pub fn focus(&mut self, index: usize, layout: &Layout) -> Vec<Effect> {
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

    /// The grid's keys: moves, page edges, pages, and opening the focused card.
    pub fn grid(&mut self, action: GridAction, layout: &Layout) -> Step<BrowseUp> {
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
            GridAction::Open { index, new_tab } => self.open(index, new_tab),
        }
    }

    /// Open card `index` (in a new tab with `new_tab`).
    pub fn open(&mut self, index: usize, new_tab: bool) -> Step<BrowseUp> {
        let Some(card) = self.cards().get(index) else {
            return Step::none();
        };
        let up = BrowseUp::Open {
            id: K::card_id(card).to_owned(),
            title: K::card_title(card).to_owned(),
            new_tab,
        };
        self.focused = Some(index);
        Step::up(up)
    }

    pub fn update(
        &mut self,
        msg: BrowseMsg<K::Sort>,
        tab: TabId,
        layout: &Layout,
    ) -> Step<BrowseUp> {
        self.tab = tab;
        match msg {
            BrowseMsg::Page(page) => Step::effects(self.go_to_page(page)),
            BrowseMsg::GoToInput(text) => {
                self.go_to = text.chars().filter(char::is_ascii_digit).take(7).collect();
                Step::none()
            }
            BrowseMsg::GoToSubmit => {
                let target = self.go_to.parse::<u32>().ok();
                self.go_to.clear();
                match target {
                    Some(page) => Step::effects(self.go_to_page(page)),
                    None => Step::none(),
                }
            }
            BrowseMsg::PageSize(size) => {
                if !PAGE_SIZES.contains(&size) || size == self.page_size {
                    return Step::none();
                }
                // Keep the first visible card on screen (005).
                let first = self.first_visible(layout);
                self.page_size = size;
                self.page = page_of(first, size);
                let on_page = (first - (self.page - 1) * size) as usize;
                self.scroll = layout.row_top(on_page, self.mode);
                self.focused = None;
                self.refreshing = false;
                self.changed = true;
                Step::effects(self.load())
            }
            BrowseMsg::Sort(sort) => {
                if sort == self.query.sort {
                    return Step::none();
                }
                self.query.sort = sort;
                if sort == K::Sort::RANDOM && self.query.seed.is_none() {
                    self.query.seed = Some(new_seed());
                }
                Step::effects(self.requery())
            }
            BrowseMsg::ToggleDirection => {
                self.query.direction = match self.query.direction {
                    SortDirection::Asc => SortDirection::Desc,
                    SortDirection::Desc => SortDirection::Asc,
                };
                Step::effects(self.requery())
            }
            BrowseMsg::Reshuffle => {
                let old = self.query.seed;
                while self.query.seed == old {
                    self.query.seed = Some(new_seed());
                }
                Step::effects(self.requery())
            }
            BrowseMsg::Mode(mode) => {
                if mode == self.mode {
                    return Step::none();
                }
                Step::effect(self.set_mode(mode, layout))
            }
            BrowseMsg::Scrolled { y } => {
                self.scroll = y;
                Step::none()
            }
            BrowseMsg::Focus(index) => Step::effects(self.focus(index, layout)),
            BrowseMsg::Grid(action) => self.grid(action, layout),
            BrowseMsg::Open { index, new_tab } => self.open(index, new_tab),
            // The session handles these for the preview (it needs no list state).
            BrowseMsg::Hover(_) | BrowseMsg::HoverEnd(_) => Step::none(),
            BrowseMsg::AutoScroll(auto) => {
                self.auto_scroll = auto;
                Step::none()
            }
            BrowseMsg::AutoScrolled(_) => {
                self.auto_scroll = None;
                Step::none()
            }
            BrowseMsg::Retry => {
                self.refreshing = false;
                Step::effects(self.load())
            }
            BrowseMsg::SearchInput(text) => {
                self.search_draft = Some(text);
                let generation = next_generation();
                self.search_pending = Some(generation);
                Step::effect(Effect::SearchDue {
                    tab: self.tab,
                    generation,
                    after: SEARCH_DELAY,
                })
            }
            BrowseMsg::SearchSubmit => Step::effects(self.run_search()),
            BrowseMsg::FilterMenu(open) => {
                self.panel = if open {
                    Panel::Menu { highlight: 0 }
                } else {
                    Panel::None
                };
                Step::none()
            }
            BrowseMsg::MenuMove(delta) => {
                match &mut self.panel {
                    Panel::Menu { highlight } | Panel::Saved { highlight } => {
                        *highlight = highlight.saturating_add_signed(delta as isize);
                    }
                    Panel::Editor(editor) => editor.update(EditorMsg::PickerMove(delta)),
                    Panel::None => {}
                }
                Step::none()
            }
            BrowseMsg::Add(kind) => {
                if kind.info().is_placeholder() || !kind.applies_to(K::ENTITY) {
                    return Step::none();
                }
                let existing = self
                    .query
                    .criteria
                    .iter()
                    .find(|c| c.kind == kind && !kind.info().repeatable())
                    .map(Editor::from_criterion);
                self.open_editor(existing.unwrap_or_else(|| Editor::new(kind)))
            }
            BrowseMsg::Edit(key) => {
                let Some(c) = self.query.criteria.iter().find(|c| c.sort_key() == key) else {
                    return Step::none();
                };
                if c.info().is_placeholder() {
                    return Step::none();
                }
                let editor = Editor::from_criterion(c);
                self.open_editor(editor)
            }
            BrowseMsg::Remove(key) => {
                let before = self.query.criteria.len();
                self.query.remove_criterion(&key);
                if self.query.criteria.len() == before {
                    return Step::none();
                }
                if matches!(&self.panel, Panel::Editor(e) if e.replacing.as_deref() == Some(key.as_str()))
                {
                    self.panel = Panel::None;
                }
                Step::effects(self.criteria_changed())
            }
            BrowseMsg::ClearFilters => {
                if self.query.criteria.is_empty() && self.query.search.is_empty() {
                    return Step::none();
                }
                self.query.criteria.clear();
                self.query.search.clear();
                self.search_draft = None;
                self.search_pending = None;
                self.panel = Panel::None;
                Step::effects(self.criteria_changed())
            }
            BrowseMsg::Editor(m) => self.editor_msg(m),
            BrowseMsg::SavedMenu(open) => {
                if open {
                    self.panel = Panel::Saved { highlight: 0 };
                    Step::up(BrowseUp::Need(K::saved_filters()))
                } else {
                    self.panel = Panel::None;
                    Step::none()
                }
            }
            BrowseMsg::ApplySaved(filter) => {
                self.adopt(&filter);
                self.new_query = true;
                self.panel = Panel::None;
                self.page = 1;
                self.scroll = 0.0;
                self.focused = None;
                self.refreshing = false;
                self.changed = true;
                Step::effects(self.load())
            }
            BrowseMsg::DismissLeftOut => {
                self.left_out.clear();
                Step::none()
            }
            BrowseMsg::FilterTo(criterion) => {
                self.query.criteria = vec![criterion];
                self.query.search.clear();
                self.search_draft = None;
                Step::effects(self.criteria_changed())
            }
        }
    }

    /// A page arrived. `waiting`: the session is still connecting, so a failure isn't final.
    /// On success the page is showing; the section then does its own work and calls
    /// [`List::after_arrival`] for the scroll and the neighbours' prefetch.
    pub fn arrive(
        &mut self,
        generation: u64,
        result: Result<K::Page, AppError>,
        waiting: bool,
    ) -> Arrival {
        if self.pending != Some(generation) {
            return Arrival::NotOurs;
        }
        self.pending = None;
        let refreshing = std::mem::take(&mut self.refreshing);
        match result {
            Ok(page) => {
                let (count, number, cards) = K::page_parts(page);
                self.page = number;
                if let Some(f) = self.focused {
                    self.focused = (!cards.is_empty()).then(|| f.min(cards.len() - 1));
                }
                self.data = Data::Ready { count, cards };
                self.loaded = generation;
                Arrival::Arrived { refreshing }
            }
            Err(e) => {
                tracing::debug!(error = %e, page = self.page, entity = ?K::ENTITY, "page unavailable");
                if !refreshing {
                    self.data = if waiting {
                        Data::Loading
                    } else {
                        Data::Unreachable
                    };
                }
                Arrival::Failed
            }
        }
    }

    /// After a page arrived: back to the scroll position (unless it refreshed in place), and the
    /// neighbouring pages prefetched with the next page's thumbnails warmed.
    pub fn after_arrival(&self, refreshing: bool) -> Vec<Effect> {
        let mut effects = Vec::new();
        if !refreshing {
            effects.push(self.scroll_to(self.scroll));
        }
        let last = self.last_page().unwrap_or(self.page);
        let next = (self.page < last).then_some(self.page + 1);
        let pages: Vec<u32> = next
            .into_iter()
            .chain((self.page > 1).then(|| self.page - 1))
            .collect();
        if !pages.is_empty() {
            effects.push(K::prefetch(
                self.query.normalized(),
                pages,
                self.page_size,
                next,
            ));
        }
        effects
    }

    /// A page arrived, for a list with nothing of its own to do on arrival.
    pub fn page_loaded(
        &mut self,
        generation: u64,
        result: Result<K::Page, AppError>,
        waiting: bool,
    ) -> Vec<Effect> {
        match self.arrive(generation, result, waiting) {
            Arrival::Arrived { refreshing } => self.after_arrival(refreshing),
            Arrival::NotOurs | Arrival::Failed => Vec::new(),
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
