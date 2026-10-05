//! The Settings screen (007 T056; capabilities C4, C13): Servers, Keyboard, Troubleshooting, and
//! About. The page shown is saved with the tab; each page's data loads when it opens.

pub mod about;
pub mod servers;
pub mod troubleshooting;

use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length};
use serde::{Deserialize, Serialize};

pub use servers::{EditForm, Editor, ServersMsg, ServersState};
pub use troubleshooting::{Clear, TroubleMsg, Troubleshooting};

use crate::effects::Effect;
use crate::machine::Step;
use crate::screens::Context;
use crate::shell::keymap;
use crate::shell::ShellMsg;
use crate::widgets::theme;

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

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SettingsState {
    pub page: SettingsPage,
    /// Boxed: the editor is large next to the other screens' state.
    #[serde(skip)]
    pub servers: Box<ServersState>,
    #[serde(skip)]
    pub troubleshooting: Troubleshooting,
}

#[derive(Debug, Clone)]
pub enum SettingsMsg {
    Servers(ServersMsg),
    Trouble(TroubleMsg),
}

/// Events for the shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsUp {
    /// Open the "Add a server" form.
    AddServer,
}

impl SettingsState {
    pub fn at(page: SettingsPage) -> Self {
        Self {
            page,
            ..Self::default()
        }
    }

    /// Only the saved fields.
    pub fn persistent(&self) -> Self {
        Self::at(self.page)
    }

    /// The page's entry actions.
    pub fn enter(&mut self) -> Vec<Effect> {
        match self.page {
            SettingsPage::Servers => vec![Effect::LoadProfiles],
            SettingsPage::Troubleshooting => self.troubleshooting.enter(),
            SettingsPage::Keyboard | SettingsPage::About => Vec::new(),
        }
    }

    /// Show `page`.
    pub fn open(&mut self, page: SettingsPage) -> Vec<Effect> {
        if page == self.page {
            return Vec::new();
        }
        self.page = page;
        self.enter()
    }

    pub fn update(&mut self, msg: SettingsMsg) -> Step<SettingsUp> {
        match msg {
            SettingsMsg::Servers(m) => {
                let (effects, add) = self.servers.update(m);
                Step {
                    effects,
                    up: add.then_some(SettingsUp::AddServer),
                }
            }
            SettingsMsg::Trouble(m) => Step::effects(self.troubleshooting.update(m)),
        }
    }

    pub fn view<'a>(&'a self, ctx: &Context<'a>) -> Element<'a, ShellMsg> {
        let mut pages = column![].spacing(4).width(Length::Fixed(200.0));
        for page in SettingsPage::ALL {
            pages = pages.push(
                button(text(page.label()))
                    .width(Length::Fill)
                    .padding([6, 10])
                    .style(theme::menu_row(page == self.page))
                    .on_press(ShellMsg::SettingsPage(page)),
            );
        }
        let body: Element<'a, ShellMsg> = match self.page {
            SettingsPage::Servers => self.servers.view(ctx.profile.id),
            SettingsPage::Keyboard => column![text("Keyboard").size(22), keymap::help_list()]
                .spacing(10)
                .into(),
            SettingsPage::Troubleshooting => {
                let logs = crate::services::paths::Paths::resolve()
                    .map(|p| p.logs().display().to_string())
                    .unwrap_or_else(|| "the app's data folder".into());
                self.troubleshooting.view(logs)
            }
            SettingsPage::About => about::view(ctx),
        };
        let body: Element<'a, ShellMsg> = if self.page == SettingsPage::Keyboard {
            body
        } else {
            scrollable(container(body).width(Length::Fill).max_width(760.0))
                .height(Length::Fill)
                .into()
        };
        row![pages, container(body).padding([0, 24]).width(Length::Fill)]
            .padding(16)
            .into()
    }
}
