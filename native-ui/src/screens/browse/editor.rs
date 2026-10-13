//! The criterion editor's state (008 FR-002, FR-003, FR-003a): one criterion being added or
//! changed, its value held as the form's fields until it's applied. Building checks it with the
//! core's validation, so only a usable criterion reaches the query.

use stash_core::adapter::choices::ChoiceKind;
use stash_core::scenes::filter::kinds::{CriterionKind, ValueType};
use stash_core::scenes::filter::{
    Criterion, CriterionValue, Duplication, Item, Modifier, Orientation, Resolution,
};

/// Which duplicate kind a toggle changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DupField {
    Phash,
    Url,
    StashId,
    Title,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Editor {
    pub kind: CriterionKind,
    pub modifier: Modifier,
    /// Text, a number, a date, a caption code, a phash, a StashID, or a custom field's value.
    pub text: String,
    /// A range's second value, or a phash distance.
    pub text2: String,
    pub flag: bool,
    pub resolution: Resolution,
    pub orientation: Vec<Orientation>,
    pub items: Vec<Item>,
    pub excluded: Vec<Item>,
    /// Include every sub-level (tags, studios, groups).
    pub depth_all: bool,
    /// A StashID's endpoint, or a custom field's name.
    pub name: String,
    pub duplication: Duplication,
    /// The picker's search text, highlighted row, and scroll.
    pub picker_query: String,
    pub picker_highlight: usize,
    pub picker_scroll: f32,
    pub error: Option<String>,
    /// Editing an existing chip: its sort key (it's replaced in place).
    pub replacing: Option<String>,
}

#[derive(Debug, Clone)]
pub enum EditorMsg {
    Modifier(Modifier),
    Text(String),
    Text2(String),
    Name(String),
    Flag(bool),
    Resolution(Resolution),
    ToggleOrientation(Orientation),
    Duplication(DupField, bool),
    DepthAll(bool),
    PickerQuery(String),
    PickerMove(i32),
    PickerScrolled(f32),
    /// Include an item (or, if it's already chosen, take it out).
    Toggle(Item),
    /// Exclude an item (Includes / Includes all only).
    Exclude(Item),
    Remove(String),
    Apply,
    Cancel,
}

/// The choice list a kind picks from, if it's a picker kind.
pub fn choice_kind(kind: CriterionKind) -> Option<ChoiceKind> {
    match kind {
        CriterionKind::Performers => Some(ChoiceKind::Performers),
        CriterionKind::Tags | CriterionKind::PerformerTags => Some(ChoiceKind::Tags),
        CriterionKind::Studios => Some(ChoiceKind::Studios),
        CriterionKind::Groups => Some(ChoiceKind::Groups),
        CriterionKind::Galleries => Some(ChoiceKind::Galleries),
        CriterionKind::Folder => Some(ChoiceKind::Folders),
        _ => None,
    }
}

impl Editor {
    /// A new criterion of `kind`, with the web UI's defaults.
    pub fn new(kind: CriterionKind) -> Self {
        let info = kind.info();
        Self {
            kind,
            modifier: info.default_modifier(),
            text: if let ValueType::Choice(options) = info.value_type {
                options[0].to_owned()
            } else {
                String::new()
            },
            text2: String::new(),
            flag: true,
            resolution: Resolution::FullHd,
            orientation: Vec::new(),
            items: Vec::new(),
            excluded: Vec::new(),
            depth_all: false,
            name: String::new(),
            duplication: Duplication {
                phash: true,
                ..Duplication::default()
            },
            picker_query: String::new(),
            picker_highlight: 0,
            picker_scroll: 0.0,
            error: None,
            replacing: None,
        }
    }

    /// The editor for an existing criterion (a chip being changed).
    pub fn from_criterion(c: &Criterion) -> Self {
        let mut e = Self::new(c.kind);
        e.modifier = c.modifier;
        e.replacing = Some(c.sort_key());
        match &c.value {
            CriterionValue::None => {}
            CriterionValue::Text(t) => e.text = t.clone(),
            CriterionValue::Int(a, b) => {
                e.text = a.to_string();
                e.text2 = b.map(|b| b.to_string()).unwrap_or_default();
            }
            CriterionValue::Date(a, b) => {
                e.text = a.clone();
                e.text2 = b.clone().unwrap_or_default();
            }
            CriterionValue::Bool(b) => e.flag = *b,
            CriterionValue::Resolution(r) => e.resolution = *r,
            CriterionValue::Orientation(o) => e.orientation = o.clone(),
            CriterionValue::Items {
                items,
                excluded,
                depth,
            } => {
                e.items = items.clone();
                e.excluded = excluded.clone();
                e.depth_all = depth.is_some_and(|d| d != 0);
            }
            CriterionValue::StashId { endpoint, id } => {
                e.name = endpoint.clone();
                e.text = id.clone();
            }
            CriterionValue::Phash { hash, distance } => {
                e.text = hash.clone();
                e.text2 = distance.map(|d| d.to_string()).unwrap_or_default();
            }
            CriterionValue::Duplicated(d) => e.duplication = *d,
            CriterionValue::CustomField { field, values } => {
                e.name = field.clone();
                let text =
                    |v: &serde_json::Value| v.as_str().map_or_else(|| v.to_string(), str::to_owned);
                e.text = values.first().map(text).unwrap_or_default();
                e.text2 = values.get(1).map(text).unwrap_or_default();
            }
        }
        e
    }

