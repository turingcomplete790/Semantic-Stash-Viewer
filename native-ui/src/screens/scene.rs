//! The Scene screen (007 T048, 008 US2; capability C15), designed for the native app: the cover
//! (or the player, when this tab owns playback) beside the details panel: title, facts, Play,
//! studio, performers, tags, groups, markers, description, the primary file, and the "All files"
//! and "Galleries" sections opened on demand. Details and cover come through the cache first; the
//! first paint uses the card's title, so nothing moves when the details arrive (FR-010).

use std::time::Duration;

use iced::widget::image::Handle;
use iced::widget::{
    button, center, column, container, image, mouse_area, opaque, row, rule, scrollable, stack,
    text, Space,
};
use iced::{Alignment, ContentFit, Element, Length};
use serde::{Deserialize, Serialize};
use stash_core::adapter::chips::{ChipDetails, ChipKind};
use stash_core::scenes::filter::kinds::CriterionKind;
use stash_core::scenes::filter::{Criterion, CriterionValue, Item, Modifier};
use stash_core::scenes::{FileFacts, GalleryChip, Named, SceneDetails};
use stash_core::AppError;

use super::scenes::grid::duration;
use super::scenes::layout::Layout;
use super::scenes::Thumbs;
use crate::effects::Effect;
use crate::library::{Library, Loadable};
use crate::machine::Step;
use crate::messages::for_error;
use crate::shell::{ShellMsg, TabId};
use crate::widgets::{icon, theme, Icon};

/// How long the pointer or focus rests on a chip before its card loads (FR-011b).
pub const CHIP_REST: Duration = Duration::from_millis(400);
/// A performer chip's round picture (008 FR-011a, clarified 2026-10-07), in logical pixels.
pub const PERFORMER_ICON: f32 = 40.0;
/// A performer card's portrait (008 FR-011b): this wide, and half again as tall.
pub const PORTRAIT: f32 = 240.0;

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

/// A section opened on demand ("All files", "Galleries"): loaded once, then shown or hidden.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Section<T> {
    #[default]
    Closed,
    Loading,
    Ready {
        data: T,
        open: bool,
    },
    Failed(String),
}

impl<T> Section<T> {
    fn is_open(&self) -> bool {
        matches!(
            self,
            Section::Loading | Section::Ready { open: true, .. } | Section::Failed(_)
        )
    }
}

/// A chip on the page that has a details card.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChipRef {
    pub kind: ChipKind,
    pub id: String,
}

#[derive(Debug, Clone)]
pub enum SceneMsg {
    ToggleFiles,
    ToggleGalleries,
    /// The pointer or focus reached a chip.
    ChipEnter(ChipRef),
    /// It left.
    ChipLeave(ChipRef),
    /// Close the chip card (Escape).
    CloseCard,
    /// Play from a marker.
    Marker(f64),
}

/// Events for the tab.
#[derive(Debug, Clone, PartialEq)]
pub enum SceneUp {
    /// Play this scene from `seconds`.
    PlayAt(f64),
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
    #[serde(skip)]
    pub files: Section<Vec<FileFacts>>,
    #[serde(skip)]
    pub galleries: Section<Vec<GalleryChip>>,
    /// The chip under the pointer: which, its rest's generation, and whether it has rested.
    #[serde(skip)]
    pub chip_hover: Option<(ChipRef, u64, bool)>,
}

