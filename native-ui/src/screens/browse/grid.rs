//! A list's grid (007 T038; generic since 012): as many columns as fit, cards drawn by the list's
//! [`Kind`], only the rows in view built, rebuilt only when what they show changes. Click opens,
//! Ctrl+click and middle-click open in a new tab; the keyboard's card has a ring.

use iced::widget::{
    button, center, column, container, image, keyed_column, lazy, mouse_area, row, scrollable,
    text, Space,
};
use iced::{Alignment, ContentFit, Element, Length};

use super::controls::{self, Map};
use super::filters;
use super::layout::{Layout, PAD};
use super::preview::Previews;
use super::state::{scroll_id_for, BrowseMsg, Data, Kind, List};
use super::thumbs::{key_of, placeholder, Thumbs};
use crate::screens::Context;
use crate::shell::{ShellMsg, TabId};
use crate::widgets::scroll_watch::scroll_watch;
use crate::widgets::theme;

/// What a card draws: its still, or its preview's frame while that plays (008 FR-019).
#[derive(Clone, Copy)]
pub struct Pictures<'a> {
    pub thumbs: &'a Thumbs,
    pub previews: &'a Previews,
}

/// A card's picture at `width` × `height`: its preview's frame, else its thumbnail, else the
/// placeholder.
pub fn thumb<K: Kind>(
    pictures: Pictures<'_>,
    card: &K::Card,
    width: f32,
    height: f32,
) -> Element<'static, ShellMsg> {
    let handle = pictures
        .previews
        .frame_for(K::card_id(card))
        .cloned()
        .or_else(|| {
            K::card_thumb(card)
                .and_then(key_of)
                .and_then(|k| pictures.thumbs.get(&k).cloned())
        })
        .unwrap_or_else(placeholder);
    image(handle)
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .content_fit(ContentFit::Cover)
        .border_radius(6)
        .into()
}

/// A card's body as a button: click opens it, middle-click in a new tab, the pointer resting on
/// it may start its preview.
pub fn clickable<K: Kind>(
    body: Element<'static, ShellMsg>,
    index: usize,
    focused: bool,
    width: Length,
    height: f32,
    msg: Map<K>,
) -> Element<'static, ShellMsg> {
    mouse_area(
        button(body)
            .padding(0)
            .width(width)
            .height(Length::Fixed(height))
            .style(theme::card(focused))
            .on_press(msg(BrowseMsg::Open {
                index,
                new_tab: false,
            })),
    )
    .on_middle_press(msg(BrowseMsg::Open {
        index,
        new_tab: true,
    }))
    .on_enter(msg(BrowseMsg::Hover(index)))
    .on_exit(msg(BrowseMsg::HoverEnd(index)))
    .into()
}

/// `text` cut to what a line `width` wide at `size` could ever show (the narrowest glyphs are
/// about a third of the size wide). The rest would be clipped anyway, and iced shapes the whole
/// string of a line that doesn't wrap: long file-name titles made rows slow to appear.
pub fn fit(text: &str, width: f32, size: f32) -> String {
    let max = ((width / (size * 0.3)).ceil() as usize).max(8);
    match text.char_indices().nth(max) {
        Some((cut, _)) => text[..cut].to_owned(),
        None => text.to_owned(),
    }
}

/// Title and details, one line each, clipped together (one clip per card keeps the widget tree
/// small: layout runs on every scroll frame).
pub fn lines(
    title_text: &str,
    details: &str,
    width: f32,
    title: f32,
    detail: f32,
    gap: f32,
) -> container::Container<'static, ShellMsg> {
    container(
        column![
            text(fit(title_text, width, title))
                .size(title)
                .wrapping(text::Wrapping::None),
            text(fit(details, width, detail))
                .size(detail)
                .color(theme::MUTED)
                .wrapping(text::Wrapping::None),
        ]
        .spacing(gap),
    )
    .clip(true)
    .width(Length::Fill)
}

