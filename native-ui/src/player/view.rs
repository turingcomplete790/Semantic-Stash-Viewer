//! The player screen (006 T019): mpv's video with the controls stacked on top (002 FR-008–FR-011,
//! FR-015). Commands go straight to `player::Player`; state comes back as snapshots.

use std::sync::Arc;
use std::time::Instant;

use iced::widget::{
    button, center, column, container, mouse_area, pick_list, responsive, row, shader, slider,
    space, stack, text,
};
use iced::{mouse, Alignment, Background, Color, Element, Length, Theme};
use player::{Player, PlayerSnapshot, PlayerStateKind};

use super::controls::{hover_time, next_speed, AutoHide, Direction, SeekBar, SKIP_SECONDS, SPEEDS};
use super::video::primitive::Video;
use super::video::Shared;

#[derive(Debug, Clone)]
pub enum Msg {
    Snapshot(PlayerSnapshot),
    TogglePause,
    Skip(f64),
    SeekDrag(f64),
    SeekRelease,
    Hover {
        x: f32,
        width: f32,
    },
    HoverEnd,
    Volume(f64),
    VolumeBy(f64),
    ToggleMute,
    Speed(f64),
    SpeedStep(Direction),
    Frame(Direction),
    Replay,
    /// Mouse movement or a key: show the controls.
    Activity,
    /// Re-check auto-hide.
    Tick,
}

pub struct PlayerScreen {
    pub snapshot: PlayerSnapshot,
    hide: AutoHide,
    seek: SeekBar,
    dragging: Option<f64>,
    hover: Option<(f32, f64)>,
}

impl Default for PlayerScreen {
    fn default() -> Self {
        Self {
            snapshot: PlayerSnapshot::default(),
            hide: AutoHide::new(Instant::now()),
            seek: SeekBar::default(),
            dragging: None,
            hover: None,
        }
    }
}

impl PlayerScreen {
    pub fn playing(&self) -> bool {
        self.snapshot.state == PlayerStateKind::Playing
    }

    pub fn controls_visible(&self) -> bool {
        self.dragging.is_some() || self.hide.visible(Instant::now(), self.playing())
    }

    fn duration(&self) -> f64 {
        self.snapshot.duration_seconds.unwrap_or(0.0)
    }

    pub fn update(&mut self, msg: Msg, player: &Player) {
        let now = Instant::now();
        match msg {
            Msg::Snapshot(s) => {
                self.snapshot = s;
                return;
            }
            Msg::Tick => return,
            Msg::Activity => {}
            Msg::TogglePause => player.toggle_pause(),
            Msg::Skip(seconds) => {
                let target = (self.seek.peek(self.snapshot.position_seconds, now) + seconds)
                    .clamp(0.0, self.duration().max(0.0));
                self.seek.seek_to(target, now);
                player.seek_relative(seconds);
            }
            Msg::SeekDrag(t) => {
                self.dragging = Some(t);
                // Keyframe-fast while dragging (as the web player).
                player.seek(t, false);
            }
            Msg::SeekRelease => {
                if let Some(t) = self.dragging.take() {
                    self.seek.seek_to(t, now);
                    player.seek(t, true);
                }
            }
            Msg::Hover { x, width } => {
                self.hover = Some((x, hover_time(x, width, self.duration())))
            }
            Msg::HoverEnd => self.hover = None,
            Msg::Volume(v) => player.set_volume(v.clamp(0.0, 100.0)),
            Msg::VolumeBy(d) => player.set_volume((self.snapshot.volume + d).clamp(0.0, 100.0)),
            Msg::ToggleMute => player.set_muted(!self.snapshot.muted),
            Msg::Speed(s) => player.set_speed(s),
            Msg::SpeedStep(d) => player.set_speed(next_speed(self.snapshot.speed, d)),
            Msg::Frame(Direction::Forward) => player.frame_step_forward(),
            Msg::Frame(Direction::Back) => player.frame_step_back(),
            Msg::Replay => player.replay(),
        }
        self.hide.activity(now);
    }

