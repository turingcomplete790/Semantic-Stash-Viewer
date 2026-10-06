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
