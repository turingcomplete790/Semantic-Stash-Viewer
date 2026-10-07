//! Semantic tag types, flag parsing, and colour rules (spec FR-003c to FR-003e;
//! contracts/tag-colour.md).
//!
//! Colours belong to tag flags, not tags: a rule pairs a tag custom field (`isRole`) with a
//! colour, and a tag takes the colour of the first rule whose flag it has set. The rules live in
//! Stash's UI configuration under `semanticStashViewer.tagColours`.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Where the colour rules live in Stash's UI configuration (a nested key).
pub const TAG_COLOURS_KEY: &str = "semanticStashViewer.tagColours";

/// The stored format's version.
pub const TAG_COLOURS_VERSION: u32 = 1;

/// A colour as `#rrggbb`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// Parse `#rrggbb` (case-insensitive). Anything else is `None`.
    pub fn parse(text: &str) -> Option<Self> {
        let hex = text.strip_prefix('#')?;
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        Some(Self(byte(0)?, byte(2)?, byte(4)?))
    }
}

impl fmt::Display for Rgb {
    /// Lower case `#rrggbb`, the written form.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

impl Serialize for Rgb {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Rgb::parse(&text).ok_or_else(|| serde::de::Error::custom("expected #rrggbb"))
    }
}

/// One colour rule: tags with `field` set are drawn in `colour`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColourRule {
    pub field: String,
    pub colour: Rgb,
}

/// Whether `field` is a valid tag custom field key for a rule: letters, digits, and `_`,
/// 1–64 characters.
pub fn valid_rule_field(field: &str) -> bool {
    (1..=64).contains(&field.len()) && field.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The default rules, used when the key is absent or unreadable: one per semantic type, in the
/// plugin's current type priority (highest first), with RPG palette colours.
pub fn default_colour_rules() -> Vec<ColourRule> {
    [
        ("isLabel", Rgb(0x5b, 0x8f, 0xd4)),
        ("isAction", Rgb(0xd4, 0x40, 0x40)),
        ("isRole", Rgb(0x88, 0x40, 0xc0)),
        ("isTrait", Rgb(0x3a, 0x9e, 0x50)),
        ("plugtag", Rgb(0xc8, 0x95, 0x2a)),
        ("isMiseEnScene", Rgb(0xb0, 0x88, 0x58)),
        ("isVariation", Rgb(0x7d, 0xd8, 0x7a)),
    ]
    .into_iter()
    .map(|(field, colour)| ColourRule {
        field: field.to_owned(),
        colour,
    })
    .collect()
}

/// The colour rules stored under [`TAG_COLOURS_KEY`] (`{ version, rules: [{field, colour}] }`).
///
/// A missing or unreadable value gives the defaults. Invalid rules are skipped; their field names
/// (never colours or tag data) come back so the caller can log them once.
pub fn parse_colour_rules(value: Option<&serde_json::Value>) -> (Vec<ColourRule>, Vec<String>) {
    let Some(rules) = value
        .and_then(|v| v.get("rules"))
        .and_then(serde_json::Value::as_array)
    else {
        return (default_colour_rules(), Vec::new());
    };
    let mut parsed = Vec::new();
    let mut skipped = Vec::new();
    for rule in rules {
        let field = rule.get("field").and_then(serde_json::Value::as_str);
        let colour = rule
            .get("colour")
            .and_then(serde_json::Value::as_str)
            .and_then(Rgb::parse);
        match (field, colour) {
            (Some(field), Some(colour)) if valid_rule_field(field) => parsed.push(ColourRule {
                field: field.to_owned(),
                colour,
            }),
            (field, _) => skipped.push(field.unwrap_or("(no field)").to_owned()),
        }
    }
    (parsed, skipped)
}

// ---- The semantic vocabulary ------------------------------------------------------------------

/// The plugin's semantic tag types, each marked on a tag by its own custom-field flag
/// (design documents, roadmap Milestone 1D §2/§4), in the plugin's current priority (highest
/// first): a tag flagged with several types takes the first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SemanticKind {
    Label,
    Action,
    Role,
    Trait,
    Pluggable,
    MiseEnScene,
    Variation,
}