    /// The screen. `on` maps this screen's messages into the app's; `close` and `fullscreen` are
    /// the app's own (they change the screen and the window).
    pub fn view<'a, M: Clone + 'a>(
        &'a self,
        video: Option<&'a Arc<Shared>>,
        video_error: Option<String>,
        on: impl Fn(Msg) -> M + Copy + 'a,
        close: M,
        fullscreen: M,
    ) -> Element<'a, M> {
        let picture: Element<'a, M> = match video {
            Some(shared) => shader(Video::new(Arc::clone(shared)))
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            None => space::horizontal().into(),
        };
        let mut layers = stack![container(picture)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::Style::default().background(Color::BLACK))];

        if let Some(message) = self.status_message(video_error) {
            layers = layers.push(center(
                container(text(message).size(16))
                    .padding(16)
                    .style(container::rounded_box),
            ));
        }
        let visible = self.controls_visible();
        if visible {
            layers = layers.push(self.overlay(on, close, fullscreen));
        }
        mouse_area(layers)
            .on_move(move |_| on(Msg::Activity))
            .interaction(if visible {
                mouse::Interaction::None
            } else {
                mouse::Interaction::Hidden
            })
            .into()
    }

    fn status_message(&self, video_error: Option<String>) -> Option<String> {
        if let Some(e) = video_error {
            return Some(format!("Video can't be shown: {e}"));
        }
        match self.snapshot.state {
            PlayerStateKind::Loading => Some("Opening…".into()),
            PlayerStateKind::Error => Some(self.snapshot.error.as_ref().map_or_else(
                || "This scene can't be played.".into(),
                |e| sentence(&e.to_string()),
            )),
            _ => None,
        }
    }

    fn overlay<'a, M: Clone + 'a>(
        &'a self,
        on: impl Fn(Msg) -> M + Copy + 'a,
        close: M,
        fullscreen: M,
    ) -> Element<'a, M> {
        let s = &self.snapshot;
        let title = s.title.clone().unwrap_or_default();
        let top = container(
            row![
                text(title).size(18).width(Length::Fill),
                button(text("✕")).on_press(close).style(button::text),
            ]
            .align_y(Alignment::Center),
        )
        .padding(12)
        .style(bar_style);

        let duration = self.duration();
        let position = self
            .dragging
            .unwrap_or_else(|| self.seek.peek(s.position_seconds, Instant::now()));
        let hover_label: Element<'a, M> = match self.hover {
            Some((x, t)) => row![
                space::horizontal().width(Length::Fixed((x - 24.0).max(0.0))),
                text(format_time(t)).size(12)
            ]
            .into(),
            None => text(" ").size(12).into(),
        };
        let seek_bar = responsive(move |size| {
            let width = size.width;
            mouse_area(
                slider(
                    0.0..=duration.max(0.001),
                    position.min(duration.max(0.001)),
                    move |t| on(Msg::SeekDrag(t)),
                )
                .step(0.1)
                .on_release(on(Msg::SeekRelease)),
            )
            .on_move(move |p| on(Msg::Hover { x: p.x, width }))
            .on_exit(on(Msg::HoverEnd))
            .into()
        })
        .height(Length::Fixed(24.0));

        let paused = s.paused || s.state == PlayerStateKind::Paused;
        let mut buttons = row![
            button(text("−10")).on_press(on(Msg::Skip(-SKIP_SECONDS))),
            if s.state == PlayerStateKind::Ended {
                button(text("Replay")).on_press(on(Msg::Replay))
            } else {
                button(text(if paused { "Play" } else { "Pause" })).on_press(on(Msg::TogglePause))
            },
            button(text("+10")).on_press(on(Msg::Skip(SKIP_SECONDS))),
        ]
        .spacing(6)
        .align_y(Alignment::Center);
        if paused {
            buttons = buttons
                .push(button(text("‹ frame")).on_press(on(Msg::Frame(Direction::Back))))
                .push(button(text("frame ›")).on_press(on(Msg::Frame(Direction::Forward))));
        }
        let speed = pick_list(
            SPEEDS.map(Speed),
            SPEEDS
                .iter()
                .copied()
                .find(|v| (v - s.speed).abs() < 1e-6)
                .map(Speed),
            move |v: Speed| on(Msg::Speed(v.0)),
        )
        .placeholder(format!("{}×", trim(s.speed)));
        let bottom = container(
            column![
                hover_label,
                seek_bar,
                row![
                    buttons,
                    text(format!(
                        "{} / {}",
                        format_time(position),
                        format_time(duration)
                    ))
                    .size(14),
                    space::horizontal(),
                    speed,
                    button(text(if s.muted { "Unmute" } else { "Mute" }))
                        .on_press(on(Msg::ToggleMute)),
                    slider(0.0..=100.0, s.volume, move |v| on(Msg::Volume(v)))
                        .width(Length::Fixed(120.0)),
                    button(text(if s.fullscreen {
                        "Exit fullscreen"
                    } else {
                        "Fullscreen"
                    }))
                    .on_press(fullscreen),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            ]
            .spacing(4),
        )
        .padding(12)
        .style(bar_style);

        column![top, space::vertical(), bottom].into()
    }
}

/// A speed in the menu, shown as `1.25×`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Speed(f64);

impl std::fmt::Display for Speed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}×", trim(self.0))
    }
}

fn trim(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn bar_style(_: &Theme) -> container::Style {
    container::Style::default().background(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.6)))
}

fn sentence(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => format!(
            "{}{}.",
            c.to_uppercase(),
            chars.as_str().trim_end_matches('.')
        ),
        None => String::new(),
    }
}

/// `m:ss`, or `h:mm:ss` from an hour (as the web player's `formatDuration`).
pub fn format_time(seconds: f64) -> String {
    let total = seconds.max(0.0).floor() as u64;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}
