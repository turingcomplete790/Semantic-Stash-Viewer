//! The shell (007 data model "Shell"; spec US2, capabilities C10, C11): the navigation bar, the
//! tabs, and the active tab's screen. Overlays sit above it in the session (`overlays`).

pub mod keymap;
pub mod overlays;
pub mod tab;

use iced::widget::{button, column, container, mouse_area, row, text, Space};
use iced::{Alignment, Element, Length};
use stash_core::connection::ServerInfo;

use crate::effects::Effect;
use crate::machine::Step;
use crate::screens::{Context, Screen, Section, SettingsPage};
use crate::session::snapshot::{SavedSession, SavedTab};
use crate::widgets::{icon, icon_button, one_line, theme, Icon};

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
    ServerMenu,
    Notifications,
    KeyboardHelp,
}

impl Shell {
    /// One Home tab.
    pub fn new() -> (Self, Vec<Effect>) {
        let shell = Self {
            tabs: vec![Tab::new(1, Screen::home())],
            selected: 0,
            next_id: 2,
        };
        let effects = shell.active().enter();
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
        let shell = Self {
            selected: saved.selected.min(tabs.len() - 1),
            tabs,
            next_id,
        };
        let effects = shell.active().enter();
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
        self.active().enter()
    }

    fn new_tab(&mut self, at: usize, screen: Screen) -> Vec<Effect> {
        if self.tabs.len() >= MAX_TABS {
            return Vec::new();
        }
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.insert(at, Tab::new(id, screen));
        self.selected = at;
        self.active().enter()
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
            self.active().enter()
        } else {
            Vec::new()
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

    pub fn update(&mut self, msg: ShellMsg) -> Step<Up> {
        let len = self.tabs.len();
        let effects = match msg {
            ShellMsg::Section(section) => self.active_mut().open_section(section),
            ShellMsg::Open(screen) => self.active_mut().open(screen),
            ShellMsg::OpenInNewTab(screen) => self.new_tab(self.selected + 1, screen),
            ShellMsg::Back => self.active_mut().back(),
            ShellMsg::Forward => self.active_mut().forward(),
            ShellMsg::NewTab => self.new_tab(len, Screen::home()),
            ShellMsg::Close(id) => match self.index_of(id) {
                Some(i) => self.close(i),
                None => Vec::new(),
            },
            ShellMsg::CloseActive => self.close(self.selected),
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
            ShellMsg::SettingsPage(page) => {
                if let Screen::Settings(s) = self.active_mut().current_mut() {
                    s.page = page;
                }
                Vec::new()
            }
            ShellMsg::Play(id) => return Step::up(Up::Play(id)),
            ShellMsg::ServerMenu => return Step::up(Up::ServerMenu),
            ShellMsg::Notifications => return Step::up(Up::Notifications),
            ShellMsg::KeyboardHelp => return Step::up(Up::KeyboardHelp),
        };
        Step::effects(effects)
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

    /// The navigation bar and the tab strip. `right` goes at the bar's end (the connection
    /// indicator).
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
                icon_button(Icon::Bell, 18.0, Some(ShellMsg::Notifications)),
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
        self.active().current().view(ctx)
    }
}
