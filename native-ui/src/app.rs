//! The native app: state, messages, update, view, subscriptions (006 T006, T019).

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use iced::keyboard;
use iced::widget::{button, center, column, container, row, scrollable, text};
use iced::{window, Element, Length, Subscription, Task, Theme};
use player::{PlayerSnapshot, PlayerStateKind};
use stash_core::adapter::scenes::recent_scenes;
use stash_core::cache::refresh::RefreshPolicy;
use stash_core::connection::snapshot::{ConnectionSnapshot, SessionState};
use stash_core::scenes::SceneListItem;

use crate::measure::{self, Finish};
use crate::player::controls::{key_action, Action};
use crate::player::video::{self, VideoSurface};
use crate::player::view::{format_time, Msg as PlayerMsg, PlayerScreen};
use crate::services::Services;

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
        .title("Semantic Stash Viewer (native spike)")
        .theme(|_: &App| Theme::Dark)
        .subscription(App::subscription)
        .exit_on_close_request(false)
        .window_size((1600.0, 1000.0))
        .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Home,
    Player,
}

#[derive(Debug, Clone)]
pub enum Message {
    Connection(ConnectionSnapshot),
    RecentLoaded(Result<Vec<SceneListItem>, String>),
    Open(String),
    Opened(Result<(), String>),
    Player(PlayerMsg),
    ClosePlayer,
    ToggleFullscreen,
    WindowId(Option<window::Id>),
    WindowMode(window::Mode),
    Resized,
    Key(keyboard::Key),
    FrameReady,
    MeasureDone,
    CloseRequested,
}

pub struct App {
    services: Arc<Services>,
    connection: ConnectionSnapshot,
    recent: Option<Result<Vec<SceneListItem>, String>>,
    loading_recent: bool,
    open_error: Option<String>,
    screen: Screen,
    player: PlayerScreen,
    video: Option<VideoSurface>,
    video_error: Option<String>,
    window: Option<window::Id>,
    interactive_marked: bool,
    measuring: bool,
}