impl SemanticKind {
    /// In priority order, highest first.
    pub const ALL: [SemanticKind; 7] = [
        SemanticKind::Label,
        SemanticKind::Action,
        SemanticKind::Role,
        SemanticKind::Trait,
        SemanticKind::Pluggable,
        SemanticKind::MiseEnScene,
        SemanticKind::Variation,
    ];

    /// The tag custom field that marks this type.
    pub fn flag(self) -> &'static str {
        match self {
            SemanticKind::Label => "isLabel",
            SemanticKind::Action => "isAction",
            SemanticKind::Role => "isRole",
            SemanticKind::Trait => "isTrait",
            SemanticKind::Pluggable => "plugtag",
            SemanticKind::MiseEnScene => "isMiseEnScene",
            SemanticKind::Variation => "isVariation",
        }
    }

    pub fn from_flag(flag: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.flag() == flag)
    }

    pub fn label(self) -> &'static str {
        match self {
            SemanticKind::Label => "Label",
            SemanticKind::Action => "Action",
            SemanticKind::Role => "Role",
            SemanticKind::Trait => "Trait",
            SemanticKind::Pluggable => "Pluggable",
            SemanticKind::MiseEnScene => "Mise-en-scène",
            SemanticKind::Variation => "Variation",
        }
    }
}

/// The custom field linking a common variant to its canonical tag (roadmap Milestone 1C).
pub const VARIANT_OF_FIELD: &str = "variantOfTagId";

/// Whether a flag value counts as set: `1`, `"1"`, `true`, or `"true"` (flags are 0/1, but
/// Stash may hand back either form). Anything else, including 0, isn't set.
pub fn flag_is_set(value: Option<&serde_json::Value>) -> bool {
    match value {
        Some(serde_json::Value::Number(n)) => n.as_i64() == Some(1) || n.as_f64() == Some(1.0),
        Some(serde_json::Value::String(s)) => s == "1" || s == "true",
        Some(serde_json::Value::Bool(b)) => *b,
        _ => false,
    }
}

/// A tag as the vocabulary keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VocabularyTag {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
}

/// The semantic vocabulary of one server (spec FR-003c to FR-003e; data-model
/// "SemanticVocabulary"): semantic tags by type, common variants, and which tags each colour rule
/// matches. Built from one `SemanticVocabulary` request and cached per server.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SemanticVocabulary {
    /// Tags with a type flag set, with their type.
    pub tags: std::collections::BTreeMap<String, (VocabularyTag, SemanticKind)>,
    /// Common variants: the tag and its canonical tag's id.
    pub variants: std::collections::BTreeMap<String, (VocabularyTag, String)>,
    /// Tags the flag queries returned but whose flags aren't set (for example 0): ordinary tags,
    /// listed so the picker can show them with the ordinary ones (the ordinary list's "every
    /// flag is null" filter leaves them out).
    pub unflagged: Vec<VocabularyTag>,
    /// For each colour rule's field, the ids of the tags with it set.
    pub rule_matches: std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
}

impl SemanticVocabulary {
    /// A tag's semantic type: its own, else its canonical tag's (a common variant), else none.
    pub fn kind_of(&self, id: &str) -> Option<SemanticKind> {
        if let Some((_, kind)) = self.tags.get(id) {
            return Some(*kind);
        }
        let (_, canonical) = self.variants.get(id)?;
        self.tags.get(canonical).map(|(_, kind)| *kind)
    }

