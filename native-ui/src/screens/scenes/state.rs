//! The Scenes screen's state and transitions (007 T036; data model "Screen"; 005's paging rules,
//! behaviour that stays B3). One request per page through the core's cache; when a page arrives,
//! its neighbours are prefetched and the next page's thumbnails warmed, so moving there is instant.

use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use stash_core::playback::playlist::{self, Command, Event, Playlist, Repeat, Settings};
use stash_core::scenes::paging::{last_page, page_of, PAGE_SIZES};
use stash_core::scenes::query::{SceneQuery, SceneSort, SortDirection};
use stash_core::scenes::{SceneCard, ScenePage};
use stash_core::AppError;

use stash_core::scenes::filter::kinds::CriterionKind;
use stash_core::scenes::filter::saved::{DisplayMode, SavedFilter};
use stash_core::scenes::filter::Criterion;

use super::editor::{Editor, EditorMsg};
use super::keyboard::{GridAction, PageDirection};
use super::layout::{Layout, QueueFit};
use super::queue::{PendingPlay, QueueFocus, QueueState, Via};
use crate::effects::Effect;
use crate::library::Need;
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

/// How long after the last keystroke a search runs (008 data model "Search").
pub const SEARCH_DELAY: std::time::Duration = std::time::Duration::from_millis(300);

/// The search box's widget id.
pub const SEARCH_ID: &str = "scenes-search";

/// The picker's search field's widget id.
pub const PICKER_ID: &str = "scenes-picker";

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
    /// The saved filter applied to this tab, if the criteria haven't changed since.
    #[serde(default)]
    pub applied: Option<Applied>,
    /// The queue view's selected scene (010 FR-007): after a restore, that card if it's on the
    /// page, otherwise the first.
    #[serde(default)]
    pub queue_selected: Option<String>,
    /// Autoplay (011 FR-004, YouTube's meaning): the queue starts on its own and moves on when a
    /// scene ends; on by default. Reads 010's Play next.
    #[serde(default = "yes", alias = "play_next")]
    pub autoplay: bool,
    /// Repeat: Off, All, One (011 FR-006).
    #[serde(default)]
    pub repeat: Repeat,
    /// Shuffle (011 FR-007).
    #[serde(default)]
    pub shuffle: bool,
    /// The playlist machine's state for this queue (011 R1): rebuilt when a page arrives, so a
    /// restored tab starts a new round.
    #[serde(skip)]
    pub playlist: Playlist,
    /// The query changed: the arriving page is a new result (a new shuffled round).
    #[serde(skip)]
    new_results: bool,
    /// The queue view's selection and panel (010).
    #[serde(skip)]
    pub queue: QueueState,
    #[serde(skip)]
    pub data: Data,
    /// What's typed in the search box, before the debounce runs it (`None`: the query's search).
    #[serde(skip)]
    pub search_draft: Option<String>,
    /// The debounce waiting to run the search.
    #[serde(skip)]
    search_pending: Option<u64>,
    #[serde(skip)]
    pub panel: Panel,
    /// Parts of the last applied saved filter the viewer couldn't apply (FR-005).
    #[serde(skip)]
    pub left_out: Vec<String>,
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
    /// A card to play now that its page arrived (Play next across pages), for the session.
    #[serde(skip)]
    play_due: Option<String>,
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
            applied: None,
            queue_selected: None,
            autoplay: true,
            repeat: Repeat::Off,
            shuffle: false,
            playlist: Playlist::default(),
            new_results: false,
            queue: QueueState::default(),
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
            play_due: None,
        }
    }
}

fn cards_of(data: &Data) -> &[SceneCard] {
    match data {
        Data::Ready { cards, .. } => cards,
        _ => &[],
    }
}

