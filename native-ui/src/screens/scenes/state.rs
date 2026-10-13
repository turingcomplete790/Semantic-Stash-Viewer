//! The Scenes screen's state and transitions (007 T036; data model "Screen"; 005's paging rules,
//! behaviour that stays B3). Since 012 the list itself (query, page, filters, search, saved
//! filters) is the browse layer's [`List`]; this adds what is only Scenes': the queue view (010)
//! and its playlist machine (011).

use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};
use stash_core::browse::entity::Entity;
use stash_core::playback::playlist::{self, Command, Event, Playlist, Repeat, Settings};
use stash_core::scenes::filter::kinds::CriterionKind;
use stash_core::scenes::filter::saved::SavedFilter;
use stash_core::scenes::filter::Criterion;
use stash_core::scenes::query::{SceneQuery, SceneSort};
use stash_core::scenes::{SceneCard, ScenePage};
use stash_core::AppError;

use super::editor::EditorMsg;
use super::keyboard::{GridAction, PageDirection};
use super::layout::{Layout, QueueFit};
use super::queue::{PendingPlay, QueueFocus, QueueState, Via};
use crate::effects::Effect;
use crate::library::Need;
use crate::machine::Step;
use crate::screens::browse::state::{
    self as browse, cards_of, Arrival, BrowseMsg, BrowseUp, Kind, List,
};
use crate::screens::Mode;
use crate::shell::TabId;
use crate::widgets::scroll_watch::{AutoScroll, AutoScrolled};

pub use crate::screens::browse::state::{Applied, Panel, PICKER_ID, SEARCH_DELAY, SEARCH_ID};

/// What the Scenes screen has to show (transient).
pub type Data = browse::Data<SceneCard>;

/// Scenes as a browse list.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SceneKind;

impl Kind for SceneKind {
    type Sort = SceneSort;
    type Card = SceneCard;
    type Page = ScenePage;
    const ENTITY: Entity = Entity::Scenes;

    fn card_id(card: &SceneCard) -> &str {
        &card.id
    }

    fn card_title(card: &SceneCard) -> &str {
        &card.title
    }

    fn card_thumb(card: &SceneCard) -> Option<&str> {
        card.thumb.as_deref()
    }

    fn card_view(
        card: &SceneCard,
        index: usize,
        focused: bool,
        marked: bool,
        mode: Mode,
        layout: &Layout,
        pictures: crate::screens::browse::grid::Pictures<'_>,
        msg: crate::screens::browse::controls::Map<Self>,
    ) -> iced::Element<'static, crate::shell::ShellMsg> {
        super::grid::card_view(card, index, focused, marked, mode, layout, pictures, msg)
    }

    fn preview_key(card: &SceneCard) -> Option<String> {
        Some(card.id.clone())
    }

    fn page_parts(page: ScenePage) -> (u32, u32, Vec<SceneCard>) {
        (page.count, page.page, page.items)
    }

    fn load(tab: TabId, generation: u64, query: SceneQuery, page: u32, size: u32) -> Effect {
        Effect::LoadScenesPage {
            tab,
            generation,
            query,
            page,
            size,
        }
    }

    fn prefetch(query: SceneQuery, pages: Vec<u32>, size: u32, warm: Option<u32>) -> Effect {
        Effect::PrefetchScenes {
            query,
            pages,
            size,
            warm,
        }
    }

    fn saved_filters() -> Need {
        Need::SavedFilters
    }

    fn sorts() -> &'static [SceneSort] {
        SceneSort::all()
    }

    fn sort_label(sort: SceneSort) -> &'static str {
        sort.label()
    }

    fn modes() -> &'static [(Mode, &'static str)] {
        &[
            (Mode::Grid, "Grid"),
            (Mode::List, "List"),
            (Mode::Wall, "Wall"),
            (Mode::Queue, "Queue"),
        ]
    }

    fn saved(library: &crate::library::Library) -> &crate::library::Loadable<Vec<SavedFilter>> {
        &library.saved
    }

    const NOUN: &'static str = "scenes";
    const ITEM: &'static str = "scene";
    const SEARCH_PLACEHOLDER: &'static str = "Search scenes";
}

