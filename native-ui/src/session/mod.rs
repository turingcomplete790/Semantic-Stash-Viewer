//! The Session state: one active server, as regions active at the same time (007 research R1):
//! the connection, the shell (overlays now; tabs in US2), and playback.

pub mod connection;
pub mod playback;

use std::sync::Arc;

use iced::widget::{button, center, column, container, opaque, row, stack, text, Space};
use iced::{Element, Length};
use stash_core::connection::snapshot::ConnectionSnapshot;
use stash_core::profiles::{ProfileDraft, ServerProfile};
use stash_core::AppError;
use uuid::Uuid;

use crate::effects::{Effect, Reply};
use crate::machine::Step;
use crate::onboarding;
use crate::player::video::Shared;
use crate::shell::overlays::{self, KeyPrompt, Overlay, KEY_PROMPT_ID};
use crate::widgets::theme;

pub struct Session {
    pub profile: ServerProfile,
    pub servers: Vec<ServerProfile>,
    pub connection: connection::Connection,
    pub playback: playback::Playback,
    pub overlay: Overlay,
    ever_connected: bool,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Connection(ConnectionSnapshot),
    Playback(playback::Msg),
    Overlay(overlays::Msg),
    OpenServerMenu,
    SwitchServer(Uuid),
    /// The "Add a server" overlay's form.
    AddServer(onboarding::Msg),
    SceneOpened(Result<(), AppError>),
    Reply(Reply),
    Key(iced::keyboard::Key),
}

/// Events for the app.
#[derive(Debug, Clone, PartialEq)]
pub enum Up {
    /// The session connected for the first time since it was entered (cold-start mark).
    FirstConnected,
    /// The connection came back after a loss.
    Reconnected,
    /// Leave this session for another saved server.
    Switch(Uuid),
}

impl Session {
    /// Enter the session for `profile`: connect, and load the saved servers for the menu.
    pub fn enter(profile: ServerProfile, launch: bool) -> (Self, Vec<Effect>) {
        let effects = vec![
            Effect::Connect {
                profile: profile.id,
                launch,
            },
            Effect::LoadProfiles,
        ];
        (
            Self {
                servers: vec![profile.clone()],
                profile,
                connection: connection::Connection::default(),
                playback: playback::Playback::default(),
                overlay: Overlay::None,
                ever_connected: false,
            },
            effects,
        )
    }

