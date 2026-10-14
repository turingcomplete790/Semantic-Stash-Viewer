//! The filter bar (active criteria as removable chips, the applied saved filter, what a saved
//! filter left out), the filter menu (every criterion by group, the semantic placeholders
//! described and disabled), the criterion editor, and the saved filters (008 FR-002 to FR-005,
//! FR-003b, FR-003f). Generic since 012: each list's own criteria and saved filters.

use std::fmt;

use iced::widget::{
    button, checkbox, column, container, mouse_area, opaque, pick_list, row, scrollable, stack,
    text, text_input, Space,
};
use iced::{Alignment, Element, Length};
use stash_core::scenes::filter::kinds::{CriterionKind, Group, ValueType};
use stash_core::scenes::filter::{
    CriterionValue, Modifier, Orientation, Resolution, SemanticFilter,
};

use super::controls::Map;
use super::editor::{chip_label, DupField, Editor, EditorMsg};
use super::layout::PAD;
use super::picker;
use super::state::{BrowseMsg, Kind, List, Panel};
use super::thumbs::Thumbs;
use crate::library::{Library, Loadable};
use crate::shell::ShellMsg;
use crate::widgets::theme;

// ---- The bar ---------------------------------------------------------------------------------

/// A chip's colours: a tag criterion with one tag takes that tag's rule colour.
fn chip_colour(
    library: &Library,
    c: &stash_core::scenes::filter::Criterion,
) -> Option<stash_core::semantic::Rgb> {
    match (&c.kind, &c.value) {
        (
            CriterionKind::Tags | CriterionKind::PerformerTags,
            CriterionValue::Items {
                items, excluded, ..
            },
        ) if items.len() == 1 && excluded.is_empty() => library.tag_colour(&items[0].id),
        _ => None,
    }
}

/// The active criteria as chips (click to change, ✕ to remove), "Clear all", the applied saved
/// filter's name, and any parts a saved filter left out. Nothing when there are none.
pub fn bar<'a, K: Kind>(
    state: &'a List<K>,
    library: &'a Library,
    msg: Map<K>,
) -> Option<Element<'a, ShellMsg>> {
    let criteria = &state.query.criteria;
    if criteria.is_empty() && state.applied.is_none() && state.left_out.is_empty() {
        return None;
    }
    let mut chips = row![].spacing(6).align_y(Alignment::Center);
    if let Some(applied) = &state.applied {
        chips = chips.push(
            text(format!("{}:", applied.name))
                .size(13)
                .color(theme::FROSTED_BLUE),
        );
    }
    for c in criteria {
        let key = c.sort_key();
        let label = chip_label(c);
        let colour = chip_colour(library, c);
        let body = button(text(label).size(12)).padding([3, 8]).on_press_maybe(
            (!c.info().is_placeholder()).then(|| msg(BrowseMsg::Edit(key.clone()))),
        );
        let body = match colour {
            Some(rgb) => body.style(move |_, status| {
                let bg = picker::colour(rgb);
                button::Style {
                    background: Some(bg.into()),
                    text_color: picker::on_colour(rgb),
                    border: iced::Border {
                        radius: 10.0.into(),
                        width: if matches!(status, button::Status::Hovered) {
                            1.0
                        } else {
                            0.0
                        },
                        color: theme::TEXT,
                    },
                    ..button::Style::default()
                }
            }),
            None => body.style(button::secondary),
        };
        chips = chips.push(
            row![
                body,
                button(text("✕").size(11))
                    .padding([3, 6])
                    .style(button::text)
                    .on_press(msg(BrowseMsg::Remove(key))),
            ]
            .align_y(Alignment::Center),
        );
    }
    if !criteria.is_empty() {
        chips = chips.push(
            button(text("Clear all").size(12))
                .padding([3, 8])
                .style(button::text)
                .on_press(msg(BrowseMsg::ClearFilters)),
        );
    }
    let mut col = column![
        scrollable(chips).direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new().width(4).scroller_width(4),
        ))
    ]
    .spacing(6);
    if !state.left_out.is_empty() {
        col = col.push(
            container(
                row![
                    text(format!(
                        "Part of this saved filter couldn't be applied: {}.",
                        state.left_out.join(", ")
                    ))
                    .size(12),
                    Space::new().width(Length::Fill),
                    button(text("Dismiss").size(12))
                        .padding([2, 8])
                        .style(button::text)
                        .on_press(msg(BrowseMsg::DismissLeftOut)),
                ]
                .align_y(Alignment::Center),
            )
            .padding([4, 8])
            .style(theme::problem),
        );
    }
    Some(
        container(col)
            .padding([0.0, PAD])
            .width(Length::Fill)
            .into(),
    )
}

