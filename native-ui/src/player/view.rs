//! The player screen (006 T019): mpv's video with the controls stacked on top (002 FR-008–FR-011,
//! FR-015). Commands go straight to `player::Player`; state comes back as snapshots.

use std::sync::Arc;
use std::time::Instant;

use iced::widget::{
    button, center, column, container, mouse_area, pick_list, responsive, row, shader, slider,
    space, stack, text,
};
use iced::{mouse, Alignment, Background, Color, Element, Length, Theme};
use player::{PlayerSnapshot, PlayerStateKind};

use crate::effects::{Effect, PlayerAction};

use super::controls::{
    chapter_at, chapter_target, hover_time, next_speed, AutoHide, Direction, SeekBar,
    RESTART_OFFER, SKIP_SECONDS, SPEEDS,
};
use super::video::primitive::Video;
use super::video::Shared;

/// The playing scene's seek-bar sprites (008 FR-021): the decoded sheet and its index.
#[derive(Debug, Clone)]
pub struct Sprites {
    pub cues: Arc<Vec<stash_core::sprites::SpriteCue>>,
    pub sheet: iced::widget::image::Handle,
}

/// The seek-bar thumbnail's width (Stash's sprites are 160 × 90).
const SPRITE_WIDTH: f32 = 160.0;

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
    /// "Start from the beginning" after a resume (008 FR-012).
    StartOver,
    /// The next or previous chapter (008 FR-013).
    Chapter(Direction),
    /// "Play transcoded" after the file couldn't be decoded (008 FR-023).
    PlayTranscoded,
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
    /// The scene's markers as `(start seconds, title)`, in time order (008 FR-013).
    pub chapters: Vec<(f64, String)>,
    /// "Start from the beginning" shows until then (008 FR-012).
    pub restart_offer: Option<Instant>,
    /// Seek-bar thumbnails, once loaded (asked for on the first hover over the bar).
    pub sprites: Option<Sprites>,
    pub sprites_requested: bool,
    /// Why the direct stream failed, once a transcode was chosen (both show if that fails too).
    pub direct_failure: Option<String>,
}

impl Default for PlayerScreen {
    fn default() -> Self {
        Self {
            snapshot: PlayerSnapshot::default(),
            hide: AutoHide::new(Instant::now()),
            seek: SeekBar::default(),
            dragging: None,
            hover: None,
            chapters: Vec::new(),
            restart_offer: None,
            sprites: None,
            sprites_requested: false,
            direct_failure: None,
        }
    }
}

impl PlayerScreen {
    pub fn playing(&self) -> bool {
        self.snapshot.state == PlayerStateKind::Playing
    }

    /// The file couldn't be decoded and no transcode was tried yet: offer one (008 FR-023). A
    /// network failure, or a transcode that failed, offers nothing.
    pub fn offers_transcode(&self) -> bool {
        self.snapshot.state == PlayerStateKind::Error
            && !self.snapshot.transcoded
            && self
                .snapshot
                .error
                .as_ref()
                .is_some_and(player::PlayerError::is_decode_failure)
    }

    /// Offer "Start from the beginning" from `now` for [`RESTART_OFFER`].
    pub fn offer_restart(&mut self, now: Instant) {
        self.restart_offer = Some(now + RESTART_OFFER);
    }

    /// Whether "Start from the beginning" is showing.
    pub fn restart_offered(&self) -> bool {
        self.restart_offer
            .is_some_and(|until| Instant::now() < until)
    }

    /// The screen changes by itself (controls hiding, the offer ending): a timer must run.
    pub fn needs_tick(&self) -> bool {
        (self.playing() && self.controls_visible()) || self.restart_offered()
    }

    pub fn controls_visible(&self) -> bool {
        self.dragging.is_some() || self.hide.visible(Instant::now(), self.playing())
    }

    fn duration(&self) -> f64 {
        self.snapshot.duration_seconds.unwrap_or(0.0)
    }

