//! The Session state: one active server, as regions active at the same time (007 research R1):
//! the connection, the shell (navigation bar, tabs, screens) with its overlays, and playback.

pub mod connection;
pub mod playback;
pub mod snapshot;

use std::sync::Arc;

use iced::keyboard::{Key, Modifiers};
use iced::widget::{button, center, column, container, opaque, stack, text};
use iced::Element;
use stash_core::connection::snapshot::{ConnectionSnapshot, SessionState};
use stash_core::profiles::{ProfileDraft, ServerProfile};
use stash_core::AppError;
use uuid::Uuid;

use crate::effects::{Effect, Reply};
use crate::machine::Step;
use crate::onboarding;
use crate::player::video::Shared;
use crate::screens::Context;
use crate::shell::keymap::{self, Action, Level};
use crate::shell::overlays::{self, KeyPrompt, Overlay, KEY_PROMPT_ID};
use crate::shell::{self as shell_mod, Shell, ShellMsg};
use crate::widgets::theme;

pub struct Session {
    pub profile: ServerProfile,
    pub servers: Vec<ServerProfile>,
    pub connection: connection::Connection,
    pub playback: playback::Playback,
    pub overlay: Overlay,
    pub shell: Shell,
    ever_connected: bool,
    /// The saved session has been read (or there was none): saving can't overwrite it now.
    restored: bool,
    /// Bumped on every change to the shell; only the latest pending save is written.
    save_generation: u64,
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
    Key(Key, Modifiers),
    Shell(ShellMsg),
    /// A debounced save came due.
    SaveDue(u64),
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
    /// Enter the session for `profile`: connect, load the saved servers for the menu, and restore
    /// this server's tabs.
    pub fn enter(profile: ServerProfile, launch: bool) -> (Self, Vec<Effect>) {
        let (shell, shell_effects) = Shell::new();
        let mut effects = vec![
            Effect::Connect {
                profile: profile.id,
                launch,
            },
            Effect::LoadProfiles,
            Effect::LoadSession {
                profile: profile.id,
            },
        ];
        effects.extend(shell_effects);
        (
            Self {
                servers: vec![profile.clone()],
                profile,
                connection: connection::Connection::default(),
                playback: playback::Playback::default(),
                overlay: Overlay::None,
                shell,
                ever_connected: false,
                restored: false,
                save_generation: 0,
            },
            effects,
        )
    }

    /// Save now, before leaving (quit or a server switch); nothing until the saved session was
    /// read, so a fresh shell never replaces it.
    pub fn save_now(&self) -> Option<Effect> {
        self.restored.then(|| Effect::SaveSessionNow {
            profile: self.profile.id,
            session: self.shell.capture(),
        })
    }

    fn unreachable(&self) -> bool {
        matches!(
            self.connection.snapshot.state,
            SessionState::Offline { .. }
                | SessionState::Failed { .. }
                | SessionState::AuthFailed { .. }
        )
    }

    fn shell(&mut self, msg: ShellMsg) -> Step<Up> {
        let Step { mut effects, up } = self.shell.update(msg);
        if self.restored {
            self.save_generation += 1;
            effects.push(Effect::SaveSessionLater {
                generation: self.save_generation,
            });
        }
        let step = Step::effects(effects);
        match up {
            None => step,
            Some(shell_mod::Up::Play(id)) => step.with(Effect::OpenScene(id)),
            Some(shell_mod::Up::ServerMenu) => {
                self.overlay = Overlay::ServerMenu;
                step.with(Effect::LoadProfiles)
            }
            Some(shell_mod::Up::KeyboardHelp) => {
                self.overlay = Overlay::KeyboardHelp;
                step
            }
            // The notification centre arrives with US5.
            Some(shell_mod::Up::Notifications) => step,
        }
    }

    fn key(&mut self, key: &Key, mods: Modifiers) -> Step<Up> {
        if self.overlay != Overlay::None {
            if *key == Key::Named(iced::keyboard::key::Named::Escape) {
                self.overlay = Overlay::None;
            }
            return Step::none();
        }
        if self.playback.active() {
            // The player is the innermost level and owns the window while it's open.
            let plain = !mods.control() && !mods.alt() && !mods.logo();
            if plain {
                if let Some(effects) = self.playback.key(key) {
                    return Step::effects(effects);
                }
            }
            return Step::none();
        }
        let msg = match keymap::resolve(key, mods, &[Level::Shell]) {
            Some(Action::NewTab) => ShellMsg::NewTab,
            Some(Action::CloseTab) => ShellMsg::CloseActive,
            Some(Action::NextTab) => ShellMsg::NextTab,
            Some(Action::PrevTab) => ShellMsg::PrevTab,
            Some(Action::SelectTab(i)) => ShellMsg::SelectIndex(i),
            Some(Action::MoveTabLeft) => ShellMsg::MoveLeft,
            Some(Action::MoveTabRight) => ShellMsg::MoveRight,
            Some(Action::Back) => ShellMsg::Back,
            Some(Action::Forward) => ShellMsg::Forward,
            Some(Action::KeyboardHelp) => ShellMsg::KeyboardHelp,
            Some(Action::FocusNext) => return Step::effect(Effect::FocusNext),
            Some(Action::FocusPrevious) => return Step::effect(Effect::FocusPrevious),
            Some(Action::Player) | None => return Step::none(),
        };
        self.shell(msg)
    }

    pub fn update(&mut self, msg: Msg) -> Step<Up> {
        match msg {
            Msg::Connection(snapshot) => {
                if let Some(info) = &snapshot.server {
                    self.shell.server_info(info);
                }
                let up = self.connection.observe(snapshot);
                if self.unreachable() {
                    self.shell.unreachable();
                }
                self.connection_event(up)
            }
            Msg::Shell(m) => self.shell(m),
            Msg::SaveDue(generation) => {
                if generation == self.save_generation && self.restored {
                    Step::effect(Effect::SaveSession {
                        profile: self.profile.id,
                        session: self.shell.capture(),
                    })
                } else {
                    Step::none()
                }
            }
            Msg::Reply(Reply::Summary { tab, info }) => {
                let unreachable = self.unreachable();
                self.shell.summary_loaded(tab, info, unreachable);
                Step::none()
            }
            Msg::Reply(Reply::SessionLoaded { profile, saved }) => {
                if profile != self.profile.id || self.restored {
                    return Step::none();
                }
                self.restored = true;
                match saved.and_then(Shell::restore) {
                    Some((shell, mut effects)) => {
                        self.shell = shell;
                        // Restored Home screens pick up a summary that's already here.
                        if let Some(info) = self.connection.snapshot.server.clone() {
                            self.shell.server_info(&info);
                            effects.retain(|e| !matches!(e, Effect::LoadSummary { .. }));
                        }
                        Step::effects(effects)
                    }
                    None => Step::none(),
                }
            }
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
            Msg::Key(key, mods) => self.key(&key, mods),
        }
    }

    fn connection_event(&mut self, up: Option<connection::Up>) -> Step<Up> {
        match up {
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
            let indicator = self
                .connection
                .indicator(&self.profile.display_name, ShellMsg::ServerMenu);
            let ctx = Context {
                profile: &self.profile,
                connection: &self.connection,
            };
            let page: Element<'a, ShellMsg> =
                column![self.shell.chrome(indicator), self.shell.content(&ctx)].into();
            page.map(Msg::Shell)
        };
        let overlay: Option<Element<'a, Msg>> = match &self.overlay {
            Overlay::None => None,
            Overlay::KeyboardHelp => Some(overlays::keyboard_help().map(Msg::Overlay)),
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