static NEXT_REST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl SceneState {
    pub fn new(id: &str, title: &str) -> Self {
        Self {
            scene_id: id.to_owned(),
            title: title.to_owned(),
            details: Details::Loading,
            cover: Cover::Loading,
            pending: false,
            files: Section::Closed,
            galleries: Section::Closed,
            chip_hover: None,
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
            Err(AppError::SceneNotFound { .. }) => {
                Details::Failed("This scene no longer exists on the server.".into())
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

    pub fn files_loaded(&mut self, result: Result<Vec<FileFacts>, AppError>) {
        self.files = section(result);
    }

    pub fn galleries_loaded(&mut self, result: Result<Vec<GalleryChip>, AppError>) {
        self.galleries = section(result);
    }

    /// The chip whose rest `generation` came due, if the pointer is still on it.
    pub fn chip_rested(&mut self, generation: u64) -> Option<ChipRef> {
        match &mut self.chip_hover {
            Some((chip, g, rested)) if *g == generation => {
                *rested = true;
                Some(chip.clone())
            }
            _ => None,
        }
    }

    /// Whether this screen waits for chip rest `generation`.
    pub fn awaits_rest(&self, generation: u64) -> bool {
        matches!(&self.chip_hover, Some((_, g, false)) if *g == generation)
    }

    pub fn update(&mut self, msg: SceneMsg, tab: TabId) -> Step<SceneUp> {
        match msg {
            SceneMsg::ToggleFiles => {
                let id = self.scene_id.clone();
                Step::effects(toggle(&mut self.files, || Effect::LoadSceneFiles {
                    tab,
                    id,
                }))
            }
            SceneMsg::ToggleGalleries => {
                let id = self.scene_id.clone();
                Step::effects(toggle(&mut self.galleries, || Effect::LoadSceneGalleries {
                    tab,
                    id,
                }))
            }
            SceneMsg::ChipEnter(chip) => {
                if matches!(&self.chip_hover, Some((c, _, _)) if *c == chip) {
                    return Step::none();
                }
                let generation = NEXT_REST.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                self.chip_hover = Some((chip, generation, false));
                Step::effect(Effect::ChipRestDue {
                    tab,
                    generation,
                    after: CHIP_REST,
                })
            }
            SceneMsg::ChipLeave(chip) => {
                if matches!(&self.chip_hover, Some((c, _, _)) if *c == chip) {
                    self.chip_hover = None;
                }
                Step::none()
            }
            SceneMsg::CloseCard => {
                self.chip_hover = None;
                Step::none()
            }
            SceneMsg::Marker(seconds) => Step::up(SceneUp::PlayAt(seconds)),
        }
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
        stack![
            container(picture)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_| container::Style::default().background(iced::Color::BLACK)),
            center(play),
        ]
        .into()
    }

    /// Title, facts, people, tags, groups, markers, description, files, and galleries; the
    /// resting chip's card floats at the panel's foot.
    pub fn details_view<'a>(
        &'a self,
        library: &'a Library,
        thumbs: &'a Thumbs,
    ) -> Element<'a, ShellMsg> {
        let body: Element<'a, ShellMsg> = match &self.details {
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
            Details::Ready(d) => facts(self, d, library, thumbs),
        };
        let page =
            scrollable(container(body).padding([16, 20]).width(Length::Fill)).height(Length::Fill);
        // Always a stack, card or not, so the page's widget state is never handed to another
        // widget when a card appears.
        let card: Element<'a, ShellMsg> = match &self.chip_hover {
            Some((chip, _, true)) => {
                let portrait = self
                    .portrait_wanted()
                    .map(|id| thumbs.get(&super::scenes::picker::portrait_key(id)));
                chip_card(chip, portrait, library)
            }
            _ => Space::new().into(),
        };
        stack![page, card].into()
    }

    /// The performer whose portrait the open chip card shows: a rested performer chip, for a
    /// performer Stash has an image of.
    pub fn portrait_wanted(&self) -> Option<&str> {
        let Some((chip, _, true)) = &self.chip_hover else {
            return None;
        };
        let Details::Ready(d) = &self.details else {
            return None;
        };
        (chip.kind == ChipKind::Performer)
            .then(|| d.performers.iter().find(|p| p.id == chip.id && p.has_image))
            .flatten()
            .map(|p| p.id.as_str())
    }

    /// The whole screen when the cover shows.
    pub fn view<'a>(
        &'a self,
        window: &Layout,
        library: &'a Library,
        thumbs: &'a Thumbs,
    ) -> Element<'a, ShellMsg> {
        layout(
            self.cover_view(),
            self.details_view(library, thumbs),
            window,
        )
    }
}

fn section<T>(result: Result<T, AppError>) -> Section<T> {
    match result {
        Ok(data) => Section::Ready { data, open: true },
        Err(e) => {
            let m = for_error(&e);
            Section::Failed(format!("{} {}", m.title, m.detail))
        }
    }
}

/// Open or close a section; the first open loads it (once).
fn toggle<T>(s: &mut Section<T>, load: impl FnOnce() -> Effect) -> Vec<Effect> {
    match s {
        Section::Closed | Section::Failed(_) => {
            *s = Section::Loading;
            vec![load()]
        }
        Section::Loading => Vec::new(),
        Section::Ready { open, .. } => {
            *open = !*open;
            Vec::new()
        }
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

/// A chip on the page: opens Scenes filtered to it (Ctrl+click or middle-click: a new tab), and
/// its card loads after the pointer rests on it.
fn chip<'a>(
    kind: CriterionKind,
    item: &Named,
    label: Element<'a, ShellMsg>,
    style: impl Fn(&iced::Theme, button::Status) -> button::Style + 'a,
) -> Element<'a, ShellMsg> {
    let criterion = only(kind, item);
    let chip_kind = match kind {
        CriterionKind::Performers => ChipKind::Performer,
        CriterionKind::Studios => ChipKind::Studio,
        _ => ChipKind::Tag,
    };
    let r = ChipRef {
        kind: chip_kind,
        id: item.id.clone(),
    };
    mouse_area(
        button(label)
            .padding([3, 10])
            .style(style)
            .on_press(ShellMsg::FilterTo {
                criterion: criterion.clone(),
                new_tab: false,
            }),
    )
    .on_middle_press(ShellMsg::FilterTo {
        criterion,
        new_tab: true,
    })
    .on_enter(ShellMsg::Scene(SceneMsg::ChipEnter(r.clone())))
    .on_exit(ShellMsg::Scene(SceneMsg::ChipLeave(r)))
    .into()
}