/// The rows in the window, with spacers for the rest so the scrolled height stays exact.
fn cards<K: Kind>(
    list: &List<K>,
    layout: &Layout,
    pictures: Pictures<'_>,
    (first, end): (usize, usize),
    marked: Option<&str>,
    msg: Map<K>,
) -> Element<'static, ShellMsg> {
    let all = list.cards();
    let cols = layout.cols(list.mode);
    let row_h = layout.row_height(list.mode);
    let total_rows = all.len().div_ceil(cols);
    // Rows are keyed by their place on the page, so a row keeps its widget state (shaped
    // text) while the window moves; only rows entering the window are built fresh.
    let mut rows = keyed_column::<usize, ShellMsg, iced::Theme, iced::Renderer>([]);
    rows = rows.push(usize::MAX - 1, Space::new().height(first as f32 * row_h));
    for r in first..end {
        let mut line = row![].spacing(layout.gap(list.mode));
        for (j, c) in all[r * cols..((r + 1) * cols).min(all.len())]
            .iter()
            .enumerate()
        {
            let i = r * cols + j;
            let focused = list.focused == Some(i);
            let is_marked = marked == Some(K::card_id(c));
            line = line.push(K::card_view(
                c, i, focused, is_marked, list.mode, layout, pictures, msg,
            ));
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

fn hash_of(value: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}

/// The cards, rebuilt only when what they show changes: scrolling (which updates the saved
/// position on every frame) then only redraws.
fn cached_cards<'a, K: Kind>(
    list: &'a List<K>,
    ctx: &Context<'a>,
    marked: Option<&'a str>,
    msg: Map<K>,
) -> Element<'a, ShellMsg> {
    let layout = ctx.layout;
    let pictures = Pictures {
        thumbs: ctx.thumbs,
        previews: ctx.previews,
    };
    let window = layout.window(list.scroll, list.mode, list.cards().len());
    // A playing preview changes the cards at each of its frames.
    let preview = ctx
        .previews
        .key()
        .map(|(scene, frame)| (hash_of(scene), frame));
    let key = (
        list.loaded,
        list.mode,
        list.focused,
        marked.map(hash_of),
        layout.width.to_bits(),
        layout.height.to_bits(),
        (layout.min_card.to_bits(), layout.aspect.to_bits()),
        ctx.thumbs.revision(),
        window,
        preview,
    );
    lazy(key, move |_| {
        cards(list, &layout, pictures, window, marked, msg)
    })
    .into()
}

/// The list's screen: the bar, the filter chips, and the paged cards with the pager below them;
/// `wrap` places the scrolled list in the section's layout (Scenes' queue panel), and `marked`
/// is the card with a mark (the scene playing); `extra` goes in the top bar (Images' card size).
#[allow(clippy::too_many_arguments)]
pub fn view<'a, K: Kind>(
    list: &'a List<K>,
    ctx: &Context<'a>,
    tab: TabId,
    marked: Option<&'a str>,
    msg: Map<K>,
    extra: Option<Element<'a, ShellMsg>>,
    wrap: impl FnOnce(Element<'a, ShellMsg>) -> Element<'a, ShellMsg>,
) -> Element<'a, ShellMsg> {
    let body: Element<'a, ShellMsg> = match &list.data {
        Data::Loading => center(text(format!("Loading {}…", K::NOUN)).color(theme::MUTED)).into(),
        Data::Unreachable => center(
            container(
                column![
                    text("Can't reach the server, and this page isn't cached.").size(15),
                    button(text("Try again")).on_press(msg(BrowseMsg::Retry)),
                ]
                .spacing(10)
                .align_x(Alignment::Center),
            )
            .padding(16)
            .style(theme::problem),
        )
        .into(),
        Data::Ready { cards: c, .. } if c.is_empty() => {
            let filtered = !list.query.criteria.is_empty() || !list.query.search.is_empty();
            let noun = K::NOUN;
            let mut col = column![text(if filtered {
                format!("No {noun} match these filters.")
            } else {
                format!("No {noun}.")
            })
            .color(theme::MUTED)]
            .spacing(10)
            .align_x(Alignment::Center);
            if filtered {
                col =
                    col.push(button(text("Clear filters")).on_press(msg(BrowseMsg::ClearFilters)));
            }
            center(col).into()
        }
        Data::Ready { .. } => {
            let scrolled: Element<'a, ShellMsg> = scroll_watch(
                scrollable(
                    column![
                        cached_cards(list, ctx, marked, msg),
                        Space::new().height(8),
                        controls::pager(list, msg),
                    ]
                    .width(Length::Fill)
                    .padding(iced::Padding {
                        top: 4.0,
                        right: PAD,
                        bottom: PAD,
                        left: PAD,
                    }),
                )
                .id(scroll_id_for(K::ENTITY, tab))
                .height(Length::Fill),
                ctx.layout.row_height(list.mode),
                list.scroll,
                move |y| msg(BrowseMsg::Scrolled { y }),
            )
            .auto_scroll(list.auto_scroll, move |r| msg(BrowseMsg::AutoScrolled(r)))
            .into();
            wrap(scrolled)
        }
    };
    let mut page = column![controls::top(list, msg, extra)];
    if let Some(bar) = filters::bar(list, ctx.library, msg) {
        page = page.push(bar);
    }
    // The filter panels float above the grid rather than pushing it down.
    let body = match filters::panel(list, ctx.library, ctx.thumbs, msg) {
        Some(panel) => iced::widget::stack![body, panel].into(),
        None => body,
    };
    page.push(body).into()
}
