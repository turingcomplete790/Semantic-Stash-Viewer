//! Saved scene filters in the web UI's format (spec FR-004, FR-005; research R3).
//!
//! Stash stores what the web UI writes: `find_filter` (search, sort, page size), `object_filter`
//! in the *saved criterion* format (`applyToSavedCriterion`: multi-value criteria carry
//! `{items: [{id, label}], excluded, depth}`, ranges `{value, value2}`), and `ui_options` (display
//! mode). This decodes that into the filter model. Whatever the viewer can't apply exactly is
//! named in `left_out`, never silently dropped.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::kinds::{CriterionKind, ValueType};
use super::{Criterion, CriterionValue, Duplication, Item, Modifier, Orientation, Resolution};
use crate::scenes::query::{SceneQuery, SceneSort, SortDirection};

/// The web UI's display modes (`ui_options.display_mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisplayMode {
    Grid,
    List,
    Wall,
    Tagger,
}

/// A saved scene filter, decoded. Read-only here (saving is Phase 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedFilter {
    pub id: String,
    pub name: String,
    pub query: SceneQuery,
    pub page_size: Option<u32>,
    pub mode: Option<DisplayMode>,
    /// What the viewer couldn't apply (criterion keys, an unknown sort), for the notice.
    pub left_out: Vec<String>,
}

/// Decode one saved filter from its three stored parts.
pub fn decode_saved_filter(
    id: &str,
    name: &str,
    find_filter: Option<&Value>,
    object_filter: Option<&Value>,
    ui_options: Option<&Value>,
) -> SavedFilter {
    let mut left_out = Vec::new();
    let mut query = SceneQuery::default();
    let mut page_size = None;
    if let Some(find) = find_filter {
        if let Some(q) = find.get("q").and_then(Value::as_str) {
            query.search = q.trim().to_owned();
        }
        if let Some(sort) = find.get("sort").and_then(Value::as_str) {
            match parse_sort(sort) {
                Some((s, seed)) => {
                    query.sort = s;
                    query.seed = seed;
                }
                None => left_out.push(format!("sort: {sort}")),
            }
        }
        if let Some(d) = find.get("direction").and_then(Value::as_str) {
            query.direction = if d.eq_ignore_ascii_case("asc") {
                SortDirection::Asc
            } else {
                SortDirection::Desc
            };
        }
        page_size = find
            .get("per_page")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n > 0);
    }
    if let Some(object) = object_filter {
        let (criteria, skipped) = decode_criteria(object);
        query.criteria = criteria;
        left_out.extend(skipped);
    }
    let mode = ui_options
        .and_then(|u| u.get("display_mode"))
        .and_then(Value::as_u64)
        .and_then(|m| match m {
            0 => Some(DisplayMode::Grid),
            1 => Some(DisplayMode::List),
            2 => Some(DisplayMode::Wall),
            3 => Some(DisplayMode::Tagger),
            _ => None,
        });
    SavedFilter {
        id: id.to_owned(),
        name: name.to_owned(),
        query,
        page_size,
        mode,
        left_out,
    }
}

/// The web UI's default scene filter (`defaultFilters.scenes`, the same shape plus `mode`).
pub fn decode_default_filter(value: &Value) -> SavedFilter {
    decode_saved_filter(
        "default",
        "Default",
        value.get("find_filter"),
        value.get("object_filter"),
        value.get("ui_options"),
    )
}

fn parse_sort(sort: &str) -> Option<(SceneSort, Option<u32>)> {
    if let Some(seed) = sort.strip_prefix("random_") {
        // Stash seeds are u64; the viewer keeps 32 bits (any seed works).
        let seed = seed.parse::<u64>().ok()?;
        return Some((SceneSort::Random, Some((seed % u64::from(u32::MAX)) as u32)));
    }
    if sort == "random" {
        return Some((SceneSort::Random, Some(1)));
    }
    serde_json::from_value(Value::String(sort.to_owned()))
        .ok()
        .map(|s| (s, None))
}

/// The criteria in an `object_filter`, and the keys left out (unknown, or a value the viewer
/// can't read).
pub fn decode_criteria(object: &Value) -> (Vec<Criterion>, Vec<String>) {
    let mut criteria = Vec::new();
    let mut left_out = Vec::new();
    let Some(map) = object.as_object() else {
        return (criteria, left_out);
    };
    for (key, value) in map {
        let Some(kind) = CriterionKind::from_key(key) else {
            left_out.push(key.clone());
            continue;
        };
        if kind == CriterionKind::CustomField {
            match custom_fields(value) {
                Some(found) => criteria.extend(found),
                None => left_out.push(key.clone()),
            }
            continue;
        }
        match decode_one(kind, value) {
            Some(c) => criteria.push(c),
            None => left_out.push(key.clone()),
        }
    }
    (criteria, left_out)
}