/// The shared views' list messages, as the shell's Scenes messages.
pub fn shell_msg(m: BrowseMsg<SceneSort>) -> crate::shell::ShellMsg {
    crate::shell::ShellMsg::Scenes(m.into())
}

/// The cache key for a page (the demo's, so caches compare): `scenes:q:<hash>:s:<size>:p:<page>`.
pub fn page_key(query: &SceneQuery, page: u32, size: u32) -> String {
    browse::page_key(Entity::Scenes, query, page, size)
}

/// The scrollable's widget id in tab `tab`.
pub fn scroll_id(tab: TabId) -> String {
    browse::scroll_id_for(Entity::Scenes, tab)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenesState {
    /// The list: query, page, size, mode, scroll, focus, applied filter (saved), and its
    /// transient state. Flattened, so the saved form is 011's.
    #[serde(flatten)]
    pub list: List<SceneKind>,
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
    /// The queue view's selection and panel (010).
    #[serde(skip)]
    pub queue: QueueState,
    /// A card to play now that its page arrived (Play next across pages), for the session.
    #[serde(skip)]
    play_due: Option<String>,
}

impl Deref for ScenesState {
    type Target = List<SceneKind>;

    fn deref(&self) -> &List<SceneKind> {
        &self.list
    }
}

impl DerefMut for ScenesState {
    fn deref_mut(&mut self) -> &mut List<SceneKind> {
        &mut self.list
    }
}

impl Default for ScenesState {
    fn default() -> Self {
        Self {
            list: List::default(),
            queue_selected: None,
            autoplay: true,
            repeat: Repeat::Off,
            shuffle: false,
            playlist: Playlist::default(),
            queue: QueueState::default(),
            play_due: None,
        }
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

impl From<BrowseMsg<SceneSort>> for ScenesMsg {
    /// The list's message as Scenes' (the shared views send list messages).
    fn from(m: BrowseMsg<SceneSort>) -> Self {
        use BrowseMsg as B;
        match m {
            B::Page(p) => ScenesMsg::Page(p),
            B::GoToInput(t) => ScenesMsg::GoToInput(t),
            B::GoToSubmit => ScenesMsg::GoToSubmit,
            B::PageSize(s) => ScenesMsg::PageSize(s),
            B::Sort(s) => ScenesMsg::Sort(s),
            B::ToggleDirection => ScenesMsg::ToggleDirection,
            B::Reshuffle => ScenesMsg::Reshuffle,
            B::Mode(m) => ScenesMsg::Mode(m),
            B::Scrolled { y } => ScenesMsg::Scrolled { y },
            B::Focus(i) => ScenesMsg::Focus(i),
            B::Grid(a) => ScenesMsg::Grid(a),
            B::Open { index, new_tab } => ScenesMsg::Open { index, new_tab },
            B::Hover(i) => ScenesMsg::Hover(i),
            B::HoverEnd(i) => ScenesMsg::HoverEnd(i),
            B::Retry => ScenesMsg::Retry,
            B::SearchInput(t) => ScenesMsg::SearchInput(t),
            B::SearchSubmit => ScenesMsg::SearchSubmit,
            B::FilterMenu(o) => ScenesMsg::FilterMenu(o),
            B::MenuMove(d) => ScenesMsg::MenuMove(d),
            B::Add(k) => ScenesMsg::Add(k),
            B::Edit(k) => ScenesMsg::Edit(k),
            B::Remove(k) => ScenesMsg::Remove(k),
            B::ClearFilters => ScenesMsg::ClearFilters,
            B::Editor(e) => ScenesMsg::Editor(e),
            B::SavedMenu(o) => ScenesMsg::SavedMenu(o),
            B::ApplySaved(f) => ScenesMsg::ApplySaved(f),
            B::DismissLeftOut => ScenesMsg::DismissLeftOut,
            B::FilterTo(c) => ScenesMsg::FilterTo(c),
            B::AutoScroll(a) => ScenesMsg::AutoScroll(a),
            B::AutoScrolled(a) => ScenesMsg::AutoScrolled(a),
        }
    }
}

impl ScenesMsg {
    /// The list's part of this message, or the message back when it's Scenes' own.
    fn into_browse(self) -> Result<BrowseMsg<SceneSort>, ScenesMsg> {
        use BrowseMsg as B;
        Ok(match self {
            ScenesMsg::Page(p) => B::Page(p),
            ScenesMsg::GoToInput(t) => B::GoToInput(t),
            ScenesMsg::GoToSubmit => B::GoToSubmit,
            ScenesMsg::PageSize(s) => B::PageSize(s),
            ScenesMsg::Sort(s) => B::Sort(s),
            ScenesMsg::ToggleDirection => B::ToggleDirection,
            ScenesMsg::Reshuffle => B::Reshuffle,
            ScenesMsg::Scrolled { y } => B::Scrolled { y },
            ScenesMsg::Open { index, new_tab } => B::Open { index, new_tab },
            ScenesMsg::Hover(i) => B::Hover(i),
            ScenesMsg::HoverEnd(i) => B::HoverEnd(i),
            ScenesMsg::Retry => B::Retry,
            ScenesMsg::SearchInput(t) => B::SearchInput(t),
            ScenesMsg::SearchSubmit => B::SearchSubmit,
            ScenesMsg::FilterMenu(o) => B::FilterMenu(o),
            ScenesMsg::MenuMove(d) => B::MenuMove(d),
            ScenesMsg::Add(k) => B::Add(k),
            ScenesMsg::Edit(k) => B::Edit(k),
            ScenesMsg::Remove(k) => B::Remove(k),
            ScenesMsg::ClearFilters => B::ClearFilters,
            ScenesMsg::Editor(e) => B::Editor(e),
            ScenesMsg::SavedMenu(o) => B::SavedMenu(o),
            ScenesMsg::ApplySaved(f) => B::ApplySaved(f),
            ScenesMsg::DismissLeftOut => B::DismissLeftOut,
            ScenesMsg::FilterTo(c) => B::FilterTo(c),
            ScenesMsg::AutoScroll(a) => B::AutoScroll(a),
            ScenesMsg::AutoScrolled(a) => B::AutoScrolled(a),
            other => return Err(other),
        })
    }
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

impl From<BrowseUp> for ScenesUp {
    fn from(up: BrowseUp) -> Self {
        match up {
            BrowseUp::Open { id, title, new_tab } => ScenesUp::Open { id, title, new_tab },
            BrowseUp::Need(n) => ScenesUp::Need(n),
        }
    }
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

/// A shuffle round's seed (the machine takes randomness only as input).
fn new_seed64() -> u64 {
    let bytes = uuid::Uuid::new_v4().into_bytes();
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

fn up(step: Step<BrowseUp>) -> Step<ScenesUp> {
    Step {
        effects: step.effects,
        up: step.up.map(ScenesUp::from),
    }
}

impl ScenesState {
    /// A fresh screen at `page` with `page_size` per page.
    pub fn at(page: u32, page_size: u32) -> Self {
        let mut s = Self::default();
        s.list.page = page;
        s.list.page_size = page_size;
        s
    }

    /// Only the saved fields.
    pub fn persistent(&self) -> Self {
        Self {
            list: self.list.persistent(),
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
        s.list.adopt(filter);
        s
    }

    /// After the list changed its results, the queue resets (010 FR-006).
    fn after_list(&mut self) {
        if self.list.take_changed() {
            self.results_changed();
        }
    }

    fn go_to_page(&mut self, page: u32) -> Vec<Effect> {
        let effects = self.list.go_to_page(page);
        self.after_list();
        effects
    }

    /// A debounced search came due.
    pub fn search_due(&mut self, generation: u64) -> Vec<Effect> {
        let effects = self.list.search_due(generation);
        self.after_list();
        effects
    }

    /// The search, a filter, the sort, the page size, or the page changed: the queue's first
    /// scene will be selected when the new page arrives (010 FR-006), and starts there with
    /// Autoplay on (011 FR-004).
    fn results_changed(&mut self) {
        self.queue.clear();
        self.queue.pending_play = if self.list.mode == Mode::Queue {
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
        (self.list.page - 1) * self.list.page_size + local as u32
    }

    /// The card on this page for result index `i`.
    fn local(&self, i: u32) -> Option<usize> {
        let on_page = i.checked_sub((self.list.page - 1) * self.list.page_size)? as usize;
        (on_page < self.list.cards().len()).then_some(on_page)
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
                let at = self.list.cards().iter().position(|c| c.id == id);
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
            Command::Play(_) => self.list.cards().get(i).map(|c| c.id.clone()),
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
        if self.list.mode != Mode::Queue {
            return None;
        }
        self.selection_plays(Event::Autostart)
    }

    /// A playlist event from the session (Ended, Skipped, Next, Previous): what to do.
    pub fn queue_event(&mut self, event: Event, tab: TabId, layout: &Layout) -> Advance {
        self.list.tab = tab;
        if self.list.mode != Mode::Queue || !self.list.ready() {
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
                    let effects = self.go_to_page(i / self.list.page_size + 1);
                    self.queue.pending_play = PendingPlay::Index(i);
                    Advance::Loading(effects)
                }
            },
            Command::Replay => {
                let id = self
                    .playlist
                    .playing
                    .and_then(|i| self.local(i))
                    .and_then(|l| self.list.cards().get(l))
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
        if let Some(i) = self.list.cards().iter().position(|c| c.id == id) {
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
                self.list.tab,
                layout,
            );
        }
        Step::effects(self.queue_select_here(index, via, layout))
    }

    /// Select queue row `index` in the list, keeping it in view.
    fn queue_select_here(&mut self, index: usize, via: Via, layout: &Layout) -> Vec<Effect> {
        let mut effects = self
            .queue
            .select(index, via, cards_of(&self.list.data), self.list.tab);
        self.sync_queue_selection();
        if let Some(i) = self.queue.selected {
            if let Some(y) = layout.reveal(i, self.list.scroll, self.list.mode) {
                self.list.scroll = y;
                effects.push(self.list.scroll_to(y));
            }
        }
        effects
    }

    /// Play card `index` from the queue: it's selected (its details load) and plays.
    fn queue_play(&mut self, index: usize, layout: &Layout) -> Advance {
        let effects = self.queue_select_here(index, Via::Pointer, layout);
        match self.queue.selected.and_then(|i| self.list.cards().get(i)) {
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
        if self.list.mode == Mode::Queue {
            self.queue.pending_play = PendingPlay::Keyboard;
        }
    }

    /// The card to play now that its page arrived, once.
    pub fn take_play_due(&mut self) -> Option<String> {
        self.play_due.take()
    }

    /// The keyboard's card and the saved id follow the queue's selection.
    fn sync_queue_selection(&mut self) {
        self.list.focused = self.queue.selected;
        self.queue_selected = self
            .queue
            .selected
            .and_then(|i| self.list.cards().get(i))
            .map(|c| c.id.clone());
    }

    /// Which card the queue selects when a page shows: the saved one if it's on the page, else
    /// the keyboard's (a page edge), else the first.
    fn queue_start(&self) -> usize {
        self.queue_selected
            .as_deref()
            .and_then(|id| self.list.cards().iter().position(|c| c.id == id))
            .or(self.list.focused)
            .unwrap_or(0)
    }

    /// A rest for the queue's selection came due.
    /// Its effects, and the scene to play when a keyboard move started it (011 R2).
    pub fn queue_rest_due(&mut self, generation: u64, tab: TabId) -> (Vec<Effect>, Option<String>) {
        self.list.tab = tab;
        let (effects, plays) = self.queue.rest_due(generation, tab);
        let play = if plays { self.chosen() } else { None };
        (effects, play)
    }

    /// Whether this screen's queue waits for rest `generation`.
    pub fn awaits_queue_rest(&self, generation: u64) -> bool {
        self.list.mode == Mode::Queue && self.queue.awaits_rest(generation)
    }

    pub fn update(&mut self, msg: ScenesMsg, tab: TabId, layout: &Layout) -> Step<ScenesUp> {
        self.list.tab = tab;
        match msg {
            ScenesMsg::Mode(mode) => {
                if mode == self.list.mode {
                    return Step::none();
                }
                let leaving = self.list.mode == Mode::Queue;
                let mut step = Step::effect(self.list.set_mode(mode, layout));
                if mode == Mode::Queue {
                    // Choosing Queue loads the selection at once and, with Autoplay on, plays it
                    // (011 FR-004).
                    if !self.list.cards().is_empty() {
                        let start = self.queue_start();
                        let more = self.queue_select(start, Via::Pointer, layout);
                        step.effects.extend(more.effects);
                        step.up = more.up.or_else(|| self.autostart().map(ScenesUp::Play));
                    } else if !self.list.ready() {
                        self.queue.pending_play = PendingPlay::Start;
                    }
                } else if leaving {
                    self.queue.clear();
                }
                step
            }
            // 011 R6: a click plays the row (the playing row: selects only).
            ScenesMsg::QueueSelect(index) if self.list.mode == Mode::Queue => {
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
            ScenesMsg::QueuePlay(index) if self.list.mode == Mode::Queue => {
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
            ScenesMsg::Focus(index) => Step::effects(self.focus(index, layout)),
            ScenesMsg::Grid(action) => self.grid(action, layout),
            other => match other.into_browse() {
                Ok(m) => {
                    let step = up(self.list.update(m, tab, layout));
                    self.after_list();
                    step
                }
                Err(_) => Step::none(),
            },
        }
    }

    fn focus(&mut self, index: usize, layout: &Layout) -> Vec<Effect> {
        if self.list.cards().is_empty() {
            return Vec::new();
        }
        if self.list.mode == Mode::Queue {
            // The list's arrows move the queue's selection (010 R3).
            return self.queue_select(index, Via::Keyboard, layout).effects;
        }
        self.list.focus(index, layout)
    }

    fn grid(&mut self, action: GridAction, layout: &Layout) -> Step<ScenesUp> {
        let last = self.list.last_page().unwrap_or(self.list.page);
        match action {
            GridAction::Move(index) => Step::effects(self.focus(index, layout)),
            GridAction::Edge(PageDirection::Next) if self.list.page < last => {
                let effects = self.go_to_page(self.list.page + 1);
                self.list.focused = Some(0);
                self.keyboard_edge();
                Step::effects(effects)
            }
            GridAction::Edge(PageDirection::Previous) if self.list.page > 1 => {
                let effects = self.go_to_page(self.list.page - 1);
                self.list.focused = Some(self.list.page_size as usize - 1);
                self.keyboard_edge();
                Step::effects(effects)
            }
            // Enter plays the queue's selection (010 R5); Ctrl+Enter still opens a new tab.
            GridAction::Open {
                index,
                new_tab: false,
            } if self.list.mode == Mode::Queue => {
                self.update(ScenesMsg::QueuePlay(index), self.list.tab, layout)
            }
            other => {
                let step = up(self.list.grid(other, layout));
                self.after_list();
                step
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
        let count = result.as_ref().ok().map(|p| p.count);
        let refreshing = match self.list.arrive(generation, result, waiting) {
            Arrival::Arrived { refreshing } => refreshing,
            Arrival::NotOurs | Arrival::Failed => return Vec::new(),
        };
        // The machine follows the result: a new query or a new size starts it over (a new
        // shuffled round, 011 FR-007).
        let count = count.unwrap_or(0);
        if self.list.take_new_query() || self.playlist.count != count {
            self.machine(Event::ResultsChanged {
                count,
                seed: new_seed64(),
            });
        }
        let mut effects = Vec::new();
        if self.list.mode == Mode::Queue {
            if self.list.cards().is_empty() {
                self.queue.clear();
            } else {
                // The machine's scene is on this page (Next, Previous, a shuffled pick, Repeat
                // all), or the queue may start here.
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
                    cards_of(&self.list.data),
                    self.list.tab,
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
        effects.extend(self.list.after_arrival(refreshing));
        effects
    }
}
