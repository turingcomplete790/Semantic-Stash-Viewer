//! The grid and the list (007 T038), designed for the native app: as many columns as fit with
//! cards at least 240 px wide, a 16:9 thumbnail, one line each for title and details. Click opens
//! the scene, Ctrl+click and middle-click open it in a new tab; the keyboard's card has a ring.

use iced::widget::{
    button, center, column, container, image, keyed_column, lazy, mouse_area, row, scrollable,
    text, Space,
};
use iced::{Alignment, ContentFit, Element, Length};
use stash_core::scenes::SceneCard;

use super::controls;
use super::layout::{Layout, CARD_TEXT, GAP, LIST_ROW, PAD};
use super::state::{scroll_id, Data, ScenesMsg, ScenesState};
use super::thumbs::{placeholder, thumb_key, Thumbs};
use crate::screens::{Context, Mode};
use crate::shell::{ShellMsg, TabId};
use crate::widgets::scroll_watch::scroll_watch;
use crate::widgets::theme;

fn msg(m: ScenesMsg) -> ShellMsg {
    ShellMsg::Scenes(m)
}

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

fn thumb(thumbs: &Thumbs, card: &SceneCard, width: f32, height: f32) -> Element<'static, ShellMsg> {
    let handle = thumb_key(card)
        .and_then(|k| thumbs.get(&k).cloned())
        .unwrap_or_else(placeholder);
    image(handle)
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .content_fit(ContentFit::Cover)
        .border_radius(6)
        .into()
}

fn clickable(
    body: Element<'static, ShellMsg>,
    index: usize,
    focused: bool,
    width: Length,
    height: f32,
) -> Element<'static, ShellMsg> {
    mouse_area(
        button(body)
            .padding(0)
            .width(width)
            .height(Length::Fixed(height))
            .style(theme::card(focused))
            .on_press(msg(ScenesMsg::Open {
                index,
                new_tab: false,
            })),
    )
    .on_middle_press(msg(ScenesMsg::Open {
        index,
        new_tab: true,
    }))
    .into()
}

/// `text` cut to what a line `width` wide at `size` could ever show (the narrowest glyphs are
/// about a third of the size wide). The rest would be clipped anyway, and iced shapes the whole
/// string of a line that doesn't wrap: long file-name titles made rows slow to appear.
fn fit(text: &str, width: f32, size: f32) -> String {
    let max = ((width / (size * 0.3)).ceil() as usize).max(8);
    match text.char_indices().nth(max) {
        Some((cut, _)) => text[..cut].to_owned(),
        None => text.to_owned(),
    }
}

/// Title and details, one line each, clipped together (one clip per card keeps the widget tree
/// small: layout runs on every scroll frame).
fn lines(
    c: &SceneCard,
    width: f32,
    title: f32,
    detail: f32,
    gap: f32,
) -> container::Container<'static, ShellMsg> {
    container(
        column![
            text(fit(&c.title, width, title))
                .size(title)
                .wrapping(text::Wrapping::None),
            text(fit(&details(c), width, detail))
                .size(detail)
                .color(theme::MUTED)
                .wrapping(text::Wrapping::None),
        ]
        .spacing(gap),
    )
    .clip(true)
    .width(Length::Fill)
}

fn card(
    c: &SceneCard,
    index: usize,
    focused: bool,
    layout: &Layout,
    thumbs: &Thumbs,
) -> Element<'static, ShellMsg> {
    let w = layout.card_width();
    let body = column![
        thumb(thumbs, c, w, layout.thumb_height()),
        lines(c, w, 14.0, 12.0, 2.0).padding([4, 8]),
    ];
    clickable(
        body.into(),
        index,
        focused,
        Length::Fixed(w),
        layout.thumb_height() + CARD_TEXT,
    )
}