fn decode_one(kind: CriterionKind, saved: &Value) -> Option<Criterion> {
    let info = kind.info();
    if info.is_placeholder() {
        // Kept as it came, for Phase 4.
        return Some(Criterion::new(kind, Modifier::Equals, CriterionValue::None));
    }
    let mut modifier = saved
        .get("modifier")
        .and_then(Value::as_str)
        .and_then(Modifier::from_stash_name)
        .unwrap_or(Modifier::Equals);
    let raw = saved.get("value");
    if modifier.is_null_check() {
        let c = Criterion::new(kind, modifier, CriterionValue::None);
        return c.validate().is_ok().then_some(c);
    }
    let value = match info.value_type {
        ValueType::Text { .. } | ValueType::Captions => {
            CriterionValue::Text(raw?.as_str()?.to_owned())
        }
        ValueType::Number { .. } | ValueType::Duration { .. } => {
            let (a, b) = range(raw?, saved)?;
            CriterionValue::Int(number(&a)?, b.as_ref().and_then(number))
        }
        ValueType::Date | ValueType::Timestamp { .. } => {
            let (a, b) = range(raw?, saved)?;
            CriterionValue::Date(
                a.as_str()?.to_owned(),
                b.as_ref().and_then(Value::as_str).map(str::to_owned),
            )
        }
        ValueType::Bool | ValueType::StringBool => {
            modifier = Modifier::Equals;
            CriterionValue::Bool(match raw? {
                Value::Bool(b) => *b,
                Value::String(s) => s == "true",
                _ => return None,
            })
        }
        ValueType::Choice(options) => {
            modifier = Modifier::Equals;
            let t = raw?.as_str()?;
            options
                .contains(&t)
                .then(|| CriterionValue::Text(t.to_owned()))?
        }
        ValueType::Resolution => CriterionValue::Resolution(Resolution::from_name(raw?.as_str()?)?),
        ValueType::Orientation => {
            modifier = Modifier::Equals;
            let list = raw?.as_array()?;
            CriterionValue::Orientation(
                list.iter()
                    .filter_map(|v| v.as_str().and_then(Orientation::from_name))
                    .collect(),
            )
        }
        ValueType::Items { modifiers, .. } => {
            let (items, excluded, depth) = items(raw?)?;
            // An old "excludes" where the kind no longer offers it: the web UI rewrites it as
            // "includes" with those items excluded.
            if modifier == Modifier::Excludes && !modifiers.contains(&Modifier::Excludes) {
                modifier = Modifier::Includes;
                let mut all = excluded;
                all.extend(items);
                CriterionValue::Items {
                    items: Vec::new(),
                    excluded: all,
                    depth,
                }
            } else {
                CriterionValue::Items {
                    items,
                    excluded,
                    depth,
                }
            }
        }
        ValueType::StashId => match raw? {
            Value::String(s) => CriterionValue::StashId {
                endpoint: String::new(),
                id: s.clone(),
            },
            v => CriterionValue::StashId {
                endpoint: v
                    .get("endpoint")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                id: v
                    .get("stashID")
                    .or_else(|| v.get("stash_id"))
                    .and_then(Value::as_str)?
                    .to_owned(),
            },
        },
        ValueType::Phash => {
            let v = raw?;
            CriterionValue::Phash {
                hash: v.get("value").and_then(Value::as_str)?.to_owned(),
                distance: v
                    .get("distance")
                    .and_then(Value::as_i64)
                    .and_then(|d| i32::try_from(d).ok()),
            }
        }
        ValueType::Duplicated => {
            modifier = Modifier::Equals;
            let v = raw?;
            let flag = |k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
            CriterionValue::Duplicated(Duplication {
                // `duplicated` is the old name for phash duplicates.
                phash: flag("phash") || flag("duplicated"),
                url: flag("url"),
                stash_id: flag("stash_id"),
                title: flag("title"),
            })
        }
        ValueType::CustomField | ValueType::Semantic => return None,
    };
    let c = Criterion::new(kind, modifier, value);
    c.validate().is_ok().then_some(c)
}

/// `{value, value2}`, or the old shapes: a bare value with `value2` beside it.
fn range(raw: &Value, saved: &Value) -> Option<(Value, Option<Value>)> {
    match raw {
        Value::Object(o) => Some((
            o.get("value")?.clone(),
            o.get("value2").filter(|v| !v.is_null()).cloned(),
        )),
        v => Some((
            v.clone(),
            saved.get("value2").filter(|v| !v.is_null()).cloned(),
        )),
    }
}

fn number(v: &Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_f64().map(|f| f.round() as i64))
        .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
}

fn item(v: &Value) -> Option<Item> {
    Some(Item {
        id: match v.get("id")? {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => return None,
        },
        label: v
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
    })
}

/// `{items, excluded, depth}`, or a bare list of `{id, label}` (flat criteria such as galleries).
fn items(raw: &Value) -> Option<(Vec<Item>, Vec<Item>, Option<i32>)> {
    let list = |v: Option<&Value>| -> Vec<Item> {
        v.and_then(Value::as_array)
            .map(|a| a.iter().filter_map(item).collect())
            .unwrap_or_default()
    };
    match raw {
        Value::Array(_) => Some((list(Some(raw)), Vec::new(), None)),
        Value::Object(o) => Some((
            list(o.get("items")),
            list(o.get("excluded")),
            o.get("depth")
                .and_then(Value::as_i64)
                .and_then(|d| i32::try_from(d).ok()),
        )),
        _ => None,
    }
}

fn custom_fields(raw: &Value) -> Option<Vec<Criterion>> {
    let list = raw.as_array()?;
    let mut out = Vec::new();
    for entry in list {
        let field = entry.get("field").and_then(Value::as_str)?.to_owned();
        let modifier = entry
            .get("modifier")
            .and_then(Value::as_str)
            .and_then(Modifier::from_stash_name)?;
        let values = match entry.get("value") {
            Some(Value::Array(a)) => a.clone(),
            Some(Value::Null) | None => Vec::new(),
            Some(v) => vec![v.clone()],
        };
        let c = Criterion::new(
            CriterionKind::CustomField,
            modifier,
            CriterionValue::CustomField { field, values },
        );
        if c.validate().is_err() {
            return None;
        }
        out.push(c);
    }
    Some(out)
}
