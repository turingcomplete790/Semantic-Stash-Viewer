//! The image viewer (012 US2, contracts/images-ui.md): a mode of the Images screen showing one
//! image at a time in the session's mpv. Its rules are the core's pure machine
//! ([`stash_core::images::viewer`]); this holds what the window adds (the pointer, the controls'
//! fade, the picture's size) and draws the picture with its top bar and edge arrows.

use std::time::{Duration, Instant};

use iced::widget::{button, center, column, container, mouse_area, row, stack, text, Space};
use iced::{mouse, Alignment, Color, Element, Length, Point};
use stash_core::images::viewer::{Event, View, Viewer};

use super::state::ImagesMsg;
use crate::screens::browse::layout::Layout;
use crate::shell::ShellMsg;
use crate::widgets::{icon_button, one_line, theme, Icon};

/// The controls fade after this long without pointer movement.
pub const CONTROLS_FADE: Duration = Duration::from_secs(2);
/// The navigation bar and tab strip above the picture.
pub const CHROME: f32 = 88.0;
/// One zoom step in log2: a key's (half a doubling) and a wheel notch's.
pub const ZOOM_KEY: f32 = 0.5;
pub const ZOOM_WHEEL: f32 = 0.25;
/// One arrow-key pan step, in fractions of the shown picture.
pub const PAN_KEY: f32 = 0.05;

/// The viewer's window-side state (transient: a restart shows the grid at the image last viewed).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ViewerState {
    pub machine: Viewer,
    /// The result index whose page is loading (a step across a page edge); shown on arrival.
    pub pending: Option<u32>,
    /// The image in the player (its id), what the player's snapshots are about.
    pub showing: Option<String>,
    pub title: String,
    /// The picture's size in image pixels, once shown.
    pub picture: Option<(u32, u32)>,
    /// Why the image couldn't be shown (with `machine.failed`).
    pub error: Option<String>,
    pub pointer: Option<Point>,
    /// Where a drag (pan) is, while the button is down.
    pub drag: Option<Point>,
    /// The last pointer movement (the controls show for [`CONTROLS_FADE`] after it).
    pub activity: Option<Instant>,
}

#[derive(Debug, Clone)]
pub enum ViewerMsg {
    /// A key or a control: the machine's event (zoom about the centre).
    Event(Event),
    /// A wheel turn (`y` notches, positive away from the user); `ctrl` zooms instead of
    /// stepping (the session fills it in from the keyboard's state).
    Wheel {
        y: f32,
        ctrl: bool,
    },
    Pointer(Point),
    Press,
    Release,
    DoubleClick,
    Close,
}

impl ViewerState {
    /// Whether the top bar and the edge arrows show.
    pub fn controls_visible(&self, now: Instant) -> bool {
        self.drag.is_some()
            || self
                .activity
                .is_none_or(|t| now.duration_since(t) < CONTROLS_FADE)
    }

    /// The picture's area: the tab's content (the whole window in fullscreen).
    pub fn area(layout: &Layout) -> (f32, f32) {
        (layout.width, (layout.height - CHROME).max(1.0))
    }

    /// The size the picture shows at with zoom 0 (fitted, or one image pixel per screen pixel),
    /// rotated, in logical pixels.
    pub fn base_size(&self, layout: &Layout, view: &View) -> Option<(f32, f32)> {
        let (w, h) = self.picture?;
        let (w, h) = if view.quarter_turns % 2 == 1 {
            (h as f32, w as f32)
        } else {
            (w as f32, h as f32)
        };
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        if view.actual {
            return Some((w / layout.scale, h / layout.scale));
        }
        let (aw, ah) = Self::area(layout);
        let fit = (aw / w).min(ah / h);
        Some((w * fit, h * fit))
    }

    /// The pointer as the machine's zoom anchor: from the picture's centre, in fractions of its
    /// zoom-0 size (the centre without a pointer or a picture size).
    pub fn anchor(&self, layout: &Layout) -> (f32, f32) {
        let view = self.machine.view;
        let (Some(p), Some((bw, bh))) = (self.pointer, self.base_size(layout, &view)) else {
            return (0.0, 0.0);
        };
        let (aw, ah) = Self::area(layout);
        ((p.x - aw / 2.0) / bw, (p.y - ah / 2.0) / bh)
    }

