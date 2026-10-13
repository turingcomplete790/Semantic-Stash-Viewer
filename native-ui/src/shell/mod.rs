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
use crate::screens::images::{ImagesMsg, ImagesUp};
use crate::screens::scenes::keyboard::{grid_key, GridKeys};
use crate::screens::scenes::layout::Layout;
use crate::screens::scenes::thumbs::{thumb_key, ThumbKey};
use crate::screens::scenes::{ScenesMsg, ScenesUp};
use crate::screens::{Context, Screen, Section, SettingsPage};
use crate::session::snapshot::{SavedSession, SavedTab};
use crate::widgets::{icon, icon_button, one_line, theme, Icon};
use iced::keyboard::{key::Named, Key, Modifiers};
use stash_core::images::{ImagePage, ImageSort};
use stash_core::scenes::filter::saved::SavedFilterOf;
use stash_core::scenes::ScenePage;
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
    /// The server's default scene filter: new Scenes screens start from it (008 FR-004).
    pub default_scenes: Option<stash_core::scenes::filter::saved::SavedFilter>,
    /// The server's default image filter: new Images screens start from it (012 FR-005).
    pub default_images: Option<SavedFilterOf<ImageSort>>,
}

#[derive(Debug, Clone)]
pub enum ShellMsg {
    /// The navigation bar.
    Section(Section),
    /// Settings → Playback: a switch flipped (008).
    PlaybackChange(crate::screens::settings::playback::PlaybackChange),
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
    /// For the active Images screen (012).
    Images(ImagesMsg),
    /// For the active Scene screen.
    Scene(crate::screens::scene::SceneMsg),
    /// Open Scenes filtered to a performer, tag, or studio (008 FR-006), here or in a new tab.
    FilterTo {
        criterion: stash_core::scenes::filter::Criterion,
        new_tab: bool,
    },
    // Handled above the shell.
    Play(String),
    /// The player's controls drawn inside a screen (the queue view's panel, 010).
    Player(crate::session::playback::Msg),
    /// The queue view's Next (`true`) or Previous scene (010 FR-010).
    QueueStep(bool),
    ServerMenu,
    Notifications,
    KeyboardHelp,
}

/// Events for the session.
#[derive(Debug, Clone, PartialEq)]
pub enum Up {
    Play(String),
    /// A tab was closed (playback it owned stops).
    Closed(TabId),
    /// Open the "Add a server" form.
    AddServer,
    ServerMenu,
    Notifications,
    KeyboardHelp,
    /// A screen needs something from the library.
    Need(crate::library::Need),
    /// Play a scene from a point (a marker).
    PlayAt {
        id: String,
        seconds: f64,
    },
    /// Settings → Playback changed (008).
    PlaybackChange(crate::screens::settings::playback::PlaybackChange),
    /// Show an image in the player for the active tab's viewer (012).
    ShowImage {
        id: String,
        title: String,
        looped: bool,
    },
    /// The active tab's viewer closed: its image leaves the player.
    ImageClosed,
}

impl Shell {
    /// A fresh screen for a section: Scenes and Images start from the server's default filters.
    fn fresh(&self, section: Section) -> Screen {
        match (section, &self.default_scenes, &self.default_images) {
            (Section::Scenes, Some(filter), _) => {
                Screen::Scenes(Box::new(crate::screens::ScenesState::from_saved(filter)))
            }
            (Section::Images, _, Some(filter)) => {
                Screen::Images(Box::new(crate::screens::ImagesState::from_saved(filter)))
            }
            _ => Screen::for_section(section),
        }
    }