fn list_row(
    c: &SceneCard,
    index: usize,
    focused: bool,
    layout: &Layout,
    thumbs: &Thumbs,
) -> Element<'static, ShellMsg> {
    let list_text_width = layout.width - 2.0 * PAD - 128.0 - 12.0;
    let body = row![
        thumb(thumbs, c, 128.0, 72.0),
        lines(c, list_text_width, 15.0, 13.0, 4.0),
    ]
    .spacing(12)
    .padding(2)
    .align_y(Alignment::Center);
    clickable(body.into(), index, focused, Length::Fill, LIST_ROW)
}

/// The rows in the window, with spacers for the rest so the scrolled height stays exact.
fn cards(
    state: &ScenesState,
    layout: &Layout,
    thumbs: &Thumbs,
    (first, end): (usize, usize),
) -> Element<'static, ShellMsg> {
    let all = state.cards();
    let cols = layout.cols(state.mode);
    let row_h = layout.row_height(state.mode);
    let total_rows = all.len().div_ceil(cols);
    // Rows are keyed by their place on the page, so a row keeps its widget state (shaped
    // text) while the window moves; only rows entering the window are built fresh.
    let mut rows = keyed_column::<usize, ShellMsg, iced::Theme, iced::Renderer>([]);
    rows = rows.push(usize::MAX - 1, Space::new().height(first as f32 * row_h));
    for r in first..end {
        let mut line = row![].spacing(GAP);
        for (j, c) in all[r * cols..((r + 1) * cols).min(all.len())]
            .iter()
            .enumerate()
        {
            let i = r * cols + j;
            let focused = state.focused == Some(i);
            line = line.push(match state.mode {
                Mode::Grid => card(c, i, focused, layout, thumbs),
                Mode::List => list_row(c, i, focused, layout, thumbs),
            });
        }
        rows = rows.push(r, container(line).height(Length::Fixed(row_h)));
    }
    rows = rows.push(
        usize::MAX,
        Space::new().height((total_rows - end) as f32 * row_h),
    );
    rows.into()
}

/// The cards, rebuilt only when what they show changes: scrolling (which updates the saved
/// position on every frame) then only redraws.
fn cached_cards<'a>(state: &'a ScenesState, ctx: &Context<'a>) -> Element<'a, ShellMsg> {
    let layout = ctx.layout;
    let thumbs = ctx.thumbs;
    let window = layout.window(state.scroll, state.mode, state.cards().len());
    let key = (
        state.loaded,
        state.mode == Mode::Grid,
        state.focused,
        layout.width.to_bits(),
        layout.height.to_bits(),
        thumbs.revision(),
        window,
    );
    lazy(key, move |_| cards(state, &layout, thumbs, window)).into()
}

pub fn view<'a>(state: &'a ScenesState, ctx: &Context<'a>, tab: TabId) -> Element<'a, ShellMsg> {
    let body: Element<'a, ShellMsg> = match &state.data {
        Data::Loading => center(text("Loading scenes…").color(theme::MUTED)).into(),
        Data::Unreachable => center(
            container(
                column![
                    text("Can't reach the server, and this page isn't cached.").size(15),
                    button(text("Try again")).on_press(msg(ScenesMsg::Retry)),
                ]
                .spacing(10)
                .align_x(Alignment::Center),
            )
            .padding(16)
            .style(theme::problem),
        )
        .into(),
        Data::Ready { cards: c, .. } if c.is_empty() => {
            center(text("No scenes match.").color(theme::MUTED)).into()
        }
        Data::Ready { .. } => scroll_watch(
            scrollable(
                column![
                    cached_cards(state, ctx),
                    Space::new().height(8),
                    controls::pager(state),
                ]
                .padding(iced::Padding {
                    top: 4.0,
                    right: PAD,
                    bottom: PAD,
                    left: PAD,
                }),
            )
            .id(scroll_id(tab))
            .height(Length::Fill),
            ctx.layout.row_height(state.mode),
            state.scroll,
            |y| msg(ScenesMsg::Scrolled { y }),
        )
        .auto_scroll(state.auto_scroll, |r| msg(ScenesMsg::AutoScrolled(r)))
        .into(),
    };
    column![controls::top(state), body].into()
}
