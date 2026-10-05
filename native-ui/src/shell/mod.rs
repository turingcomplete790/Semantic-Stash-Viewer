//! The shell (007 data model "Shell"; spec US2, capabilities C10, C11): the navigation bar, the
//! tabs, and the active tab's screen. Overlays sit above it in the session (`overlays`).

pub mod keymap;
pub mod notifications;
pub mod overlays;
pub mod tab;

use iced::widget::{button, column, container, mouse_area, row, text, Space};
use iced::{Alignment, Element, Length};
use stash_core::connection::ServerInfo;

use crate::effects::Effect;
use crate::machine::Step;
use crate::screens::scenes::keyboard::{grid_key, GridKeys};
use crate::screens::scenes::layout::Layout;
use crate::screens::scenes::{ScenesMsg, ScenesUp};
use crate::screens::{Context, Screen, Section, SettingsPage};
use crate::session::snapshot::{SavedSession, SavedTab};
use crate::widgets::{icon, icon_button, one_line, theme, Icon};
use iced::keyboard::{key::Named, Key, Modifiers};
use stash_core::scenes::{SceneCard, ScenePage};
use stash_core::AppError;

pub use tab::{Tab, TabId, MAX_HISTORY};

/// The most tabs open at once.
pub const MAX_TABS: usize = 50;

/// The new-tab button's widget id.
pub const NEW_TAB_ID: &str = "new-tab";

/// The close button's widget id on a tab.
pub fn close_tab_id(tab: TabId) -> iced::widget::Id {
    iced::widget::Id::from(format!("close-tab-{tab}"))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Shell {
    /// At least one.
    pub tabs: Vec<Tab>,
    pub selected: usize,
    next_id: TabId,
    /// The window's size (the grid's columns, keyboard moves, scroll positions).
    pub layout: Layout,
    /// Something changed since the shell was made (a restore then keeps the current tab).
    touched: bool,
}

#[derive(Debug, Clone)]
pub enum ShellMsg {
    /// The navigation bar.
    Section(Section),
    /// Open a screen in the active tab.
    Open(Screen),
    /// Open a screen in a new tab next to the active one.
    OpenInNewTab(Screen),
    Back,
    Forward,
    NewTab,
    Close(TabId),
    CloseActive,
    Select(TabId),
    SelectIndex(usize),
    NextTab,
    PrevTab,
    MoveLeft,
    MoveRight,
    SettingsPage(SettingsPage),
    /// For the active Settings screen.
    Settings(crate::screens::settings::SettingsMsg),
    /// For the active Scenes screen.
    Scenes(ScenesMsg),
    // Handled above the shell.
    Play(String),
    ServerMenu,
    Notifications,
    KeyboardHelp,
}

/// Events for the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Up {
    Play(String),
    /// A tab was closed (playback it owned stops).
    Closed(TabId),
    /// Open the "Add a server" form.
    AddServer,
    ServerMenu,
    Notifications,
    KeyboardHelp,
}

impl Shell {
    /// One Home tab.
    pub fn new() -> (Self, Vec<Effect>) {
        let mut shell = Self {
            tabs: vec![Tab::new(1, Screen::home())],
            selected: 0,
            next_id: 2,
            layout: Layout::default(),
            touched: false,
        };
        let effects = shell.active_mut().enter();
        (shell, effects)
    }

    /// The shell from a saved session; `None` if it has no usable tab. Restored screens run their
    /// entry actions as they become active, so data comes from the cache first.
    pub fn restore(saved: SavedSession) -> Option<(Self, Vec<Effect>)> {
        let tabs: Vec<Tab> = saved
            .tabs
            .into_iter()
            .take(MAX_TABS)
            .enumerate()
            .filter_map(|(i, t)| Tab::restore(i as TabId + 1, t.history, t.cursor))
            .collect();
        if tabs.is_empty() {
            return None;
        }
        let next_id = tabs.iter().map(|t| t.id).max().unwrap_or(0) + 1;
        let mut shell = Self {
            selected: saved.selected.min(tabs.len() - 1),
            tabs,
            next_id,
            layout: Layout::default(),
            touched: false,
        };
        let effects = shell.active_mut().enter();
        Some((shell, effects))
    }

    /// The saved form: persistent fields only, within the limits.
    pub fn capture(&self) -> SavedSession {
        SavedSession {
            selected: self.selected.min(MAX_TABS - 1),
            tabs: self
                .tabs
                .iter()
                .take(MAX_TABS)
                .map(|t| SavedTab {
                    history: t.history.iter().map(Screen::persistent).collect(),
                    cursor: t.cursor,
                })
                .collect(),
        }
    }

