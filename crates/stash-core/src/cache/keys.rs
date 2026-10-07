//! Cache keys for 008's entries, in one place so kinds never collide (contracts/query-budget.md;
//! plan "Storage").
//!
//! Every key starts with its kind; ids are Stash ids (digits), so no two kinds share a key.

/// A picker's choice list (`choices:performers`, `choices:tags`, …).
pub fn choices(kind: &str) -> String {
    format!("choices:{kind}")
}

/// The semantic vocabulary: semantic tags, common variants, colour-rule matches.
pub const SEMANTIC_VOCABULARY: &str = "semantic:vocabulary";

/// The vocabulary's key for colour rules whose extra (non-semantic) fields are `extra`: the
/// matches differ per set of fields, so each set has its own entry.
pub fn semantic_vocabulary(extra: &[&str]) -> String {
    if extra.is_empty() {
        SEMANTIC_VOCABULARY.to_owned()
    } else {
        format!("{SEMANTIC_VOCABULARY}:{}", extra.join(","))
    }
}

/// The server's UI settings (default filter, minimum play percent, colour rules).
pub const UI_SETTINGS: &str = "ui:settings";

/// The saved scene filters.
pub const SAVED_SCENE_FILTERS: &str = "saved-filters:scenes";

/// A scene page's default request.
pub fn scene_details(id: &str) -> String {
    format!("scene:details:{id}")
}

/// A scene's "All files" section.
pub fn scene_files(id: &str) -> String {
    format!("scene:files:{id}")
}

/// A scene's galleries section.
pub fn scene_galleries(id: &str) -> String {
    format!("scene:galleries:{id}")
}

/// One chip's details card (`chip:performer:12`).
pub fn chip(kind: &str, id: &str) -> String {
    format!("chip:{kind}:{id}")
}

/// A scene's animated preview bytes.
pub fn preview(id: &str) -> String {
    format!("preview:{id}")
}

/// A scene's sprite sheet and VTT index.
pub fn sprite(id: &str) -> String {
    format!("sprite:{id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn keys_from_different_kinds_never_collide() {
        let id = "12";
        let keys = [
            choices("tags"),
            choices("performers"),
            SEMANTIC_VOCABULARY.to_owned(),
            UI_SETTINGS.to_owned(),
            SAVED_SCENE_FILTERS.to_owned(),
            scene_details(id),
            scene_files(id),
            scene_galleries(id),
            chip("performer", id),
            chip("tag", id),
            chip("studio", id),
            preview(id),
            sprite(id),
        ];
        let unique: HashSet<&String> = keys.iter().collect();
        assert_eq!(unique.len(), keys.len());
    }

    #[test]
    fn scene_details_keeps_the_key_007_used() {
        assert_eq!(scene_details("7"), "scene:details:7");
    }
}
