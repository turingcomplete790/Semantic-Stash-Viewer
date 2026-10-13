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
use stash_core::playback::playlist;
use stash_core::profiles::{ProfileDraft, ServerProfile};
use stash_core::AppError;
use uuid::Uuid;

use crate::effects::{Effect, Reply};
use crate::machine::Step;
use crate::onboarding;
use crate::player::video::Shared;
use crate::screens::browse::state::{BrowseMsg, Kind, List};
use crate::screens::images::ImagesMsg;
use crate::screens::scenes::layout::Layout;
use crate::screens::scenes::{ScenesMsg, Thumbs};
use crate::screens::{Context, Mode};
use crate::shell::keymap::{self, Action, Level};
use crate::shell::overlays::{self, KeyPrompt, Overlay, KEY_PROMPT_ID};
use crate::shell::TabId;
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
    /// What every tab of this server shares: UI settings, saved filters, picker lists, the
    /// semantic vocabulary (008).
    pub library: crate::library::Library,
    /// Held modifier keys (Ctrl+click opens in a new tab).
    modifiers: Modifiers,
    ever_connected: bool,
    /// The saved session has been read (or there was none): saving can't overwrite it now.
    restored: bool,
    /// Bumped on every change to the shell; only the latest pending save is written.
    save_generation: u64,
    /// The UI bench is running: its tabs aren't saved.
    pub saving_suspended: bool,
    /// Settings → Playback (008), the same for every server.
    pub prefs: crate::services::settings::PlaybackPreferences,
    /// Play history sends that failed this session, and whether the centre was told (once per
    /// server, contracts/play-history.md).
    history_failures: u32,
    history_notified: bool,
    /// The one card preview (008 FR-019).
    pub previews: crate::screens::scenes::preview::Previews,
}

/// Failed play history sends before the notification centre says so.
const HISTORY_FAILURES_NOTIFIED: u32 = 3;