    /// One Home tab.
    pub fn new() -> (Self, Vec<Effect>) {
        let mut shell = Self {
            tabs: vec![Tab::new(1, Screen::home())],
            selected: 0,
            next_id: 2,
            layout: Layout::default(),
            touched: false,
            default_scenes: None,
            default_images: None,
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
            default_scenes: None,
            default_images: None,
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
            ShellMsg::Section(section) => {
                let fresh = self.fresh(section);
                self.active_mut().open_section_with(section, fresh)
            }
            ShellMsg::Open(screen) => self.active_mut().open(screen),
            ShellMsg::OpenInNewTab(screen) => self.new_tab(self.selected + 1, screen),
            // Back closes the image viewer first (it's a mode of the Images screen).
            ShellMsg::Back if self.active_viewer().is_some() => {
                let Screen::Images(state) = self.active_mut().current_mut() else {
                    return Step::none();
                };
                let step = state.close_viewer();
                return self.images_up(step);
            }
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
                match up {
                    Some(ScenesUp::Open { id, title, new_tab }) => {
                        let screen = Screen::scene(&id, &title);
                        effects.extend(if new_tab {
                            self.new_tab(self.selected + 1, screen)
                        } else {
                            self.active_mut().open(screen)
                        });
                    }
                    Some(ScenesUp::Play(id)) => {
                        return Step {
                            effects,
                            up: Some(Up::Play(id)),
                        }
                    }
                    Some(ScenesUp::Need(need)) => {
                        return Step {
                            effects,
                            up: Some(Up::Need(need)),
                        }
                    }
                    None => {}
                }
                effects
            }
            ShellMsg::Images(m) => {
                let (tab, layout) = (self.active().id, self.layout);
                let Screen::Images(state) = self.active_mut().current_mut() else {
                    return Step::none();
                };
                let step = state.update(m, tab, &layout);
                return self.images_up(step);
            }
            ShellMsg::FilterTo { criterion, new_tab } => self.filter_to(criterion, new_tab),
            ShellMsg::PlaybackChange(change) => return Step::up(Up::PlaybackChange(change)),
            ShellMsg::Scene(m) => {
                let tab = self.active().id;
                // The Scene screen, or the queue view's panel (010).
                let Some(state) = self.active_mut().current_mut().scene_panel_mut() else {
                    return Step::none();
                };
                let id = state.scene_id.clone();
                return state.update(m, tab).map_up(|up| match up {
                    crate::screens::scene::SceneUp::PlayAt(seconds) => Up::PlayAt { id, seconds },
                });
            }
            ShellMsg::Play(id) => return Step::up(Up::Play(id)),
            ShellMsg::Player(_) | ShellMsg::QueueStep(_) => return Step::none(),
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

    /// A page of images arrived for a request from tab `tab` (012), as [`Shell::scenes_page`];
    /// the viewer may show an image from it.
    pub fn images_page(
        &mut self,
        tab: TabId,
        generation: u64,
        result: Result<ImagePage, AppError>,
        waiting: bool,
    ) -> Step<Up> {
        let Some(i) = self.index_of(tab) else {
            return Step::none();
        };
        let active = i == self.selected;
        let Some(state) = self.tabs[i]
            .history
            .iter_mut()
            .find_map(|screen| match screen {
                Screen::Images(state) if state.awaits(generation) => Some(state),
                _ => None,
            })
        else {
            return Step::none();
        };
        let step = state.page_loaded(generation, result, waiting);
        // Only the tab on screen shows images (a background tab's viewer waits for its turn).
        match step.up {
            Some(ImagesUp::Show { .. }) if !active => Step::effects(step.effects),
            _ => self.images_up(step),
        }
    }

    /// An Images screen's event, as the shell's.
    fn images_up(&mut self, step: Step<ImagesUp>) -> Step<Up> {
        let Step { mut effects, up } = step;
        let up = match up {
            None => None,
            Some(ImagesUp::Need(need)) => Some(Up::Need(need)),
            Some(ImagesUp::Show { id, title, looped }) => Some(Up::ShowImage { id, title, looped }),
            Some(ImagesUp::Closed) => Some(Up::ImageClosed),
            Some(ImagesUp::OpenInNewTab(state)) => {
                effects.extend(self.new_tab(self.selected + 1, Screen::Images(state)));
                None
            }
        };
        Step { effects, up }
    }

    /// Apply `f` to the active tab's open viewer (a key the session resolved).
    pub fn with_viewer(
        &mut self,
        f: impl FnOnce(&mut crate::screens::ImagesState) -> Step<ImagesUp>,
    ) -> Step<Up> {
        self.touched = true;
        let window = self.layout;
        let step = match self.active_mut().current_mut() {
            Screen::Images(s) if s.viewer.is_some() => {
                s.window = window;
                f(s)
            }
            _ => return Step::none(),
        };
        self.images_up(step)
    }

    /// The active tab's Images viewer, if one is open.
    pub fn active_viewer(&self) -> Option<&crate::screens::ImagesState> {
        match self.active().current() {
            Screen::Images(s) if s.viewer.is_some() => Some(s),
            _ => None,
        }
    }

    /// The Images screen in tab `tab` whose viewer shows `id`.
    fn viewer_in(&mut self, tab: TabId, id: &str) -> Option<&mut crate::screens::ImagesState> {
        let i = self.index_of(tab)?;
        self.tabs[i]
            .history
            .iter_mut()
            .find_map(|screen| match screen {
                Screen::Images(s) if s.viewer_image() == Some(id) => Some(&mut **s),
                _ => None,
            })
    }

    /// The image `id` is on screen in tab `tab`'s viewer.
    pub fn image_shown(&mut self, tab: TabId, id: &str, picture: Option<(u32, u32)>) {
        if let Some(s) = self.viewer_in(tab, id) {
            s.image_shown(id, picture);
        }
    }

    /// The image `id` couldn't be shown in tab `tab`'s viewer.
    pub fn image_failed(&mut self, tab: TabId, id: &str, reason: String) {
        if let Some(s) = self.viewer_in(tab, id) {
            s.image_failed(id, reason);
        }
    }

    /// A scene took the player: every open viewer closes (one player, 012 FR-019).
    pub fn close_viewers(&mut self) -> Vec<Effect> {
        let mut effects = Vec::new();
        for tab in &mut self.tabs {
            for screen in &mut tab.history {
                if let Screen::Images(s) = screen {
                    effects.extend(s.close_viewer().effects);
                }
            }
        }
        effects
    }

    /// A search's debounce came due for tab `tab`.
    pub fn search_due(&mut self, tab: TabId, generation: u64) -> Vec<Effect> {
        let Some(i) = self.index_of(tab) else {
            return Vec::new();
        };
        self.tabs[i]
            .history
            .iter_mut()
            .find_map(|screen| match screen {
                Screen::Scenes(state) if state.awaits_search(generation) => {
                    Some(state.search_due(generation))
                }
                Screen::Images(state) if state.awaits_search(generation) => {
                    Some(state.search_due(generation))
                }
                _ => None,
            })
            .unwrap_or_default()
    }

    /// Open Scenes filtered to `criterion` (a performer, tag, or studio chip), here or in a new
    /// tab (008 FR-006).
    pub fn filter_to(
        &mut self,
        criterion: stash_core::scenes::filter::Criterion,
        new_tab: bool,
    ) -> Vec<Effect> {
        let mut state = crate::screens::ScenesState::default();
        state.query.criteria = vec![criterion];
        let screen = Screen::Scenes(Box::new(state));
        if new_tab {
            self.new_tab(self.selected + 1, screen)
        } else {
            self.active_mut().open(screen)
        }
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

    /// A scene's files arrived for tab `tab`.
    pub fn scene_files(
        &mut self,
        tab: TabId,
        id: &str,
        result: Result<Vec<stash_core::scenes::FileFacts>, AppError>,
    ) {
        if let Some(state) = self.scene_in(tab, id) {
            state.files_loaded(result);
        }
    }

    /// A scene's galleries arrived for tab `tab`.
    pub fn scene_galleries(
        &mut self,
        tab: TabId,
        id: &str,
        result: Result<Vec<stash_core::scenes::GalleryChip>, AppError>,
    ) {
        if let Some(state) = self.scene_in(tab, id) {
            state.galleries_loaded(result);
        }
    }

    /// The tag colours were saved (or not): every Tag colours page shows it.
    pub fn tag_colours_saved(
        &mut self,
        result: &Result<stash_core::adapter::filters::UiSettings, AppError>,
    ) {
        for s in self.settings_screens() {
            s.tag_colours.saved(match result {
                Ok(settings) => Ok(settings.colour_rules.clone()),
                Err(e) => Err(crate::messages::for_error(e).title.to_owned()),
            });
        }
    }

    /// The server's colour rules: Tag colours pages not being edited show them.
    pub fn sync_tag_colours(&mut self, rules: &[stash_core::semantic::ColourRule]) {
        for s in self.settings_screens() {
            s.tag_colours.sync(rules.to_vec());
        }
    }

    /// Whether the active screen is Settings → Tag colours.
    pub fn tag_colours_open(&self) -> bool {
        matches!(
            self.active().current(),
            Screen::Settings(s) if s.page == crate::screens::settings::SettingsPage::TagColours
        )
    }

    /// A chip rest came due in tab `tab`: the chip, if the pointer is still on it.
    pub fn chip_rested(
        &mut self,
        tab: TabId,
        generation: u64,
    ) -> Option<crate::screens::scene::ChipRef> {
        let i = self.index_of(tab)?;
        self.tabs[i]
            .history
            .iter_mut()
            .find_map(|screen| match screen.scene_panel_mut() {
                Some(s) if s.awaits_rest(generation) => s.chip_rested(generation),
                _ => None,
            })
    }

    /// The queue's selection rest came due in tab `tab` (010 R3): its panel loads.
    /// Its effects, and the scene to play when a keyboard move started the rest (011 R2).
    pub fn queue_rest_due(&mut self, tab: TabId, generation: u64) -> (Vec<Effect>, Option<String>) {
        let Some(i) = self.index_of(tab) else {
            return (Vec::new(), None);
        };
        self.tabs[i]
            .history
            .iter_mut()
            .find_map(|screen| match screen {
                Screen::Scenes(s) if s.awaits_queue_rest(generation) => Some(s),
                _ => None,
            })
            .map(|s| s.queue_rest_due(generation, tab))
            .unwrap_or_default()
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
                // The queue view's panel (010).
                Screen::Scenes(s) => s.queue.panel_for(id),
                _ => None,
            })
    }

