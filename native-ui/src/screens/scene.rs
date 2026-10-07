//! The Scene screen (007 T048; capability C15), designed for the native app: the cover (or the
//! player, when this tab owns playback), and under it the title, the facts, Play, the people and
//! tags, the description, and the file. Details and cover come through the cache first.

use iced::widget::image::Handle;
use iced::widget::{
    button, center, column, container, image, mouse_area, row, scrollable, text, Space,
};
use iced::{Alignment, ContentFit, Element, Length};
use serde::{Deserialize, Serialize};
use stash_core::scenes::filter::kinds::CriterionKind;
use stash_core::scenes::filter::{Criterion, CriterionValue, Item, Modifier};
use stash_core::scenes::{Named, SceneDetails};
use stash_core::AppError;

use super::scenes::grid::duration;
use super::scenes::layout::Layout;
use crate::effects::Effect;
use crate::messages::for_error;
use crate::shell::{ShellMsg, TabId};
use crate::widgets::{icon, theme, Icon};

/// The details' state (transient).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Details {
    #[default]
    Loading,
    Ready(Box<SceneDetails>),
    Failed(String),
}

/// The cover's state (transient).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Cover {
    #[default]
    Loading,
    Ready(Handle),
    /// Stash has none, or it couldn't be read.
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneState {
    pub scene_id: String,
    /// Shown as the tab's title before the details load (and updated from them).
    pub title: String,
    #[serde(skip)]
    pub details: Details,
    #[serde(skip)]
    pub cover: Cover,
    /// Details and cover requests in flight.
    #[serde(skip)]
    pending: bool,
}

impl SceneState {
    pub fn new(id: &str, title: &str) -> Self {
        Self {
            scene_id: id.to_owned(),
            title: title.to_owned(),
            details: Details::Loading,
            cover: Cover::Loading,
            pending: false,
        }
    }

    /// Only the saved fields.
    pub fn persistent(&self) -> Self {
        Self::new(&self.scene_id, &self.title)
    }

    pub fn needs_load(&self) -> bool {
        !matches!(self.details, Details::Ready(_)) && !self.pending
    }

    /// Entry: load the details and cover unless they're here or on their way.
    pub fn enter(&mut self, tab: TabId) -> Vec<Effect> {
        if !self.needs_load() {
            return Vec::new();
        }
        self.pending = true;
        self.details = Details::Loading;
        let id = self.scene_id.clone();
        let mut effects = vec![Effect::LoadSceneDetails {
            tab,
            id: id.clone(),
        }];
        if !matches!(self.cover, Cover::Ready(_)) {
            effects.push(Effect::LoadCover { tab, id });
        }
        effects
    }

    pub fn details_loaded(&mut self, result: Result<SceneDetails, AppError>) {
        self.pending = false;
        self.details = match result {
            Ok(d) => {
                self.title = d.title.clone();
                Details::Ready(Box::new(d))
            }
            Err(e) => {
                let m = for_error(&e);
                Details::Failed(format!("{} {}", m.title, m.detail))
            }
        };
    }

    pub fn cover_loaded(&mut self, cover: Option<Handle>) {
        self.cover = cover.map_or(Cover::None, Cover::Ready);
    }

    /// The cover with a Play button over it.
    pub fn cover_view(&self) -> Element<'_, ShellMsg> {
        let picture: Element<'_, ShellMsg> = match &self.cover {
            Cover::Ready(handle) => image(handle.clone())
                .width(Length::Fill)
                .height(Length::Fill)
                .content_fit(ContentFit::Contain)
                .into(),
            Cover::Loading => Space::new().width(Length::Fill).height(Length::Fill).into(),
            Cover::None => center(text("No cover").color(theme::MUTED)).into(),
        };
        let play = button(
            row![icon(Icon::Play, 22.0), text("Play").size(18)]
                .spacing(10)
                .align_y(Alignment::Center),
        )
        .padding([12, 28])
        .style(theme::chip)
        .on_press(ShellMsg::Play(self.scene_id.clone()));
        iced::widget::stack![
            container(picture)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_| container::Style::default().background(iced::Color::BLACK)),
            center(play),
        ]
        .into()
    }

    /// Title, facts, people, tags, description, and file.
    pub fn details_view(&self) -> Element<'_, ShellMsg> {
        let body: Element<'_, ShellMsg> = match &self.details {
            Details::Loading => column![
                text(self.title.clone()).size(26),
                text("Loading the scene…").color(theme::MUTED),
            ]
            .spacing(8)
            .into(),
            Details::Failed(message) => column![
                text(self.title.clone()).size(26),
                container(text(message.clone()).size(14))
                    .padding(12)
                    .style(theme::problem),
            ]
            .spacing(10)
            .into(),
            Details::Ready(d) => facts(d),
        };
        scrollable(container(body).padding([16, 20]).width(Length::Fill))
            .height(Length::Fill)
            .into()
    }

    /// The whole screen when the cover shows.
    pub fn view(&self, window: &Layout) -> Element<'_, ShellMsg> {
        layout(self.cover_view(), self.details_view(), window)
    }
}

/// The details panel's width: widescreen displays have room to the side, so the picture keeps
/// the full height. About 1/7 of the window on 21:9 and 1/5.7 on 16:9, never under 260 px.
/// (Dragging panel sizes is on the roadmap.)
pub fn side_width(window: &Layout) -> f32 {
    const WIDE: f32 = 21.0 / 9.0;
    const HD: f32 = 16.0 / 9.0;
    let aspect = window.width / window.height.max(1.0);
    let share = (5.7 + (aspect - HD) * (7.0 - 5.7) / (WIDE - HD)).clamp(4.5, 8.0);
    (window.width / share).max(260.0)
}

