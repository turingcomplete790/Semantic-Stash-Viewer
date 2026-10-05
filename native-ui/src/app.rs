//! The root of the UI's state machine (007 research R1): `App` is `Onboarding` (no saved server)
//! or `Session` (one active server). This module also holds the iced glue: subscriptions turn the
//! core's streams into messages, and every step's effects run through `effects.rs`.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use iced::{keyboard, window, Element, Subscription, Task};
use player::PlayerSnapshot;
use stash_core::connection::snapshot::ConnectionSnapshot;

use crate::effects::{self, Effect, Reply};
use crate::machine::Step;
use crate::measure::{self, Finish};
use crate::onboarding::{self, ServerForm};
use crate::player::video::{self, VideoSurface};
use crate::services::Services;
use crate::session::{self, Session};

/// Subscriptions are built from function pointers, so they reach the services through this.
static SERVICES: OnceLock<Arc<Services>> = OnceLock::new();

fn services() -> &'static Arc<Services> {
    SERVICES
        .get()
        .expect("services are set before the app starts")
}

pub fn run(services: Arc<Services>) -> iced::Result {
    let _ = SERVICES.set(services);
    iced::application(App::boot, App::update, App::view)
        .title("Semantic Stash Viewer")
        .theme(|_: &App| crate::widgets::theme::theme())
        .subscription(App::subscription)
        .exit_on_close_request(false)
        .window(window_settings())
        .run()
}

/// The app's top-level state.
pub enum State {
    Onboarding(ServerForm),
    Session(Box<Session>),
}

#[derive(Debug, Clone)]
pub enum Message {
    Onboarding(onboarding::Msg),
    Session(session::Msg),
    /// Results of effects that aren't addressed to a single level.
    Reply(Reply),
    // Inputs from the core and the window, routed into the active state.
    Connection(ConnectionSnapshot),
    Player(PlayerSnapshot),
    Key(keyboard::Key, keyboard::Modifiers),
    /// Background work finished with nothing to report.
    Idle,
    FrameReady,
    Tick,
    WindowId(Option<window::Id>),
    Resized(iced::Size),
    Modifiers(keyboard::Modifiers),
    CacheChanged(String),
    WindowMode(window::Mode),
    CloseRequested,
    MeasureDone,
    /// A window frame (the UI bench's clock).
    Frame(std::time::Instant),
}

pub struct App {
    services: Arc<Services>,
    state: State,
    /// The video path lives as long as the app (mpv's output needs it while a file plays).
    video: Option<VideoSurface>,
    video_error: Option<String>,
    window: Option<window::Id>,
    interactive_marked: bool,
    bench: Option<measure::bench::Bench>,
    /// The window's size, handed to each new session.
    layout: crate::screens::scenes::layout::Layout,
}

impl App {
    fn boot() -> (Self, Task<Message>) {
        let services = Arc::clone(services());
        let (video, video_error) = match services.player.as_ref() {
            Some(p) => match VideoSurface::start(Arc::clone(p)) {
                Ok(v) => (Some(v), None),
                Err(e) => {
                    tracing::error!(error = %e, "video path unavailable");
                    (None, Some(e.to_string()))
                }
            },
            None => (None, Some("the player isn't available".into())),
        };
        let (state, effects) = initial_state(&services);
        let mut app = Self {
            services,
            state,
            video,
            video_error,
            window: None,
            interactive_marked: false,
            bench: None,
            layout: crate::screens::scenes::layout::Layout::default(),
        };
        let mut tasks = vec![window::oldest().map(Message::WindowId)];
        tasks.push(app.run_effects(effects));
        if let Some(ids) = measure::measure_scenes() {
            let services = Arc::clone(&app.services);
            tasks.push(Task::perform(
                measure::playback::run(services, ids, measure::measure_long(), Arc::new(|_| {})),
                |()| Message::MeasureDone,
            ));
        }
        (app, Task::batch(tasks))
    }