    /// A pointer movement of `dx`, `dy` logical pixels as a pan, in fractions of the shown picture.
    pub fn pan_by(&self, layout: &Layout, dx: f32, dy: f32) -> Option<(f32, f32)> {
        let view = self.machine.view;
        let (bw, bh) = self.base_size(layout, &view)?;
        let scale = view.zoom.exp2();
        Some((dx / (bw * scale), dy / (bh * scale)))
    }
}

/// `"7 of 214"`, from 1.
pub fn position(machine: &Viewer) -> String {
    format!("{} of {}", machine.at + 1, machine.count)
}

fn msg(m: ViewerMsg) -> ShellMsg {
    ShellMsg::Images(ImagesMsg::Viewer(m))
}

fn event(e: Event) -> ShellMsg {
    msg(ViewerMsg::Event(e))
}

/// The top bar: close, position and title, rotate, fit or actual size.
fn top_bar<'a>(v: &'a ViewerState) -> Element<'a, ShellMsg> {
    let actual = v.machine.view.actual;
    container(
        row![
            icon_button(Icon::Close, 16.0, Some(msg(ViewerMsg::Close))),
            text(position(&v.machine)).size(14).color(theme::MUTED),
            one_line(v.title.clone(), 14.0),
            Space::new().width(Length::Fill),
            icon_button(
                Icon::RotateCw,
                16.0,
                Some(event(Event::Rotate { clockwise: true }))
            ),
            icon_button(
                if actual {
                    Icon::Minimize
                } else {
                    Icon::Maximize
                },
                16.0,
                Some(event(Event::ToggleActual))
            ),
        ]
        .spacing(10)
        .padding([6, 10])
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .style(|_| container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.55).into()),
        text_color: Some(theme::TEXT),
        ..Default::default()
    })
    .into()
}

/// An edge arrow, disabled at the result's ends.
fn edge(which: Icon, enabled: bool, step: Event) -> Element<'static, ShellMsg> {
    container(
        button(crate::widgets::icon(which, 28.0))
            .padding(10)
            .style(theme::chip)
            .on_press_maybe(enabled.then_some(event(step))),
    )
    .height(Length::Fill)
    .center_y(Length::Fill)
    .padding(8)
    .into()
}

/// The viewer: `picture` is the player's picture (black when the player shows something else).
pub fn view<'a>(v: &'a ViewerState, picture: Element<'a, ShellMsg>) -> Element<'a, ShellMsg> {
    let mut layers = stack![container(picture)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style::default().background(Color::BLACK))];
    if v.machine.failed == Some(v.machine.at) {
        let reason = v
            .error
            .clone()
            .unwrap_or_else(|| "The file couldn't be read.".to_owned());
        layers = layers.push(center(
            container(
                column![
                    text("Couldn't show this image").size(16),
                    text(reason).size(13).color(theme::MUTED),
                ]
                .spacing(8)
                .align_x(Alignment::Center),
            )
            .padding(16)
            .style(container::rounded_box),
        ));
    }
    let visible = v.controls_visible(Instant::now());
    if visible {
        let m = &v.machine;
        layers = layers.push(column![
            top_bar(v),
            row![
                edge(Icon::ChevronLeft, m.at > 0, Event::Previous),
                Space::new().width(Length::Fill),
                edge(Icon::ChevronRight, m.at + 1 < m.count, Event::Next),
            ]
            .height(Length::Fill),
        ]);
    }
    mouse_area(layers)
        .on_move(|p| msg(ViewerMsg::Pointer(p)))
        .on_press(msg(ViewerMsg::Press))
        .on_release(msg(ViewerMsg::Release))
        .on_double_click(msg(ViewerMsg::DoubleClick))
        .on_scroll(|delta| {
            let y = match delta {
                mouse::ScrollDelta::Lines { y, .. } => y,
                mouse::ScrollDelta::Pixels { y, .. } => y / 40.0,
            };
            msg(ViewerMsg::Wheel { y, ctrl: false })
        })
        .interaction(if v.drag.is_some() {
            mouse::Interaction::Grabbing
        } else if visible {
            mouse::Interaction::None
        } else {
            mouse::Interaction::Hidden
        })
        .into()
}