    pub fn picker(&self) -> Option<ChoiceKind> {
        choice_kind(self.kind)
    }

    pub fn update(&mut self, msg: EditorMsg) {
        self.error = None;
        match msg {
            EditorMsg::Modifier(m) => {
                self.modifier = m;
                if m != Modifier::Includes && m != Modifier::IncludesAll {
                    self.excluded.clear();
                }
            }
            EditorMsg::Text(t) => self.text = t,
            EditorMsg::Text2(t) => self.text2 = t,
            EditorMsg::Name(t) => self.name = t,
            EditorMsg::Flag(b) => self.flag = b,
            EditorMsg::Resolution(r) => self.resolution = r,
            EditorMsg::ToggleOrientation(o) => {
                if let Some(i) = self.orientation.iter().position(|x| *x == o) {
                    self.orientation.remove(i);
                } else {
                    self.orientation.push(o);
                }
            }
            EditorMsg::Duplication(field, on) => match field {
                DupField::Phash => self.duplication.phash = on,
                DupField::Url => self.duplication.url = on,
                DupField::StashId => self.duplication.stash_id = on,
                DupField::Title => self.duplication.title = on,
            },
            EditorMsg::DepthAll(b) => self.depth_all = b,
            EditorMsg::PickerQuery(q) => {
                self.picker_query = q;
                self.picker_highlight = 0;
                self.picker_scroll = 0.0;
            }
            EditorMsg::PickerMove(delta) => {
                self.picker_highlight = self.picker_highlight.saturating_add_signed(delta as isize);
            }
            EditorMsg::PickerScrolled(y) => self.picker_scroll = y,
            EditorMsg::Toggle(item) => {
                self.excluded.retain(|x| x.id != item.id);
                if let Some(i) = self.items.iter().position(|x| x.id == item.id) {
                    self.items.remove(i);
                } else {
                    self.items.push(item);
                }
            }
            EditorMsg::Exclude(item) => {
                if matches!(self.modifier, Modifier::Includes | Modifier::IncludesAll) {
                    self.items.retain(|x| x.id != item.id);
                    if !self.excluded.iter().any(|x| x.id == item.id) {
                        self.excluded.push(item);
                    }
                }
            }
            EditorMsg::Remove(id) => {
                self.items.retain(|x| x.id != id);
                self.excluded.retain(|x| x.id != id);
            }
            EditorMsg::Apply | EditorMsg::Cancel => {}
        }
    }

    /// The criterion, or why it can't be used yet.
    pub fn build(&self) -> Result<Criterion, String> {
        let info = self.kind.info();
        let value = if self.modifier.is_null_check() {
            match info.value_type {
                ValueType::CustomField => CriterionValue::CustomField {
                    field: self.name.trim().to_owned(),
                    values: Vec::new(),
                },
                _ => CriterionValue::None,
            }
        } else {
            let range = self.modifier.is_range();
            let int = |t: &str, what: &str| -> Result<i64, String> {
                t.trim()
                    .parse::<i64>()
                    .map_err(|_| format!("{what} must be a whole number"))
            };
            match info.value_type {
                ValueType::Text { .. } | ValueType::Captions => {
                    CriterionValue::Text(self.text.trim().to_owned())
                }
                ValueType::Number { .. } => CriterionValue::Int(
                    int(&self.text, "The value")?,
                    if range {
                        Some(int(&self.text2, "The second value")?)
                    } else {
                        None
                    },
                ),
                ValueType::Duration { .. } => CriterionValue::Int(
                    parse_duration(&self.text)?,
                    if range {
                        Some(parse_duration(&self.text2)?)
                    } else {
                        None
                    },
                ),
                ValueType::Date | ValueType::Timestamp { .. } => CriterionValue::Date(
                    self.text.trim().to_owned(),
                    range.then(|| self.text2.trim().to_owned()),
                ),
                ValueType::Bool | ValueType::StringBool => CriterionValue::Bool(self.flag),
                ValueType::Choice(_) => CriterionValue::Text(self.text.clone()),
                ValueType::Resolution => CriterionValue::Resolution(self.resolution),
                ValueType::Orientation => CriterionValue::Orientation(self.orientation.clone()),
                ValueType::Items { hierarchical, .. } => CriterionValue::Items {
                    items: self.items.clone(),
                    excluded: self.excluded.clone(),
                    depth: hierarchical.then_some(if self.depth_all { -1 } else { 0 }),
                },
                ValueType::StashId => CriterionValue::StashId {
                    endpoint: self.name.trim().to_owned(),
                    id: self.text.trim().to_owned(),
                },
                ValueType::Phash => CriterionValue::Phash {
                    hash: self.text.trim().to_owned(),
                    distance: if self.text2.trim().is_empty() {
                        None
                    } else {
                        Some(int(&self.text2, "The distance")? as i32)
                    },
                },
                ValueType::Duplicated => CriterionValue::Duplicated(self.duplication),
                ValueType::CustomField => {
                    let value = |t: &str| -> serde_json::Value {
                        let t = t.trim();
                        t.parse::<i64>()
                            .map(serde_json::Value::from)
                            .unwrap_or_else(|_| serde_json::Value::from(t))
                    };
                    let mut values = vec![value(&self.text)];
                    if range {
                        values.push(value(&self.text2));
                    }
                    CriterionValue::CustomField {
                        field: self.name.trim().to_owned(),
                        values,
                    }
                }
                ValueType::Semantic => {
                    return Err("Semantic filters arrive with the semantic engine".into())
                }
            }
        };
        let c = Criterion::new(self.kind, self.modifier, value);
        match c.validate() {
            Ok(()) => Ok(c),
            Err(_) => Err(match info.value_type {
                ValueType::Items { .. } => "Choose at least one".into(),
                _ if self.modifier.is_range() => "Fill in both values".into(),
                _ => "Fill in a value".into(),
            }),
        }
    }
}