// ---- The menu --------------------------------------------------------------------------------

/// A row of the filter menu.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuRow {
    Header(&'static str),
    /// An active criterion (Enter changes it, Delete removes it).
    Active {
        key: String,
        label: String,
    },
    Kind(CriterionKind),
    Placeholder(SemanticFilter),
}

impl MenuRow {
    /// Whether the keyboard can land on it.
    pub fn actionable(&self) -> bool {
        matches!(self, MenuRow::Active { .. } | MenuRow::Kind(_))
    }
}

/// The menu's rows: the active criteria, then every kind by group, then the semantic group.
pub fn menu_rows<K: Kind>(state: &List<K>) -> Vec<MenuRow> {
    let mut rows = Vec::new();
    if !state.query.criteria.is_empty() {
        rows.push(MenuRow::Header("Active filters"));
        for c in &state.query.criteria {
            rows.push(MenuRow::Active {
                key: c.sort_key(),
                label: chip_label(c),
            });
        }
    }
    for group in Group::ALL {
        if group == Group::Semantic {
            rows.push(MenuRow::Header(group.label()));
            rows.extend(SemanticFilter::ALL.iter().map(|s| MenuRow::Placeholder(*s)));
            continue;
        }
        let kinds: Vec<CriterionKind> = CriterionKind::ALL
            .iter()
            .copied()
            .filter(|k| k.info().group == group && k.applies_to(K::ENTITY))
            .collect();
        if kinds.is_empty() {
            continue;
        }
        rows.push(MenuRow::Header(group.label()));
        rows.extend(kinds.into_iter().map(MenuRow::Kind));
    }
    rows
}

/// The actionable row the highlight is on (clamped).
pub fn menu_target<K: Kind>(state: &List<K>, highlight: usize) -> Option<MenuRow> {
    let actionable: Vec<MenuRow> = menu_rows(state)
        .into_iter()
        .filter(MenuRow::actionable)
        .collect();
    (!actionable.is_empty()).then(|| actionable[highlight.min(actionable.len() - 1)].clone())
}

fn menu<K: Kind>(state: &List<K>, highlight: usize, msg: Map<K>) -> Element<'_, ShellMsg> {
    let target = menu_target(state, highlight);
    let mut col = column![].spacing(2);
    for r in menu_rows(state) {
        let selected = target.as_ref() == Some(&r);
        let element: Element<'_, ShellMsg> = match r {
            MenuRow::Header(label) => container(text(label).size(12).color(theme::FROSTED_BLUE))
                .padding([6, 4])
                .into(),
            MenuRow::Active { key, label } => row![
                button(text(label).size(13))
                    .padding([3, 8])
                    .width(Length::Fill)
                    .style(theme::menu_row(selected))
                    .on_press(msg(BrowseMsg::Edit(key.clone()))),
                button(text("Remove").size(12))
                    .padding([3, 8])
                    .style(button::text)
                    .on_press(msg(BrowseMsg::Remove(key))),
            ]
            .into(),
            MenuRow::Kind(kind) => button(text(kind.info().label).size(13))
                .padding([3, 8])
                .width(Length::Fill)
                .style(theme::menu_row(selected))
                .on_press(msg(BrowseMsg::Add(kind)))
                .into(),
            MenuRow::Placeholder(s) => container(
                column![
                    text(s.label()).size(13).color(theme::MUTED),
                    text(format!(
                        "{} (coming with the semantic engine)",
                        s.description()
                    ))
                    .size(11)
                    .color(theme::MUTED),
                ]
                .spacing(1),
            )
            .padding([3, 8])
            .into(),
        };
        col = col.push(element);
    }
    scrollable(col).height(Length::Fixed(360.0)).into()
}

// ---- The editor ------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ModifierChoice(Modifier);

impl fmt::Display for ModifierChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolutionChoice(Resolution);

impl fmt::Display for ResolutionChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.name())
    }
}

fn input<'a>(
    placeholder: &'a str,
    value: &'a str,
    on: fn(String) -> EditorMsg,
    edit: impl picker::Edit,
) -> Element<'a, ShellMsg> {
    text_input(placeholder, value)
        .on_input(move |t| edit(on(t)))
        .on_submit(edit(EditorMsg::Apply))
        .size(14)
        .padding([6, 10])
        .width(Length::Fixed(260.0))
        .into()
}