/// Long names are cut in chips (full on the card and in the page's text).
fn short(name: &str) -> String {
    const MAX: usize = 32;
    match name.char_indices().nth(MAX) {
        Some((cut, _)) => format!("{}…", &name[..cut]),
        None => name.to_owned(),
    }
}

fn tag_chip<'a>(t: &Named, library: &'a Library) -> Element<'a, ShellMsg> {
    let kind = library.vocabulary.ready().and_then(|v| v.kind_of(&t.id));
    let colour = library.tag_colour(&t.id);
    let mut label = row![text(short(&t.name)).size(13)]
        .spacing(5)
        .align_y(Alignment::Center);
    if let Some(k) = kind {
        label = label.push(text(k.label()).size(10));
    }
    chip(
        CriterionKind::Tags,
        t,
        label.into(),
        theme::tag_chip(colour),
    )
}

fn performer_chip<'a>(
    p: &stash_core::scenes::PerformerChip,
    thumbs: &'a Thumbs,
) -> Element<'a, ShellMsg> {
    let named = Named {
        id: p.id.clone(),
        name: p.name.clone(),
    };
    let mut label = row![].spacing(6).align_y(Alignment::Center);
    if p.has_image {
        let key = super::scenes::picker::performer_key(&p.id);
        label = label.push(match thumbs.get(&key) {
            Some(h) => Element::from(
                image(h.clone())
                    .width(PERFORMER_ICON)
                    .height(PERFORMER_ICON)
                    .content_fit(ContentFit::Cover)
                    .border_radius(PERFORMER_ICON / 2.0),
            ),
            None => Space::new()
                .width(PERFORMER_ICON)
                .height(PERFORMER_ICON)
                .into(),
        });
    }
    label = label.push(text(short(&p.name)).size(13));
    chip(
        CriterionKind::Performers,
        &named,
        label.into(),
        theme::tag_chip(None),
    )
}

fn heading(label: &str) -> Element<'_, ShellMsg> {
    text(label).size(13).color(theme::FROSTED_BLUE).into()
}

fn wrap<'a>(items: Vec<Element<'a, ShellMsg>>) -> Element<'a, ShellMsg> {
    row(items).spacing(6).wrap().vertical_spacing(6).into()
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

fn file_facts(f: &FileFacts, primary: bool) -> Element<'_, ShellMsg> {
    let mut head = row![text(f.basename.clone()).size(13)].spacing(8);
    if primary {
        head = head.push(text("plays").size(11).color(theme::ICY_AQUA));
    }
    let facts = [
        format!("{}×{}", f.width, f.height),
        f.video_codec.to_uppercase(),
        f.audio_codec.to_uppercase(),
        format!("{:.2} fps", f.frame_rate),
        format!("{:.1} Mb/s", f.bit_rate as f64 / 1e6),
        format!("{:.2} GB", f.size as f64 / 1e9),
        duration(f.duration),
        f.format.to_uppercase(),
    ]
    .join(" · ");
    let mut col = column![head, text(facts).size(12).color(theme::MUTED)].spacing(2);
    col = col.push(text(f.path.clone()).size(11).color(theme::MUTED));
    for (kind, value) in &f.fingerprints {
        col = col.push(
            text(format!("{kind}: {value}"))
                .size(11)
                .color(theme::MUTED),
        );
    }
    col.into()
}

fn section_button<'a>(label: String, open: bool, msg: SceneMsg) -> Element<'a, ShellMsg> {
    button(text(format!("{} {label}", if open { "▾" } else { "▸" })).size(13))
        .padding([2, 4])
        .style(button::text)
        .on_press(ShellMsg::Scene(msg))
        .into()
}

