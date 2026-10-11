//! Local search over cached choice lists (spec FR-003a): case-insensitive and accent-folded,
//! over names and aliases, so a 20,000-name picker narrows within a frame with no request.

use icu_normalizer::DecomposingNormalizerBorrowed;

/// `text` lower-cased with accents removed (`Émilie` → `emilie`).
pub fn fold(text: &str) -> String {
    let nfd = DecomposingNormalizerBorrowed::new_nfd();
    nfd.normalize(text)
        .chars()
        // Combining diacritical marks (and their extensions), left over from decomposition.
        .filter(|c| !matches!(*c as u32, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F))
        .flat_map(char::to_lowercase)
        .collect()
}

/// A searchable entry: its folded name and aliases, built once per list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folded {
    pub name: String,
    pub aliases: Vec<String>,
}

impl Folded {
    pub fn new(name: &str, aliases: &[String]) -> Self {
        Self {
            name: fold(name),
            aliases: aliases.iter().map(|a| fold(a)).collect(),
        }
    }

    /// How well a folded `query` matches: `Some(0)` a name prefix, `Some(1)` within the name,
    /// `Some(2)` an alias; `None` no match. An empty query matches everything.
    pub fn rank(&self, query: &str) -> Option<u8> {
        if query.is_empty() || self.name.starts_with(query) {
            Some(0)
        } else if self.name.contains(query) {
            Some(1)
        } else if self.aliases.iter().any(|a| a.contains(query)) {
            Some(2)
        } else {
            None
        }
    }
}

/// The indexes of `entries` matching `query`, best first (then in list order).
pub fn search(entries: &[Folded], query: &str) -> Vec<usize> {
    let query = fold(query.trim());
    let mut hits: Vec<(u8, usize)> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| e.rank(&query).map(|r| (r, i)))
        .collect();
    hits.sort_unstable();
    hits.into_iter().map(|(_, i)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folding_ignores_case_and_accents() {
        assert_eq!(fold("Émilie Brontë"), "emilie bronte");
        assert_eq!(fold("ÅSA"), "asa");
        assert_eq!(fold("plain"), "plain");
    }

    #[test]
    fn prefixes_beat_substrings_beat_aliases() {
        let entries = vec![
            Folded::new("Anna Belle", &[]),
            Folded::new("Bella", &[]),
            Folded::new("Zed", &["Isabella".to_owned()]),
            Folded::new("Other", &[]),
        ];
        assert_eq!(search(&entries, "BEL"), vec![1, 0, 2]);
        assert_eq!(search(&entries, "").len(), 4);
        assert!(search(&entries, "nothing").is_empty());
    }
}