    fn run_effects(&mut self, effects: Vec<Effect>) -> Task<Message> {
        Task::batch(
            effects
                .into_iter()
                .map(|e| effects::run(e, &self.services, self.window)),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let effects = match message {
            Message::Onboarding(m) => self.onboarding(m),
            Message::Session(m) => {
                if let (
                    Some(bench),
                    session::Msg::Shell(crate::shell::ShellMsg::Scenes(
                        crate::screens::scenes::ScenesMsg::AutoScrolled(result),
                    )),
                ) = (self.bench.as_mut(), &m)
                {
                    bench.scrolled(result.clone());
                }
                self.session(m)
            }
            Message::Reply(Reply::SceneOpened(r)) => self.session(session::Msg::SceneOpened(r)),
            Message::Reply(reply @ Reply::ProfileCreated(_))
                if matches!(self.state, State::Onboarding(_)) =>
            {
                self.onboarding(onboarding::Msg::Reply(reply))
            }
            Message::Reply(reply) => self.session(session::Msg::Reply(reply)),
            Message::Connection(s) => self.session(session::Msg::Connection(s)),
            Message::Player(s) => {
                self.session(session::Msg::Playback(session::playback::Msg::Snapshot(s)))
            }
            Message::Key(key, mods) => self.session(session::Msg::Key(key, mods)),
            Message::Idle => Vec::new(),
            Message::FrameReady => Vec::new(),
            Message::Tick => self.session(session::Msg::Playback(
                session::playback::Msg::Controls(crate::player::view::Msg::Tick),
            )),
            Message::WindowId(id) => {
                self.window = id;
                Vec::new()
            }
            Message::Modifiers(m) => self.session(session::Msg::Modifiers(m)),
            Message::CacheChanged(key) => self.session(session::Msg::CacheChanged(key)),
            Message::Resized(size) => {
                self.layout = crate::screens::scenes::layout::Layout::new(size.width, size.height);
                let effects = self.session(session::Msg::Layout(self.layout));
                let task = self.run_effects(effects);
                return match self.window {
                    Some(id) => Task::batch([task, window::mode(id).map(Message::WindowMode)]),
                    None => task,
                };
            }
            Message::WindowMode(mode) => {
                // The desktop can take the window out of fullscreen (Meta on KDE).
                let fullscreen = mode == window::Mode::Fullscreen;
                if let Some(p) = &self.services.player {
                    if p.snapshot().fullscreen != fullscreen {
                        p.set_fullscreen_flag(fullscreen);
                    }
                }
                Vec::new()
            }
            Message::Frame(now) => {
                let (Some(bench), State::Session(session)) = (self.bench.as_mut(), &mut self.state)
                else {
                    return Task::none();
                };
                let (mut effects, done) = bench.frame(now, session);
                if done {
                    self.bench = None;
                    if measure::should_exit(Finish::Bench) {
                        effects.extend(self.quit_effects());
                    }
                }
                effects
            }
            Message::MeasureDone => {
                if measure::should_exit(Finish::Measure) {
                    self.quit_effects()
                } else {
                    Vec::new()
                }
            }
            Message::CloseRequested => self.quit_effects(),
        };
        self.run_effects(effects)
    }

    fn quit_effects(&mut self) -> Vec<Effect> {
        if let Some(mut v) = self.video.take() {
            v.stop();
        }
        let mut effects = Vec::new();
        if let State::Session(session) = &self.state {
            effects.extend(session.save_now());
        }
        effects.push(Effect::Quit);
        effects
    }

    fn onboarding(&mut self, msg: onboarding::Msg) -> Vec<Effect> {
        let State::Onboarding(form) = &mut self.state else {
            return Vec::new();
        };
        let Step { mut effects, up } = form.update(msg);
        if let Some(onboarding::Up::Saved(profile)) = up {
            let (mut session, entry) = Session::enter(profile, false);
            session.shell.layout = self.layout;
            self.state = State::Session(Box::new(session));
            effects.extend(entry);
        }
        effects
    }

    fn session(&mut self, msg: session::Msg) -> Vec<Effect> {
        let State::Session(session) = &mut self.state else {
            return Vec::new();
        };
        let Step { mut effects, up } = session.update(msg);
        if let Some(session::Up::Switch(id)) = &up {
            // Leave this session for another saved server (001 US4).
            tracing::info!(to = %id, known = session.servers.len(), "switching server");
            if let Some(profile) = session.servers.iter().find(|p| p.id == *id).cloned() {
                // Each server keeps its own tabs.
                effects.extend(session.save_now());
                let (mut next, entry) = Session::enter(profile, false);
                next.shell.layout = self.layout;
                self.state = State::Session(Box::new(next));
                effects.extend(entry);
            }
            return effects;
        }
        if let Some(session::Up::FirstConnected) = up {
            if !self.interactive_marked {
                self.interactive_marked = true;
                if measure::mark_interactive() {
                    effects.extend(self.quit_effects());
                } else if measure::bench_requested() {
                    self.bench = Some(measure::bench::Bench::new(session));
                }
            }
        }
        effects
    }

    fn view(&self) -> Element<'_, Message> {
        match &self.state {
            State::Onboarding(form) => form.view().map(Message::Onboarding),
            State::Session(session) => {
                let video_error = self.video_error.clone().or_else(|| {
                    self.video
                        .as_ref()
                        .and_then(|v| v.shared().error())
                        .map(|e| e.to_string())
                });
                session
                    .view(self.video.as_ref().map(VideoSurface::shared), video_error)
                    .map(Message::Session)
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            Subscription::run(connection_stream).map(Message::Connection),
            Subscription::run(player_stream).map(Message::Player),
            window::close_requests().map(|_| Message::CloseRequested),
            window::resize_events().map(|(_, size)| Message::Resized(size)),
            Subscription::run(cache_stream).map(Message::CacheChanged),
            // Key events a focused widget captured (typing in a field) never reach the machine.
            iced::event::listen_with(|event, status, _window| match (event, status) {
                (
                    iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }),
                    iced::event::Status::Ignored,
                ) => Some(Message::Key(key, modifiers)),
                (iced::Event::Keyboard(keyboard::Event::ModifiersChanged(m)), _) => {
                    Some(Message::Modifiers(m))
                }
                _ => None,
            }),
        ];
        if self
            .bench
            .as_ref()
            .is_some_and(measure::bench::Bench::wants_frames)
        {
            subs.push(window::frames().map(Message::Frame));
        }
        if let State::Session(session) = &self.state {
            if session.playback.active() {
                subs.push(Subscription::run(video::frames).map(|()| Message::FrameReady));
                if session.playback.screen.playing() && session.playback.screen.controls_visible() {
                    subs.push(iced::time::every(Duration::from_millis(250)).map(|_| Message::Tick));
                }
                if self.video.as_ref().is_some_and(|v| v.shared().resizing()) {
                    subs.push(window::frames().map(|_| Message::FrameReady));
                }
            }
        }
        Subscription::batch(subs)
    }
}

