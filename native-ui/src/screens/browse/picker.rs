//! The picker (008 FR-003a, FR-003c, FR-003d): the full cached list for performers, tags,
//! studios, groups, or galleries, narrowing on each key (names and aliases, accent-folded).
//! Only the rows in view are built, so a 20,000-name list opens and narrows within a frame.
//! The Tags picker shows the semantic vocabulary grouped by type, each tag in its rule colour,
//! above the ordinary tags.

use iced::widget::{button, column, container, image, mouse_area, row, scrollable, text, Space};
use iced::{Alignment, Color, ContentFit, Element, Length};
use stash_core::adapter::choices::ChoiceKind;
use stash_core::scenes::filter::search::{fold, search, Folded};
use stash_core::scenes::filter::{Item, Modifier};
use stash_core::semantic::{Rgb, SemanticKind};

use super::editor::{Editor, EditorMsg};
use super::state::PICKER_ID;
use super::thumbs::{ThumbKey, Thumbs};
use crate::library::{Library, Loadable};
use crate::shell::ShellMsg;
use crate::widgets::theme;

/// A row's height in the list.
pub const ROW_H: f32 = 30.0;
/// The list's visible height.
pub const LIST_H: f32 = 300.0;
/// The list's scrollable id.
pub const LIST_ID: &str = "scenes-picker-list";

/// How the picker's editor messages reach the shell (the list's message mapper, 012).
pub trait Edit: Fn(EditorMsg) -> ShellMsg + Copy + 'static {}

impl<F: Fn(EditorMsg) -> ShellMsg + Copy + 'static> Edit for F {}

/// One row of the list.
#[derive(Debug, Clone, PartialEq)]
pub enum Row {
    Header(String),
    Entry {
        item: Item,
        colour: Option<Rgb>,
        badge: Option<SemanticKind>,
        /// A performer's picture version (008 FR-011c); `None` for no picture and other kinds.
        picture: Option<String>,
    },
}

/// The rows for the editor's list and query, or `None` while the list is loading.
pub fn rows(library: &Library, editor: &Editor) -> Option<Vec<Row>> {
    let kind = editor.picker()?;
    let choices = match library.choices.get(&kind) {
        Some(Loadable::Ready(c)) => c,
        _ => return None,
    };
    let query = fold(editor.picker_query.trim());
    let mut out = Vec::new();
    let rules = library.colour_rules();
    if kind == ChoiceKind::Tags {
        if let Some(vocabulary) = library.vocabulary.ready() {
            for sk in SemanticKind::ALL {
                let tags = vocabulary.of_kind(sk);
                let hits: Vec<_> = tags
                    .into_iter()
                    .filter(|t| Folded::new(&t.name, &t.aliases).rank(&query).is_some())
                    .collect();
                if hits.is_empty() {
                    continue;
                }
                out.push(Row::Header(sk.label().to_owned()));
                for t in hits {
                    out.push(Row::Entry {
                        item: Item {
                            id: t.id.clone(),
                            label: t.name.clone(),
                        },
                        colour: vocabulary.colour_of(&t.id, &rules),
                        badge: Some(sk),
                        picture: None,
                    });
                }
            }
            let extras: Vec<_> = vocabulary
                .ordinary_extras()
                .into_iter()
                .filter(|t| Folded::new(&t.name, &t.aliases).rank(&query).is_some())
                .collect();
            let ordinary = search(&choices.folded, &editor.picker_query);
            if !ordinary.is_empty() || !extras.is_empty() {
                out.push(Row::Header("Tags".to_owned()));
            }
            for t in extras {
                out.push(Row::Entry {
                    item: Item {
                        id: t.id.clone(),
                        label: t.name.clone(),
                    },
                    colour: vocabulary.colour_of(&t.id, &rules),
                    badge: None,
                    picture: None,
                });
            }
            out.extend(ordinary.into_iter().map(|i| {
                let c = &choices.list.entries[i];
                Row::Entry {
                    item: Item {
                        id: c.id.clone(),
                        label: c.name.clone(),
                    },
                    colour: vocabulary.colour_of(&c.id, &rules),
                    badge: None,
                    picture: None,
                }
            }));
            return Some(out);
        }
    }
    out.extend(
        search(&choices.folded, &editor.picker_query)
            .into_iter()
            .map(|i| {
                let c = &choices.list.entries[i];
                Row::Entry {
                    item: Item {
                        id: c.id.clone(),
                        label: c.name.clone(),
                    },
                    colour: None,
                    badge: None,
                    picture: c.image.clone(),
                }
            }),
    );
    Some(out)
}