fn facts<'a>(
    state: &'a SceneState,
    d: &'a SceneDetails,
    library: &'a Library,
    thumbs: &'a Thumbs,
) -> Element<'a, ShellMsg> {
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
    let mut col = column![text(d.title.clone()).size(26)].spacing(10);
    let mut meta = row![text(line.join(" · ")).size(14).color(theme::MUTED)]
        .spacing(16)
        .align_y(Alignment::Center);
    if let Some(r) = d.rating100 {
        meta = meta.push(text(stars(r)).size(14).color(theme::FROSTED_BLUE));
    }
    col = col.push(meta);

    // Activity: organized, plays, O-counter, time watched, resume point.
    let mut activity: Vec<String> = Vec::new();
    if d.organized {
        activity.push("Organized".into());
    }
    if d.play_count > 0 {
        activity.push(format!(
            "Played {} time{}",
            d.play_count,
            if d.play_count == 1 { "" } else { "s" }
        ));
    }
    if d.o_count > 0 {
        activity.push(format!("O-counter {}", d.o_count));
    }
    if let Some(w) = d.play_duration {
        activity.push(format!("watched {}", duration(w)));
    }
    if let Some(r) = d.resume_time {
        activity.push(format!("resumes at {}", duration(r)));
    }
    if !activity.is_empty() {
        col = col.push(text(activity.join(" · ")).size(13).color(theme::MUTED));
    }

    if let Some(studio) = &d.studio {
        col = col.push(
            row![
                text("Studio").size(14).color(theme::MUTED),
                chip(
                    CriterionKind::Studios,
                    studio,
                    text(short(&studio.name)).size(13).into(),
                    theme::tag_chip(None)
                )
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
                .map(|p| performer_chip(p, thumbs))
                .collect(),
        ));
    }
    if let Some(director) = &d.director {
        col = col.push(text(format!("Directed by {director}")).size(14));
    }
    if !d.tags.is_empty() {
        col = col.push(heading("Tags"));
        col = col.push(wrap(d.tags.iter().map(|t| tag_chip(t, library)).collect()));
    }
    if !d.groups.is_empty() {
        col = col.push(heading("Groups"));
        col = col.push(wrap(
            d.groups
                .iter()
                .map(|g| {
                    let label = match g.scene_index {
                        Some(i) => format!("{} · #{i}", short(&g.name)),
                        None => short(&g.name),
                    };
                    container(text(label).size(13))
                        .padding([3, 10])
                        .style(theme::panel)
                        .into()
                })
                .collect(),
        ));
    }
    if !d.markers.is_empty() {
        col = col.push(heading("Markers"));
        let mut list = column![].spacing(2);
        for m in &d.markers {
            let time = match m.end_seconds {
                Some(end) => format!("{}–{}", duration(m.seconds), duration(end)),
                None => duration(m.seconds),
            };
            list = list.push(
                button(
                    row![
                        text(time)
                            .size(12)
                            .color(theme::FROSTED_BLUE)
                            .width(Length::Fixed(90.0)),
                        text(m.title.clone()).size(13).width(Length::Fill),
                        text(m.tag.clone()).size(11).color(theme::MUTED),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
                .padding([3, 6])
                .width(Length::Fill)
                .style(theme::menu_row(false))
                .on_press(ShellMsg::Scene(SceneMsg::Marker(m.seconds))),
            );
        }
        col = col.push(list);
    }
    if let Some(details) = &d.details {
        col = col.push(text(details.clone()).size(14));
    }
    if !d.urls.is_empty() {
        col = col.push(text(d.urls.join("\n")).size(12).color(theme::MUTED));
    }
    if let Some(file) = file_line(d) {
        col = col.push(text(file).size(12).color(theme::MUTED));
    }

    // "All files" and "Galleries", loaded when opened.
    let files_label = if d.file_count > 1 {
        format!("All files ({})", d.file_count)
    } else {
        "File details".to_owned()
    };
    col = col.push(section_button(
        files_label,
        state.files.is_open(),
        SceneMsg::ToggleFiles,
    ));
    match &state.files {
        Section::Loading => col = col.push(text("Loading…").size(12).color(theme::MUTED)),
        Section::Failed(m) => col = col.push(text(m.clone()).size(12).color(theme::ROSEWOOD)),
        Section::Ready { data, open: true } => {
            for (i, f) in data.iter().enumerate() {
                col = col.push(file_facts(f, i == 0));
            }
        }
        _ => {}
    }
    col = col.push(section_button(
        "Galleries".to_owned(),
        state.galleries.is_open(),
        SceneMsg::ToggleGalleries,
    ));
    match &state.galleries {
        Section::Loading => col = col.push(text("Loading…").size(12).color(theme::MUTED)),
        Section::Failed(m) => col = col.push(text(m.clone()).size(12).color(theme::ROSEWOOD)),
        Section::Ready { data, open: true } if data.is_empty() => {
            col = col.push(text("No galleries.").size(12).color(theme::MUTED));
        }
        Section::Ready { data, open: true } => {
            col = col.push(wrap(
                data.iter()
                    .map(|g| {
                        container(
                            text(format!("{} · {} images", short(&g.title), g.image_count))
                                .size(13),
                        )
                        .padding([3, 10])
                        .style(theme::panel)
                        .into()
                    })
                    .collect(),
            ));
        }
        _ => {}
    }
    col.into()
}

/// The resting chip's card, at the foot of the details panel. `portrait`: for a performer with
/// an image, its handle once decoded (`Some(None)` keeps its room while it comes).
fn chip_card<'a>(
    chip: &ChipRef,
    portrait: Option<Option<&'a Handle>>,
    library: &'a Library,
) -> Element<'a, ShellMsg> {
    let heading = |s: String| text(s).size(17);
    let info: Element<'a, ShellMsg> = match library.chips.get(&(chip.kind, chip.id.clone())) {
        Some(Loadable::Ready(details)) => match details {
            ChipDetails::Performer {
                name,
                disambiguation,
                gender,
                birthdate,
                country,
                favorite,
                scene_count,
            } => {
                let mut title = row![heading(name.clone())]
                    .spacing(6)
                    .align_y(Alignment::Center);
                if *favorite {
                    title = title.push(text("♥").size(15).color(theme::ROSEWOOD));
                }
                let mut col = column![title].spacing(6);
                if let Some(d) = disambiguation {
                    col = col.push(text(d.clone()).size(12).color(theme::MUTED));
                }
                col = col.push(rule::horizontal(1));
                for (label, value) in [
                    ("Gender", gender.clone()),
                    ("Born", birthdate.clone()),
                    ("Country", country.clone()),
                    ("Scenes", Some(scene_count.to_string())),
                ] {
                    if let Some(value) = value {
                        col = col.push(card_fact(label, value));
                    }
                }
                col.into()
            }
            ChipDetails::Tag {
                name,
                description,
                scene_count,
            } => {
                let mut col = column![heading(name.clone()), rule::horizontal(1)].spacing(6);
                if let Some(d) = description {
                    col = col.push(text(d.clone()).size(13));
                }
                col.push(card_fact("Scenes", scene_count.to_string()))
                    .into()
            }
            ChipDetails::Studio {
                name,
                parent,
                scene_count,
            } => {
                let mut col = column![heading(name.clone()), rule::horizontal(1)].spacing(6);
                if let Some(p) = parent {
                    col = col.push(card_fact("Part of", p.clone()));
                }
                col.push(card_fact("Scenes", scene_count.to_string()))
                    .into()
            }
        },
        Some(Loadable::Failed) => text("Couldn't load the details.")
            .size(12)
            .color(theme::ROSEWOOD)
            .into(),
        _ => text("Loading…").size(12).color(theme::MUTED).into(),
    };
    // The details sit in a column of their own beside the portrait, top-aligned, and one frame
    // holds both, sized to them rather than to the panel.
    let content: Element<'a, ShellMsg> = match portrait {
        Some(handle) => {
            let picture: Element<'a, ShellMsg> = match handle {
                Some(h) => image(h.clone())
                    .width(PORTRAIT)
                    .height(PORTRAIT * 1.5)
                    .content_fit(ContentFit::Cover)
                    .border_radius(6)
                    .into(),
                None => Space::new().width(PORTRAIT).height(PORTRAIT * 1.5).into(),
            };
            row![picture, container(info).width(CARD_INFO)]
                .spacing(CARD_PAD)
                .into()
        }
        None => container(info).width(CARD_INFO * 1.5).into(),
    };
    container(opaque(
        container(content)
            .padding(CARD_PAD)
            .style(theme::hover_card),
    ))
    .padding(10)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_y(iced::alignment::Vertical::Bottom)
    .into()
}

/// The width of a chip card's details column.
const CARD_INFO: f32 = 220.0;
/// Space inside a chip card's frame, and between its portrait and details.
const CARD_PAD: f32 = 14.0;

/// One labelled fact on a chip card.
fn card_fact<'a>(label: &'a str, value: String) -> Element<'a, ShellMsg> {
    row![
        text(label).size(12).color(theme::MUTED).width(64),
        text(value).size(13),
    ]
    .spacing(8)
    .into()
}