#[derive(Debug, Clone)]
pub enum Msg {
    Connection(ConnectionSnapshot),
    Playback(playback::Msg),
    Overlay(overlays::Msg),
    OpenServerMenu,
    SwitchServer(Uuid),
    /// The "Add a server" overlay's form.
    AddServer(onboarding::Msg),
    /// A preview's frame timer (only while a preview plays).
    PreviewFrame(std::time::Instant),
    /// The player opened a scene; the resume point it started at, if it resumed.
    SceneOpened(Result<Option<f64>, AppError>),
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
        effects.push(Effect::LoadPlaybackPreferences);
        let mut library = crate::library::Library::default();
        effects.extend(library.want_ui());
        let mut playback = playback::Playback::default();
        playback.profile = Some(profile_id);
        (
            Self {
                library,
                servers: vec![profile.clone()],
                profile,
                connection: connection::Connection::default(),
                playback,
                overlay: Overlay::None,
                shell,
                thumbs: Thumbs::default(),
                notifications: crate::shell::notifications::Notifications::for_profile(profile_id),
                modifiers: Modifiers::empty(),
                ever_connected: false,
                restored: false,
                save_generation: 0,
                saving_suspended: false,
                prefs: crate::services::settings::PlaybackPreferences::default(),
                history_failures: 0,
                history_notified: false,
                previews: Default::default(),
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

    /// Leaving this session (quit or a server switch): save its tabs, end the viewing (its final
    /// play history report), and stop playback.
    pub fn leave(&mut self) -> Vec<Effect> {
        let mut effects: Vec<Effect> = self.save_now().into_iter().collect();
        effects.extend(self.playback.finish());
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
        // Opened outside a tab (the playback measurement): it shows in the active tab.
        if self.playback.owner.is_none() {
            return true;
        }
        let tab = self.shell.active();
        let playing = self.playback.playing_scene();
        self.playback.owner == Some(tab.id)
            && match tab.current() {
                crate::screens::Screen::Scene(s) => Some(s.scene_id.as_str()) == playing,
                // The queue view shows it in its panel while it's the selection (010 R2).
                crate::screens::Screen::Scenes(s) if s.mode == Mode::Queue => {
                    s.queue.panel.as_deref().map(|p| p.scene_id.as_str()) == playing
                }
                // The image viewer shows its image (012).
                crate::screens::Screen::Images(s) => {
                    self.playback.image.is_some()
                        && s.viewer_image() == self.playback.image.as_deref()
                }
                _ => false,
            }
    }

    /// What became of an image open goes to the viewer that asked (012).
    fn image_events(&mut self) {
        let (Some(event), Some(owner)) = (self.playback.take_image_event(), self.playback.owner)
        else {
            return;
        };
        match event {
            playback::ImageEvent::Shown { id, picture } => {
                self.shell.image_shown(owner, &id, picture);
            }
            playback::ImageEvent::Failed { id, reason } => {
                self.shell.image_failed(owner, &id, reason);
            }
        }
    }

    /// Tab `tab`'s Scenes screen, if it shows the queue view.
    fn queue_tab(&mut self, tab: TabId) -> Option<&mut crate::screens::ScenesState> {
        self.shell
            .tabs
            .iter_mut()
            .find(|t| t.id == tab)
            .and_then(|t| match t.current_mut() {
                crate::screens::Screen::Scenes(s) if s.mode == Mode::Queue => Some(&mut **s),
                _ => None,
            })
    }

    /// The queue's playing marks follow playback: only the owner tab's list marks its scene.
    fn sync_queue_playing(&mut self) {
        let playing = self
            .playback
            .active()
            .then(|| self.playback.playing_scene().map(str::to_owned))
            .flatten();
        let owner = self.playback.owner;
        for tab in &mut self.shell.tabs {
            let want = if owner == Some(tab.id) {
                playing.clone()
            } else {
                None
            };
            for screen in &mut tab.history {
                if let crate::screens::Screen::Scenes(s) = screen {
                    if s.queue.playing != want {
                        s.queue.playing.clone_from(&want);
                    }
                }
            }
        }
    }

    /// A later save of the tabs (after a change outside the shell's own messages).
    fn save_later(&mut self) -> Option<Effect> {
        self.restored.then(|| {
            self.save_generation += 1;
            Effect::SaveSessionLater {
                generation: self.save_generation,
            }
        })
    }

    /// A playlist event for tab `tab`'s queue (011 R1): play what the machine chose, load the page
    /// that has it (it plays when it arrives), or replay the scene from the beginning.
    fn queue_advance(&mut self, tab: TabId, event: playlist::Event) -> Step<Up> {
        use crate::screens::scenes::Advance;
        let layout = self.shell.layout;
        let Some(state) = self.queue_tab(tab) else {
            return Step::none();
        };
        let mut effects = match state.queue_event(event, tab, &layout) {
            Advance::Play { id, mut effects } => {
                effects.extend(self.playback.open(id, tab));
                self.sync_chapters();
                self.sync_queue_playing();
                effects
            }
            Advance::Loading(effects) => effects,
            // Repeat one: a new viewing from 0:00, never the resume point (011 R4); the player
            // replays the scene it holds, or opens it at 0:00 if it holds another.
            Advance::Replay { id } => {
                let holds = self.playback.active()
                    && self.playback.owner == Some(tab)
                    && self.playback.playing_scene() == Some(id.as_str());
                let effects = if holds {
                    self.playback.replay()
                } else {
                    self.playback.open_at(id, tab, Some(0.0))
                };
                self.sync_chapters();
                self.sync_queue_playing();
                effects
            }
            Advance::Stop => return Step::none(),
        };
        effects.extend(self.thumbnails());
        effects.extend(self.save_later());
        Step::effects(effects)
    }

    /// The user switched to another tab: a queue tab starts its selection with Autoplay on, only
    /// while the player is idle (011 R6); one whose page isn't here yet starts when it arrives.
    fn autoplay_on_switch(&mut self) -> Vec<Effect> {
        if self.playback.active() {
            return Vec::new();
        }
        let tab = self.shell.active().id;
        let Some(state) = self.queue_tab(tab) else {
            return Vec::new();
        };
        if !state.ready() {
            state.queue.pending_play = crate::screens::scenes::queue::PendingPlay::Start;
            return Vec::new();
        }
        match state.autostart() {
            Some(id) => self.queue_open(id, tab),
            None => Vec::new(),
        }
    }

    /// Play scene `id` for queue tab `tab`.
    fn queue_open(&mut self, id: String, tab: TabId) -> Vec<Effect> {
        let effects = self.playback.open(id, tab);
        self.sync_chapters();
        self.sync_queue_playing();
        effects
    }

    /// The queue's Next or Previous control (`n`, `p`, or the panel's button): from the scene
    /// this tab plays, or the selection when it plays none.
    fn queue_step(&mut self, forward: bool) -> Step<Up> {
        let tab = self.shell.active().id;
        let event = if forward {
            playlist::Event::Next
        } else {
            playlist::Event::Previous
        };
        self.queue_advance(tab, event)
    }

    /// A scene reached its end: its queue's machine decides what follows (011 R4, R5).
    fn ended(&mut self) -> Step<Up> {
        match self.playback.owner {
            Some(owner) => self.queue_advance(owner, playlist::Event::Ended),
            None => Step::none(),
        }
    }

    /// The player's state, for the owner's queue: a scene that began playing is the machine's
    /// position (and ends a run of skips); a failure moves nothing (011 FR-010).
    fn queue_follow(&mut self) {
        let Some(owner) = self.playback.owner else {
            return;
        };
        let state = self.playback.snapshot().state;
        // The scene mpv reports, not the one asked for: a scene still opening hasn't started.
        let id = self.playback.snapshot().scene_id.clone();
        let Some(queue) = self.queue_tab(owner) else {
            return;
        };
        match (state, id) {
            (player::PlayerStateKind::Playing, Some(id)) => queue.queue_started(&id),
            (player::PlayerStateKind::Error, _) => queue.queue_failed(),
            _ => {}
        }
    }

    /// Whether the now-playing bar shows (something plays, out of sight).
    pub fn now_playing(&self) -> bool {
        // An image never shows the now-playing bar (012).
        self.playback.active() && self.playback.image.is_none() && !self.player_visible()
    }

    fn back_to_scene(&mut self) -> Step<Up> {
        let (Some(owner), Some(id)) = (
            self.playback.owner,
            self.playback.playing_scene().map(str::to_owned),
        ) else {
            return Step::none();
        };
        let mut step = self.shell(ShellMsg::Select(owner));
        // The queue view: select the playing scene, so the player shows in the panel (010).
        let in_queue = self
            .active_scenes()
            .filter(|s| s.mode == Mode::Queue)
            .and_then(|s| s.cards().iter().position(|c| c.id == id));
        if let (false, Some(index)) = (self.player_visible(), in_queue) {
            let more = self.shell(ShellMsg::Scenes(ScenesMsg::QueueSelect(index)));
            step.effects.extend(more.effects);
        }
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
    pub(crate) fn thumbnails(&mut self) -> Vec<Effect> {
        use crate::screens::scene::{PERFORMER_ICON, PORTRAIT};
        let width = self.thumb_width();
        let mut effects = self.thumbs.want(&self.shell.active_thumbs(), width);
        // Performer images on the scene page (008 FR-011a), only those Stash has, decoded at
        // their drawn size on this display; and the open performer card's portrait (FR-011b)
        // from the same cached image.
        if let Some(scene) = self.shell.active().current().scene_panel() {
            if let crate::screens::scene::Details::Ready(d) = &scene.details {
                let keys: Vec<_> = d
                    .performers
                    .iter()
                    .filter_map(|p| {
                        let version = p.image.as_deref()?;
                        Some(crate::screens::scenes::picker::performer_key(
                            &p.id, version,
                        ))
                    })
                    .collect();
                let icon = self.shell.layout.physical(PERFORMER_ICON);
                effects.extend(self.thumbs.want(&keys, icon));
            }
            if let Some(key) = scene.portrait_wanted() {
                let portrait = self.shell.layout.physical(PORTRAIT);
                effects.extend(self.thumbs.want([&key], portrait));
            }
        }
        // Performer images for the picker's visible rows only (008 FR-003a).
        if let Some(crate::screens::scenes::Panel::Editor(editor)) = self.active_panel() {
            let keys = crate::screens::scenes::picker::visible_performers(&self.library, editor);
            effects.extend(self.thumbs.want(&keys, 64));
        }
        effects
    }

    /// The width thumbnails are decoded at: a grid card's (list rows draw them smaller), in steps
    /// of 32 px so small window changes reuse them.
    fn thumb_width(&self) -> u32 {
        let w = self.shell.active_layout().card_width().ceil() as u32;
        w.div_ceil(32) * 32
    }

    fn shell(&mut self, msg: ShellMsg) -> Step<Up> {
        // The player's controls in the queue panel, and the queue's Next and Previous.
        match msg {
            ShellMsg::Player(m) => return self.update(Msg::Playback(m)),
            ShellMsg::QueueStep(forward) => return self.queue_step(forward),
            _ => {}
        }
        // The pointer crossing cards (also as they scroll under it): only the preview cares, so
        // none of the rest below runs for it (008 FR-019).
        if let ShellMsg::Scenes(ScenesMsg::Hover(index) | ScenesMsg::HoverEnd(index)) = msg {
            let on = matches!(msg, ShellMsg::Scenes(ScenesMsg::Hover(_)));
            let scene = self
                .active_scenes()
                .and_then(|s| s.cards().get(index))
                .map(|c| c.id.clone());
            let Some(scene) = scene else {
                return Step::none();
            };
            // Stash doesn't say whether a preview was generated, so the first rest tries.
            return Step::effects(if on {
                self.previews.enter(scene, true)
            } else {
                self.previews.left(&scene)
            });
        }
        // The viewer's input (pointer movement among it) skips the grid's work below; a wheel
        // turn zooms with Ctrl held (012).
        if let ShellMsg::Images(ImagesMsg::Viewer(m)) = msg {
            let m = match m {
                crate::screens::images::ViewerMsg::Wheel { y, .. } => {
                    crate::screens::images::ViewerMsg::Wheel {
                        y,
                        ctrl: self.modifiers.control(),
                    }
                }
                other => other,
            };
            let Step { effects, up } = self.shell.update(ShellMsg::Images(ImagesMsg::Viewer(m)));
            return self.shell_up(Step::effects(effects), up);
        }
        // Image cards have no preview yet (US3).
        if let ShellMsg::Images(ImagesMsg::List(BrowseMsg::Hover(_) | BrowseMsg::HoverEnd(_))) = msg
        {
            return Step::none();
        }
        let scrolled = if matches!(msg, ShellMsg::Scenes(ScenesMsg::Scrolled { .. })) {
            self.previews.scrolled(std::time::Instant::now())
        } else {
            Vec::new()
        };
        // A click with Ctrl held opens the scene in a new tab.
        let msg = match msg {
            ShellMsg::Scenes(ScenesMsg::Open {
                index,
                new_tab: false,
            }) if self.modifiers.control() => ShellMsg::Scenes(ScenesMsg::Open {
                index,
                new_tab: true,
            }),
            ShellMsg::Images(ImagesMsg::List(BrowseMsg::Open {
                index,
                new_tab: false,
            })) if self.modifiers.control() => ShellMsg::Images(ImagesMsg::List(BrowseMsg::Open {
                index,
                new_tab: true,
            })),
            ShellMsg::FilterTo {
                criterion,
                new_tab: false,
            } if self.modifiers.control() => ShellMsg::FilterTo {
                criterion,
                new_tab: true,
            },
            other => other,
        };
        let before = self.shell.active().id;
        let Step { mut effects, up } = self.shell.update(msg);
        if self.shell.active().id != before {
            effects.extend(self.autoplay_on_switch());
        }
        effects.extend(scrolled);
        effects.extend(self.thumbnails());
        effects.extend(self.sync_tag_colours());
        // Another screen: no cards, so no preview.
        if self.active_scenes().is_none() {
            effects.extend(self.previews.clear());
        }
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
            Some(shell_mod::Up::ShowImage { id, title, looped }) => {
                let owner = self.shell.active().id;
                let step = step.with_all(self.playback.open_image(id, title, looped, owner));
                self.sync_queue_playing();
                step
            }
            Some(shell_mod::Up::ImageClosed) => {
                if self.playback.image.is_some()
                    && self.playback.owner == Some(self.shell.active().id)
                {
                    step.with_all(self.playback.update(playback::Msg::Close))
                } else {
                    step
                }
            }
            Some(shell_mod::Up::Play(id)) => {
                let owner = self.shell.active().id;
                let step = step.with_all(self.shell.close_viewers());
                let step = step.with_all(self.playback.open(id, owner));
                self.sync_chapters();
                self.sync_queue_playing();
                step
            }
            Some(shell_mod::Up::PlaybackChange(change)) => {
                let prefs = change.apply(self.prefs);
                self.apply_prefs(prefs);
                step.with(Effect::SavePlaybackPreferences(prefs))
            }
            Some(shell_mod::Up::AddServer) => {
                self.overlay = Overlay::AddServer(onboarding::ServerForm::default());
                step.with(Effect::Focus(onboarding::ADDRESS_ID))
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
            Some(shell_mod::Up::Need(need)) => step.with_all(self.library.want(need)),
            Some(shell_mod::Up::PlayAt { id, seconds }) => {
                let owner = self.shell.active().id;
                let step = step.with_all(self.shell.close_viewers());
                let step = step.with_all(self.playback.open_at(id, owner, Some(seconds)));
                self.sync_chapters();
                self.sync_queue_playing();
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
        // The image viewer replaces the player's and the grid's keys while it shows (012); the
        // shell's still work.
        if self.shell.active_viewer().is_some() {
            return self
                .viewer_key(key, mods)
                .or_else(|| self.shell_key(key, mods))
                .unwrap_or_else(Step::none);
        }
        if let Some(step) = self.queue_key(key, mods) {
            return step;
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
        if let Some(step) = self.list_panel_key(key, mods) {
            return step;
        }
        if let Some(step) = self.tag_colours_key(key, mods) {
            return step;
        }
        // Escape closes a chip's card on the scene page (the overlay rules).
        if *key == Key::Named(iced::keyboard::key::Named::Escape) {
            if let Some(scene) = self.shell.active().current().scene_panel() {
                if scene.chip_hover.is_some() {
                    return self.shell(ShellMsg::Scene(crate::screens::scene::SceneMsg::CloseCard));
                }
            }
        }
        // With the queue's panel focused, the list's keys don't move the selection (010 R5).
        let panel_focused = self.active_scenes().is_some_and(|s| {
            s.mode == Mode::Queue
                && s.queue.focus == crate::screens::scenes::queue::QueueFocus::Panel
        });
        if panel_focused {
            return self.list_key(key, mods).unwrap_or_else(Step::none);
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
            Some(
                Action::Player
                | Action::Scenes
                | Action::Images
                | Action::QueueList
                | Action::QueueFocus
                | Action::QueueNext
                | Action::QueuePrevious
                | Action::QueueAutoplay
                | Action::QueueShuffle
                | Action::QueueRepeat
                | Action::QueueOpen { .. }
                | Action::Viewer(_),
            )
            | None => return self.list_key(key, mods).unwrap_or_else(Step::none),
            Some(
                Action::FocusSearch | Action::FilterMenu | Action::SavedFilters | Action::Settings,
            ) => return Step::none(),
        };
        self.shell(msg)
    }

    /// The shell's own keys alone (tabs, history, help, focus); `None` for any other key.
    fn shell_key(&mut self, key: &Key, mods: Modifiers) -> Option<Step<Up>> {
        let msg = match keymap::resolve(key, mods, &[Level::Shell])? {
            Action::NewTab => ShellMsg::NewTab,
            Action::CloseTab => ShellMsg::CloseActive,
            Action::NextTab => ShellMsg::NextTab,
            Action::PrevTab => ShellMsg::PrevTab,
            Action::SelectTab(i) => ShellMsg::SelectIndex(i),
            Action::MoveTabLeft => ShellMsg::MoveLeft,
            Action::MoveTabRight => ShellMsg::MoveRight,
            Action::Back => ShellMsg::Back,
            Action::Forward => ShellMsg::Forward,
            Action::KeyboardHelp => ShellMsg::KeyboardHelp,
            Action::FocusNext => return Some(Step::effect(Effect::FocusNext)),
            Action::FocusPrevious => return Some(Step::effect(Effect::FocusPrevious)),
            _ => return None,
        };
        Some(self.shell(msg))
    }

    /// The image viewer's keys (012 contracts/images-ui.md); `None` for a key it doesn't bind.
    fn viewer_key(&mut self, key: &Key, mods: Modifiers) -> Option<Step<Up>> {
        use crate::screens::images::viewer::ZOOM_KEY;
        use crate::shell::keymap::ViewerKey as K;
        use stash_core::images::viewer::Event;
        let Some(Action::Viewer(k)) = keymap::resolve(key, mods, &[Level::Viewer]) else {
            return None;
        };
        let fullscreen = self.playback.active() && self.playback.snapshot().fullscreen;
        let at_first = self
            .shell
            .active_viewer()
            .and_then(|s| s.viewer.as_ref())
            .is_some_and(|v| v.machine.at == 0);
        let zoom = |delta: f32| Event::Zoom {
            delta,
            anchor: (0.0, 0.0),
        };
        let step = match k {
            K::Fullscreen => return Some(Step::effect(Effect::SetFullscreen(!fullscreen))),
            K::Close if fullscreen => return Some(Step::effect(Effect::SetFullscreen(false))),
            K::Close => self.shell.with_viewer(|s| s.close_viewer()),
            K::Back if at_first => self.shell.with_viewer(|s| s.close_viewer()),
            K::Next if *key == Key::Named(iced::keyboard::key::Named::Space) => {
                self.shell.with_viewer(|s| s.viewer_event(Event::Next))
            }
            K::Next => self.shell.with_viewer(|s| s.arrow(1, 0)),
            K::Previous => self.shell.with_viewer(|s| s.arrow(-1, 0)),
            K::Back => self.shell.with_viewer(|s| s.viewer_event(Event::Previous)),
            K::Up => self.shell.with_viewer(|s| s.arrow(0, -1)),
            K::Down => self.shell.with_viewer(|s| s.arrow(0, 1)),
            K::First => self.shell.with_viewer(|s| s.viewer_event(Event::First)),
            K::Last => self.shell.with_viewer(|s| s.viewer_event(Event::Last)),
            K::ZoomIn => self.shell.with_viewer(|s| s.viewer_event(zoom(ZOOM_KEY))),
            K::ZoomOut => self.shell.with_viewer(|s| s.viewer_event(zoom(-ZOOM_KEY))),
            K::Fit => self.shell.with_viewer(|s| s.viewer_event(Event::Fit)),
            K::ActualSize => self.shell.with_viewer(|s| {
                if s.viewer.as_ref().is_some_and(|v| v.machine.view.actual) {
                    Step::none()
                } else {
                    s.viewer_event(Event::ToggleActual)
                }
            }),
            K::Rotate => {
                let clockwise = !mods.shift();
                self.shell
                    .with_viewer(|s| s.viewer_event(Event::Rotate { clockwise }))
            }
        };
        let Step { effects, up } = step;
        Some(self.shell_up(Step::effects(effects), up))
    }

    /// The queue view's keys (010 contracts/queue-ui.md), before the player's: the list's keys
    /// with the list focused (the arrows then move the selection, not the volume), and F6, `n`,
    /// `p`, `a`, `r`, `x`, `o` in either area. In fullscreen only Next, Previous, and the playlist
    /// controls apply.
    fn queue_key(&mut self, key: &Key, mods: Modifiers) -> Option<Step<Up>> {
        use crate::screens::scenes::queue::QueueFocus;
        let state = self.active_scenes().filter(|s| s.mode == Mode::Queue)?;
        if state.panel != crate::screens::scenes::Panel::None {
            return None;
        }
        let focus = state.queue.focus;
        let selected = state.queue.selected;
        let fullscreen = self.playback.active() && self.playback.snapshot().fullscreen;
        let action = keymap::resolve(key, mods, &[Level::Queue])?;
        if fullscreen
            && !matches!(
                action,
                Action::QueueNext
                    | Action::QueuePrevious
                    | Action::QueueAutoplay
                    | Action::QueueShuffle
                    | Action::QueueRepeat
            )
        {
            return None;
        }
        Some(match action {
            Action::QueueList if focus == QueueFocus::List => {
                let Step { mut effects, up } = self.shell.screen_key(key, mods)?;
                effects.extend(self.thumbnails());
                effects.extend(self.save_later());
                self.shell_up(Step::effects(effects), up)
            }
            Action::QueueList => return None,
            Action::QueueFocus => self.shell(ShellMsg::Scenes(ScenesMsg::QueueFocus)),
            Action::QueueNext => self.queue_step(true),
            Action::QueuePrevious => self.queue_step(false),
            Action::QueueAutoplay => self.shell(ShellMsg::Scenes(ScenesMsg::ToggleAutoplay)),
            Action::QueueShuffle => self.shell(ShellMsg::Scenes(ScenesMsg::ToggleShuffle)),
            Action::QueueRepeat => self.shell(ShellMsg::Scenes(ScenesMsg::CycleRepeat)),
            Action::QueueOpen { new_tab } => {
                let index = selected?;
                self.shell(ShellMsg::Scenes(ScenesMsg::Open { index, new_tab }))
            }
            _ => return None,
        })
    }

    /// Settings → Tag colours shows the server's rules (once they're here) and their counts.
    fn sync_tag_colours(&mut self) -> Vec<Effect> {
        if self.library.ui.ready().is_some() {
            let rules = self.library.colour_rules();
            self.shell.sync_tag_colours(&rules);
        }
        if self.shell.tag_colours_open() {
            self.library.want(crate::library::Need::Vocabulary)
        } else {
            Vec::new()
        }
    }

    /// The active Scenes screen, if that's what shows.
    fn active_scenes(&self) -> Option<&crate::screens::ScenesState> {
        match self.shell.active().current() {
            crate::screens::Screen::Scenes(s) => Some(s),
            _ => None,
        }
    }

    /// The active list's open panel (Scenes or Images), if any.
    fn active_panel(&self) -> Option<&crate::screens::browse::state::Panel> {
        match self.shell.active().current() {
            crate::screens::Screen::Scenes(s) => Some(&s.panel),
            crate::screens::Screen::Images(s) => Some(&s.panel),
            _ => None,
        }
    }

    /// Keys for an open filter panel on Scenes or Images (008 FR-003f): move, choose, remove,
    /// exclude, close. Keys a focused field takes (typing, Enter in a field) never get here.
    fn list_panel_key(&mut self, key: &Key, mods: Modifiers) -> Option<Step<Up>> {
        use crate::screens::scenes::picker;
        let msg = match self.shell.active().current() {
            crate::screens::Screen::Scenes(s) => panel_key(
                &s.list,
                &self.library,
                key,
                mods,
                crate::screens::scenes::state::shell_msg,
            )?,
            crate::screens::Screen::Images(s) => panel_key(
                &s.list,
                &self.library,
                key,
                mods,
                crate::screens::images::shell_msg,
            )?,
            _ => return None,
        };
        let Some(msg) = msg else {
            return Some(Step::none());
        };
        let mut step = self.shell(msg);
        // Keep the picker's highlighted row in view.
        if let Some(crate::screens::browse::state::Panel::Editor(editor)) = self.active_panel() {
            if let Some(y) = picker::reveal(&self.library, editor) {
                step.effects.push(Effect::ScrollTo {
                    id: picker::LIST_ID.to_owned(),
                    y,
                });
            }
        }
        Some(step)
    }

    /// Settings → Tag colours' keys (008 FR-003g): move, reorder, change, remove, save.
    fn tag_colours_key(&mut self, key: &Key, mods: Modifiers) -> Option<Step<Up>> {
        use crate::screens::settings::tag_colours::TagColoursMsg as T;
        use iced::keyboard::key::Named;
        if !self.shell.tag_colours_open() {
            return None;
        }
        let crate::screens::Screen::Settings(s) = self.shell.active().current() else {
            return None;
        };
        let at = s.tag_colours.highlight;
        let msg = match key {
            Key::Named(Named::ArrowUp) if mods.alt() => T::MoveUp(at),
            Key::Named(Named::ArrowDown) if mods.alt() => T::MoveDown(at),
            Key::Named(Named::ArrowUp) => T::Highlight(-1),
            Key::Named(Named::ArrowDown) => T::Highlight(1),
            Key::Named(Named::Enter) => T::Edit(at),
            Key::Named(Named::Delete) => T::Remove(at),
            Key::Character(c) if mods.control() && c.eq_ignore_ascii_case("s") => T::Save,
            _ => return None,
        };
        Some(self.shell(ShellMsg::Settings(
            crate::screens::settings::SettingsMsg::TagColours(msg),
        )))
    }

    /// A list level's own keys (008 FR-003f; Images since 012): search, filter menu, saved
    /// filters.
    fn list_key(&mut self, key: &Key, mods: Modifiers) -> Option<Step<Up>> {
        use crate::screens::browse::state::SEARCH_ID;
        use crate::screens::Screen;
        let images = match self.shell.active().current() {
            Screen::Scenes(_) => false,
            Screen::Images(_) => true,
            _ => return None,
        };
        let level = if images { Level::Images } else { Level::Scenes };
        let open = |saved: bool| {
            if images {
                crate::screens::images::shell_msg(if saved {
                    BrowseMsg::SavedMenu(true)
                } else {
                    BrowseMsg::FilterMenu(true)
                })
            } else {
                crate::screens::scenes::state::shell_msg(if saved {
                    BrowseMsg::SavedMenu(true)
                } else {
                    BrowseMsg::FilterMenu(true)
                })
            }
        };
        match keymap::resolve(key, mods, &[level])? {
            Action::FocusSearch => Some(Step::effect(Effect::Focus(SEARCH_ID))),
            Action::FilterMenu => Some(self.shell(open(false))),
            Action::SavedFilters => Some(self.shell(open(true))),
            _ => None,
        }
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
                    let mut retry = self.shell.retry_active();
                    retry.extend(self.library.want_ui());
                    // Play history waiting for this server goes now (contracts/play-history.md).
                    retry.push(Effect::FlushPlayHistory {
                        profile: self.profile.id,
                        deadline: None,
                    });
                    retry
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
            Msg::CacheChanged(key) => {
                let mut effects = self.shell.cache_changed(&key);
                effects.extend(self.library.cache_changed(&key));
                Step::effects(effects)
            }
            Msg::Reply(Reply::UiSettings(result)) => {
                let default = result
                    .as_ref()
                    .ok()
                    .and_then(|s| s.default_scene_filter.as_ref())
                    .map(stash_core::scenes::filter::saved::decode_default_filter);
                let default_images = result
                    .as_ref()
                    .ok()
                    .and_then(|s| s.default_image_filter.as_ref())
                    .map(|v| {
                        stash_core::scenes::filter::saved::decode_default_filter_for(
                            stash_core::browse::entity::Entity::Images,
                            v,
                        )
                    });
                if let Ok(settings) = &result {
                    self.shell.default_scenes = default;
                    self.shell.default_images = default_images;
                    self.playback.minimum_percent = Some(settings.minimum_play_percent);
                }
                let mut effects = self.library.ui_loaded(result);
                effects.extend(self.sync_tag_colours());
                Step::effects(effects)
            }
            Msg::Reply(Reply::SavedFilters(result)) => {
                self.library.saved_loaded(result);
                Step::none()
            }
            Msg::Reply(Reply::ImageOpened { id, result }) => {
                if let Err(e) = result {
                    tracing::warn!(error = %e, "couldn't open the image");
                    let reason = crate::messages::for_error(&e).title.to_owned();
                    self.playback.image_open_failed(&id, reason);
                    self.image_events();
                }
                Step::none()
            }
            Msg::Reply(Reply::SavedImageFilters(result)) => {
                self.library.saved_images_loaded(result);
                Step::none()
            }
            Msg::Reply(Reply::ImagesPage {
                tab,
                generation,
                result,
            }) => {
                let waiting = self.waiting();
                let Step { mut effects, up } =
                    self.shell.images_page(tab, generation, result, waiting);
                effects.extend(self.thumbnails());
                self.shell_up(Step::effects(effects), up)
            }
            Msg::Reply(Reply::Choices { kind, result }) => {
                self.library.choices_loaded(kind, result);
                Step::effects(self.thumbnails())
            }
            Msg::Reply(Reply::Vocabulary(result)) => {
                self.library.vocabulary_loaded(result);
                Step::none()
            }
            Msg::Reply(Reply::SceneFiles { tab, id, result }) => {
                self.shell.scene_files(tab, &id, result);
                Step::none()
            }
            Msg::Reply(Reply::SceneGalleries { tab, id, result }) => {
                self.shell.scene_galleries(tab, &id, result);
                Step::none()
            }
            Msg::Reply(Reply::ChipRestDue { tab, generation }) => {
                match self.shell.chip_rested(tab, generation) {
                    Some(chip) => {
                        let mut effects = self.library.want_chip(chip.kind, &chip.id);
                        // A performer's card shows their portrait (008 FR-011b).
                        effects.extend(self.thumbnails());
                        Step::effects(effects)
                    }
                    None => Step::none(),
                }
            }
            Msg::Reply(Reply::QueueRestDue { tab, generation }) => {
                let (mut effects, play) = self.shell.queue_rest_due(tab, generation);
                // The keyboard's selection plays where it rests, in the tab on screen (011 R2).
                if let (Some(id), true) = (play, self.shell.active().id == tab) {
                    effects.extend(self.queue_open(id, tab));
                }
                effects.extend(self.thumbnails());
                Step::effects(effects)
            }
            Msg::Reply(Reply::ChipDetails { kind, id, result }) => {
                self.library.chip_loaded(kind, id, result);
                Step::none()
            }
            Msg::Reply(Reply::TagColoursSaved(result)) => {
                let effects = match &result {
                    Ok(settings) => self.library.colours_saved(settings.clone()),
                    Err(_) => Vec::new(),
                };
                self.shell.tag_colours_saved(&result);
                Step::effects(effects)
            }
            Msg::Reply(Reply::SearchDue { tab, generation }) => {
                let mut effects = self.shell.search_due(tab, generation);
                effects.extend(self.thumbnails());
                if self.restored && !effects.is_empty() {
                    self.save_generation += 1;
                    effects.push(Effect::SaveSessionLater {
                        generation: self.save_generation,
                    });
                }
                Step::effects(effects)
            }
            Msg::Reply(Reply::ScenesPage {
                tab,
                generation,
                result,
            }) => {
                let waiting = self.waiting();
                let mut effects = self.shell.scenes_page(tab, generation, result, waiting);
                // Play next crossed into this page: its first (or last) card plays now (010 R4).
                let layout = self.shell.layout;
                let due = self.queue_tab(tab).and_then(|s| {
                    let id = s.take_play_due()?;
                    // Keep the playing card in view (the last card of a previous page).
                    if let Some(i) = s.queue.selected {
                        let step = s.update(ScenesMsg::Focus(i), tab, &layout);
                        effects.extend(step.effects);
                    }
                    Some(id)
                });
                if let Some(id) = due {
                    effects.extend(self.playback.open(id, tab));
                    self.sync_chapters();
                    self.sync_queue_playing();
                }
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
                        let previous = std::mem::replace(&mut self.shell, shell);
                        self.shell.layout = layout;
                        // Something was already done before the saved tabs arrived (a quick
                        // click at launch): keep that tab too, selected, so nothing is lost.
                        if previous.touched() {
                            let old_id = previous.active().id;
                            let new_id = self.shell.adopt(previous.active().clone());
                            if self.playback.owner == Some(old_id) {
                                self.playback.owner = Some(new_id);
                            }
                        }
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
            Msg::Playback(m) => {
                let effects = self.playback.update(m);
                self.image_events();
                self.sync_queue_playing();
                self.queue_follow();
                let step = Step::effects(effects);
                match self.playback.take_ended() {
                    Some(_) => {
                        let next = self.ended();
                        step.with_all(next.effects)
                    }
                    None => step,
                }
            }
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
            Msg::SceneOpened(Ok(resumed)) => {
                self.playback.resumed(resumed);
                Step::none()
            }
            Msg::Reply(Reply::PlaybackPreferences(prefs)) => {
                self.apply_prefs(prefs);
                Step::none()
            }
            Msg::Reply(Reply::PlayHistoryFlushed { profile, outcome }) => {
                self.history_flushed(profile, &outcome)
            }
            // A scene deleted on the server: its queue moves past it (010); the machine stops after
            // a whole result's worth in a row, so a queue of only deleted scenes ends (011 R7).
            Msg::SceneOpened(Err(AppError::SceneNotFound { .. }))
                if self
                    .playback
                    .owner
                    .is_some_and(|t| self.queue_tab(t).is_some()) =>
            {
                let owner = self.playback.owner.unwrap_or_default();
                self.queue_advance(owner, playlist::Event::Skipped)
            }
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
                let loaded = result.is_ok();
                self.shell.scene_details(tab, &id, *result);
                self.sync_chapters();
                // Tag chips need the vocabulary for their types and colours (FR-003d).
                let mut effects = if loaded {
                    self.library.want(crate::library::Need::Vocabulary)
                } else {
                    Vec::new()
                };
                effects.extend(self.thumbnails());
                Step::effects(effects)
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
                self.shell.settings_profiles(&list);
                self.servers = list;
                Step::none()
            }
            Msg::Reply(Reply::ProfileSaved { id, result }) => {
                self.shell.settings_saved(id, &result);
                match result {
                    Ok(p) => {
                        if let Some(slot) = self.servers.iter_mut().find(|s| s.id == p.id) {
                            *slot = p.clone();
                        }
                        // The active server changed: connect with its new settings.
                        if p.id == self.profile.id {
                            self.profile = p;
                            return Step::effect(Effect::Connect {
                                profile: self.profile.id,
                                launch: false,
                            });
                        }
                        Step::none()
                    }
                    Err(_) => Step::none(),
                }
            }
            Msg::Reply(Reply::ProfileDeleted(result)) => {
                self.shell.settings_deleted(&result);
                Step::effect(Effect::LoadProfiles)
            }
            Msg::Reply(Reply::CacheSize(size)) => {
                self.shell.settings_cache_size(size);
                Step::none()
            }
            Msg::Reply(Reply::CacheCleared(result)) => {
                self.shell.settings_cleared(&result);
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
            Msg::Key(key, mods) => {
                let step = self.key(&key, mods);
                // The keyboard's card previews as the pointer's does (008 FR-019); the queue's
                // compact rows don't preview (010).
                let focused = self
                    .active_scenes()
                    .filter(|s| s.mode != Mode::Queue)
                    .and_then(|s| {
                        s.focused
                            .and_then(|i| s.cards().get(i))
                            .map(|c| (c.id.clone(), true))
                    });
                let more = self.previews.focus(focused);
                step.with_all(more)
            }
            Msg::PreviewFrame(now) => {
                self.previews.tick(now);
                Step::none()
            }
            Msg::Reply(Reply::PreviewDue(generation)) => {
                let width = self.shell.layout.card_width().ceil() as u32;
                Step::effects(
                    self.previews
                        .due(generation, width, std::time::Instant::now()),
                )
            }
            Msg::Reply(Reply::Preview {
                generation,
                frames,
                done,
            }) => {
                self.previews
                    .frames(generation, frames, done, std::time::Instant::now());
                Step::none()
            }
            Msg::Reply(Reply::Sprite { scene_id, sheet }) => {
                self.playback.sprites_loaded(&scene_id, sheet);
                Step::none()
            }
            Msg::Reply(_) => Step::none(),
        }
    }

    /// Settings → Playback: kept here and handed to the player region.
    fn apply_prefs(&mut self, prefs: crate::services::settings::PlaybackPreferences) {
        self.prefs = prefs;
        self.playback.resume = prefs.resume;
        self.playback.save_history = prefs.save_history;
    }

    /// The playing scene's markers become the player's chapters once its page is loaded.
    fn sync_chapters(&mut self) {
        let Some(playing) = self.playback.scene_id.clone() else {
            return;
        };
        let chapters = self
            .shell
            .tabs
            .iter()
            .find_map(|tab| match tab.current().scene_panel() {
                Some(s) if s.scene_id == playing => match &s.details {
                    crate::screens::scene::Details::Ready(d) => Some(
                        d.markers
                            .iter()
                            .map(|m| (m.seconds, m.title.clone()))
                            .collect::<Vec<_>>(),
                    ),
                    _ => None,
                },
                _ => None,
            });
        if let Some(chapters) = chapters {
            self.playback.set_chapters(chapters);
        }
    }

    /// A play history flush finished: after a few failures while connected, the notification
    /// centre says so, once per server (contracts/play-history.md).
    fn history_flushed(
        &mut self,
        profile: Uuid,
        outcome: &stash_core::playback::history::FlushOutcome,
    ) -> Step<Up> {
        if profile != self.profile.id || outcome.failed.is_none() {
            return Step::none();
        }
        if !self.connection.connected() {
            // Offline: it's sent on reconnect; that isn't a failure to report.
            return Step::none();
        }
        self.history_failures += 1;
        if self.history_failures >= HISTORY_FAILURES_NOTIFIED && !self.history_notified {
            self.history_notified = true;
            return Step::effect(Effect::PlayHistoryUnsaved {
                profile,
                left: outcome.left,
            });
        }
        Step::none()
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

    /// The image viewer with the player's picture while it shows this viewer's image (012).
    fn viewer_view<'a>(
        &'a self,
        images: &'a crate::screens::ImagesState,
        video: Option<&'a Arc<Shared>>,
    ) -> Element<'a, Msg> {
        let Some(v) = images.viewer.as_ref() else {
            return Space::new().into();
        };
        let picture: Element<'a, ShellMsg> = if self.player_visible() {
            crate::player::view::picture(video)
        } else {
            Space::new().into()
        };
        crate::screens::images::viewer::view(v, picture).map(Msg::Shell)
    }

    /// Whether the viewer's controls are showing and will fade (a timer redraws until then).
    pub fn viewer_controls_fading(&self) -> bool {
        self.shell
            .active_viewer()
            .and_then(|s| s.viewer.as_ref())
            .is_some_and(|v| v.activity.is_some() && v.controls_visible(std::time::Instant::now()))
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
                // An image in fullscreen keeps the viewer's controls (012).
                match self.shell.active_viewer() {
                    Some(images) => self.viewer_view(images, video),
                    None => player(),
                }
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
                    _ if self.playback.active() && self.playback.owner.is_none() => player(),
                    // The queue view: the player in its panel (010 R2).
                    crate::screens::Screen::Scenes(scenes)
                        if scenes.mode == Mode::Queue && self.player_visible() =>
                    {
                        let ctx = Context {
                            profile: &self.profile,
                            connection: &self.connection,
                            thumbs: &self.thumbs,
                            library: &self.library,
                            layout: self.shell.layout,
                            playback: self.prefs,
                            previews: &self.previews,
                        };
                        let mut player: Element<'a, ShellMsg> = self.playback.screen.view(
                            video,
                            video_error.clone(),
                            |m| ShellMsg::Player(playback::Msg::Controls(m)),
                            ShellMsg::Player(playback::Msg::Close),
                            ShellMsg::Player(playback::Msg::ToggleFullscreen),
                        );
                        // Autoplay off: at the end, Replay (the player's) and Next (011 R5).
                        if !scenes.autoplay
                            && self.playback.snapshot().state == player::PlayerStateKind::Ended
                        {
                            player = stack![
                                player,
                                container(
                                    button(text("Next").size(14))
                                        .style(theme::chip)
                                        .padding([8, 18])
                                        .on_press(ShellMsg::QueueStep(true)),
                                )
                                .padding(16)
                                .width(iced::Length::Fill)
                                .height(iced::Length::Fill)
                                .align_x(iced::alignment::Horizontal::Right)
                                .align_y(iced::alignment::Vertical::Top),
                            ]
                            .into();
                        }
                        crate::screens::scenes::grid::view_with(
                            scenes,
                            &ctx,
                            self.shell.active().id,
                            Some(player),
                        )
                        .map(Msg::Shell)
                    }
                    crate::screens::Screen::Images(images) if images.viewer.is_some() => {
                        self.viewer_view(images, video)
                    }
                    crate::screens::Screen::Scene(scene) if self.player_visible() => {
                        crate::screens::scene::layout(
                            player(),
                            scene
                                .details_view(&self.library, &self.thumbs)
                                .map(Msg::Shell),
                            &self.shell.layout,
                        )
                    }
                    _ => {
                        let ctx = Context {
                            profile: &self.profile,
                            connection: &self.connection,
                            thumbs: &self.thumbs,
                            library: &self.library,
                            layout: self.shell.layout,
                            playback: self.prefs,
                            previews: &self.previews,
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

/// What a key does in `list`'s open filter panel (008 FR-003f): the message to send, `Some(None)`
/// when the key is taken but does nothing, `None` when the panel doesn't take it.
fn panel_key<K: Kind>(
    list: &List<K>,
    library: &crate::library::Library,
    key: &Key,
    mods: Modifiers,
    msg: crate::screens::browse::controls::Map<K>,
) -> Option<Option<ShellMsg>> {
    use crate::screens::browse::editor::EditorMsg;
    use crate::screens::browse::filters::{menu_target, MenuRow};
    use crate::screens::browse::picker;
    use crate::screens::browse::state::Panel;
    use iced::keyboard::key::Named;
    let named = match key {
        Key::Named(n) => Some(*n),
        _ => None,
    };
    let m = match (&list.panel, named) {
        (Panel::None, _) => return None,
        (Panel::Editor(_), Some(Named::Escape)) => BrowseMsg::Editor(EditorMsg::Cancel),
        (Panel::Menu { .. }, Some(Named::Escape)) => BrowseMsg::FilterMenu(false),
        (Panel::Saved { .. }, Some(Named::Escape)) => BrowseMsg::SavedMenu(false),
        (_, Some(Named::ArrowDown)) => BrowseMsg::MenuMove(1),
        (_, Some(Named::ArrowUp)) => BrowseMsg::MenuMove(-1),
        (Panel::Menu { highlight }, Some(Named::Enter)) => match menu_target(list, *highlight)? {
            MenuRow::Active { key, .. } => BrowseMsg::Edit(key),
            MenuRow::Kind(kind) => BrowseMsg::Add(kind),
            _ => return Some(None),
        },
        (Panel::Menu { highlight }, Some(Named::Delete | Named::Backspace)) => {
            match menu_target(list, *highlight)? {
                MenuRow::Active { key, .. } => BrowseMsg::Remove(key),
                _ => return Some(None),
            }
        }
        (Panel::Saved { highlight }, Some(Named::Enter)) => {
            let saved = K::saved(library).ready()?;
            let filter = saved.get((*highlight).min(saved.len().checked_sub(1)?))?;
            BrowseMsg::ApplySaved(Box::new(filter.clone()))
        }
        (Panel::Editor(editor), Some(Named::Enter)) => match picker::highlighted(library, editor) {
            Some(item) if editor.picker().is_some() && !mods.control() => {
                BrowseMsg::Editor(EditorMsg::Toggle(item))
            }
            _ => BrowseMsg::Editor(EditorMsg::Apply),
        },
        (Panel::Editor(editor), None)
            if mods.control()
                && matches!(key, Key::Character(c) if c.eq_ignore_ascii_case("e")) =>
        {
            let item = picker::highlighted(library, editor)?;
            BrowseMsg::Editor(EditorMsg::Exclude(item))
        }
        _ => return None,
    };
    Some(Some(msg(m)))
}