    /// A tag's colour under `rules`: the first rule (in order) whose flag the tag has set; else,
    /// for a common variant, its canonical tag's; else none (contracts/tag-colour.md).
    pub fn colour_of(&self, id: &str, rules: &[ColourRule]) -> Option<Rgb> {
        let own = |id: &str| {
            rules.iter().find_map(|r| {
                self.rule_matches
                    .get(&r.field)
                    .is_some_and(|ids| ids.contains(id))
                    .then_some(r.colour)
            })
        };
        own(id).or_else(|| {
            let (_, canonical) = self.variants.get(id)?;
            own(canonical)
        })
    }

    /// Tags that arrived with the vocabulary but belong with ordinary tags in the picker: flags
    /// not set, and common variants whose canonical tag has no type.
    pub fn ordinary_extras(&self) -> Vec<&VocabularyTag> {
        let mut out: Vec<&VocabularyTag> = self.unflagged.iter().collect();
        out.extend(
            self.variants
                .iter()
                .filter(|(id, _)| self.kind_of(id).is_none() && !self.tags.contains_key(*id))
                .map(|(_, (tag, _))| tag),
        );
        out
    }

    /// The vocabulary's tags of one type, sorted by name.
    pub fn of_kind(&self, kind: SemanticKind) -> Vec<&VocabularyTag> {
        let mut tags: Vec<&VocabularyTag> = self
            .tags
            .values()
            .filter(|(_, k)| *k == kind)
            .map(|(t, _)| t)
            .collect();
        tags.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        tags
    }
}

/// The extra colour-rule fields: those that aren't semantic flags (which the vocabulary queries
/// anyway), deduplicated, in rule order.
pub fn extra_rule_fields(rules: &[ColourRule]) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    for r in rules {
        let f = r.field.as_str();
        if SemanticKind::from_flag(f).is_none() && f != VARIANT_OF_FIELD && !out.contains(&f) {
            out.push(f);
        }
    }
    out
}

/// The `SemanticVocabulary` request text for `rules`: one aliased "flag is not null" query per
/// type flag, one for common variants, and one per extra colour-rule field (research R12). Built
/// by hand because the aliases follow the rules; field names are checked identifiers
/// ([`valid_rule_field`]), so nothing else can reach the text.
pub fn vocabulary_query(rules: &[ColourRule]) -> String {
    let find = |alias: &str, field: &str, selection: &str| {
        format!(
            "  {alias}: findTags(filter: {{per_page: -1}}, tag_filter: {{custom_fields: [{{field: \"{field}\", modifier: NOT_NULL}}]}}) {{\n    tags {{\n      {selection}\n    }}\n  }}\n"
        )
    };
    let full = "id\n      name\n      aliases\n      custom_fields";
    let mut q = String::from("query SemanticVocabulary {\n");
    for kind in SemanticKind::ALL {
        q.push_str(&find(kind.flag(), kind.flag(), full));
    }
    q.push_str(&find("variant", VARIANT_OF_FIELD, full));
    for (i, field) in extra_rule_fields(rules).into_iter().enumerate() {
        if valid_rule_field(field) {
            q.push_str(&find(
                &format!("rule_{i}"),
                field,
                "id\n      custom_fields",
            ));
        }
    }
    q.push_str("}\n");
    q
}