fn yes() -> bool {
    true
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
    /// A queue row was clicked: select it, loading its details at once (010 FR-004).
    QueueSelect(usize),
    /// A queue row was double-clicked: select and play it (010 FR-008).
    QueuePlay(usize),
    /// F6: move the keyboard between the queue's list and panel (010 R5).
    QueueFocus,
    /// Turn Autoplay on or off (011 FR-004).
    ToggleAutoplay,
    /// Turn Shuffle on or off (011 FR-007).
    ToggleShuffle,
    /// Repeat: Off → All → One → Off (011 FR-006).
    CycleRepeat,
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
    ApplySaved(Box<SavedFilter>),
    DismissLeftOut,
    /// Show scenes matching exactly this criterion (from a performer, tag, or studio chip).
    FilterTo(Criterion),
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
    /// Something from the library (a picker's list, the vocabulary, the saved filters).
    Need(Need),
    /// Play this scene, owned by this tab (the queue view, 010).
    Play(String),
}

/// What the session does with the playlist machine's command (011 R2).
#[derive(Debug)]
pub enum Advance {
    /// Play this card now (it's selected).
    Play { id: String, effects: Vec<Effect> },
    /// The scene's page is loading; the scene plays when it arrives.
    Loading(Vec<Effect>),
    /// Play this scene again from the beginning, as a new viewing (Repeat one, 011 R4).
    Replay { id: String },
    /// Nothing to play.
    Stop,
}

