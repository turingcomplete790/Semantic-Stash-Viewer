//! An image's details (012 US4, FR-017): read-only, beside the viewer's picture. Every field the
//! web UI's image page shows; empty ones are left out. Performer, tag, and studio chips filter
//! Images (FR-018), with the scene page's chips and tag colours.

use iced::widget::{column, row, scrollable, text};
use iced::{Alignment, Element, Length};
use stash_core::images::{ImageDetails, ImageFile};
use stash_core::scenes::filter::kinds::CriterionKind;

use super::viewer::{Details, DetailsSlot};
use crate::library::Library;
use crate::screens::scene::{
    chip_for, heading, performer_chip_for, short, stars, tag_chip_for, wrap, ChipTarget,
};
use crate::screens::scenes::grid::duration;
use crate::screens::scenes::Thumbs;
use crate::shell::ShellMsg;
use crate::widgets::theme;

/// `JPEG · 4000×3000 · 3.2 MB` (and a clip's length).
pub fn file_line(f: &ImageFile) -> String {
    let mut parts = Vec::new();
    if !f.format.is_empty() {
        parts.push(f.format.to_uppercase());
    }
    if f.width > 0 && f.height > 0 {
        parts.push(format!("{}×{}", f.width, f.height));
    }
    if f.size > 0 {
        parts.push(format!("{:.1} MB", f.size as f64 / 1e6));
    }
    if let Some(d) = f.duration {
        parts.push(duration(d));
    }
    parts.join(" · ")
}

/// The line under the title: code, date, photographer.
pub fn byline(d: &ImageDetails) -> Option<String> {
    let parts: Vec<String> = [
        d.code.clone(),
        d.date.clone(),
        d.photographer.as_ref().map(|p| format!("by {p}")),
    ]
    .into_iter()
    .flatten()
    .filter(|s| !s.trim().is_empty())
    .collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// Rating, organized, O-count: what's set.
pub fn facts(d: &ImageDetails) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(r) = d.rating100.filter(|r| *r > 0) {
        parts.push(stars(r));
    }
    if d.organized {
        parts.push("Organized".to_owned());
    }
    if d.o_count > 0 {
        parts.push(format!("O-count {}", d.o_count));
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

fn ready<'a>(
    d: &'a ImageDetails,
    library: &'a Library,
    thumbs: &'a Thumbs,
) -> Element<'a, ShellMsg> {
    let mut col = column![text(d.title.clone()).size(18)].spacing(10);
    if let Some(line) = byline(d) {
        col = col.push(text(line).size(13).color(theme::MUTED));
    }
    if let Some(line) = facts(d) {
        col = col.push(text(line).size(13));
    }
    if let Some(studio) = &d.studio {
        col = col.push(
            row![
                text("Studio").size(14).color(theme::MUTED),
                chip_for(
                    ChipTarget::Images,
                    CriterionKind::Studios,
                    studio,
                    text(short(&studio.name)).size(13).into(),
                    theme::tag_chip(None),
                ),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
    }
    if !d.performers.is_empty() {
        col = col.push(heading("Performers"));
        col = col.push(wrap(
            d.performers
                .iter()
                .map(|p| performer_chip_for(ChipTarget::Images, p, thumbs))
                .collect(),
        ));
    }
    if !d.tags.is_empty() {
        col = col.push(heading("Tags"));
        col = col.push(wrap(
            d.tags
                .iter()
                .map(|t| tag_chip_for(ChipTarget::Images, t, library))
                .collect(),
        ));
    }
    if !d.galleries.is_empty() {
        col = col.push(heading("Galleries"));
        for g in &d.galleries {
            col = col.push(text(format!("{} ({} images)", g.title, g.image_count)).size(13));
        }
    }
    if !d.urls.is_empty() {
        col = col.push(heading("URLs"));
        for u in &d.urls {
            col = col.push(text(u.clone()).size(12).color(theme::MUTED));
        }
    }
    if let Some(details) = d.details.as_ref().filter(|t| !t.trim().is_empty()) {
        col = col.push(heading("Details"));
        col = col.push(text(details.clone()).size(13));
    }
    if let Some(f) = &d.file {
        col = col.push(heading("File"));
        col = col.push(text(file_line(f)).size(13));
        if !f.path.is_empty() {
            col = col.push(text(f.path.clone()).size(11).color(theme::MUTED));
        }
    }
    col.into()
}

/// The panel for `slot` (nothing yet: loading).
pub fn view<'a>(
    slot: Option<&'a DetailsSlot>,
    library: &'a Library,
    thumbs: &'a Thumbs,
) -> Element<'a, ShellMsg> {
    let body: Element<'a, ShellMsg> = match slot.map(|s| &s.state) {
        Some(Details::Ready(d)) => ready(d, library, thumbs),
        Some(Details::Failed(why)) => text(why.clone()).size(13).color(theme::MUTED).into(),
        Some(Details::Loading) | None => text("Loading…").size(13).color(theme::MUTED).into(),
    };
    scrollable(
        iced::widget::container(body)
            .padding(16)
            .width(Length::Fill),
    )
    .height(Length::Fill)
    .into()
}
