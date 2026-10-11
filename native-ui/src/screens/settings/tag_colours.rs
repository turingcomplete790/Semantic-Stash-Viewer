//! Settings → Tag colours (008 FR-003g; constitution Principle XI): the colour rules in order,
//! each a tag custom field flag and a colour, with how many tags it colours. Change a colour,
//! add a rule for another flag, remove an added rule, reorder, reset to the defaults, and save
//! (one write of the whole `semanticStashViewer.tagColours` value).

use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Alignment, Color, Element, Length};
use stash_core::semantic::{default_colour_rules, valid_rule_field, ColourRule, Rgb, SemanticKind};

use crate::effects::Effect;
use crate::library::Library;
use crate::shell::ShellMsg;
use crate::widgets::theme;

/// The hex field's widget id.
pub const HEX_ID: &str = "tag-colour-hex";
/// The new rule's field input id.
pub const FIELD_ID: &str = "tag-colour-field";

/// The colours offered as swatches: the RPG palette (constitution Principle XI), then the app's
/// accents.
pub const SWATCHES: [Rgb; 21] = [
    Rgb(0x0d, 0x0a, 0x05),
    Rgb(0x1c, 0x15, 0x08),
    Rgb(0x2e, 0x20, 0x10),
    Rgb(0x4a, 0x34, 0x20),
    Rgb(0x7a, 0x5c, 0x38),
    Rgb(0xb0, 0x88, 0x58),
    Rgb(0xd4, 0xb8, 0x80),
    Rgb(0xf0, 0xe0, 0xb0),
    Rgb(0xc8, 0x95, 0x2a),
    Rgb(0x7d, 0xd8, 0x7a),
    Rgb(0x3a, 0x9e, 0x50),
    Rgb(0x5b, 0x8f, 0xd4),
    Rgb(0x2a, 0x50, 0x90),
    Rgb(0xd4, 0x40, 0x40),
    Rgb(0x88, 0x40, 0xc0),
    Rgb(0xe8, 0xe0, 0xc8),
    Rgb(0xc3, 0x3c, 0x54),
    Rgb(0x25, 0x4e, 0x70),
    Rgb(0x37, 0x71, 0x8e),
    Rgb(0x8e, 0xe3, 0xef),
    Rgb(0xae, 0xf3, 0xe7),
];

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Saving {
    #[default]
    Idle,
    Saving,
    Saved,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TagColoursPage {
    /// The rules being edited; `None` until the server's rules are here.
    pub rules: Option<Vec<ColourRule>>,
    /// Changed since the last save or load.
    pub dirty: bool,
    /// The rule whose colour is being changed.
    pub editing: Option<usize>,
    /// The hex field's text.
    pub hex: String,
    /// The new rule's field name.
    pub new_field: String,
    pub error: Option<String>,
    pub saving: Saving,
    /// The keyboard's rule.
    pub highlight: usize,
}

#[derive(Debug, Clone)]
pub enum TagColoursMsg {
    Edit(usize),
    SetColour(usize, Rgb),
    Hex(String),
    ApplyHex,
    MoveUp(usize),
    MoveDown(usize),
    Remove(usize),
    NewField(String),
    AddRule,
    Reset,
    Save,
    /// The keyboard: move the highlight.
    Highlight(i32),
}

/// Whether a rule is one of the semantic types' defaults (they can be recoloured and reordered,
/// not removed).
pub fn is_semantic(rule: &ColourRule) -> bool {
    SemanticKind::from_flag(&rule.field).is_some()
}

impl TagColoursPage {
    /// The server's rules arrived (or were saved): take them unless the user is mid-edit.
    pub fn sync(&mut self, rules: Vec<ColourRule>) {
        if !self.dirty {
            self.rules = Some(rules);
            self.highlight = self.highlight.min(self.len().saturating_sub(1));
        }
    }

    fn len(&self) -> usize {
        self.rules.as_ref().map_or(0, Vec::len)
    }

    pub fn saved(&mut self, result: Result<Vec<ColourRule>, String>) {
        match result {
            Ok(rules) => {
                self.dirty = false;
                self.rules = Some(rules);
                self.saving = Saving::Saved;
            }
            Err(message) => self.saving = Saving::Failed(message),
        }
    }

    pub fn update(&mut self, msg: TagColoursMsg) -> Vec<Effect> {
        let Some(rules) = self.rules.as_mut() else {
            return Vec::new();
        };
        self.error = None;
        let mut changed = true;
        let mut effects = Vec::new();
        match msg {
            TagColoursMsg::Edit(i) if i < rules.len() => {
                self.editing = Some(i);
                self.highlight = i;
                self.hex = rules[i].colour.to_string();
                changed = false;
                effects.push(Effect::Focus(HEX_ID));
            }
            TagColoursMsg::SetColour(i, c) if i < rules.len() => {
                rules[i].colour = c;
                self.hex = c.to_string();
            }
            TagColoursMsg::Hex(t) => {
                self.hex = t;
                changed = false;
            }
            TagColoursMsg::ApplyHex => match (self.editing, Rgb::parse(self.hex.trim())) {
                (Some(i), Some(c)) if i < rules.len() => rules[i].colour = c,
                _ => {
                    self.error = Some("Colours look like #c8952a".into());
                    changed = false;
                }
            },
            TagColoursMsg::MoveUp(i) if i > 0 && i < rules.len() => {
                rules.swap(i, i - 1);
                self.highlight = i - 1;
                self.editing = self.editing.map(|e| if e == i { i - 1 } else { e });
            }
            TagColoursMsg::MoveDown(i) if i + 1 < rules.len() => {
                rules.swap(i, i + 1);
                self.highlight = i + 1;
                self.editing = self.editing.map(|e| if e == i { i + 1 } else { e });
            }
            TagColoursMsg::Remove(i) if i < rules.len() && !is_semantic(&rules[i]) => {
                rules.remove(i);
                self.editing = None;
                self.highlight = self.highlight.min(rules.len().saturating_sub(1));
            }
            TagColoursMsg::NewField(t) => {
                self.new_field = t;
                changed = false;
            }
            TagColoursMsg::AddRule => {
                let field = self.new_field.trim().to_owned();
                if !valid_rule_field(&field) {
                    self.error = Some("A field name is letters, digits, and _ (up to 64)".into());
                    changed = false;
                } else if rules.iter().any(|r| r.field == field) {
                    self.error = Some(format!("{field} already has a rule"));
                    changed = false;
                } else {
                    rules.push(ColourRule {
                        field,
                        colour: Rgb(0xc8, 0x95, 0x2a),
                    });
                    self.new_field.clear();
                    self.editing = Some(rules.len() - 1);
                    self.highlight = rules.len() - 1;
                    self.hex = rules[rules.len() - 1].colour.to_string();
                }
            }
            TagColoursMsg::Reset => {
                *rules = default_colour_rules();
                self.editing = None;
                self.highlight = 0;
            }
            TagColoursMsg::Save => {
                changed = false;
                if self.saving != Saving::Saving {
                    self.saving = Saving::Saving;
                    effects.push(Effect::SaveTagColours(rules.clone()));
                }
            }
            TagColoursMsg::Highlight(delta) => {
                changed = false;
                self.highlight = self
                    .highlight
                    .saturating_add_signed(delta as isize)
                    .min(rules.len().saturating_sub(1));
            }
            _ => changed = false,
        }
        if changed {
            self.dirty = true;
            self.saving = Saving::Idle;
        }
        effects
    }

    pub fn view<'a>(&'a self, library: &'a Library) -> Element<'a, ShellMsg> {
        let m = |msg: TagColoursMsg| {
            ShellMsg::Settings(crate::screens::settings::SettingsMsg::TagColours(msg))
        };
        let Some(rules) = &self.rules else {
            return column![
                text("Tag colours").size(22),
                text("Reading the server's colours…").color(theme::MUTED),
            ]
            .spacing(10)
            .into();
        };
        let counts = library.vocabulary.ready();
        let mut list = column![].spacing(4);
        for (i, rule) in rules.iter().enumerate() {
            let fill = Color::from_rgb8(rule.colour.0, rule.colour.1, rule.colour.2);
            let swatch = container(Space::new().width(22).height(22)).style(move |_| {
                container::Style::default()
                    .background(fill)
                    .border(iced::border::rounded(4))
            });
            let label = match SemanticKind::from_flag(&rule.field) {
                Some(k) => format!("{} ({})", k.label(), rule.field),
                None => rule.field.clone(),
            };
            let count = counts
                .and_then(|v| v.rule_matches.get(&rule.field))
                .map_or_else(|| "–".to_owned(), |ids| format!("{} tags", ids.len()));
            let mut line = row![
                button(swatch)
                    .padding(2)
                    .style(theme::menu_row(self.editing == Some(i)))
                    .on_press(m(TagColoursMsg::Edit(i))),
                text(label).size(14).width(Length::Fill),
                text(count)
                    .size(12)
                    .color(theme::MUTED)
                    .width(Length::Fixed(80.0)),
                button(text("↑").size(13))
                    .style(button::text)
                    .on_press_maybe((i > 0).then(|| m(TagColoursMsg::MoveUp(i)))),
                button(text("↓").size(13))
                    .style(button::text)
                    .on_press_maybe((i + 1 < rules.len()).then(|| m(TagColoursMsg::MoveDown(i)))),
            ]
            .spacing(8)
            .align_y(Alignment::Center);
            line = line.push(
                button(text("Remove").size(12))
                    .style(button::text)
                    .on_press_maybe((!is_semantic(rule)).then(|| m(TagColoursMsg::Remove(i)))),
            );
            list = list.push(
                container(line)
                    .padding([2, 6])
                    .style(theme::row_highlight(i == self.highlight)),
            );
            if self.editing == Some(i) {
                let mut swatches = row![].spacing(4);
                for c in SWATCHES {
                    let fill = Color::from_rgb8(c.0, c.1, c.2);
                    swatches = swatches.push(
                        button(Space::new().width(18).height(18))
                            .padding(0)
                            .style(move |_, _| button::Style {
                                background: Some(fill.into()),
                                border: iced::Border {
                                    radius: 3.0.into(),
                                    width: if c == rule.colour { 2.0 } else { 0.0 },
                                    color: theme::TEXT,
                                },
                                ..button::Style::default()
                            })
                            .on_press(m(TagColoursMsg::SetColour(i, c))),
                    );
                }
                list = list.push(
                    column![
                        swatches.wrap(),
                        row![
                            text_input("#rrggbb", &self.hex)
                                .id(HEX_ID)
                                .on_input(move |t| m(TagColoursMsg::Hex(t)))
                                .on_submit(m(TagColoursMsg::ApplyHex))
                                .size(13)
                                .width(Length::Fixed(110.0)),
                            button(text("Use").size(12)).on_press(m(TagColoursMsg::ApplyHex)),
                        ]
                        .spacing(6)
                        .align_y(Alignment::Center),
                    ]
                    .spacing(6)
                    .padding([4, 34]),
                );
            }
        }
        let add = row![
            text_input("Another tag flag (custom field name)", &self.new_field)
                .id(FIELD_ID)
                .on_input(move |t| m(TagColoursMsg::NewField(t)))
                .on_submit(m(TagColoursMsg::AddRule))
                .size(13)
                .width(Length::Fixed(300.0)),
            button(text("Add rule").size(13)).on_press(m(TagColoursMsg::AddRule)),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        let status: Element<'a, ShellMsg> = match &self.saving {
            Saving::Saving => text("Saving…").size(13).color(theme::MUTED).into(),
            Saving::Saved => text("Saved to the server.")
                .size(13)
                .color(theme::ICY_AQUA)
                .into(),
            Saving::Failed(e) => text(format!("The colours weren't saved: {e}"))
                .size(13)
                .color(theme::ROSEWOOD)
                .into(),
            Saving::Idle if self.dirty => {
                text("Not saved yet.").size(13).color(theme::MUTED).into()
            }
            Saving::Idle => Space::new().into(),
        };
        let mut col = column![
            text("Tag colours").size(22),
            text(
                "Tags take the colour of the first rule, top to bottom, whose flag they have set. \
                 Saved on the server, so other machines and the semantic-tagging plugin share them."
            )
            .size(13)
            .color(theme::MUTED),
            scrollable(list).height(Length::Shrink),
            add,
        ]
        .spacing(12);
        if let Some(e) = &self.error {
            col = col.push(text(e.clone()).size(13).color(theme::ROSEWOOD));
        }
        col.push(
            row![
                button(text("Save").size(14))
                    .padding([6, 16])
                    .on_press_maybe(
                        (self.dirty && self.saving != Saving::Saving)
                            .then(|| m(TagColoursMsg::Save))
                    ),
                button(text("Reset to defaults").size(14))
                    .padding([6, 16])
                    .style(button::secondary)
                    .on_press(m(TagColoursMsg::Reset)),
                status,
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .into()
    }
}