/// The entries only (headers skipped), as the keyboard counts them.
fn entries(rows: &[Row]) -> Vec<&Item> {
    rows.iter()
        .filter_map(|r| match r {
            Row::Entry { item, .. } => Some(item),
            Row::Header(_) => None,
        })
        .collect()
}

/// The keyboard's row: the highlight, kept within the entries.
pub fn highlighted(library: &Library, editor: &Editor) -> Option<Item> {
    let rows = rows(library, editor)?;
    let list = entries(&rows);
    if list.is_empty() {
        return None;
    }
    Some(list[editor.picker_highlight.min(list.len() - 1)].clone())
}

/// The picture keys of the performers in the list's visible rows (their pictures load, nothing
/// else does; performers without a picture ask for nothing, 008 FR-011c).
pub fn visible_performers(library: &Library, editor: &Editor) -> Vec<ThumbKey> {
    if editor.picker() != Some(ChoiceKind::Performers) {
        return Vec::new();
    }
    let Some(rows) = rows(library, editor) else {
        return Vec::new();
    };
    let first = (editor.picker_scroll / ROW_H).floor() as usize;
    let shown = (LIST_H / ROW_H).ceil() as usize + 2;
    rows.iter()
        .skip(first)
        .take(shown)
        .filter_map(|r| match r {
            Row::Entry {
                item,
                picture: Some(version),
                ..
            } => Some(performer_key(&item.id, version)),
            _ => None,
        })
        .collect()
}

/// A performer picture's thumbnail key, at its version (008 FR-011c: a changed picture is a new
/// key, so the old one is never shown again).
pub fn performer_key(id: &str, version: &str) -> ThumbKey {
    (format!("performer:{id}"), version.to_owned())
}

/// A performer's portrait (the scene page's chip card, 008 FR-011b): its own handle, decoded
/// larger, from the same cached image as [`performer_key`] (never a second download).
pub fn portrait_key(id: &str, version: &str) -> ThumbKey {
    (format!("portrait:{id}"), version.to_owned())
}

/// Where the list must scroll to keep `highlight` (an entry index) in view, if it isn't.
pub fn reveal(library: &Library, editor: &Editor) -> Option<f32> {
    let rows = rows(library, editor)?;
    let mut n = 0;
    let at = rows.iter().position(|r| match r {
        Row::Entry { .. } => {
            n += 1;
            n - 1 == editor.picker_highlight
        }
        Row::Header(_) => false,
    })?;
    let top = at as f32 * ROW_H;
    if top < editor.picker_scroll {
        Some(top)
    } else if top + ROW_H > editor.picker_scroll + LIST_H {
        Some(top + ROW_H - LIST_H)
    } else {
        None
    }
}

/// A chip colour as iced's.
pub fn colour(c: Rgb) -> Color {
    Color::from_rgb8(c.0, c.1, c.2)
}

/// The text colour on a chip of `c`: dark or light, whichever has at least 4.5:1 contrast.
pub fn on_colour(c: Rgb) -> Color {
    let t = theme::chip_text(c);
    Color::from_rgb8(t.0, t.1, t.2)
}

fn swatch(c: Option<Rgb>) -> Element<'static, ShellMsg> {
    let fill = c.map(colour);
    container(Space::new().width(10).height(10))
        .style(move |_| container::Style {
            background: fill.map(Into::into),
            border: iced::Border {
                radius: 5.0.into(),
                width: if fill.is_some() { 0.0 } else { 1.0 },
                color: theme::MUTED,
            },
            ..container::Style::default()
        })
        .into()
}

/// The picker: search field, list (only rows in view built), and what's chosen.
pub fn view<'a>(
    library: &'a Library,
    editor: &'a Editor,
    thumbs: &'a Thumbs,
    msg: impl Edit,
) -> Element<'a, ShellMsg> {
    let kind = editor.picker();
    let highlighted_item = highlighted(library, editor);
    let search = iced::widget::text_input("Type to narrow the list", &editor.picker_query)
        .id(PICKER_ID)
        .on_input(move |t| msg(EditorMsg::PickerQuery(t)))
        .on_submit_maybe(highlighted_item.clone().map(|i| msg(EditorMsg::Toggle(i))))
        .size(14)
        .padding([6, 10]);
    let list: Element<'a, ShellMsg> = match rows(library, editor) {
        None => container(text("Loading the list…").color(theme::MUTED))
            .padding(10)
            .height(Length::Fixed(LIST_H))
            .into(),
        Some(rows) if rows.is_empty() => container(text("Nothing matches.").color(theme::MUTED))
            .padding(10)
            .height(Length::Fixed(LIST_H))
            .into(),
        Some(rows) => window(rows, editor, highlighted_item.as_ref(), kind, thumbs, msg),
    };
    column![search, list, chosen(editor, msg)].spacing(8).into()
}

