//! Overlays above the session (007 data model "Shell"): one at a time, Escape closes it. US1 adds
//! the key prompt and the server menu, US2 keyboard help; US5 adds notifications.

use iced::widget::{button, column, container, row, text, text_input, Space};
use iced::{Alignment, Element, Length};
use stash_core::connection::snapshot::ConnectionSnapshot;
use stash_core::profiles::ServerProfile;
use stash_core::AppError;
use uuid::Uuid;

use crate::messages::for_error;
use crate::onboarding::ServerForm;
use crate::widgets::{icon, theme, Icon};

pub const KEY_PROMPT_ID: &str = "key-prompt";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Overlay {
    #[default]
    None,
    /// The server rejected or needs a key (001 FR-017): enter the current one.
    KeyPrompt(KeyPrompt),
    /// Switch servers, see the connection's details, add a server.
    ServerMenu,
    /// Add another server (the onboarding form); the app switches to it once it connects.
    AddServer(ServerForm),
    /// Every keyboard shortcut (F1 or ?).
    KeyboardHelp,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KeyPrompt {
    pub key: String,
    pub saving: bool,
    pub error: Option<AppError>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    KeyChanged(String),
    SubmitKey,
    SwitchTo(Uuid),
    AddServer,
    Close,
}

/// The key prompt's view.
pub fn key_prompt(prompt: &KeyPrompt, server: &ServerProfile) -> Element<'static, Msg> {
    let mut body = column![
        text(format!("{} needs an API key", server.display_name)).size(20),
        text("Stash rejected the saved key, or now asks for one. Paste the current key from Stash's Settings → Security.").size(14),
        text_input("API key", &prompt.key)
            .id(KEY_PROMPT_ID)
            .secure(true)
            .on_input_maybe((!prompt.saving).then_some(Msg::KeyChanged))
            .on_submit(Msg::SubmitKey)
            .padding(10),
        row![
            button(text("Cancel")).on_press(Msg::Close).style(button::secondary),
            Space::new().width(Length::Fill),
            button(text(if prompt.saving { "Checking…" } else { "Save and connect" }))
                .on_press_maybe((!prompt.saving && !prompt.key.trim().is_empty()).then_some(Msg::SubmitKey)),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(10);
    if let Some(e) = &prompt.error {
        let m = for_error(e);
        body = body.push(text(format!("{} {}", m.title, m.detail)).size(13));
    }
    panel(body.into(), 440.0)
}

/// The server menu: the active server's connection details, every saved server to switch to,
/// and a way to manage them.
pub fn server_menu<'a>(
    current: &'a ServerProfile,
    servers: &'a [ServerProfile],
    connection: &'a ConnectionSnapshot,
    security: &'a str,
) -> Element<'a, Msg> {
    let mut details = column![
        text(current.display_name.clone()).size(18),
        text(current.base_url.to_string()).size(13),
        text(security.to_owned()).size(13),
    ]
    .spacing(2);
    if let Some(info) = &connection.server {
        details = details.push(
            text(format!(
                "Stash {} · {} scenes · {} images",
                info.version, info.counts.scenes, info.counts.images
            ))
            .size(13),
        );
    }
    let mut list = column![text("Switch server").size(13)].spacing(4);
    for s in servers {
        let active = s.id == current.id;
        list = list.push(
            button(
                row![
                    icon(Icon::Server, 16.0),
                    text(s.display_name.clone()).width(Length::Fill),
                    text(if active { "Active" } else { "" }).size(12),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .padding([6, 8])
            .style(theme::menu_row(active))
            .on_press_maybe((!active).then_some(Msg::SwitchTo(s.id))),
        );
    }
    panel(
        column![
            details,
            list,
            row![
                button(text("Add a server…"))
                    .on_press(Msg::AddServer)
                    .style(button::secondary),
                Space::new().width(Length::Fill),
                button(text("Close"))
                    .on_press(Msg::Close)
                    .style(button::text),
            ],
        ]
        .spacing(14)
        .into(),
        400.0,
    )
}

/// The keyboard help: the keymap's table.
pub fn keyboard_help() -> Element<'static, Msg> {
    panel(
        column![
            row![
                text("Keyboard shortcuts").size(20),
                Space::new().width(Length::Fill),
                button(text("Close"))
                    .on_press(Msg::Close)
                    .style(button::text),
            ]
            .align_y(Alignment::Center),
            container(super::keymap::help_list()).height(Length::Fixed(520.0)),
        ]
        .spacing(12)
        .into(),
        460.0,
    )
}

fn panel<'a>(content: Element<'a, Msg>, width: f32) -> Element<'a, Msg> {
    container(
        container(content)
            .padding(18)
            .width(Length::Fixed(width))
            .style(theme::panel),
    )
    .padding(12)
    .into()
}
