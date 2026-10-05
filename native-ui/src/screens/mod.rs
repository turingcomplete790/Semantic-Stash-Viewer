//! The screens a tab shows (007 data model "Screen"): whole states kept in the tab's history, so
//! going back returns exactly what was left (behaviour that stays B1). Each screen's persistent
//! fields are saved with the session; transient ones (`#[serde(skip)]`) are rebuilt by its entry
//! effects. Home is complete here; Scenes, Scene, and Settings fill in with US3 and US4.

pub mod home;
pub mod scene;
pub mod scenes;
pub mod settings;

use iced::Element;
use serde::{Deserialize, Serialize};
use stash_core::profiles::ServerProfile;

use crate::effects::Effect;
use crate::session::connection::Connection;
use crate::shell::{ShellMsg, TabId};

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
pub use settings::{SettingsPage, SettingsState};

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
        Screen::Settings(SettingsState::at(page))
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
            Screen::Settings(s) => Screen::Settings(s.persistent()),
        }
    }

    /// Entry actions when the screen becomes active in tab `tab`. A screen that already has its
    /// data asks for nothing, so switching back to it never reloads (B1).
    pub fn enter(&mut self, tab: TabId) -> Vec<Effect> {
        match self {
            Screen::Home(h) if h.needs_load() => vec![Effect::LoadSummary { tab }],
            Screen::Scenes(s) => s.enter(tab),
            Screen::Scene(s) => s.enter(tab),
            Screen::Settings(s) => s.enter(),
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
            Screen::Settings(s) => s.view(ctx),
        }
    }
}