/// `90`, `1:30`, or `1:02:03` as seconds.
pub fn parse_duration(text: &str) -> Result<i64, String> {
    let mut total: i64 = 0;
    let parts: Vec<&str> = text.trim().split(':').collect();
    if parts.is_empty() || parts.len() > 3 || parts.iter().any(|p| p.trim().is_empty()) {
        return Err("Durations look like 90, 1:30, or 1:02:03".into());
    }
    for p in parts {
        let n: i64 = p
            .trim()
            .parse()
            .map_err(|_| "Durations look like 90, 1:30, or 1:02:03".to_owned())?;
        total = total * 60 + n;
    }
    Ok(total)
}

/// A criterion as a chip's text: `Tags: A, B (+ all)` / `Duration > 10:00`.
pub fn chip_label(c: &Criterion) -> String {
    let info = c.info();
    let value = match &c.value {
        CriterionValue::None if info.is_placeholder() => "not yet available".to_owned(),
        CriterionValue::None => String::new(),
        CriterionValue::Text(t) => format!("“{t}”"),
        CriterionValue::Int(a, b) => {
            let show = |n: i64| match info.value_type {
                ValueType::Duration { .. } => crate::screens::scenes::grid::duration(n as f64),
                _ => n.to_string(),
            };
            match b {
                Some(b) if c.modifier.is_range() => format!("{} and {}", show(*a), show(*b)),
                _ => show(*a),
            }
        }
        CriterionValue::Date(a, b) => match b {
            Some(b) if c.modifier.is_range() => format!("{a} and {b}"),
            _ => a.clone(),
        },
        CriterionValue::Bool(b) => if *b { "yes" } else { "no" }.to_owned(),
        CriterionValue::Resolution(r) => r.name().to_owned(),
        CriterionValue::Orientation(o) => o
            .iter()
            .map(|o| format!("{o:?}").to_lowercase())
            .collect::<Vec<_>>()
            .join(", "),
        CriterionValue::Items {
            items,
            excluded,
            depth,
        } => {
            let names = |list: &[Item]| {
                list.iter()
                    .map(|i| {
                        if i.label.is_empty() {
                            format!("#{}", i.id)
                        } else {
                            i.label.clone()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let mut out = names(items);
            if !excluded.is_empty() {
                if !out.is_empty() {
                    out.push_str("; ");
                }
                out.push_str(&format!("not {}", names(excluded)));
            }
            if depth.is_some_and(|d| d != 0) {
                out.push_str(" (+ sub-levels)");
            }
            out
        }
        CriterionValue::StashId { endpoint, id } => {
            if endpoint.is_empty() {
                id.clone()
            } else {
                format!("{id} ({endpoint})")
            }
        }
        CriterionValue::Phash { hash, distance } => match distance {
            Some(d) => format!("{hash} (±{d})"),
            None => hash.clone(),
        },
        CriterionValue::Duplicated(d) => [
            (d.phash, "phash"),
            (d.url, "URL"),
            (d.stash_id, "StashID"),
            (d.title, "title"),
        ]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, n)| *n)
        .collect::<Vec<_>>()
        .join(", "),
        CriterionValue::CustomField { field, values } => {
            let shown: Vec<String> = values
                .iter()
                .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_owned))
                .collect();
            if shown.is_empty() {
                field.clone()
            } else {
                format!("{field} {}", shown.join(" and "))
            }
        }
    };
    let modifier = if info.modifiers().is_empty() || info.is_placeholder() {
        String::new()
    } else {
        format!(" {}", c.modifier.label())
    };
    if value.is_empty() {
        format!("{}{modifier}", info.label)
    } else {
        format!("{}{modifier} {value}", info.label)
    }
}
