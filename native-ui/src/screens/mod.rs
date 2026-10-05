//! The screens a tab shows (007 data model "Screen"): whole states kept in the tab's history, so
//! going back returns exactly what was left (behaviour that stays B1). Each screen's persistent
//! fields are saved with the session; transient ones (`#[serde(skip)]`) are rebuilt by its entry
//! effects. Home is complete here; Scenes, Scene, and Settings fill in with US3 and US4.

pub mod home;
pub mod scene;
pub mod scenes;

use iced::widget::{button, column, container, row, text, Space};
use iced::{Element, Length};
use serde::{Deserialize, Serialize};
use stash_core::profiles::ServerProfile;

use crate::effects::Effect;
use crate::session::connection::Connection;
use crate::shell::keymap;
use crate::shell::{ShellMsg, TabId};
use crate::widgets::theme;

/// What screen views read beyond their own state.
pub struct Context<'a> {
    pub profile: &'a ServerProfile,
    pub connection: &'a Connection,
    pub thumbs: &'a scenes::Thumbs,
    /// The window's size, for the grid's columns.
    pub layout: scenes::layout::Layout,
}

pub use scene::SceneState;
pub use scenes::ScenesState;

/// The navigation bar's sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Home,
    Scenes,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Grid,
    List,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SettingsPage {
    #[default]
    Servers,
    Keyboard,
    Troubleshooting,
    About,
}

impl SettingsPage {
    pub const ALL: [SettingsPage; 4] = [
        SettingsPage::Servers,
        SettingsPage::Keyboard,
        SettingsPage::Troubleshooting,
        SettingsPage::About,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SettingsPage::Servers => "Servers",
            SettingsPage::Keyboard => "Keyboard",
            SettingsPage::Troubleshooting => "Troubleshooting",
            SettingsPage::About => "About",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SettingsState {
    pub page: SettingsPage,
}

/// One screen state, externally tagged by variant in the saved session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Screen {
    Home(home::Home),
    Scenes(ScenesState),
    Scene(SceneState),
    Settings(SettingsState),
}

impl Screen {
    pub fn home() -> Self {
        Screen::Home(home::Home::default())
    }

    pub fn scenes() -> Self {
        Screen::Scenes(ScenesState::default())
    }

    pub fn scene(id: &str, title: &str) -> Self {
        Screen::Scene(SceneState::new(id, title))
    }

    pub fn settings(page: SettingsPage) -> Self {
        Screen::Settings(SettingsState { page })
    }

    /// A fresh screen for a navigation bar section.
    pub fn for_section(section: Section) -> Self {
        match section {
            Section::Home => Screen::home(),
            Section::Scenes => Screen::scenes(),
            Section::Settings => Screen::settings(SettingsPage::default()),
        }
    }

    pub fn section(&self) -> Option<Section> {
        match self {
            Screen::Home(_) => Some(Section::Home),
            Screen::Scenes(_) => Some(Section::Scenes),
            Screen::Settings(_) => Some(Section::Settings),
            Screen::Scene(_) => None,
        }
    }

    /// The tab's title while this screen is showing.
    pub fn title(&self) -> String {
        match self {
            Screen::Home(_) => "Home".into(),
            Screen::Scenes(_) => "Scenes".into(),
            Screen::Scene(s) => s.title.clone(),
            Screen::Settings(_) => "Settings".into(),
        }
    }

    /// Only the fields that are saved: what a restored screen starts from.
    pub fn persistent(&self) -> Screen {
        match self {
            Screen::Home(_) => Screen::home(),
            Screen::Scenes(s) => Screen::Scenes(s.persistent()),
            Screen::Scene(s) => Screen::Scene(s.persistent()),
            other => other.clone(),
        }
    }

    /// Entry actions when the screen becomes active in tab `tab`. A screen that already has its
    /// data asks for nothing, so switching back to it never reloads (B1).
    pub fn enter(&mut self, tab: TabId) -> Vec<Effect> {
        match self {
            Screen::Home(h) if h.needs_load() => vec![Effect::LoadSummary { tab }],
            Screen::Scenes(s) => s.enter(tab),
            Screen::Scene(s) => s.enter(tab),
            _ => Vec::new(),
        }
    }

    /// Nothing to show yet and nothing on its way (retried when the connection comes up).
    pub fn needs_load(&self) -> bool {
        match self {
            Screen::Home(h) => h.needs_load(),
            Screen::Scenes(s) => s.needs_load(),
            Screen::Scene(s) => s.needs_load(),
            _ => false,
        }
    }

    /// The screen's body.
    pub fn view<'a>(&'a self, ctx: &Context<'a>, tab: TabId) -> Element<'a, ShellMsg> {
        match self {
            Screen::Home(h) => h.view(ctx),
            Screen::Scenes(s) => scenes::view(s, ctx, tab),
            Screen::Scene(s) => s.view(&ctx.layout),
            Screen::Settings(s) => settings_view(s),
        }
    }
}

fn settings_view(state: &SettingsState) -> Element<'_, ShellMsg> {
    let mut pages = column![].spacing(4).width(Length::Fixed(200.0));
    for page in SettingsPage::ALL {
        pages = pages.push(
            button(text(page.label()))
                .width(Length::Fill)
                .padding([6, 10])
                .style(theme::menu_row(page == state.page))
                .on_press(ShellMsg::SettingsPage(page)),
        );
    }
    let body: Element<'_, ShellMsg> = match state.page {
        SettingsPage::Keyboard => keymap::help_list(),
        page => text(format!("{} arrives with US4.", page.label()))
            .color(theme::MUTED)
            .into(),
    };
    row![
        pages,
        container(body).padding([0, 24]).width(Length::Fill),
        Space::new().width(0)
    ]
    .padding(16)
    .into()
}