impl App {
    fn boot() -> (Self, Task<Message>) {
        let services = Arc::clone(services());
        auto_connect(&services);
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
        let connection = services.snapshot();
        let mut app = Self {
            services,
            connection,
            recent: None,
            loading_recent: false,
            open_error: None,
            screen: Screen::Home,
            player: PlayerScreen::default(),
            video,
            video_error,
            window: None,
            interactive_marked: false,
            measuring: false,
        };
        let mut tasks = vec![window::oldest().map(Message::WindowId)];
        if let Some(ids) = measure::measure_scenes() {
            app.measuring = true;
            let services = Arc::clone(&app.services);
            tasks.push(Task::perform(
                measure::playback::run(services, ids, measure::measure_long(), Arc::new(|_| {})),
                |()| Message::MeasureDone,
            ));
        }
        (app, Task::batch(tasks))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Connection(snapshot) => {
                // Load whenever connected with nothing loaded yet: the session can already be
                // up by the time the app first looks (a fast local start), so waiting for the
                // change to "connected" could wait forever.
                let connected = matches!(snapshot.state, SessionState::Connected);
                let reconnected =
                    connected && !matches!(self.connection.state, SessionState::Connected);
                self.connection = snapshot;
                if connected && (self.recent.is_none() || reconnected) && !self.loading_recent {
                    self.loading_recent = true;
                    return self.load_recent();
                }
            }
            Message::RecentLoaded(result) => {
                self.loading_recent = false;
                self.recent = Some(result);
                if !self.interactive_marked {
                    self.interactive_marked = true;
                    if measure::mark_interactive() {
                        return self.quit();
                    }
                    // The UI bench arrives with the Scenes grid (T031); until then a bench run
                    // says so and ends, so the harness moves on.
                    if measure::bench_requested() {
                        measure::emit(
                            &serde_json::json!({"bench": "done", "note": "bench not built yet (006 T031)"}),
                        );
                        if measure::should_exit(Finish::Bench) {
                            return self.quit();
                        }
                    }
                }
            }
            Message::Open(id) => {
                self.open_error = None;
                let services = Arc::clone(&self.services);
                return Task::perform(
                    async move {
                        services
                            .open_scene(&id)
                            .await
                            .map(|_| ())
                            .map_err(|e| e.to_string())
                    },
                    Message::Opened,
                );
            }
            Message::Opened(Ok(())) => self.screen = Screen::Player,
            Message::Opened(Err(e)) => self.open_error = Some(e),
            Message::Player(msg) => {
                if let PlayerMsg::Snapshot(s) = &msg {
                    self.follow_player(s);
                }
                if let Some(p) = self.services.player.as_ref() {
                    self.player.update(msg, p);
                }
            }
            Message::ClosePlayer => return self.close_player(),
            Message::ToggleFullscreen => {
                return self.set_fullscreen(!self.player.snapshot.fullscreen)
            }
            Message::WindowId(id) => self.window = id,
            Message::WindowMode(mode) => {
                // The desktop can take the window out of fullscreen (Meta on KDE).
                let fullscreen = mode == window::Mode::Fullscreen;
                if let Some(p) = &self.services.player {
                    if p.snapshot().fullscreen != fullscreen {
                        p.set_fullscreen_flag(fullscreen);
                    }
                }
            }
            Message::Resized => {
                if let Some(id) = self.window {
                    return window::mode(id).map(Message::WindowMode);
                }
            }
            Message::Key(key) => return self.key(&key),
            Message::FrameReady => {}
            Message::MeasureDone => {
                self.measuring = false;
                if measure::should_exit(Finish::Measure) {
                    return self.quit();
                }
            }
            Message::CloseRequested => return self.quit(),
        }
        Task::none()
    }

    /// Show the player whenever something plays (including the measurement run's scenes), and
    /// leave fullscreen when playback stops by any path.
    fn follow_player(&mut self, s: &PlayerSnapshot) {
        let was_idle = self.player.snapshot.state == PlayerStateKind::Idle;
        if s.state != PlayerStateKind::Idle && was_idle && self.screen == Screen::Home {
            self.screen = Screen::Player;
        }
        if s.state == PlayerStateKind::Idle && !was_idle && self.measuring {
            // Between measured scenes: stay on the player screen.
        }
    }

    fn key(&mut self, key: &keyboard::Key) -> Task<Message> {
        if self.screen != Screen::Player {
            return Task::none();
        }
        let Some(p) = self.services.player.clone() else {
            return Task::none();
        };
        let paused = self.player.snapshot.paused;
        let Some(action) = key_action(key, paused) else {
            return Task::none();
        };
        let msg = match action {
            Action::TogglePause => PlayerMsg::TogglePause,
            Action::SeekRelative(s) => PlayerMsg::Skip(s),
            Action::Volume(d) => PlayerMsg::VolumeBy(d),
            Action::Speed(d) => PlayerMsg::SpeedStep(d),
            Action::NormalSpeed => PlayerMsg::Speed(1.0),
            Action::FrameStep(d) => PlayerMsg::Frame(d),
            Action::ToggleMute => PlayerMsg::ToggleMute,
            Action::ToggleFullscreen => {
                self.player.update(PlayerMsg::Activity, &p);
                return self.set_fullscreen(!self.player.snapshot.fullscreen);
            }
            Action::Escape => {
                self.player.update(PlayerMsg::Activity, &p);
                return if self.player.snapshot.fullscreen {
                    self.set_fullscreen(false)
                } else {
                    self.close_player()
                };
            }
        };
        self.player.update(msg, &p);
        Task::none()
    }

    fn set_fullscreen(&mut self, fullscreen: bool) -> Task<Message> {
        if let Some(p) = &self.services.player {
            p.set_fullscreen_flag(fullscreen);
        }
        match self.window {
            Some(id) => window::set_mode(
                id,
                if fullscreen {
                    window::Mode::Fullscreen
                } else {
                    window::Mode::Windowed
                },
            ),
            None => Task::none(),
        }
    }

    /// Stop playback, release audio and video, and go back (FR-006, FR-007).
    fn close_player(&mut self) -> Task<Message> {
        if let Some(p) = &self.services.player {
            p.close();
        }
        self.screen = Screen::Home;
        if self.player.snapshot.fullscreen {
            return self.set_fullscreen(false);
        }
        Task::none()
    }

    /// Stop playback and the video path, then leave. Later stories save tabs here first.
    fn quit(&mut self) -> Task<Message> {
        if let Some(player) = &self.services.player {
            player.close();
        }
        if let Some(mut v) = self.video.take() {
            v.stop();
        }
        iced::exit()
    }

    fn load_recent(&self) -> Task<Message> {
        let services = Arc::clone(&self.services);
        Task::perform(
            async move {
                services
                    .read_cached(
                        "scenes:recent",
                        RefreshPolicy::Auto,
                        false,
                        |c| async move { recent_scenes(&c).await },
                    )
                    .await
                    .map(|c| c.data)
                    .map_err(|e| e.to_string())
            },
            Message::RecentLoaded,
        )
    }

    fn view(&self) -> Element<'_, Message> {
        match self.screen {
            Screen::Home => self.home(),
            Screen::Player => {
                let video_error = self.video_error.clone().or_else(|| {
                    self.video
                        .as_ref()
                        .and_then(|v| v.shared().error())
                        .map(|e| e.to_string())
                });
                self.player.view(
                    self.video.as_ref().map(VideoSurface::shared),
                    video_error,
                    Message::Player,
                    Message::ClosePlayer,
                    Message::ToggleFullscreen,
                )
            }
        }
    }

    /// The entry point until the Scenes grid exists (US2): the recently added list.
    fn home(&self) -> Element<'_, Message> {
        let status = text(connection_label(&self.connection)).size(14);
        let body: Element<'_, Message> = match &self.recent {
            None => center(text("Loading…")).into(),
            Some(Err(e)) => center(text(format!("Couldn't load scenes: {e}"))).into(),
            Some(Ok(items)) => scrollable(
                column(items.iter().map(|s| {
                    button(
                        row![
                            text(s.title.clone()).width(Length::Fill),
                            text(s.resolution.clone().unwrap_or_default()).size(13),
                            text(format_time(s.duration_seconds)).size(13),
                        ]
                        .spacing(16),
                    )
                    .width(Length::Fill)
                    .style(button::text)
                    .on_press(Message::Open(s.id.clone()))
                    .into()
                }))
                .spacing(2),
            )
            .into(),
        };
        let mut col =
            column![row![text("Recently added").size(20), status].spacing(20)].spacing(12);
        if let Some(e) = &self.open_error {
            col = col.push(text(format!("Couldn't play: {e}")).size(14));
        }
        container(col.push(body)).padding(16).into()
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            Subscription::run(connection_stream).map(Message::Connection),
            Subscription::run(player_stream).map(|s| Message::Player(PlayerMsg::Snapshot(s))),
            window::close_requests().map(|_| Message::CloseRequested),
            window::resize_events().map(|_| Message::Resized),
            keyboard::listen().filter_map(|event| match event {
                keyboard::Event::KeyPressed { key, .. } => Some(Message::Key(key)),
                _ => None,
            }),
        ];
        if self.screen == Screen::Player {
            subs.push(Subscription::run(video::frames).map(|()| Message::FrameReady));
            // Auto-hide needs a clock while the controls show during playback.
            if self.player.playing() && self.player.controls_visible() {
                subs.push(
                    iced::time::every(Duration::from_millis(250))
                        .map(|_| Message::Player(PlayerMsg::Tick)),
                );
            }
            // A resize settles after 100 ms; keep drawing until it does.
            if self.video.as_ref().is_some_and(|v| v.shared().resizing()) {
                subs.push(window::frames().map(|_| Message::FrameReady));
            }
        }
        Subscription::batch(subs)
    }
}

fn connection_label(snapshot: &ConnectionSnapshot) -> String {
    match &snapshot.state {
        SessionState::Idle => "Not connected".into(),
        SessionState::Connecting { .. } => "Connecting…".into(),
        SessionState::Connected => "Connected".into(),
        SessionState::Offline { .. } => "Offline".into(),
        SessionState::AuthFailed { .. } => "API key rejected".into(),
        SessionState::Failed { failure } => format!("Couldn't connect: {failure:?}"),
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

/// Player snapshots as messages (mpv's state, throttled by the player to ~4 position updates/s).
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

/// Connect at launch: the harness's profile if it names one (clearing its cache if asked), else
/// the last-used profile (as the web build's `auto_connect`).
fn auto_connect(services: &Services) {
    if let Some(name) = measure::harness_profile() {
        let Some(profile) = services.profile_named(&name) else {
            measure::emit(&serde_json::json!({"invalid": "profile not found"}));
            return;
        };
        if measure::clear_cache_requested() {
            services.clear_cache(profile.id);
        }
        services.connect(&profile);
    } else if let Some(profile) = services.last_used() {
        tracing::info!(id = %profile.id, "auto-connecting to last-used profile");
        services.connect(&profile);
    }
}
