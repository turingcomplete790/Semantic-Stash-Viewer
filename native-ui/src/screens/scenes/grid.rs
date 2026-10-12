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
use super::layout::{Layout, CARD_TEXT, LIST_ROW, PAD, QUEUE_ROW, SCROLLBAR};
use super::preview::Previews;
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

/// What a card draws: its still, or its preview's frame while that plays (008 FR-019).
#[derive(Clone, Copy)]
struct Pictures<'a> {
    thumbs: &'a Thumbs,
    previews: &'a Previews,
}

fn thumb(
    pictures: Pictures<'_>,
    card: &SceneCard,
    width: f32,
    height: f32,
) -> Element<'static, ShellMsg> {
    let handle = pictures
        .previews
        .frame_for(&card.id)
        .cloned()
        .or_else(|| thumb_key(card).and_then(|k| pictures.thumbs.get(&k).cloned()))
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
    .on_enter(msg(ScenesMsg::Hover(index)))
    .on_exit(msg(ScenesMsg::HoverEnd(index)))
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
    pictures: Pictures<'_>,
) -> Element<'static, ShellMsg> {
    let w = layout.card_width();
    let body = column![
        thumb(pictures, c, w, layout.thumb_height()),
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
    pictures: Pictures<'_>,
) -> Element<'static, ShellMsg> {
    let list_text_width = layout.width - 2.0 * PAD - 128.0 - 12.0;
    let body = row![
        thumb(pictures, c, 128.0, 72.0),
        lines(c, list_text_width, 15.0, 13.0, 4.0),
    ]
    .spacing(12)
    .padding(2)
    .align_y(Alignment::Center);
    clickable(body.into(), index, focused, Length::Fill, LIST_ROW)
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
        thumb(pictures, c, STILL.0, STILL.1),
        lines(c, text_width, 14.0, 12.0, 3.0),
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
            .on_press(msg(ScenesMsg::QueueSelect(index))),
    )
    .on_middle_press(msg(ScenesMsg::Open {
        index,
        new_tab: true,
    }))
    .on_double_click(msg(ScenesMsg::QueuePlay(index)))
    .into()
}

/// A wall tile (008 FR-020): the still alone, no text.
fn wall_tile(
    c: &SceneCard,
    index: usize,
    focused: bool,
    layout: &Layout,
    pictures: Pictures<'_>,
) -> Element<'static, ShellMsg> {
    let (w, h) = (layout.wall_width(), layout.wall_height());
    clickable(
        thumb(pictures, c, w, h),
        index,
        focused,
        Length::Fixed(w),
        h,
    )
}

/// The rows in the window, with spacers for the rest so the scrolled height stays exact.
fn cards(
    state: &ScenesState,
    layout: &Layout,
    pictures: Pictures<'_>,
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
        let mut line = row![].spacing(layout.gap(state.mode));
        for (j, c) in all[r * cols..((r + 1) * cols).min(all.len())]
            .iter()
            .enumerate()
        {
            let i = r * cols + j;
            let focused = state.focused == Some(i);
            line = line.push(match state.mode {
                Mode::Grid => card(c, i, focused, layout, pictures),
                Mode::List => list_row(c, i, focused, layout, pictures),
                Mode::Wall => wall_tile(c, i, focused, layout, pictures),
                Mode::Queue => {
                    let playing = state.queue.playing.as_deref() == Some(c.id.as_str());
                    queue_row(c, i, focused, playing, layout, pictures)
                }
            });
        }
        rows = rows.push(
            r,
            container(line)
                .width(Length::Fill)
                .height(Length::Fixed(row_h)),
        );
    }
    rows = rows.push(
        usize::MAX,
        Space::new().height((total_rows - end) as f32 * row_h),
    );
    // Fill the width: a list row stretches across the page (its cards aren't a fixed width).
    rows.width(Length::Fill).into()
}

/// The cards, rebuilt only when what they show changes: scrolling (which updates the saved
/// position on every frame) then only redraws.
fn cached_cards<'a>(state: &'a ScenesState, ctx: &Context<'a>) -> Element<'a, ShellMsg> {
    let layout = ctx.layout;
    let pictures = Pictures {
        thumbs: ctx.thumbs,
        previews: ctx.previews,
    };
    let window = layout.window(state.scroll, state.mode, state.cards().len());
    // A playing preview changes the cards at each of its frames.
    let preview = ctx.previews.key().map(|(scene, frame)| {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        scene.hash(&mut h);
        (h.finish(), frame)
    });
    let playing = state.queue.playing.as_ref().map(|id| {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        id.hash(&mut h);
        h.finish()
    });
    let key = (
        state.loaded,
        state.mode,
        state.focused,
        playing,
        layout.width.to_bits(),
        layout.height.to_bits(),
        ctx.thumbs.revision(),
        window,
        preview,
    );
    lazy(key, move |_| cards(state, &layout, pictures, window)).into()
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
            let filtered = !state.query.criteria.is_empty() || !state.query.search.is_empty();
            let mut col = column![text(if filtered {
                "No scenes match these filters."
            } else {
                "No scenes."
            })
            .color(theme::MUTED)]
            .spacing(10)
            .align_x(Alignment::Center);
            if filtered {
                col =
                    col.push(button(text("Clear filters")).on_press(msg(ScenesMsg::ClearFilters)));
            }
            center(col).into()
        }
        Data::Ready { .. } => {
            let list: Element<'a, ShellMsg> = scroll_watch(
                scrollable(
                    column![
                        cached_cards(state, ctx),
                        Space::new().height(8),
                        controls::pager(state),
                    ]
                    .width(Length::Fill)
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
            .into();
            if state.mode == Mode::Queue {
                super::queue::view(
                    &state.queue,
                    state.cards(),
                    super::queue::strip(state.shuffle, state.repeat, state.autoplay),
                    list,
                    player,
                    &ctx.layout,
                    ctx.library,
                    ctx.thumbs,
                )
            } else {
                list
            }
        }
    };
    let mut page = column![controls::top(state)];
    if let Some(bar) = super::filters::bar(state, ctx.library) {
        page = page.push(bar);
    }
    // The filter panels float above the grid rather than pushing it down.
    let body = match super::filters::panel(state, ctx.library, ctx.thumbs) {
        Some(panel) => iced::widget::stack![body, panel].into(),
        None => body,
    };
    page.push(body).into()
}
