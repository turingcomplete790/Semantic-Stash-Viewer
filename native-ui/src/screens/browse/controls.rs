//! A list's controls (007 T039; generic since 012): sort (the core's labels, direction, a new
//! random order), the display modes, and the page controls (position and total,
//! first/previous/next/last, go to page, page size), above the grid and again below it.

use std::fmt;

use iced::widget::{button, container, pick_list, row, text, text_input, Space};
use iced::{Alignment, Element, Length};
use stash_core::browse::paging::PAGE_SIZES;
use stash_core::browse::query::{Sort, SortDirection};

use super::layout::PAD;
use super::state::{BrowseMsg, Kind, List, Panel, SEARCH_ID};
use crate::shell::ShellMsg;
use crate::widgets::theme;

/// How a list's messages reach the shell (`ShellMsg::Scenes`, `ShellMsg::Images`).
pub type Map<K> = fn(BrowseMsg<<K as Kind>::Sort>) -> ShellMsg;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SortChoice<K: Kind>(K::Sort);

impl<K: Kind> fmt::Display for SortChoice<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(K::sort_label(self.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PerPage(u32);

impl fmt::Display for PerPage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} per page", self.0)
    }
}

/// The go-to-page field's widget id.
pub const GO_TO_ID: &str = "scenes-go-to";

/// Position, first/previous/next/last, and go to page.
pub fn pager<K: Kind>(list: &List<K>, msg: Map<K>) -> Element<'_, ShellMsg> {
    let last = list.last_page();
    let at_start = list.page <= 1;
    let at_end = last.is_none_or(|l| list.page >= l);
    let position = match (last, list.count()) {
        (Some(l), Some(c)) => format!("Page {} of {l} · {c} {}", list.page, K::NOUN),
        _ => format!("Page {}", list.page),
    };
    let nav = |label: &'static str, target: u32, enabled: bool| {
        button(text(label).size(13))
            .padding([4, 10])
            .style(button::secondary)
            .on_press_maybe(enabled.then_some(msg(BrowseMsg::Page(target))))
    };
    row![
        nav("First", 1, !at_start),
        nav("Previous", list.page.saturating_sub(1), !at_start),
        text(position).size(13),
        nav("Next", list.page + 1, !at_end),
        nav("Last", last.unwrap_or(list.page), !at_end),
        text_input("Go to page", &list.go_to)
            .id(GO_TO_ID)
            .on_input(move |t| msg(BrowseMsg::GoToInput(t)))
            .on_submit(msg(BrowseMsg::GoToSubmit))
            .size(13)
            .padding([4, 8])
            .width(Length::Fixed(100.0)),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

/// The bar above the grid; `extra` sits before the display modes (Images' card size).
pub fn top<'a, K: Kind>(
    list: &'a List<K>,
    msg: Map<K>,
    extra: Option<Element<'a, ShellMsg>>,
) -> Element<'a, ShellMsg> {
    let sorts: Vec<SortChoice<K>> = K::sorts().iter().copied().map(SortChoice).collect();
    let sizes: Vec<PerPage> = PAGE_SIZES.iter().copied().map(PerPage).collect();
    let direction = match list.query.direction {
        SortDirection::Asc => "↑ Ascending",
        SortDirection::Desc => "↓ Descending",
    };
    let filters = list.query.criteria.len();
    let filtering = matches!(list.panel, Panel::Menu { .. } | Panel::Editor(_));
    let saved_open = matches!(list.panel, Panel::Saved { .. });
    let mut bar = row![
        text_input(K::SEARCH_PLACEHOLDER, list.search_text())
            .id(SEARCH_ID)
            .on_input(move |t| msg(BrowseMsg::SearchInput(t)))
            .on_submit(msg(BrowseMsg::SearchSubmit))
            .size(13)
            .padding([4, 8])
            .width(Length::Fixed(220.0)),
        button(
            text(if filters == 0 {
                "Filter".to_owned()
            } else {
                format!("Filter ({filters})")
            })
            .size(13)
        )
        .padding([4, 10])
        .style(theme::menu_row(filtering))
        .on_press(msg(BrowseMsg::FilterMenu(!filtering))),
        button(text("Saved filters").size(13))
            .padding([4, 10])
            .style(theme::menu_row(saved_open))
            .on_press(msg(BrowseMsg::SavedMenu(!saved_open))),
        pick_list(
            sorts,
            Some(SortChoice::<K>(list.query.sort)),
            move |s: SortChoice<K>| { msg(BrowseMsg::Sort(s.0)) }
        )
        .text_size(13)
        .padding([4, 8]),
        button(text(direction).size(13))
            .padding([4, 10])
            .style(button::secondary)
            .on_press(msg(BrowseMsg::ToggleDirection)),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    if list.query.sort == K::Sort::RANDOM {
        bar = bar.push(
            button(text("Shuffle").size(13))
                .padding([4, 10])
                .style(button::secondary)
                .on_press(msg(BrowseMsg::Reshuffle)),
        );
    }
    bar = bar
        .push(Space::new().width(Length::Fill))
        .push(pager(list, msg))
        .push(
            pick_list(sizes, Some(PerPage(list.page_size)), move |p: PerPage| {
                msg(BrowseMsg::PageSize(p.0))
            })
            .text_size(13)
            .padding([4, 8]),
        );
    if let Some(extra) = extra {
        bar = bar.push(extra);
    }
    for (mode, label) in K::modes() {
        bar = bar.push(
            button(text(*label).size(13))
                .padding([4, 10])
                .style(theme::menu_row(list.mode == *mode))
                .on_press(msg(BrowseMsg::Mode(*mode))),
        );
    }
    container(bar)
        .padding([8.0, PAD])
        .width(Length::Fill)
        .into()
}