fn window<'a>(
    rows: Vec<Row>,
    editor: &'a Editor,
    highlighted: Option<&Item>,
    kind: Option<ChoiceKind>,
    thumbs: &'a Thumbs,
    msg: impl Edit,
) -> Element<'a, ShellMsg> {
    let total = rows.len();
    let first = ((editor.picker_scroll / ROW_H).floor() as usize).min(total.saturating_sub(1));
    let shown = (LIST_H / ROW_H).ceil() as usize + 2;
    let end = (first + shown).min(total);
    let can_exclude = matches!(editor.modifier, Modifier::Includes | Modifier::IncludesAll);
    // A plain column, not a keyed one: a keyed column reuses a child's state by key alone, and a
    // row's place holds a header in one frame and an entry in the next (the Tags list's groups
    // arrive with the vocabulary), which crashed iced ("Downcast widget state").
    let mut col = column![Space::new().height(first as f32 * ROW_H)];
    for r in rows.into_iter().skip(first).take(end - first) {
        let element: Element<'a, ShellMsg> = match r {
            Row::Header(label) => container(text(label).size(12).color(theme::FROSTED_BLUE))
                .padding([6, 8])
                .height(Length::Fixed(ROW_H))
                .into(),
            Row::Entry {
                item,
                colour: c,
                badge,
                picture,
            } => {
                let chosen = editor.items.iter().any(|x| x.id == item.id);
                let excluded = editor.excluded.iter().any(|x| x.id == item.id);
                let mark = if chosen {
                    "✓"
                } else if excluded {
                    "✕"
                } else {
                    " "
                };
                let mut line = row![text(mark).size(13).width(14)]
                    .spacing(8)
                    .align_y(Alignment::Center);
                if kind == Some(ChoiceKind::Performers) {
                    let handle = picture
                        .as_deref()
                        .and_then(|v| thumbs.get(&performer_key(&item.id, v)))
                        .cloned();
                    if let Some(h) = handle {
                        line = line.push(
                            image(h)
                                .width(22)
                                .height(22)
                                .content_fit(ContentFit::Cover)
                                .border_radius(11),
                        );
                    } else {
                        line = line.push(Space::new().width(22));
                    }
                }
                if badge.is_some() || c.is_some() {
                    line = line.push(swatch(c));
                }
                line = line.push(text(item.label.clone()).size(13).width(Length::Fill));
                if let Some(b) = badge {
                    line = line.push(text(b.label()).size(11).color(theme::MUTED));
                }
                let is_highlighted = highlighted.is_some_and(|h| h.id == item.id);
                let mut area = mouse_area(
                    button(line)
                        .padding([4, 8])
                        .width(Length::Fill)
                        .height(Length::Fixed(ROW_H))
                        .style(theme::menu_row(is_highlighted))
                        .on_press(msg(EditorMsg::Toggle(item.clone()))),
                );
                if can_exclude {
                    area = area.on_right_press(msg(EditorMsg::Exclude(item)));
                }
                area.into()
            }
        };
        col = col.push(element);
    }
    col = col.push(Space::new().height((total - end) as f32 * ROW_H));
    scrollable(col)
        .id(LIST_ID)
        .height(Length::Fixed(LIST_H))
        .on_scroll(move |v| msg(EditorMsg::PickerScrolled(v.absolute_offset().y)))
        .into()
}

/// What's chosen, each removable; excluded ones marked.
fn chosen(editor: &Editor, msg: impl Edit) -> Element<'_, ShellMsg> {
    if editor.items.is_empty() && editor.excluded.is_empty() {
        return text("Click to include, right-click (or Ctrl+E) to exclude.")
            .size(12)
            .color(theme::MUTED)
            .into();
    }
    let chip = |item: &Item, excluded: bool| {
        button(
            text(format!(
                "{}{} ✕",
                if excluded { "not " } else { "" },
                item.label
            ))
            .size(12),
        )
        .padding([2, 8])
        .style(button::secondary)
        .on_press(msg(EditorMsg::Remove(item.id.clone())))
    };
    let mut line = row![].spacing(6);
    for i in &editor.items {
        line = line.push(chip(i, false));
    }
    for i in &editor.excluded {
        line = line.push(chip(i, true));
    }
    scrollable(line)
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new().width(4).scroller_width(4),
        ))
        .into()
}
