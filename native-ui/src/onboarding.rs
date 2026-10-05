//! The Onboarding state: no saved server yet (007 data model "App"; spec US1, capability C1).
//!
//! One form: the server's address, an optional API key (hidden unless shown), and strict
//! certificate checking. Connecting tests the server and saves it in one step (the core checks the
//! address, the key, and the version first); on success the app enters the Session.

use iced::widget::{button, center, checkbox, column, container, row, text, text_input, Space};
use iced::{Alignment, Element, Length};
use stash_core::profiles::{ProfileDraft, ServerProfile};
use stash_core::AppError;

use crate::effects::{Effect, Reply};
use crate::machine::Step;
use crate::messages::for_error;
use crate::widgets::theme;

/// The address field's widget id (focused on entry).
pub const ADDRESS_ID: &str = "onboarding-address";
const KEY_ID: &str = "onboarding-key";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Editing,
    /// Testing and saving the server.
    Connecting,
    /// The last attempt failed; the form keeps what was typed.
    Failed(AppError),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerForm {
    /// Optional; the address is used when it's left empty.
    pub name: String,
    pub address: String,
    pub api_key: String,
    pub show_key: bool,
    pub strict_tls: bool,
    pub phase: Phase,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Name(String),
    Address(String),
    ApiKey(String),
    ToggleShowKey,
    StrictTls(bool),
    Submit,
    /// From the address field: move on to the key.
    NextField,
    Reply(Reply),
}

/// Events for the app.
#[derive(Debug, Clone, PartialEq)]
pub enum Up {
    /// A server was saved after its connection test succeeded.
    Saved(ServerProfile),
}

impl ServerForm {
    pub fn can_submit(&self) -> bool {
        !self.address.trim().is_empty() && self.phase != Phase::Connecting
    }

    fn edited(&mut self) {
        if matches!(self.phase, Phase::Failed(_)) {
            self.phase = Phase::Editing;
        }
    }

    pub fn update(&mut self, msg: Msg) -> Step<Up> {
        match msg {
            Msg::Name(n) => {
                self.name = n;
                self.edited();
                Step::none()
            }
            Msg::Address(a) => {
                self.address = a;
                self.edited();
                Step::none()
            }
            Msg::ApiKey(k) => {
                self.api_key = k;
                self.edited();
                Step::none()
            }
            Msg::ToggleShowKey => {
                self.show_key = !self.show_key;
                Step::none()
            }
            Msg::StrictTls(on) => {
                self.strict_tls = on;
                self.edited();
                Step::none()
            }
            Msg::NextField => Step::effect(Effect::Focus(KEY_ID)),
            Msg::Submit => {
                if !self.can_submit() {
                    return Step::none();
                }
                self.phase = Phase::Connecting;
                Step::effect(Effect::CreateProfile(ProfileDraft {
                    display_name: (!self.name.trim().is_empty())
                        .then(|| self.name.trim().to_owned()),
                    address: self.address.clone(),
                    api_key: (!self.api_key.trim().is_empty()).then(|| self.api_key.clone()),
                    strict_tls: self.strict_tls,
                }))
            }
            Msg::Reply(Reply::ProfileCreated(Ok(profile))) => Step::up(Up::Saved(profile)),
            Msg::Reply(Reply::ProfileCreated(Err(e))) => {
                self.phase = Phase::Failed(e);
                Step::effect(Effect::Focus(ADDRESS_ID))
            }
            Msg::Reply(_) => Step::none(),
        }
    }

    /// The whole first-run screen.
    pub fn view(&self) -> Element<'_, Msg> {
        center(self.form_view()).into()
    }

    /// The form itself (also shown in the session's "Add a server" overlay).
    pub fn form_view(&self) -> Element<'_, Msg> {
        let connecting = self.phase == Phase::Connecting;
        let address = text_input(
            "192.168.1.10:9999 or https://stash.example.com",
            &self.address,
        )
        .id(ADDRESS_ID)
        .on_input_maybe((!connecting).then_some(Msg::Address))
        .on_submit(Msg::NextField)
        .padding(10)
        .size(16);
        let key = text_input("API key (only if Stash asks for one)", &self.api_key)
            .id(KEY_ID)
            .secure(!self.show_key)
            .on_input_maybe((!connecting).then_some(Msg::ApiKey))
            .on_submit(Msg::Submit)
            .padding(10)
            .size(16);
        let key_row = row![
            key,
            button(text(if self.show_key { "Hide" } else { "Show" }).size(14))
                .on_press(Msg::ToggleShowKey)
                .style(button::text),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        let strict = checkbox(self.strict_tls)
            .label("Verify the server's certificate (strict)")
            .on_toggle_maybe((!connecting).then_some(Msg::StrictTls));
        let connect = button(
            text(if connecting {
                "Connecting…"
            } else {
                "Connect"
            })
            .size(16)
            .width(Length::Fill)
            .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .padding(10)
        .on_press_maybe(self.can_submit().then_some(Msg::Submit));

        let mut form = column![
            text("Connect to Stash").size(28),
            text("Enter your Stash server's address. The viewer checks it before saving.").size(14),
            Space::new().height(8),
            text("Address").size(13),
            address,
            text("Name (optional)").size(13),
            text_input("For example: Home server", &self.name)
                .on_input_maybe((!connecting).then_some(Msg::Name))
                .on_submit(Msg::NextField)
                .padding(10)
                .size(16),
            text("API key").size(13),
            key_row,
            strict,
            Space::new().height(8),
            connect,
        ]
        .spacing(8)
        .width(Length::Fixed(460.0));

        if let Phase::Failed(error) = &self.phase {
            let m = for_error(error);
            let mut failure = column![text(m.title).size(16), text(m.detail).size(14)].spacing(4);
            if let Some(hint) = m.hint {
                failure = failure.push(text(hint).size(13));
            }
            form = form.push(
                container(failure)
                    .padding(12)
                    .width(Length::Fill)
                    .style(theme::problem),
            );
        }
        form.into()
    }
}
