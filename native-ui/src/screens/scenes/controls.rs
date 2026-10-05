//! The Scenes controls (007 T039): sort (the core's labels, direction, a new random order), the
//! grid/list toggle, and the page controls (position and total, first/previous/next/last, go to
//! page, page size), above the grid and again below it.

use std::fmt;

use iced::widget::{button, container, pick_list, row, text, text_input, Space};
use iced::{Alignment, Element, Length};
use stash_core::scenes::paging::PAGE_SIZES;
use stash_core::scenes::query::{SceneSort, SortDirection};

use super::layout::PAD;
use super::state::{ScenesMsg, ScenesState};
use crate::screens::Mode;
use crate::shell::ShellMsg;
use crate::widgets::theme;

fn msg(m: ScenesMsg) -> ShellMsg {
    ShellMsg::Scenes(m)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Sort(SceneSort);

impl fmt::Display for Sort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.label())
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
pub fn pager(state: &ScenesState) -> Element<'_, ShellMsg> {
    let last = state.last_page();
    let at_start = state.page <= 1;
    let at_end = last.is_none_or(|l| state.page >= l);
    let position = match (last, state.count()) {
        (Some(l), Some(c)) => format!("Page {} of {l} · {c} scenes", state.page),
        _ => format!("Page {}", state.page),
    };
    let nav = |label: &'static str, target: u32, enabled: bool| {
        button(text(label).size(13))
            .padding([4, 10])
            .style(button::secondary)
            .on_press_maybe(enabled.then_some(msg(ScenesMsg::Page(target))))
    };
    row![
        nav("First", 1, !at_start),
        nav("Previous", state.page.saturating_sub(1), !at_start),
        text(position).size(13),
        nav("Next", state.page + 1, !at_end),
        nav("Last", last.unwrap_or(state.page), !at_end),
        text_input("Go to page", &state.go_to)
            .id(GO_TO_ID)
            .on_input(|t| msg(ScenesMsg::GoToInput(t)))
            .on_submit(msg(ScenesMsg::GoToSubmit))
            .size(13)
            .padding([4, 8])
            .width(Length::Fixed(100.0)),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

/// The bar above the grid.
pub fn top(state: &ScenesState) -> Element<'_, ShellMsg> {
    let sorts: Vec<Sort> = SceneSort::all().iter().copied().map(Sort).collect();
    let sizes: Vec<PerPage> = PAGE_SIZES.iter().copied().map(PerPage).collect();
    let direction = match state.query.direction {
        SortDirection::Asc => "↑ Ascending",
        SortDirection::Desc => "↓ Descending",
    };
    let mut bar = row![
        pick_list(sorts, Some(Sort(state.query.sort)), |s: Sort| msg(
            ScenesMsg::Sort(s.0)
        ))
        .text_size(13)
        .padding([4, 8]),
        button(text(direction).size(13))
            .padding([4, 10])
            .style(button::secondary)
            .on_press(msg(ScenesMsg::ToggleDirection)),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    if state.query.sort == SceneSort::Random {
        bar = bar.push(
            button(text("Shuffle").size(13))
                .padding([4, 10])
                .style(button::secondary)
                .on_press(msg(ScenesMsg::Reshuffle)),
        );
    }
    let mode = |m: Mode, label: &'static str| {
        button(text(label).size(13))
            .padding([4, 10])
            .style(theme::menu_row(state.mode == m))
            .on_press(msg(ScenesMsg::Mode(m)))
    };
    bar = bar
        .push(Space::new().width(Length::Fill))
        .push(pager(state))
        .push(
            pick_list(sizes, Some(PerPage(state.page_size)), |p: PerPage| {
                msg(ScenesMsg::PageSize(p.0))
            })
            .text_size(13)
            .padding([4, 8]),
        )
        .push(mode(Mode::Grid, "Grid"))
        .push(mode(Mode::List, "List"));
    container(bar)
        .padding([8.0, PAD])
        .width(Length::Fill)
        .into()
}