fn value_fields<'a>(
    editor: &'a Editor,
    library: &'a Library,
    thumbs: &'a Thumbs,
    entity: stash_core::browse::entity::Entity,
    edit: impl picker::Edit,
) -> Element<'a, ShellMsg> {
    if editor.modifier.is_null_check() && editor.kind != CriterionKind::CustomField {
        return Space::new().into();
    }
    let range = editor.modifier.is_range();
    let pair = |hint: &'a str| -> Element<'a, ShellMsg> {
        let mut r = row![input(hint, &editor.text, EditorMsg::Text, edit)]
            .spacing(8)
            .align_y(Alignment::Center);
        if range {
            r = r.push(text("and").size(13)).push(input(
                hint,
                &editor.text2,
                EditorMsg::Text2,
                edit,
            ));
        }
        r.into()
    };
    match editor.kind.info_for(entity).value_type {
        ValueType::Text { .. } => input("Text", &editor.text, EditorMsg::Text, edit),
        ValueType::Captions => input(
            "Language code (en, de, …)",
            &editor.text,
            EditorMsg::Text,
            edit,
        ),
        ValueType::Number { .. } => pair("Number"),
        ValueType::Duration { .. } => pair("90, 1:30, or 1:02:03"),
        ValueType::Date => pair("YYYY-MM-DD"),
        ValueType::Timestamp { .. } => pair("YYYY-MM-DD HH:MM"),
        ValueType::Bool | ValueType::StringBool => row![
            button(text("Yes").size(13))
                .style(theme::menu_row(editor.flag))
                .on_press(edit(EditorMsg::Flag(true))),
            button(text("No").size(13))
                .style(theme::menu_row(!editor.flag))
                .on_press(edit(EditorMsg::Flag(false))),
        ]
        .spacing(6)
        .into(),
        ValueType::Choice(options) => pick_list(
            options
                .iter()
                .map(|o| (*o).to_owned())
                .collect::<Vec<String>>(),
            Some(editor.text.clone()),
            move |o| edit(EditorMsg::Text(o)),
        )
        .text_size(13)
        .into(),
        ValueType::Resolution => pick_list(
            Resolution::ALL
                .iter()
                .copied()
                .map(ResolutionChoice)
                .collect::<Vec<_>>(),
            Some(ResolutionChoice(editor.resolution)),
            move |r| edit(EditorMsg::Resolution(r.0)),
        )
        .text_size(13)
        .into(),
        ValueType::Orientation => {
            let mut r = row![].spacing(6);
            for o in Orientation::ALL {
                r = r.push(
                    button(text(format!("{o:?}")).size(13))
                        .style(theme::menu_row(editor.orientation.contains(&o)))
                        .on_press(edit(EditorMsg::ToggleOrientation(o))),
                );
            }
            r.into()
        }
        ValueType::Items { hierarchical, .. } => {
            let mut col = column![picker::view(library, editor, thumbs, edit)].spacing(6);
            if hierarchical {
                col = col.push(
                    checkbox(editor.depth_all)
                        .label("Include sub-levels")
                        .on_toggle(move |b| edit(EditorMsg::DepthAll(b)))
                        .text_size(13),
                );
            }
            col.into()
        }
        ValueType::StashId => column![
            input("Endpoint (optional)", &editor.name, EditorMsg::Name, edit),
            input("StashID", &editor.text, EditorMsg::Text, edit),
        ]
        .spacing(6)
        .into(),
        ValueType::Phash => row![
            input("Phash", &editor.text, EditorMsg::Text, edit),
            input("Distance (optional)", &editor.text2, EditorMsg::Text2, edit),
        ]
        .spacing(8)
        .into(),
        ValueType::Duplicated => {
            let d = editor.duplication;
            let check = |label: &'static str, on: bool, field: DupField| {
                checkbox(on)
                    .label(label)
                    .on_toggle(move |b| edit(EditorMsg::Duplication(field, b)))
                    .text_size(13)
            };
            row![
                check("Phash", d.phash, DupField::Phash),
                check("URL", d.url, DupField::Url),
                check("StashID", d.stash_id, DupField::StashId),
                check("Title", d.title, DupField::Title),
            ]
            .spacing(12)
            .into()
        }
        ValueType::CustomField => {
            let mut col =
                column![input("Field name", &editor.name, EditorMsg::Name, edit)].spacing(6);
            if !editor.modifier.is_null_check() {
                col = col.push(pair("Value"));
            }
            col.into()
        }
        ValueType::Semantic => text("Not yet available.").color(theme::MUTED).into(),
    }
}

