//! The Session state: one active server, as regions active at the same time (007 research R1):
//! the connection, the shell (navigation bar, tabs, screens) with its overlays, and playback.

pub mod connection;
pub mod playback;
pub mod snapshot;

use std::sync::Arc;

use iced::keyboard::{Key, Modifiers};
use iced::widget::{button, column, container, mouse_area, opaque, stack, text, Space};
use iced::Element;
use stash_core::connection::snapshot::{ConnectionSnapshot, SessionState};
use stash_core::profiles::{ProfileDraft, ServerProfile};
use stash_core::AppError;
use uuid::Uuid;

use crate::effects::{Effect, Reply};
use crate::machine::Step;
use crate::onboarding;
use crate::player::video::Shared;
use crate::screens::scenes::layout::Layout;
use crate::screens::scenes::{ScenesMsg, Thumbs};
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
    /// Thumbnails for every tab.
    pub thumbs: Thumbs,
    /// The notification centre's entries and toasts.
    pub notifications: crate::shell::notifications::Notifications,
    /// Held modifier keys (Ctrl+click opens in a new tab).
    modifiers: Modifiers,
    ever_connected: bool,
    /// The saved session has been read (or there was none): saving can't overwrite it now.
    restored: bool,
    /// Bumped on every change to the shell; only the latest pending save is written.
    save_generation: u64,
    /// The UI bench is running: its tabs aren't saved.
    pub saving_suspended: bool,
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
    Modifiers(Modifiers),
    /// Cached data changed (a refresh found something new, or a clear: `*`).
    CacheChanged(String),
    /// The window's size changed.
    Layout(Layout),
    /// The now-playing bar: go to the playing scene's tab.
    BackToScene,
    /// The mouse's back (`true`) or forward button.
    MouseHistory(bool),
    Notifications(crate::shell::notifications::Msg),
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
        let profile_id = profile.id;
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
                thumbs: Thumbs::default(),
                notifications: crate::shell::notifications::Notifications::for_profile(profile_id),
                modifiers: Modifiers::empty(),
                ever_connected: false,
                restored: false,
                save_generation: 0,
                saving_suspended: false,
            },
            effects,
        )
    }

    /// Save now, before leaving (quit or a server switch); nothing until the saved session was
    /// read, so a fresh shell never replaces it.
    pub fn save_now(&self) -> Option<Effect> {
        (self.restored && !self.saving_suspended).then(|| Effect::SaveSessionNow {
            profile: self.profile.id,
            session: self.shell.capture(),
        })
    }

    /// Leaving this session (quit or a server switch): save its tabs and stop playback.
    pub fn leave(&self) -> Vec<Effect> {
        let mut effects: Vec<Effect> = self.save_now().into_iter().collect();
        if self.playback.active() {
            effects.extend(self.playback_close());
        }
        effects
    }

    fn playback_close(&self) -> Vec<Effect> {
        let mut effects = vec![Effect::Player(crate::effects::PlayerAction::Close)];
        if self.playback.snapshot().fullscreen {
            effects.push(Effect::SetFullscreen(false));
        }
        effects
    }

    /// Whether the player is on screen: fullscreen, or the owner tab showing the playing scene.
    pub fn player_visible(&self) -> bool {
        if !self.playback.active() {
            return false;
        }
        if self.playback.snapshot().fullscreen {
            return true;
        }
        let tab = self.shell.active();
        self.playback.owner == Some(tab.id)
            && matches!(
                tab.current(),
                crate::screens::Screen::Scene(s)
                    if Some(s.scene_id.as_str()) == self.playback.playing_scene()
            )
    }

    /// Whether the now-playing bar shows (something plays, out of sight).
    pub fn now_playing(&self) -> bool {
        self.playback.active() && !self.player_visible()
    }

    fn back_to_scene(&mut self) -> Step<Up> {
        let (Some(owner), Some(id)) = (
            self.playback.owner,
            self.playback.playing_scene().map(str::to_owned),
        ) else {
            return Step::none();
        };
        let mut step = self.shell(ShellMsg::Select(owner));
        if !self.player_visible() {
            // The owner tab moved on from the scene: open it there again.
            let more = self.shell(ShellMsg::Open(crate::screens::Screen::scene(&id, "")));
            step.effects.extend(more.effects);
        }
        step
    }

    fn unreachable(&self) -> bool {
        matches!(
            self.connection.snapshot.state,
            SessionState::Offline { .. }
                | SessionState::Failed { .. }
                | SessionState::AuthFailed { .. }
        )
    }

    /// The session is still connecting, so a failed read isn't final yet.
    fn waiting(&self) -> bool {
        !self.connection.connected() && !self.unreachable()
    }

    /// Thumbnails for what the active screen shows.
    fn thumbnails(&mut self) -> Vec<Effect> {
        let width = self.thumb_width();
        self.thumbs.want_cards(self.shell.active_cards(), width)
    }

    /// The width thumbnails are decoded at: a grid card's (list rows draw them smaller), in steps
    /// of 32 px so small window changes reuse them.
    fn thumb_width(&self) -> u32 {
        let w = self.shell.layout.card_width().ceil() as u32;
        w.div_ceil(32) * 32
    }

    fn shell(&mut self, msg: ShellMsg) -> Step<Up> {
        // A click with Ctrl held opens the scene in a new tab.
        let msg = match msg {
            ShellMsg::Scenes(ScenesMsg::Open {
                index,
                new_tab: false,
            }) if self.modifiers.control() => ShellMsg::Scenes(ScenesMsg::Open {
                index,
                new_tab: true,
            }),
            other => other,
        };
        let Step { mut effects, up } = self.shell.update(msg);
        effects.extend(self.thumbnails());
        if self.restored {
            self.save_generation += 1;
            effects.push(Effect::SaveSessionLater {
                generation: self.save_generation,
            });
        }
        self.shell_up(Step::effects(effects), up)
    }

    fn shell_up(&mut self, step: Step<Up>, up: Option<shell_mod::Up>) -> Step<Up> {
        match up {
            None => step,
            Some(shell_mod::Up::Play(id)) => {
                let owner = self.shell.active().id;
                step.with_all(self.playback.open(id, owner))
            }
            Some(shell_mod::Up::Closed(tab)) => {
                if self.playback.active() && self.playback.owner == Some(tab) {
                    step.with_all(self.playback_close())
                } else {
                    step
                }
            }
            Some(shell_mod::Up::ServerMenu) => {
                self.overlay = Overlay::ServerMenu;
                step.with(Effect::LoadProfiles)
            }
            Some(shell_mod::Up::KeyboardHelp) => {
                self.overlay = Overlay::KeyboardHelp;
                step
            }
            Some(shell_mod::Up::Notifications) => {
                self.overlay = Overlay::Notifications;
                step.with_all(
                    self.notifications
                        .update(crate::shell::notifications::Msg::Opened),
                )
            }
        }
    }

    fn key(&mut self, key: &Key, mods: Modifiers) -> Step<Up> {
        if self.overlay != Overlay::None {
            if *key == Key::Named(iced::keyboard::key::Named::Escape) {
                self.overlay = Overlay::None;
            }
            return Step::none();
        }
        if self.player_visible() {
            // The player is the innermost level while it's on screen (002's keys); keys it
            // doesn't bind fall through to the shell.
            let plain = !mods.control() && !mods.alt() && !mods.logo();
            if plain {
                if let Some(effects) = self.playback.key(key) {
                    return Step::effects(effects);
                }
            }
            if self.playback.snapshot().fullscreen {
                return Step::none();
            }
        }
        if let Some(Step { mut effects, up }) = self.shell.screen_key(key, mods) {
            effects.extend(self.thumbnails());
            if self.restored {
                self.save_generation += 1;
                effects.push(Effect::SaveSessionLater {
                    generation: self.save_generation,
                });
            }
            return self.shell_up(Step::effects(effects), up);
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
            Some(Action::Player | Action::Scenes) | None => return Step::none(),
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
                let retry = if up == Some(connection::Up::BecameConnected) {
                    self.shell.retry_active()
                } else {
                    Vec::new()
                };
                self.connection_event(up).with_all(retry)
            }
            Msg::Modifiers(m) => {
                self.modifiers = m;
                Step::none()
            }
            Msg::Layout(layout) => {
                self.shell.layout = layout;
                Step::none()
            }
            Msg::CacheChanged(key) => Step::effects(self.shell.cache_changed(&key)),
            Msg::Reply(Reply::ScenesPage {
                tab,
                generation,
                result,
            }) => {
                let waiting = self.waiting();
                let mut effects = self.shell.scenes_page(tab, generation, result, waiting);
                effects.extend(self.thumbnails());
                Step::effects(effects)
            }
            Msg::Reply(Reply::Thumbnails(batch)) => {
                self.thumbs.insert_all(batch);
                Step::none()
            }
            Msg::Reply(Reply::Warm(keys)) => {
                let width = self.thumb_width();
                Step::effects(self.thumbs.want(&keys, width))
            }
            Msg::Shell(m) => self.shell(m),
            Msg::SaveDue(generation) => {
                if generation == self.save_generation && self.restored && !self.saving_suspended {
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
                        // The restored shell draws in this window: keep its size (a fresh
                        // shell's default left the grid building rows for a smaller window
                        // after a server switch, with an empty band below them).
                        let layout = self.shell.layout;
                        self.shell = shell;
                        self.shell.layout = layout;
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
                let m = crate::messages::for_error(&e);
                Step::effect(Effect::Notify {
                    title: m.title,
                    detail: Some(m.detail),
                })
            }
            Msg::BackToScene => self.back_to_scene(),
            Msg::Notifications(crate::shell::notifications::Msg::Close) => {
                self.overlay = Overlay::None;
                Step::none()
            }
            Msg::Notifications(m) => {
                let opened = self.overlay == Overlay::Notifications;
                let changed = matches!(m, crate::shell::notifications::Msg::Changed(_));
                let mut effects = self.notifications.update(m);
                // New entries while the panel is open have been seen.
                if opened && changed {
                    effects.extend(
                        self.notifications
                            .update(crate::shell::notifications::Msg::Opened),
                    );
                }
                Step::effects(effects)
            }
            Msg::MouseHistory(back) => {
                if self.overlay != Overlay::None
                    || (self.playback.active() && self.playback.snapshot().fullscreen)
                {
                    Step::none()
                } else {
                    self.shell(if back {
                        ShellMsg::Back
                    } else {
                        ShellMsg::Forward
                    })
                }
            }
            Msg::Reply(Reply::SceneDetails { tab, id, result }) => {
                self.shell.scene_details(tab, &id, *result);
                Step::none()
            }
            Msg::Reply(Reply::Cover { tab, id, cover }) => {
                self.shell.scene_cover(tab, &id, cover);
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

    /// Title, play/pause, back to the scene, and close, while the player is out of sight.
    fn now_playing_bar(&self) -> Element<'_, Msg> {
        let snap = self.playback.snapshot();
        let title = snap.title.clone().unwrap_or_else(|| "Playing".into());
        let state = match snap.state {
            player::PlayerStateKind::Loading => "Opening…",
            player::PlayerStateKind::Paused => "Paused",
            player::PlayerStateKind::Ended => "Ended",
            player::PlayerStateKind::Error => "Couldn't play",
            _ => "Playing",
        };
        let toggle = crate::widgets::icon_button(
            if snap.paused {
                crate::widgets::Icon::Play
            } else {
                crate::widgets::Icon::Pause
            },
            18.0,
            Some(Msg::Playback(playback::Msg::Controls(
                crate::player::view::Msg::TogglePause,
            ))),
        );
        container(
            iced::widget::row![
                toggle,
                crate::widgets::one_line(title, 14.0),
                text(state).size(12).color(theme::MUTED),
                button(text("Back to scene").size(13))
                    .style(theme::chip)
                    .padding([4, 12])
                    .on_press(Msg::BackToScene),
                crate::widgets::icon_button(
                    crate::widgets::Icon::Close,
                    16.0,
                    Some(Msg::Playback(playback::Msg::Close)),
                ),
            ]
            .spacing(12)
            .padding([6, 12])
            .align_y(iced::Alignment::Center),
        )
        .style(theme::bar)
        .width(iced::Length::Fill)
        .into()
    }

    pub fn view<'a>(
        &'a self,
        video: Option<&'a Arc<Shared>>,
        video_error: Option<String>,
    ) -> Element<'a, Msg> {
        let player = || {
            self.playback.screen.view(
                video,
                video_error.clone(),
                |m| Msg::Playback(playback::Msg::Controls(m)),
                Msg::Playback(playback::Msg::Close),
                Msg::Playback(playback::Msg::ToggleFullscreen),
            )
        };
        let base: Element<'a, Msg> =
            if self.playback.active() && self.playback.snapshot().fullscreen {
                player()
            } else {
                // The bell and the connection indicator at the bar's end.
                let right: Element<'a, ShellMsg> = iced::widget::row![
                    self.notifications.bell(ShellMsg::Notifications),
                    self.connection
                        .indicator(&self.profile.display_name, ShellMsg::ServerMenu),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center)
                .into();
                let chrome: Element<'a, Msg> = self.shell.chrome(right).map(Msg::Shell);
                let content: Element<'a, Msg> = match self.shell.active().current() {
                    crate::screens::Screen::Scene(scene) if self.player_visible() => {
                        crate::screens::scene::layout(
                            player(),
                            scene.details_view().map(Msg::Shell),
                            &self.shell.layout,
                        )
                    }
                    _ => {
                        let ctx = Context {
                            profile: &self.profile,
                            connection: &self.connection,
                            thumbs: &self.thumbs,
                            layout: self.shell.layout,
                        };
                        self.shell.content(&ctx).map(Msg::Shell)
                    }
                };
                let mut page = column![chrome, content];
                if self.now_playing() {
                    page = page.push(self.now_playing_bar());
                }
                page.into()
            };
        let overlay: Option<Element<'a, Msg>> = match &self.overlay {
            Overlay::None => None,
            Overlay::KeyboardHelp => Some(overlays::keyboard_help().map(Msg::Overlay)),
            Overlay::Notifications => Some(self.notifications.panel().map(Msg::Notifications)),
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
        let mut layers = stack![base];
        if let Some(panel) = overlay {
            // Behind an overlay: a backdrop that closes it when clicked (the page itself doesn't
            // react while an overlay is open). The panel is opaque, so clicks inside it stay
            // with it.
            layers = layers.push(opaque(
                mouse_area(
                    container(Space::new())
                        .width(iced::Length::Fill)
                        .height(iced::Length::Fill)
                        .style(|_| {
                            container::Style::default().background(iced::Background::Color(
                                iced::Color::from_rgba(0.0, 0.0, 0.0, 0.25),
                            ))
                        }),
                )
                .on_press(Msg::Overlay(overlays::Msg::Close)),
            ));
            let placed = if self.overlay == Overlay::Notifications {
                // The notification centre opens under the bell.
                container(opaque(panel))
                    .padding(iced::Padding {
                        top: 48.0,
                        right: 12.0,
                        bottom: 12.0,
                        left: 12.0,
                    })
                    .width(iced::Length::Fill)
                    .height(iced::Length::Fill)
                    .align_right(iced::Length::Fill)
            } else {
                container(opaque(panel)).center(iced::Length::Fill)
            };
            layers = layers.push(placed);
        }
        // Toasts float over everything without taking focus or blocking the page around them.
        if let Some(toasts) = self.notifications.toast_layer() {
            layers = layers.push(toasts.map(Msg::Notifications));
        }
        layers.into()
    }
}