    pub fn update(&mut self, msg: Msg) -> Step<Up> {
        match msg {
            Msg::Connection(snapshot) => match self.connection.observe(snapshot) {
                Some(connection::Up::BecameConnected) => {
                    let first = !self.ever_connected;
                    self.ever_connected = true;
                    Step::up(if first {
                        Up::FirstConnected
                    } else {
                        Up::Reconnected
                    })
                }
                Some(connection::Up::KeyNeeded) => {
                    self.overlay = Overlay::KeyPrompt(KeyPrompt::default());
                    Step::effect(Effect::Focus(KEY_PROMPT_ID))
                }
                None => Step::none(),
            },
            Msg::Playback(m) => Step::effects(self.playback.update(m)),
            Msg::Overlay(m) => self.overlay(m),
            Msg::OpenServerMenu => {
                self.overlay = Overlay::ServerMenu;
                Step::effect(Effect::LoadProfiles)
            }
            Msg::SwitchServer(id) => {
                if id == self.profile.id {
                    Step::none()
                } else {
                    Step::up(Up::Switch(id))
                }
            }
            Msg::SceneOpened(Ok(())) => Step::none(),
            Msg::SceneOpened(Err(e)) => {
                tracing::warn!(error = %e, "couldn't open the scene");
                Step::none()
            }
            Msg::AddServer(m) => {
                let Overlay::AddServer(form) = &mut self.overlay else {
                    return Step::none();
                };
                let Step { effects, up } = form.update(m);
                match up {
                    // Saved and tested: switch to it (001 US4).
                    Some(onboarding::Up::Saved(profile)) => {
                        self.overlay = Overlay::None;
                        let id = profile.id;
                        self.servers.push(profile);
                        Step {
                            effects,
                            up: Some(Up::Switch(id)),
                        }
                    }
                    None => Step::effects(effects),
                }
            }
            Msg::Reply(reply @ Reply::ProfileCreated(_)) => {
                self.update(Msg::AddServer(onboarding::Msg::Reply(reply)))
            }
            Msg::Reply(Reply::Profiles(list)) => {
                self.servers = list;
                Step::none()
            }
            Msg::Reply(Reply::ProfileUpdated(Ok(profile))) => {
                self.profile = profile;
                self.overlay = Overlay::None;
                Step::effect(Effect::Connect {
                    profile: self.profile.id,
                    launch: false,
                })
            }
            Msg::Reply(Reply::ProfileUpdated(Err(e))) => {
                if let Overlay::KeyPrompt(prompt) = &mut self.overlay {
                    prompt.saving = false;
                    prompt.error = Some(e);
                }
                Step::none()
            }
            Msg::Reply(_) => Step::none(),
            Msg::Key(key) => {
                if self.overlay != Overlay::None {
                    if key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) {
                        self.overlay = Overlay::None;
                    }
                    return Step::none();
                }
                if self.playback.active() {
                    if let Some(effects) = self.playback.key(&key) {
                        return Step::effects(effects);
                    }
                }
                Step::none()
            }
        }
    }

    fn overlay(&mut self, msg: overlays::Msg) -> Step<Up> {
        match msg {
            overlays::Msg::KeyChanged(k) => {
                if let Overlay::KeyPrompt(prompt) = &mut self.overlay {
                    prompt.key = k;
                    prompt.error = None;
                }
                Step::none()
            }
            overlays::Msg::SubmitKey => {
                let Overlay::KeyPrompt(prompt) = &mut self.overlay else {
                    return Step::none();
                };
                if prompt.saving || prompt.key.trim().is_empty() {
                    return Step::none();
                }
                prompt.saving = true;
                Step::effect(Effect::UpdateProfile {
                    id: self.profile.id,
                    draft: ProfileDraft {
                        display_name: Some(self.profile.display_name.clone()),
                        address: self.profile.base_url.to_string(),
                        api_key: Some(prompt.key.clone()),
                        strict_tls: self.profile.strict_tls,
                    },
                })
            }
            overlays::Msg::SwitchTo(id) => {
                self.overlay = Overlay::None;
                self.update(Msg::SwitchServer(id))
            }
            overlays::Msg::AddServer => {
                self.overlay = Overlay::AddServer(onboarding::ServerForm::default());
                Step::effect(Effect::Focus(onboarding::ADDRESS_ID))
            }
            overlays::Msg::Close => {
                self.overlay = Overlay::None;
                Step::none()
            }
        }
    }

    pub fn view<'a>(
        &'a self,
        video: Option<&'a Arc<Shared>>,
        video_error: Option<String>,
    ) -> Element<'a, Msg> {
        let base: Element<'a, Msg> = if self.playback.active() {
            self.playback.screen.view(
                video,
                video_error,
                |m| Msg::Playback(playback::Msg::Controls(m)),
                Msg::Playback(playback::Msg::Close),
                Msg::Playback(playback::Msg::ToggleFullscreen),
            )
        } else {
            // The navigation bar and tabs arrive in US2; until then, the indicator and a summary.
            let bar = container(
                row![
                    self.connection
                        .indicator(&self.profile.display_name, Msg::OpenServerMenu),
                    Space::new().width(Length::Fill),
                ]
                .padding([4, 8]),
            )
            .style(theme::bar)
            .width(Length::Fill);
            let mut summary = column![
                text(self.profile.display_name.clone()).size(26),
                text(self.profile.base_url.to_string()).size(13),
                text(self.connection.label()).size(15),
            ]
            .spacing(6)
            .align_x(iced::Alignment::Center);
            if let Some(info) = &self.connection.snapshot.server {
                summary = summary.push(
                    text(format!(
                        "Stash {} · {} scenes · {} images · {} galleries · {} performers",
                        info.version,
                        info.counts.scenes,
                        info.counts.images,
                        info.counts.galleries,
                        info.counts.performers
                    ))
                    .size(13),
                );
            }
            column![bar, center(summary)].into()
        };
        let overlay: Option<Element<'a, Msg>> = match &self.overlay {
            Overlay::None => None,
            Overlay::KeyPrompt(prompt) => {
                Some(overlays::key_prompt(prompt, &self.profile).map(Msg::Overlay))
            }
            Overlay::AddServer(form) => Some(
                container(
                    column![
                        form.form_view().map(Msg::AddServer),
                        button(text("Cancel"))
                            .on_press(Msg::Overlay(overlays::Msg::Close))
                            .style(button::secondary),
                    ]
                    .spacing(12),
                )
                .padding(18)
                .style(theme::panel)
                .into(),
            ),
            Overlay::ServerMenu => Some(
                overlays::server_menu(
                    &self.profile,
                    &self.servers,
                    &self.connection.snapshot,
                    self.connection.security_label(),
                )
                .map(Msg::Overlay),
            ),
        };
        match overlay {
            None => base,
            Some(panel) => stack![base, opaque(center(panel))].into(),
        }
    }
}