    /// Cached data changed (`*`: all of it): the active screen re-reads in place.
    pub fn cache_changed(&mut self, key: &str) -> Vec<Effect> {
        let tab = self.active().id;
        match self.active_mut().current_mut() {
            Screen::Scenes(state) => state.cache_changed(key, tab),
            Screen::Images(state) => state.cache_changed(key, tab),
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

    /// The thumbnails of the cards the active screen shows.
    pub fn active_thumbs(&self) -> Vec<ThumbKey> {
        match self.active().current() {
            Screen::Scenes(state) => state.cards().iter().filter_map(thumb_key).collect(),
            Screen::Images(state) => state
                .cards()
                .iter()
                .filter_map(|c| {
                    c.thumb
                        .as_deref()
                        .and_then(crate::screens::scenes::thumbs::key_of)
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The active screen's grid geometry: Images' cards are their own size (012).
    pub fn active_layout(&self) -> Layout {
        match self.active().current() {
            Screen::Images(state) => state.layout(&self.layout),
            _ => self.layout,
        }
    }

    /// A key for the active screen (the grid's keys on Scenes and Images); `None` when it isn't
    /// one.
    pub fn screen_key(&mut self, key: &Key, mods: Modifiers) -> Option<Step<Up>> {
        if mods.alt() || mods.logo() || (mods.control() && *key != Key::Named(Named::Enter)) {
            return None;
        }
        let layout = self.active_layout();
        let (focused, count, mode) = match self.active().current() {
            Screen::Scenes(s) => (s.focused, s.cards().len(), s.mode),
            Screen::Images(s) => (s.focused, s.cards().len(), s.mode),
            _ => return None,
        };
        let keys = GridKeys {
            index: focused,
            count,
            cols: layout.cols(mode),
            ctrl: mods.control(),
        };
        let action = grid_key(key, keys)?;
        let msg = match self.active().current() {
            Screen::Images(_) => ShellMsg::Images(ImagesMsg::List(
                crate::screens::browse::state::BrowseMsg::Grid(action),
            )),
            _ => ShellMsg::Scenes(ScenesMsg::Grid(action)),
        };
        Some(self.update(msg))
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
                section(Section::Images, Icon::Images, "Images"),
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
