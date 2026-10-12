//! The Scenes grid and list (007 T038): the browse layer's grid (012) with scene cards: a 16:9
//! thumbnail and one line each for title and details; the wall's stills; and the queue view's rows
//! (010) with the playing mark.

use iced::widget::{button, column, container, mouse_area, row, Space};
use iced::{Alignment, Element, Length};
use stash_core::scenes::SceneCard;

use super::layout::{Layout, CARD_TEXT, LIST_ROW, PAD, QUEUE_ROW, SCROLLBAR};
use super::state::{shell_msg, SceneKind, ScenesMsg, ScenesState};
use crate::screens::browse::controls::Map;
use crate::screens::browse::grid::{self, clickable, lines, thumb, Pictures};
use crate::screens::{Context, Mode};
use crate::shell::{ShellMsg, TabId};
use crate::widgets::theme;

/// `1:02:03` or `4:05`.
pub fn duration(seconds: f64) -> String {
    let s = seconds.max(0.0).round() as u64;
    let (h, m, s) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// The card's second line: date, duration, resolution, studio.
pub fn details(card: &SceneCard) -> String {
    [
        card.date.clone(),
        card.duration_seconds.map(duration),
        card.resolution.clone(),
        card.studio.clone(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ")
}

/// A scene card in `mode` (the browse grid's [`crate::screens::browse::state::Kind::card_view`]).
#[allow(clippy::too_many_arguments)]
pub fn card_view(
    c: &SceneCard,
    index: usize,
    focused: bool,
    marked: bool,
    mode: Mode,
    layout: &Layout,
    pictures: Pictures<'_>,
    msg: Map<SceneKind>,
) -> Element<'static, ShellMsg> {
    match mode {
        Mode::Grid => {
            let w = layout.card_width();
            let body = column![
                thumb::<SceneKind>(pictures, c, w, layout.thumb_height()),
                lines(&c.title, &details(c), w, 14.0, 12.0, 2.0).padding([4, 8]),
            ];
            clickable::<SceneKind>(
                body.into(),
                index,
                focused,
                Length::Fixed(w),
                layout.thumb_height() + CARD_TEXT,
                msg,
            )
        }
        Mode::List => {
            let list_text_width = layout.width - 2.0 * PAD - 128.0 - 12.0;
            let body = row![
                thumb::<SceneKind>(pictures, c, 128.0, 72.0),
                lines(&c.title, &details(c), list_text_width, 15.0, 13.0, 4.0),
            ]
            .spacing(12)
            .padding(2)
            .align_y(Alignment::Center);
            clickable::<SceneKind>(body.into(), index, focused, Length::Fill, LIST_ROW, msg)
        }
        // A wall tile (008 FR-020): the still alone, no text.
        Mode::Wall => {
            let (w, h) = (layout.wall_width(), layout.wall_height());
            clickable::<SceneKind>(
                thumb::<SceneKind>(pictures, c, w, h),
                index,
                focused,
                Length::Fixed(w),
                h,
                msg,
            )
        }
        Mode::Queue => queue_row(c, index, focused, marked, layout, pictures),
    }
}

/// A queue row (010 FR-002): a small still, title, and details line. A click (or double-click)
/// plays it and the panel follows (011 FR-001), a middle-click opens its page in a new tab; no
/// hover preview, so scrolling the list stays light.
fn queue_row(
    c: &SceneCard,
    index: usize,
    selected: bool,
    playing: bool,
    layout: &Layout,
    pictures: Pictures<'_>,
) -> Element<'static, ShellMsg> {
    const STILL: (f32, f32) = (96.0, 54.0);
    const MARK: f32 = 18.0;
    let text_width = layout.queue_list_width() - 2.0 * PAD - SCROLLBAR - STILL.0 - MARK - 20.0;
    // The playing scene's mark, distinct from the selection's ring (010 FR-011).
    let mark: Element<'static, ShellMsg> = if playing {
        crate::widgets::icon(crate::widgets::Icon::Play, 14.0).into()
    } else {
        Space::new().width(14.0).into()
    };
    let body = row![
        container(mark).width(MARK).center_x(MARK),
        thumb::<SceneKind>(pictures, c, STILL.0, STILL.1),
        lines(&c.title, &details(c), text_width, 14.0, 12.0, 3.0),
    ]
    .spacing(10)
    .padding(3)
    .align_y(Alignment::Center);
    mouse_area(
        button(body)
            .padding(0)
            .width(Length::Fill)
            .height(Length::Fixed(QUEUE_ROW))
            .style(theme::card(selected))
            .on_press(ShellMsg::Scenes(ScenesMsg::QueueSelect(index))),
    )
    .on_middle_press(ShellMsg::Scenes(ScenesMsg::Open {
        index,
        new_tab: true,
    }))
    .on_double_click(ShellMsg::Scenes(ScenesMsg::QueuePlay(index)))
    .into()
}

pub fn view<'a>(state: &'a ScenesState, ctx: &Context<'a>, tab: TabId) -> Element<'a, ShellMsg> {
    view_with(state, ctx, tab, None)
}

/// The screen, with `player` in the queue panel's picture slot while this tab's scene plays there
/// (010 R2).
pub fn view_with<'a>(
    state: &'a ScenesState,
    ctx: &Context<'a>,
    tab: TabId,
    player: Option<Element<'a, ShellMsg>>,
) -> Element<'a, ShellMsg> {
    let marked = state
        .queue
        .playing
        .as_deref()
        .filter(|_| state.list.mode == Mode::Queue);
    let layout = ctx.layout;
    let (library, thumbs) = (ctx.library, ctx.thumbs);
    grid::view(&state.list, ctx, tab, marked, shell_msg, move |list| {
        if state.list.mode == Mode::Queue {
            super::queue::view(
                &state.queue,
                state.list.cards(),
                super::queue::strip(state.shuffle, state.repeat, state.autoplay),
                list,
                player,
                &layout,
                library,
                thumbs,
            )
        } else {
            list
        }
    })
}