    pub fn active(&self) -> &Tab {
        &self.tabs[self.selected]
    }

    pub fn active_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.selected]
    }

    fn index_of(&self, id: TabId) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == id)
    }

    fn select(&mut self, index: usize) -> Vec<Effect> {
        if index >= self.tabs.len() || index == self.selected {
            return Vec::new();
        }
        self.selected = index;
        self.active_mut().enter()
    }

    fn new_tab(&mut self, at: usize, screen: Screen) -> Vec<Effect> {
        if self.tabs.len() >= MAX_TABS {
            return Vec::new();
        }
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.insert(at, Tab::new(id, screen));
        self.selected = at;
        self.active_mut().enter()
    }

    fn close(&mut self, index: usize) -> Vec<Effect> {
        if self.tabs.len() <= 1 || index >= self.tabs.len() {
            return Vec::new();
        }
        self.tabs.remove(index);
        if index < self.selected {
            self.selected -= 1;
            Vec::new()
        } else if index == self.selected {
            // The tab after it takes its place; the one before when it was last.
            self.selected = index.min(self.tabs.len() - 1);
            self.active_mut().enter()
        } else {
            Vec::new()
        }
    }

    /// Close a tab, telling the session which (unless it's the last, which stays).
    fn close_tab(&mut self, id: TabId) -> Step<Up> {
        match self.index_of(id) {
            Some(i) if self.tabs.len() > 1 => Step {
                effects: self.close(i),
                up: Some(Up::Closed(id)),
            },
            _ => Step::none(),
        }
    }

    fn move_by(&mut self, delta: isize) {
        let to = self.selected as isize + delta;
        if to < 0 || to as usize >= self.tabs.len() {
            return;
        }
        self.tabs.swap(self.selected, to as usize);
        self.selected = to as usize;
    }

    /// Whether anything happened since the shell was made.
    pub fn touched(&self) -> bool {
        self.touched
    }

    /// Add a tab from another shell (the one used before the saved tabs arrived) as a new tab,
    /// selected. Returns its id here.
    pub fn adopt(&mut self, mut tab: Tab) -> TabId {
        let id = self.next_id;
        self.next_id += 1;
        tab.id = id;
        if self.tabs.len() >= MAX_TABS {
            self.tabs.pop();
        }
        self.tabs.push(tab);
        self.selected = self.tabs.len() - 1;
        id
    }

    pub fn update(&mut self, msg: ShellMsg) -> Step<Up> {
        self.touched = true;
        let len = self.tabs.len();
        let effects = match msg {
            ShellMsg::Section(section) => self.active_mut().open_section(section),
            ShellMsg::Open(screen) => self.active_mut().open(screen),
            ShellMsg::OpenInNewTab(screen) => self.new_tab(self.selected + 1, screen),
            ShellMsg::Back => self.active_mut().back(),
            ShellMsg::Forward => self.active_mut().forward(),
            ShellMsg::NewTab => self.new_tab(len, Screen::home()),
            ShellMsg::Close(id) => return self.close_tab(id),
            ShellMsg::CloseActive => return self.close_tab(self.active().id),
            ShellMsg::Select(id) => match self.index_of(id) {
                Some(i) => self.select(i),
                None => Vec::new(),
            },
            ShellMsg::SelectIndex(i) => self.select(i),
            ShellMsg::NextTab => self.select((self.selected + 1) % len),
            ShellMsg::PrevTab => self.select((self.selected + len - 1) % len),
            ShellMsg::MoveLeft => {
                self.move_by(-1);
                Vec::new()
            }
            ShellMsg::MoveRight => {
                self.move_by(1);
                Vec::new()
            }
            ShellMsg::SettingsPage(page) => match self.active_mut().current_mut() {
                Screen::Settings(s) => s.open(page),
                _ => Vec::new(),
            },
            ShellMsg::Settings(m) => {
                let Screen::Settings(s) = self.active_mut().current_mut() else {
                    return Step::none();
                };
                return s.update(m).map_up(|up| match up {
                    crate::screens::settings::SettingsUp::AddServer => Up::AddServer,
                });
            }
            ShellMsg::Scenes(m) => {
                let (tab, layout) = (self.active().id, self.layout);
                let Screen::Scenes(state) = self.active_mut().current_mut() else {
                    return Step::none();
                };
                let Step { mut effects, up } = state.update(m, tab, &layout);
                if let Some(ScenesUp::Open { id, title, new_tab }) = up {
                    let screen = Screen::scene(&id, &title);
                    effects.extend(if new_tab {
                        self.new_tab(self.selected + 1, screen)
                    } else {
                        self.active_mut().open(screen)
                    });
                }
                effects
            }
            ShellMsg::Play(id) => return Step::up(Up::Play(id)),
            ShellMsg::ServerMenu => return Step::up(Up::ServerMenu),
            ShellMsg::Notifications => return Step::up(Up::Notifications),
            ShellMsg::KeyboardHelp => return Step::up(Up::KeyboardHelp),
        };
        Step::effects(effects)
    }

    /// A page of scenes arrived for a request from tab `tab`; it lands on the screen that asked,
    /// wherever it is in the tab's history.
    pub fn scenes_page(
        &mut self,
        tab: TabId,
        generation: u64,
        result: Result<ScenePage, AppError>,
        waiting: bool,
    ) -> Vec<Effect> {
        let Some(i) = self.index_of(tab) else {
            return Vec::new();
        };
        self.tabs[i]
            .history
            .iter_mut()
            .find_map(|screen| match screen {
                Screen::Scenes(state) if state.awaits(generation) => Some(state),
                _ => None,
            })
            .map(|state| state.page_loaded(generation, result, waiting))
            .unwrap_or_default()
    }

    /// Every Settings screen, in every tab (results for Settings reach them all).
    fn settings_screens(&mut self) -> impl Iterator<Item = &mut crate::screens::SettingsState> {
        self.tabs
            .iter_mut()
            .flat_map(|t| t.history.iter_mut())
            .filter_map(|s| match s {
                Screen::Settings(s) => Some(s),
                _ => None,
            })
    }

    pub fn settings_profiles(&mut self, list: &[stash_core::profiles::ServerProfile]) {
        for s in self.settings_screens() {
            s.servers.loaded(list.to_vec());
        }
    }

    pub fn settings_saved(
        &mut self,
        id: uuid::Uuid,
        result: &Result<stash_core::profiles::ServerProfile, AppError>,
    ) {
        for s in self.settings_screens() {
            s.servers.saved(id, result.clone());
        }
    }

    pub fn settings_deleted(
        &mut self,
        result: &Result<stash_core::profiles::ServerProfile, AppError>,
    ) {
        for s in self.settings_screens() {
            s.servers.deleted(result.clone());
        }
    }

    pub fn settings_cache_size(&mut self, size: u64) {
        for s in self.settings_screens() {
            s.troubleshooting.cache_size = Some(size);
        }
    }

    pub fn settings_cleared(&mut self, result: &Result<u64, AppError>) {
        for s in self.settings_screens() {
            if s.troubleshooting.clear == crate::screens::settings::Clear::Clearing {
                s.troubleshooting.cleared(result.clone());
            }
        }
    }

    /// A scene's details arrived for tab `tab`.
    pub fn scene_details(
        &mut self,
        tab: TabId,
        id: &str,
        result: Result<stash_core::scenes::SceneDetails, AppError>,
    ) {
        if let Some(state) = self.scene_in(tab, id) {
            state.details_loaded(result);
        }
    }

    /// A scene's cover arrived for tab `tab`.
    pub fn scene_cover(
        &mut self,
        tab: TabId,
        id: &str,
        cover: Option<iced::widget::image::Handle>,
    ) {
        if let Some(state) = self.scene_in(tab, id) {
            state.cover_loaded(cover);
        }
    }

    fn scene_in(&mut self, tab: TabId, id: &str) -> Option<&mut crate::screens::SceneState> {
        let i = self.index_of(tab)?;
        self.tabs[i]
            .history
            .iter_mut()
            .find_map(|screen| match screen {
                Screen::Scene(s) if s.scene_id == id => Some(s),
                _ => None,
            })
    }

    /// Cached data changed (`*`: all of it): the active screen re-reads in place.
    pub fn cache_changed(&mut self, key: &str) -> Vec<Effect> {
        let tab = self.active().id;
        match self.active_mut().current_mut() {
            Screen::Scenes(state) => state.cache_changed(key, tab),
            _ => Vec::new(),
        }
    }

    /// The connection came up: the active screen loads what it couldn't before.
    pub fn retry_active(&mut self) -> Vec<Effect> {
        if self.active().current().needs_load() {
            self.active_mut().enter()
        } else {
            Vec::new()
        }
    }

    /// The cards the active screen shows.
    pub fn active_cards(&self) -> &[SceneCard] {
        match self.active().current() {
            Screen::Scenes(state) => state.cards(),
            _ => &[],
        }
    }

    /// A key for the active screen (the grid's keys on Scenes); `None` when it isn't one.
    pub fn screen_key(&mut self, key: &Key, mods: Modifiers) -> Option<Step<Up>> {
        if mods.alt() || mods.logo() || (mods.control() && *key != Key::Named(Named::Enter)) {
            return None;
        }
        let Screen::Scenes(state) = self.active().current() else {
            return None;
        };
        let keys = GridKeys {
            index: state.focused,
            count: state.cards().len(),
            cols: self.layout.cols(state.mode),
            ctrl: mods.control(),
        };
        let action = grid_key(key, keys)?;
        Some(self.update(ShellMsg::Scenes(ScenesMsg::Grid(action))))
    }

    /// A summary read for tab `tab`'s Home screen.
    pub fn summary_loaded(&mut self, tab: TabId, info: Option<ServerInfo>, unreachable: bool) {
        if let Some(i) = self.index_of(tab) {
            if let Screen::Home(home) = self.tabs[i].current_mut() {
                home.loaded(info, unreachable);
            }
        }
    }

    /// The live connection's summary: every Home screen shows it.
    pub fn server_info(&mut self, info: &ServerInfo) {
        self.homes(|home| home.loaded(Some(info.clone()), false));
    }

    /// The connection failed: Home screens still waiting give up.
    pub fn unreachable(&mut self) {
        self.homes(|home| home.loaded(None, true));
    }

    fn homes(&mut self, mut f: impl FnMut(&mut crate::screens::home::Home)) {
        for tab in &mut self.tabs {
            for screen in &mut tab.history {
                if let Screen::Home(home) = screen {
                    f(home);
                }
            }
        }
    }

    /// The navigation bar and the tab strip. `right` goes at the bar's end (the bell and the
    /// connection indicator).
    pub fn chrome<'a>(&'a self, right: Element<'a, ShellMsg>) -> Element<'a, ShellMsg> {
        column![self.nav_bar(right), self.tab_strip()].into()
    }

    fn nav_bar<'a>(&'a self, right: Element<'a, ShellMsg>) -> Element<'a, ShellMsg> {
        let tab = self.active();
        let here = tab.current().section();
        let section = |s: Section, which: Icon, label: &'static str| {
            button(
                row![icon(which, 16.0), text(label).size(14)]
                    .spacing(6)
                    .align_y(Alignment::Center),
            )
            .padding([5, 10])
            .style(theme::menu_row(here == Some(s)))
            .on_press(ShellMsg::Section(s))
        };
        container(
            row![
                icon_button(
                    Icon::Back,
                    16.0,
                    tab.can_go_back().then_some(ShellMsg::Back)
                ),
                icon_button(
                    Icon::Forward,
                    16.0,
                    tab.can_go_forward().then_some(ShellMsg::Forward)
                ),
                section(Section::Home, Icon::Home, "Home"),
                section(Section::Scenes, Icon::Scenes, "Scenes"),
                section(Section::Settings, Icon::Settings, "Settings"),
                Space::new().width(Length::Fill),
                right,
            ]
            .spacing(4)
            .padding([4, 8])
            .align_y(Alignment::Center),
        )
        .style(theme::bar)
        .width(Length::Fill)
        .into()
    }

    fn tab_strip(&self) -> Element<'_, ShellMsg> {
        let closable = self.tabs.len() > 1;
        let mut strip = row![].spacing(2).align_y(Alignment::Center);
        for (i, tab) in self.tabs.iter().enumerate() {
            let active = i == self.selected;
            let close = container(icon_button(
                Icon::Close,
                12.0,
                closable.then_some(ShellMsg::Close(tab.id)),
            ))
            .id(close_tab_id(tab.id));
            let body = button(
                row![one_line(tab.current().title(), 13.0), close]
                    .spacing(4)
                    .align_y(Alignment::Center),
            )
            .width(Length::Fixed(180.0))
            .padding(iced::Padding {
                top: 2.0,
                right: 4.0,
                bottom: 2.0,
                left: 10.0,
            })
            .style(theme::menu_row(active))
            .on_press(ShellMsg::Select(tab.id));
            let mut area = mouse_area(body);
            if closable {
                area = area.on_middle_press(ShellMsg::Close(tab.id));
            }
            strip = strip.push(area);
        }
        let can_add = self.tabs.len() < MAX_TABS;
        strip = strip.push(
            container(icon_button(
                Icon::Plus,
                14.0,
                can_add.then_some(ShellMsg::NewTab),
            ))
            .id(NEW_TAB_ID),
        );
        container(strip.padding([3, 8]))
            .width(Length::Fill)
            .style(theme::strip)
            .into()
    }

    /// The active screen.
    pub fn content<'a>(&'a self, ctx: &Context<'a>) -> Element<'a, ShellMsg> {
        self.active().current().view(ctx, self.active().id)
    }
}