/// Where the app starts: the harness's profile or the last-used server (a session), otherwise
/// onboarding.
fn initial_state(services: &Services) -> (State, Vec<Effect>) {
    let profile = match measure::harness_profile() {
        Some(name) => {
            let found = services.profile_named(&name);
            if found.is_none() {
                measure::emit(&serde_json::json!({"invalid": "profile not found"}));
            }
            if let (Some(p), true) = (&found, measure::clear_cache_requested()) {
                if let Err(e) = services.clear_cache(p.id) {
                    tracing::warn!(error = %e, "couldn't clear the cache for the harness run");
                }
            }
            found
        }
        None => services.last_used(),
    };
    match profile {
        Some(profile) => {
            let (session, effects) = Session::enter(profile, true);
            (State::Session(Box::new(session)), effects)
        }
        None => (
            State::Onboarding(ServerForm::default()),
            vec![Effect::Focus(onboarding::ADDRESS_ID)],
        ),
    }
}

/// Connection snapshots as messages.
fn connection_stream() -> impl iced::futures::Stream<Item = ConnectionSnapshot> {
    iced::stream::channel(16, async |mut output| {
        use iced::futures::SinkExt;
        let mut rx = services().manager.subscribe();
        let _ = output.send(services().snapshot()).await;
        loop {
            match rx.recv().await {
                Ok(s) => {
                    if output.send(s).await.is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    })
}

/// Changes to cached data, for the current profile, as messages.
fn cache_stream() -> impl iced::futures::Stream<Item = String> {
    iced::stream::channel(16, async |mut output| {
        use iced::futures::SinkExt;
        let mut rx = services().caches.changes();
        loop {
            match rx.recv().await {
                Ok((profile, key)) => {
                    if services().current_profile() == Some(profile)
                        && output.send(key).await.is_err()
                    {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let _ = output.send("*".to_owned()).await;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    })
}

/// Player snapshots as messages.
fn player_stream() -> impl iced::futures::Stream<Item = PlayerSnapshot> {
    iced::stream::channel(16, async |mut output| {
        use iced::futures::SinkExt;
        let Some(player) = services().player.as_ref() else {
            return;
        };
        let mut rx = player.subscribe();
        let first = rx.borrow().clone();
        let _ = output.send(first).await;
        while rx.changed().await.is_ok() {
            let s = rx.borrow_and_update().clone();
            if output.send(s).await.is_err() {
                break;
            }
        }
    })
}

/// The window: the app id the desktop sees (task bars, window rules) and the icon (research R15).
fn window_settings() -> window::Settings {
    window::Settings {
        size: iced::Size::new(1600.0, 1000.0),
        icon: window::icon::from_file_data(include_bytes!("../assets/icon-128.png"), None).ok(),
        #[cfg(target_os = "linux")]
        platform_specific: window::settings::PlatformSpecific {
            application_id: crate::services::APP_ID.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