    /// Apply a control and return what to send to mpv (no I/O here, 007 research R2).
    pub fn update(&mut self, msg: Msg) -> Vec<Effect> {
        let now = Instant::now();
        let action = match msg {
            Msg::Snapshot(s) => {
                self.snapshot = s;
                return Vec::new();
            }
            Msg::Tick => return Vec::new(),
            Msg::Activity => None,
            Msg::TogglePause => Some(PlayerAction::TogglePause),
            Msg::Skip(seconds) => {
                let target = (self.seek.peek(self.snapshot.position_seconds, now) + seconds)
                    .clamp(0.0, self.duration().max(0.0));
                self.seek.seek_to(target, now);
                Some(PlayerAction::SeekRelative(seconds))
            }
            Msg::SeekDrag(t) => {
                self.dragging = Some(t);
                // Keyframe-fast while dragging (as 002's player).
                Some(PlayerAction::Seek {
                    seconds: t,
                    exact: false,
                })
            }
            Msg::SeekRelease => self.dragging.take().map(|t| {
                self.seek.seek_to(t, now);
                PlayerAction::Seek {
                    seconds: t,
                    exact: true,
                }
            }),
            Msg::Hover { x, width } => {
                self.hover = Some((x, hover_time(x, width, self.duration())));
                // The sprites load on the first hover (two GETs, then cached).
                if !self.sprites_requested {
                    if let Some(scene_id) = self.snapshot.scene_id.clone() {
                        self.sprites_requested = true;
                        return vec![Effect::LoadSprite { scene_id }];
                    }
                }
                None
            }
            Msg::HoverEnd => {
                self.hover = None;
                None
            }
            Msg::Volume(v) => Some(PlayerAction::SetVolume(v.clamp(0.0, 100.0))),
            Msg::VolumeBy(d) => Some(PlayerAction::SetVolume(
                (self.snapshot.volume + d).clamp(0.0, 100.0),
            )),
            Msg::ToggleMute => Some(PlayerAction::SetMuted(!self.snapshot.muted)),
            Msg::Speed(s) => Some(PlayerAction::SetSpeed(s)),
            Msg::SpeedStep(d) => Some(PlayerAction::SetSpeed(next_speed(self.snapshot.speed, d))),
            Msg::Frame(d) => Some(PlayerAction::FrameStep(d)),
            Msg::Replay => Some(PlayerAction::Replay),
            // The playback region opens the transcode (it owns the viewing).
            Msg::PlayTranscoded => None,
            Msg::StartOver => {
                self.restart_offer = None;
                self.seek.seek_to(0.0, now);
                Some(PlayerAction::Seek {
                    seconds: 0.0,
                    exact: true,
                })
            }
            Msg::Chapter(d) => {
                let position = self.seek.peek(self.snapshot.position_seconds, now);
                chapter_target(&self.chapters, position, d).map(|t| {
                    self.seek.seek_to(t, now);
                    PlayerAction::Seek {
                        seconds: t,
                        exact: true,
                    }
                })
            }
        };
        self.hide.activity(now);
        action.map(Effect::Player).into_iter().collect()
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
            let mut status = column![text(message).size(16)]
                .spacing(12)
                .align_x(Alignment::Center);
            if self.offers_transcode() {
                status = status.push(
                    column![
                        button(text("Play transcoded")).on_press(on(Msg::PlayTranscoded)),
                        text("The server converts the file as it plays; quality may be lower.")
                            .size(12),
                    ]
                    .spacing(6)
                    .align_x(Alignment::Center),
                );
            }
            layers = layers.push(center(
                container(status).padding(16).style(container::rounded_box),
            ));
        }
        let visible = self.controls_visible();
        if visible {
            layers = layers.push(self.overlay(on, close, fullscreen));
        }
        // After a resume, whether or not the controls show (008 FR-012).
        if self.restart_offered() {
            layers = layers.push(
                container(
                    button(text("Start from the beginning"))
                        .on_press(on(Msg::StartOver))
                        .padding([8, 14]),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .padding([96, 24])
                .align_x(iced::alignment::Horizontal::Left)
                .align_y(iced::alignment::Vertical::Bottom),
            );
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
            PlayerStateKind::Error => {
                let reason = self.snapshot.error.as_ref().map_or_else(
                    || "This scene can't be played.".into(),
                    |e| sentence(&e.to_string()),
                );
                // A transcode that failed too: both reasons, and nothing more to try.
                Some(match (&self.direct_failure, self.snapshot.transcoded) {
                    (Some(direct), true) => format!(
                        "The file couldn't be played: {}. The server's transcode didn't play \
                         either: {reason}",
                        direct.trim_end_matches('.')
                    ),
                    _ => reason,
                })
            }
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
        let mut top_row = row![text(title).size(18).width(Length::Fill)]
            .spacing(10)
            .align_y(Alignment::Center);
        // Playing the server's transcode, not the file (008 FR-024): always marked.
        if s.transcoded {
            top_row = top_row.push(
                container(text("Transcoded").size(12))
                    .padding([2, 8])
                    .style(container::rounded_box),
            );
        }
        let top = container(top_row.push(button(text("✕")).on_press(close).style(button::text)))
            .padding(12)
            .style(bar_style);

        let duration = self.duration();
        let position = self
            .dragging
            .unwrap_or_else(|| self.seek.peek(s.position_seconds, Instant::now()));
        let hover_label: Element<'a, M> = match self.hover {
            Some((x, t)) => {
                let label = match chapter_at(&self.chapters, t) {
                    Some(title) => format!("{title} · {}", format_time(t)),
                    None => format_time(t),
                };
                // The frame at that time from the sprite sheet, centred over the pointer.
                let thumb: Option<Element<'a, M>> = self.sprites.as_ref().and_then(|s| {
                    let r = stash_core::sprites::lookup(&s.cues, t)?;
                    let height = SPRITE_WIDTH * r.h as f32 / r.w.max(1) as f32;
                    Some(
                        iced::widget::image(s.sheet.clone())
                            .crop(iced::Rectangle {
                                x: r.x,
                                y: r.y,
                                width: r.w,
                                height: r.h,
                            })
                            .width(Length::Fixed(SPRITE_WIDTH))
                            .height(Length::Fixed(height))
                            .into(),
                    )
                });
                let mut tip = column![].spacing(2).align_x(Alignment::Center);
                if let Some(thumb) = thumb {
                    tip = tip.push(thumb);
                }
                tip = tip.push(text(label).size(12));
                let lead = if self.sprites.is_some() {
                    x - SPRITE_WIDTH / 2.0
                } else {
                    x - 24.0
                };
                row![space::horizontal().width(Length::Fixed(lead.max(0.0))), tip].into()
            }
            None => text(" ").size(12).into(),
        };
        let chapters = &self.chapters;
        let seek_bar = responsive(move |size| {
            let width = size.width;
            let bar = mouse_area(
                slider(
                    0.0..=duration.max(0.001),
                    position.min(duration.max(0.001)),
                    move |t| on(Msg::SeekDrag(t)),
                )
                .step(0.1)
                .on_release(on(Msg::SeekRelease)),
            )
            .on_move(move |p| on(Msg::Hover { x: p.x, width }))
            .on_exit(on(Msg::HoverEnd));
            // Chapter starts as ticks across the bar (008 FR-013); they don't take the pointer.
            let mut layers = stack![bar];
            if duration > 0.0 {
                for (t, _) in chapters {
                    let x = (*t / duration).clamp(0.0, 1.0) as f32 * width;
                    layers = layers.push(
                        row![
                            space::horizontal().width(Length::Fixed((x - 1.0).max(0.0))),
                            container(space::horizontal().width(Length::Fixed(2.0)))
                                .height(Length::Fixed(10.0))
                                .style(chapter_tick),
                        ]
                        .height(Length::Fill)
                        .align_y(Alignment::Center),
                    );
                }
            }
            layers.into()
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

fn chapter_tick(_: &Theme) -> container::Style {
    container::Style::default().background(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.85)))
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