fn new_seed() -> u32 {
    let bytes = uuid::Uuid::new_v4().into_bytes();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

/// A shuffle round's seed (the machine takes randomness only as input).
fn new_seed64() -> u64 {
    let bytes = uuid::Uuid::new_v4().into_bytes();
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
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
            applied: self.applied.clone(),
            queue_selected: self.queue_selected.clone(),
            autoplay: self.autoplay,
            repeat: self.repeat,
            shuffle: self.shuffle,
            ..Self::default()
        }
    }

    /// A fresh screen showing a saved filter (the server's default for new tabs).
    pub fn from_saved(filter: &SavedFilter) -> Self {
        let mut s = Self::default();
        s.adopt(filter);
        s
    }

    /// Take a saved filter's search, criteria, sort, page size, and mode.
    fn adopt(&mut self, filter: &SavedFilter) {
        self.query = filter.query.clone();
        if self.query.sort == SceneSort::Random && self.query.seed.is_none() {
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
            Some(DisplayMode::List) => self.mode = Mode::List,
            Some(DisplayMode::Wall) => self.mode = Mode::Wall,
            // The tagger is Phase 3.
            Some(DisplayMode::Tagger) | None => {}
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

    /// Whether this screen waits for search debounce `generation`.
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
    fn open_editor(&mut self, editor: Editor) -> Step<ScenesUp> {
        let need = editor.picker().map(Need::Choices);
        let picker = editor.picker().is_some();
        self.panel = Panel::Editor(Box::new(editor));
        let mut step = match need {
            Some(n) => Step::up(ScenesUp::Need(n)),
            None => Step::none(),
        };
        if picker {
            step.effects.push(Effect::Focus(PICKER_ID));
        }
        step
    }

    fn editor_msg(&mut self, msg: EditorMsg) -> Step<ScenesUp> {
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
        self.results_changed();
        self.load()
    }

    /// The query changed: back to page 1.
    fn requery(&mut self) -> Vec<Effect> {
        self.new_results = true;
        self.page = 1;
        self.scroll = 0.0;
        self.focused = None;
        self.refreshing = false;
        self.results_changed();
        self.load()
    }

    /// The search, a filter, the sort, the page size, or the page changed: the queue's first
    /// scene will be selected when the new page arrives (010 FR-006), and starts there with
    /// Autoplay on (011 FR-004).
    fn results_changed(&mut self) {
        self.queue.clear();
        self.queue.pending_play = if self.mode == Mode::Queue {
            PendingPlay::Start
        } else {
            PendingPlay::None
        };
        self.play_due = None;
        self.queue_selected = None;
    }

    /// The tab's saved playlist controls.
    fn settings(&self) -> Settings {
        Settings {
            autoplay: self.autoplay,
            repeat: self.repeat,
            shuffle: self.shuffle,
        }
    }

    /// Step the playlist machine (011 R1); the saved controls follow its settings.
    fn machine(&mut self, event: Event) -> Command {
        let current = Playlist {
            settings: self.settings(),
            ..self.playlist
        };
        let (next, command) = playlist::step(&current, event);
        self.playlist = next;
        Settings {
            autoplay: self.autoplay,
            repeat: self.repeat,
            shuffle: self.shuffle,
        } = next.settings;
        command
    }

    /// Card `local` of this page as an index into the whole result.
    fn global(&self, local: usize) -> u32 {
        (self.page - 1) * self.page_size + local as u32
    }

    /// The card on this page for result index `i`.
    fn local(&self, i: u32) -> Option<usize> {
        let on_page = i.checked_sub((self.page - 1) * self.page_size)? as usize;
        (on_page < self.cards().len()).then_some(on_page)
    }

    /// Tell the machine what the player holds: this tab's playing scene (the session keeps
    /// `queue.playing` current), or nothing, so the queue continues from the selection.
    fn sync_playing(&mut self) {
        match self.queue.playing.clone() {
            None if self.playlist.live => {
                let at = self.queue.selected.map(|i| self.global(i));
                self.machine(Event::Idle(at));
            }
            Some(id) => {
                let at = self.cards().iter().position(|c| c.id == id);
                if let Some(i) = at.map(|l| self.global(l)) {
                    if self.playlist.playing != Some(i) || !self.playlist.live {
                        self.machine(Event::Started(i));
                    }
                }
            }
            None => {}
        }
    }

    /// The selected card, if `event` about it (`Chosen` or `Autostart`) makes the machine play it.
    fn selection_plays(&mut self, event: fn(u32) -> Event) -> Option<String> {
        let i = self.queue.selected?;
        self.sync_playing();
        match self.machine(event(self.global(i))) {
            Command::Play(_) => self.cards().get(i).map(|c| c.id.clone()),
            _ => None,
        }
    }

    /// The viewer chose the selected card (a click, Enter, the keyboard's rest, 011 R6): the scene
    /// to play, unless it's playing already.
    fn chosen(&mut self) -> Option<String> {
        self.selection_plays(Event::Chosen)
    }

    /// The queue may start on its own at the selection (choosing Queue, new results, a switch to
    /// the tab): the scene to play, with Autoplay on (011 R5).
    pub fn autostart(&mut self) -> Option<String> {
        if self.mode != Mode::Queue {
            return None;
        }
        self.selection_plays(Event::Autostart)
    }

    /// A playlist event from the session (Ended, Skipped, Next, Previous): what to do.
    pub fn queue_event(&mut self, event: Event, tab: TabId, layout: &Layout) -> Advance {
        self.tab = tab;
        if self.mode != Mode::Queue || !self.ready() {
            return Advance::Stop;
        }
        self.sync_playing();
        let command = self.machine(event);
        self.apply(command, layout)
    }

    /// Carry out the machine's command: select and play a card on this page, or load the page
    /// that has it (it plays when it arrives, 011 R2).
    fn apply(&mut self, command: Command, layout: &Layout) -> Advance {
        match command {
            Command::Play(i) => match self.local(i) {
                Some(card) => self.queue_play(card, layout),
                None => {
                    let effects = self.go_to_page(i / self.page_size + 1);
                    self.queue.pending_play = PendingPlay::Index(i);
                    Advance::Loading(effects)
                }
            },
            Command::Replay => {
                let id = self
                    .playlist
                    .playing
                    .and_then(|i| self.local(i))
                    .and_then(|l| self.cards().get(l))
                    .map(|c| c.id.clone())
                    .or_else(|| self.queue.playing.clone());
                match id {
                    Some(id) => Advance::Replay { id },
                    None => Advance::Stop,
                }
            }
            Command::Stop | Command::Nothing => Advance::Stop,
        }
    }

    /// A scene began playing in this tab: the machine's position follows it.
    pub fn queue_started(&mut self, id: &str) {
        if let Some(i) = self.cards().iter().position(|c| c.id == id) {
            let i = self.global(i);
            self.machine(Event::Started(i));
        }
    }

    /// The playing scene failed: nothing moves (011 FR-010).
    pub fn queue_failed(&mut self) {
        self.machine(Event::Failed);
    }

    /// Select queue row `index`: the panel follows and the row is kept in view. With no room for
    /// the panel, a click opens the scene's page instead (010 R7).
    fn queue_select(&mut self, index: usize, via: Via, layout: &Layout) -> Step<ScenesUp> {
        if via == Via::Pointer && layout.queue_fit() == QueueFit::ListOnly {
            return self.update(
                ScenesMsg::Open {
                    index,
                    new_tab: false,
                },
                self.tab,
                layout,
            );
        }
        Step::effects(self.queue_select_here(index, via, layout))
    }

    /// Select queue row `index` in the list, keeping it in view.
    fn queue_select_here(&mut self, index: usize, via: Via, layout: &Layout) -> Vec<Effect> {
        let mut effects = self
            .queue
            .select(index, via, cards_of(&self.data), self.tab);
        self.sync_queue_selection();
        if let Some(i) = self.queue.selected {
            if let Some(y) = layout.reveal(i, self.scroll, self.mode) {
                self.scroll = y;
                effects.push(self.scroll_to(y));
            }
        }
        effects
    }

    /// Play card `index` from the queue: it's selected (its details load) and plays.
    fn queue_play(&mut self, index: usize, layout: &Layout) -> Advance {
        let effects = self.queue_select_here(index, Via::Pointer, layout);
        match self.queue.selected.and_then(|i| self.cards().get(i)) {
            Some(card) => Advance::Play {
                id: card.id.clone(),
                effects,
            },
            None => Advance::Stop,
        }
    }

    /// The queue's keyboard crossed a page edge: the arriving page's card is a keyboard selection
    /// (it plays once it rests), not the new results' first scene.
    fn keyboard_edge(&mut self) {
        if self.mode == Mode::Queue {
            self.queue.pending_play = PendingPlay::Keyboard;
        }
    }

    /// The card to play now that its page arrived, once.
    pub fn take_play_due(&mut self) -> Option<String> {
        self.play_due.take()
    }

    /// The keyboard's card and the saved id follow the queue's selection.
    fn sync_queue_selection(&mut self) {
        self.focused = self.queue.selected;
        self.queue_selected = self
            .queue
            .selected
            .and_then(|i| self.cards().get(i))
            .map(|c| c.id.clone());
    }

    /// Which card the queue selects when a page shows: the saved one if it's on the page, else
    /// the keyboard's (a page edge), else the first.
    fn queue_start(&self) -> usize {
        self.queue_selected
            .as_deref()
            .and_then(|id| self.cards().iter().position(|c| c.id == id))
            .or(self.focused)
            .unwrap_or(0)
    }

    /// A rest for the queue's selection came due.
    /// Its effects, and the scene to play when a keyboard move started it (011 R2).
    pub fn queue_rest_due(&mut self, generation: u64, tab: TabId) -> (Vec<Effect>, Option<String>) {
        self.tab = tab;
        let (effects, plays) = self.queue.rest_due(generation, tab);
        let play = if plays { self.chosen() } else { None };
        (effects, play)
    }

    /// Whether this screen's queue waits for rest `generation`.
    pub fn awaits_queue_rest(&self, generation: u64) -> bool {
        self.mode == Mode::Queue && self.queue.awaits_rest(generation)
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
                self.results_changed();
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
                let leaving = self.mode == Mode::Queue;
                self.mode = mode;
                self.scroll = layout.row_top(first, mode);
                let mut step = Step::effect(self.scroll_to(self.scroll));
                if mode == Mode::Queue {
                    // Choosing Queue loads the selection at once and, with Autoplay on, plays it
                    // (011 FR-004).
                    if !self.cards().is_empty() {
                        let start = self.queue_start();
                        let more = self.queue_select(start, Via::Pointer, layout);
                        step.effects.extend(more.effects);
                        step.up = more.up.or_else(|| self.autostart().map(ScenesUp::Play));
                    } else if !self.ready() {
                        self.queue.pending_play = PendingPlay::Start;
                    }
                } else if leaving {
                    self.queue.clear();
                }
                step
            }
            // 011 R6: a click plays the row (the playing row: selects only).
            ScenesMsg::QueueSelect(index) if self.mode == Mode::Queue => {
                let step = self.queue_select(index, Via::Pointer, layout);
                match step.up {
                    Some(up) => Step::up(up).with_all(step.effects),
                    None => Step {
                        effects: step.effects,
                        up: self.chosen().map(ScenesUp::Play),
                    },
                }
            }
            ScenesMsg::QueueSelect(_) => Step::none(),
            ScenesMsg::QueuePlay(index) if self.mode == Mode::Queue => {
                let effects = self.queue_select_here(index, Via::Pointer, layout);
                Step {
                    effects,
                    up: self.chosen().map(ScenesUp::Play),
                }
            }
            ScenesMsg::QueuePlay(_) => Step::none(),
            ScenesMsg::QueueFocus => {
                self.queue.focus = match self.queue.focus {
                    QueueFocus::List => QueueFocus::Panel,
                    QueueFocus::Panel => QueueFocus::List,
                };
                Step::none()
            }
            ScenesMsg::ToggleAutoplay => {
                self.machine(Event::SetAutoplay(!self.autoplay));
                Step::none()
            }
            ScenesMsg::ToggleShuffle => {
                // Turning it on starts a round from the playing scene; off continues in list
                // order (011 FR-007).
                self.sync_playing();
                self.machine(Event::ToggleShuffle { seed: new_seed64() });
                Step::none()
            }
            ScenesMsg::CycleRepeat => {
                self.machine(Event::CycleRepeat);
                Step::none()
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
            // The session handles these for the preview (it needs no screen state).
            ScenesMsg::Hover(_) | ScenesMsg::HoverEnd(_) => Step::none(),
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
            ScenesMsg::SearchInput(text) => {
                self.search_draft = Some(text);
                let generation = next_generation();
                self.search_pending = Some(generation);
                Step::effect(Effect::SearchDue {
                    tab: self.tab,
                    generation,
                    after: SEARCH_DELAY,
                })
            }
            ScenesMsg::SearchSubmit => Step::effects(self.run_search()),
            ScenesMsg::FilterMenu(open) => {
                self.panel = if open {
                    Panel::Menu { highlight: 0 }
                } else {
                    Panel::None
                };
                Step::none()
            }
            ScenesMsg::MenuMove(delta) => {
                match &mut self.panel {
                    Panel::Menu { highlight } | Panel::Saved { highlight } => {
                        *highlight = highlight.saturating_add_signed(delta as isize);
                    }
                    Panel::Editor(editor) => editor.update(EditorMsg::PickerMove(delta)),
                    Panel::None => {}
                }
                Step::none()
            }
            ScenesMsg::Add(kind) => {
                if kind.info().is_placeholder() {
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
            ScenesMsg::Edit(key) => {
                let Some(c) = self.query.criteria.iter().find(|c| c.sort_key() == key) else {
                    return Step::none();
                };
                if c.info().is_placeholder() {
                    return Step::none();
                }
                let editor = Editor::from_criterion(c);
                self.open_editor(editor)
            }
            ScenesMsg::Remove(key) => {
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
            ScenesMsg::ClearFilters => {
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
            ScenesMsg::Editor(m) => self.editor_msg(m),
            ScenesMsg::SavedMenu(open) => {
                if open {
                    self.panel = Panel::Saved { highlight: 0 };
                    Step::up(ScenesUp::Need(Need::SavedFilters))
                } else {
                    self.panel = Panel::None;
                    Step::none()
                }
            }
            ScenesMsg::ApplySaved(filter) => {
                self.adopt(&filter);
                self.new_results = true;
                self.panel = Panel::None;
                self.page = 1;
                self.scroll = 0.0;
                self.focused = None;
                self.refreshing = false;
                self.results_changed();
                Step::effects(self.load())
            }
            ScenesMsg::DismissLeftOut => {
                self.left_out.clear();
                Step::none()
            }
            ScenesMsg::FilterTo(criterion) => {
                self.query.criteria = vec![criterion];
                self.query.search.clear();
                self.search_draft = None;
                Step::effects(self.criteria_changed())
            }
        }
    }

    fn focus(&mut self, index: usize, layout: &Layout) -> Vec<Effect> {
        let count = self.cards().len();
        if count == 0 {
            return Vec::new();
        }
        if self.mode == Mode::Queue {
            // The list's arrows move the queue's selection (010 R3).
            return self.queue_select(index, Via::Keyboard, layout).effects;
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
                self.keyboard_edge();
                Step::effects(effects)
            }
            GridAction::Edge(PageDirection::Previous) if self.page > 1 => {
                let effects = self.go_to_page(self.page - 1);
                self.focused = Some(self.page_size as usize - 1);
                self.keyboard_edge();
                Step::effects(effects)
            }
            GridAction::Edge(_) => Step::none(),
            GridAction::Page(PageDirection::Next) => Step::effects(self.go_to_page(self.page + 1)),
            GridAction::Page(PageDirection::Previous) => {
                Step::effects(self.go_to_page(self.page.saturating_sub(1)))
            }
            // Enter plays the queue's selection (010 R5); Ctrl+Enter still opens a new tab.
            GridAction::Open {
                index,
                new_tab: false,
            } if self.mode == Mode::Queue => {
                self.update(ScenesMsg::QueuePlay(index), self.tab, layout)
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
                // The machine follows the result: a new query or a new size starts it over (a new
                // shuffled round, 011 FR-007).
                if std::mem::take(&mut self.new_results) || self.playlist.count != page.count {
                    self.machine(Event::ResultsChanged {
                        count: page.count,
                        seed: new_seed64(),
                    });
                }
                if let Some(f) = self.focused {
                    self.focused = (!page.items.is_empty()).then(|| f.min(page.items.len() - 1));
                }
                self.data = Data::Ready {
                    count: page.count,
                    cards: page.items,
                };
                self.loaded = generation;
                let mut effects = Vec::new();
                if self.mode == Mode::Queue {
                    if self.cards().is_empty() {
                        self.queue.clear();
                    } else {
                        // The machine's scene is on this page (Next, Previous, a shuffled pick,
                        // Repeat all), or the queue may start here.
                        let pending = std::mem::take(&mut self.queue.pending_play);
                        let wanted = match pending {
                            PendingPlay::Index(i) => self.local(i),
                            _ => None,
                        };
                        let (start, via) = match pending {
                            PendingPlay::Index(_) | PendingPlay::Start => {
                                (wanted.unwrap_or_else(|| self.queue_start()), Via::Pointer)
                            }
                            PendingPlay::Keyboard => (self.queue_start(), Via::Keyboard),
                            // A restore or a refresh: load only, never play (011 R6).
                            PendingPlay::None => (self.queue_start(), Via::Arrival),
                        };
                        effects.extend(self.queue.select(
                            start,
                            via,
                            cards_of(&self.data),
                            self.tab,
                        ));
                        self.sync_queue_selection();
                        self.play_due = match pending {
                            // The machine already chose it.
                            PendingPlay::Index(_) if wanted.is_some() => self
                                .queue_selected
                                .clone()
                                .filter(|id| self.queue.playing.as_deref() != Some(id)),
                            PendingPlay::Start => self.autostart(),
                            _ => None,
                        };
                    }
                }
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