fn editor_view<'a, K: Kind>(
    editor: &'a Editor,
    library: &'a Library,
    thumbs: &'a Thumbs,
    msg: Map<K>,
) -> Element<'a, ShellMsg> {
    let edit = move |m: EditorMsg| msg(BrowseMsg::Editor(m));
    let info = editor.kind.info_for(K::ENTITY);
    let mut head = row![text(info.label).size(15)]
        .spacing(10)
        .align_y(Alignment::Center);
    let modifiers = info.modifiers();
    if !modifiers.is_empty() {
        head = head.push(
            pick_list(
                modifiers
                    .iter()
                    .copied()
                    .map(ModifierChoice)
                    .collect::<Vec<_>>(),
                Some(ModifierChoice(editor.modifier)),
                move |m| edit(EditorMsg::Modifier(m.0)),
            )
            .text_size(13),
        );
    }
    let mut col = column![head, value_fields(editor, library, thumbs, K::ENTITY, edit)].spacing(10);
    if let Some(e) = &editor.error {
        col = col.push(text(e).size(12).color(theme::ROSEWOOD));
    }
    col = col.push(
        row![
            button(
                text(if editor.replacing.is_some() {
                    "Apply"
                } else {
                    "Add"
                })
                .size(13)
            )
            .padding([4, 14])
            .on_press(edit(EditorMsg::Apply)),
            button(text("Cancel").size(13))
                .padding([4, 14])
                .style(button::secondary)
                .on_press(edit(EditorMsg::Cancel)),
        ]
        .spacing(8),
    );
    col.into()
}

// ---- Saved filters ---------------------------------------------------------------------------

fn saved<K: Kind>(library: &Library, highlight: usize, msg: Map<K>) -> Element<'_, ShellMsg> {
    match K::saved(library) {
        Loadable::Ready(list) if list.is_empty() => {
            text(format!("No saved {} filters on this server.", K::ITEM))
                .color(theme::MUTED)
                .into()
        }
        Loadable::Ready(list) => {
            let at = highlight.min(list.len() - 1);
            let mut col = column![].spacing(2);
            for (i, f) in list.iter().enumerate() {
                col = col.push(
                    button(text(f.name.clone()).size(13))
                        .padding([3, 8])
                        .width(Length::Fill)
                        .style(theme::menu_row(i == at))
                        .on_press(msg(BrowseMsg::ApplySaved(Box::new(f.clone())))),
                );
            }
            scrollable(col).height(Length::Shrink).into()
        }
        Loadable::Failed => text("Couldn't read the saved filters.")
            .color(theme::ROSEWOOD)
            .into(),
        _ => text("Loading saved filters…").color(theme::MUTED).into(),
    }
}

/// The open panel, if any, with a close button.
pub fn panel<'a, K: Kind>(
    state: &'a List<K>,
    library: &'a Library,
    thumbs: &'a Thumbs,
    msg: Map<K>,
) -> Option<Element<'a, ShellMsg>> {
    let (title, body, close): (&str, Element<'a, ShellMsg>, ShellMsg) = match &state.panel {
        Panel::None => return None,
        Panel::Menu { highlight } => (
            "Add a filter",
            menu(state, *highlight, msg),
            msg(BrowseMsg::FilterMenu(false)),
        ),
        Panel::Editor(editor) => (
            if editor.replacing.is_some() {
                "Change filter"
            } else {
                "New filter"
            },
            editor_view::<K>(editor, library, thumbs, msg),
            msg(BrowseMsg::Editor(EditorMsg::Cancel)),
        ),
        Panel::Saved { highlight } => (
            "Saved filters",
            saved::<K>(library, *highlight, msg),
            msg(BrowseMsg::SavedMenu(false)),
        ),
    };
    let panel = container(
        container(
            column![
                row![
                    text(title).size(14).color(theme::FROSTED_BLUE),
                    Space::new().width(Length::Fill),
                    button(text("Close").size(12))
                        .padding([2, 8])
                        .style(button::text)
                        .on_press(close.clone()),
                ]
                .align_y(Alignment::Center),
                body,
            ]
            .spacing(8),
        )
        .padding(12)
        .max_width(720)
        .style(theme::panel),
    )
    .padding([4.0, PAD]);
    // Floats above the grid: a click outside it closes it (the overlay rules); clicks on it
    // stay with it.
    Some(
        stack![
            mouse_area(Space::new().width(Length::Fill).height(Length::Fill)).on_press(close),
            opaque(panel),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into(),
    )
}