/// The screen's layout: the details in a side panel on the left, the picture (cover or player)
/// filling the rest.
pub fn layout<'a, M: 'a>(
    media: Element<'a, M>,
    details: Element<'a, M>,
    window: &Layout,
) -> Element<'a, M> {
    row![
        container(details)
            .width(Length::Fixed(side_width(window)))
            .height(Length::Fill)
            .style(theme::bar),
        container(media).width(Length::Fill).height(Length::Fill),
    ]
    .into()
}

fn stars(rating100: u8) -> String {
    let filled = usize::from(rating100.div_ceil(20)).min(5);
    format!("{}{}", "★".repeat(filled), "☆".repeat(5 - filled))
}

/// The criterion "scenes with exactly this performer / tag / studio" (008 FR-006).
fn only(kind: CriterionKind, item: &Named) -> Criterion {
    let modifier = match kind {
        CriterionKind::Studios => Modifier::Includes,
        _ => Modifier::IncludesAll,
    };
    Criterion::new(
        kind,
        modifier,
        CriterionValue::Items {
            items: vec![Item {
                id: item.id.clone(),
                label: item.name.clone(),
            }],
            excluded: vec![],
            depth: matches!(kind, CriterionKind::Tags | CriterionKind::Studios).then_some(0),
        },
    )
}

/// A chip that opens Scenes filtered to it (Ctrl+click or middle-click: a new tab).
fn chip<'a>(kind: CriterionKind, item: &'a Named) -> Element<'a, ShellMsg> {
    let criterion = only(kind, item);
    mouse_area(
        button(text(item.name.as_str()).size(13))
            .padding([3, 10])
            .style(|_, status| button::Style {
                background: Some(iced::Background::Color(theme::YALE_BLUE)),
                text_color: theme::TEXT,
                border: iced::Border {
                    radius: 12.0.into(),
                    width: if matches!(status, button::Status::Hovered) {
                        1.0
                    } else {
                        0.0
                    },
                    color: theme::FROSTED_BLUE,
                },
                ..button::Style::default()
            })
            .on_press(ShellMsg::FilterTo {
                criterion: criterion.clone(),
                new_tab: false,
            }),
    )
    .on_middle_press(ShellMsg::FilterTo {
        criterion,
        new_tab: true,
    })
    .into()
}

fn file_line(d: &SceneDetails) -> Option<String> {
    let f = d.file.as_ref()?;
    let mut parts: Vec<String> = Vec::new();
    if let Some(name) = &d.file_name {
        parts.push(name.clone());
    }
    if let Some(c) = &f.video_codec {
        parts.push(c.to_uppercase());
    }
    if let (Some(w), Some(h)) = (f.width, f.height) {
        parts.push(format!("{w}×{h}"));
    }
    if let Some(r) = f.frame_rate {
        parts.push(format!("{r:.2} fps"));
    }
    if let Some(b) = f.bit_rate {
        parts.push(format!("{:.1} Mb/s", b as f64 / 1e6));
    }
    if let Some(s) = f.size {
        parts.push(format!("{:.2} GB", s as f64 / 1e9));
    }
    if let Some(c) = &f.container {
        parts.push(c.to_uppercase());
    }
    Some(parts.join(" · "))
}

fn facts(d: &SceneDetails) -> Element<'_, ShellMsg> {
    let mut line: Vec<String> = Vec::new();
    if let Some(date) = &d.date {
        line.push(date.clone());
    }
    if let Some(s) = d.duration_seconds {
        line.push(duration(s));
    }

    if let Some(code) = &d.code {
        line.push(code.clone());
    }
    if d.play_count > 0 {
        line.push(format!(
            "played {} time{}",
            d.play_count,
            if d.play_count == 1 { "" } else { "s" }
        ));
    }
    let mut col = column![text(d.title.clone()).size(26)].spacing(10);
    let mut meta = row![text(line.join(" · ")).size(14).color(theme::MUTED)]
        .spacing(16)
        .align_y(Alignment::Center);
    if let Some(r) = d.rating100 {
        meta = meta.push(text(stars(r)).size(14).color(theme::FROSTED_BLUE));
    }
    col = col.push(meta);
    if let Some(studio) = &d.studio {
        col = col.push(
            row![
                text("Studio").size(14).color(theme::MUTED),
                chip(CriterionKind::Studios, studio)
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
    }
    if !d.performers.is_empty() {
        col = col.push(
            row(std::iter::once(text("With").size(15).into()).chain(
                d.performers
                    .iter()
                    .map(|p| chip(CriterionKind::Performers, p)),
            ))
            .spacing(6)
            .wrap()
            .vertical_spacing(6),
        );
    }
    if let Some(director) = &d.director {
        col = col.push(text(format!("Directed by {director}")).size(14));
    }
    if !d.tags.is_empty() {
        col = col.push(
            row(d.tags.iter().map(|t| chip(CriterionKind::Tags, t)))
                .spacing(6)
                .wrap()
                .vertical_spacing(6),
        );
    }
    if let Some(details) = &d.details {
        col = col.push(text(details.clone()).size(14));
    }
    if let Some(file) = file_line(d) {
        col = col.push(text(file).size(12).color(theme::MUTED));
    }
    col.into()
}