/// Build the vocabulary from the request's `data` (`{alias: {tags: [...]}}`) and the rules it
/// was asked with.
pub fn build_vocabulary(data: &serde_json::Value, rules: &[ColourRule]) -> SemanticVocabulary {
    use serde_json::Value;
    let tags_of = |alias: &str| -> Vec<Value> {
        data.get(alias)
            .and_then(|r| r.get("tags"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let tag = |t: &Value| VocabularyTag {
        id: t
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        name: t
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        aliases: t
            .get("aliases")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
    };
    let field = |t: &Value, f: &str| t.get("custom_fields").and_then(|c| c.get(f)).cloned();

    let mut v = SemanticVocabulary::default();
    let mut seen_unset = std::collections::BTreeMap::new();
    // Lowest priority first, so a higher-priority type overwrites on conflict.
    for kind in SemanticKind::ALL.iter().rev() {
        for t in tags_of(kind.flag()) {
            let vt = tag(&t);
            if vt.id.is_empty() {
                continue;
            }
            if flag_is_set(field(&t, kind.flag()).as_ref()) {
                v.rule_matches
                    .entry(kind.flag().to_owned())
                    .or_default()
                    .insert(vt.id.clone());
                v.tags.insert(vt.id.clone(), (vt, *kind));
            } else {
                seen_unset.insert(vt.id.clone(), vt);
            }
        }
    }
    for t in tags_of("variant") {
        let vt = tag(&t);
        let canonical = match field(&t, VARIANT_OF_FIELD) {
            Some(Value::String(s)) if !s.is_empty() => s,
            Some(Value::Number(n)) => n.to_string(),
            _ => continue,
        };
        if !vt.id.is_empty() && !v.tags.contains_key(&vt.id) {
            seen_unset.remove(&vt.id);
            v.variants.insert(vt.id.clone(), (vt, canonical));
        }
    }
    for (i, f) in extra_rule_fields(rules).into_iter().enumerate() {
        let ids = v.rule_matches.entry(f.to_owned()).or_default();
        for t in tags_of(&format!("rule_{i}")) {
            if flag_is_set(field(&t, f).as_ref()) {
                if let Some(id) = t.get("id").and_then(Value::as_str) {
                    ids.insert(id.to_owned());
                }
            }
        }
    }
    v.unflagged = seen_unset
        .into_values()
        .filter(|t| !v.tags.contains_key(&t.id))
        .collect();
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn colours_parse_case_insensitively_and_write_lower_case() {
        assert_eq!(Rgb::parse("#C8952A"), Some(Rgb(0xc8, 0x95, 0x2a)));
        assert_eq!(Rgb(0xc8, 0x95, 0x2a).to_string(), "#c8952a");
        for bad in ["red", "#abc", "c8952a", "#c8952g", ""] {
            assert_eq!(Rgb::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn rule_fields_are_short_identifiers() {
        assert!(valid_rule_field("isRole"));
        assert!(valid_rule_field("ssv_test_1"));
        assert!(!valid_rule_field(""));
        assert!(!valid_rule_field("is role"));
        assert!(!valid_rule_field("a.b"));
        assert!(!valid_rule_field(&"x".repeat(65)));
    }

    #[test]
    fn a_missing_or_unreadable_key_gives_the_seven_defaults() {
        let (rules, skipped) = parse_colour_rules(None);
        assert_eq!(rules.len(), 7);
        assert_eq!(rules[0].field, "isLabel");
        assert!(skipped.is_empty());
        let (rules, _) = parse_colour_rules(Some(&json!("not an object")));
        assert_eq!(rules, default_colour_rules());
    }

    #[test]
    fn stored_rules_keep_their_order_and_invalid_ones_are_skipped() {
        let stored = json!({ "version": 1, "rules": [
            { "field": "isRole", "colour": "#112233" },
            { "field": "bad field", "colour": "#112233" },
            { "field": "isTrait", "colour": "green" },
            { "field": "ssvTestFlag", "colour": "#ABCDEF" },
        ]});
        let (rules, skipped) = parse_colour_rules(Some(&stored));
        assert_eq!(
            rules,
            vec![
                ColourRule {
                    field: "isRole".into(),
                    colour: Rgb(0x11, 0x22, 0x33)
                },
                ColourRule {
                    field: "ssvTestFlag".into(),
                    colour: Rgb(0xab, 0xcd, 0xef)
                },
            ]
        );
        assert_eq!(skipped, vec!["bad field".to_owned(), "isTrait".to_owned()]);
    }

    #[test]
    fn an_empty_rule_list_means_no_colours_not_the_defaults() {
        let (rules, _) = parse_colour_rules(Some(&json!({ "version": 1, "rules": [] })));
        assert!(rules.is_empty());
    }
}
