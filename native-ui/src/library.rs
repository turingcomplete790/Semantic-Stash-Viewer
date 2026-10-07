//! The library region of a session (008 data model "ChoiceList", "UiSettings",
//! "SemanticVocabulary"): what every tab of one server shares: the server's UI settings (default
//! filter, play settings, tag colour rules), its saved filters, the picker lists, and the semantic
//! vocabulary.
//!
//! Each is read once through the cache (0 requests when cached; quietly refreshed in the
//! background) and kept here, so opening a picker a second time costs nothing (spec SC-002a).

use std::collections::HashMap;
use std::sync::Arc;

use stash_core::adapter::choices::{ChoiceKind, ChoiceList};
use stash_core::adapter::filters::UiSettings;
use stash_core::scenes::filter::saved::SavedFilter;
use stash_core::scenes::filter::search::Folded;
use stash_core::semantic::{default_colour_rules, ColourRule, Rgb, SemanticVocabulary};
use stash_core::AppError;

use crate::effects::Effect;

/// Something read from the server.
#[derive(Debug, Clone, Default)]
pub enum Loadable<T> {
    #[default]
    NotLoaded,
    Loading,
    Ready(T),
    Failed,
}

impl<T> Loadable<T> {
    pub fn ready(&self) -> Option<&T> {
        match self {
            Loadable::Ready(t) => Some(t),
            _ => None,
        }
    }

    fn wanted(&self) -> bool {
        matches!(self, Loadable::NotLoaded | Loadable::Failed)
    }
}

/// A picker list with its search index (folded once, searched per keystroke).
#[derive(Debug)]
pub struct Choices {
    pub list: ChoiceList,
    pub folded: Vec<Folded>,
}

/// What a screen needs from the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    Choices(ChoiceKind),
    Vocabulary,
    SavedFilters,
}

#[derive(Debug, Default)]
pub struct Library {
    pub ui: Loadable<UiSettings>,
    pub saved: Loadable<Vec<SavedFilter>>,
    pub choices: HashMap<ChoiceKind, Loadable<Arc<Choices>>>,
    pub vocabulary: Loadable<Arc<SemanticVocabulary>>,
}

impl Library {
    /// The colour rules in force (the defaults until the UI settings arrive).
    pub fn colour_rules(&self) -> Vec<ColourRule> {
        self.ui
            .ready()
            .map_or_else(default_colour_rules, |s| s.colour_rules.clone())
    }

    /// A tag's colour, once the vocabulary is here.
    pub fn tag_colour(&self, id: &str) -> Option<Rgb> {
        let vocabulary = self.vocabulary.ready()?;
        vocabulary.colour_of(id, &self.colour_rules())
    }

    /// The UI settings, read at the session's start.
    pub fn want_ui(&mut self) -> Vec<Effect> {
        if self.ui.wanted() {
            self.ui = Loadable::Loading;
            vec![Effect::LoadUiSettings]
        } else {
            Vec::new()
        }
    }

    /// Whatever `need` asks for that isn't here or on its way.
    pub fn want(&mut self, need: Need) -> Vec<Effect> {
        match need {
            Need::Choices(kind) => {
                let entry = self.choices.entry(kind).or_default();
                if !entry.wanted() {
                    return Vec::new();
                }
                *entry = Loadable::Loading;
                let mut effects = vec![Effect::LoadChoices(kind)];
                // The Tags picker shows the vocabulary's groups above ordinary tags.
                if kind == ChoiceKind::Tags {
                    effects.extend(self.want(Need::Vocabulary));
                }
                effects
            }
            Need::Vocabulary => {
                if !self.vocabulary.wanted() {
                    return Vec::new();
                }
                self.vocabulary = Loadable::Loading;
                vec![Effect::LoadVocabulary(self.colour_rules())]
            }
            Need::SavedFilters => {
                if !self.saved.wanted() {
                    return Vec::new();
                }
                self.saved = Loadable::Loading;
                vec![Effect::LoadSavedFilters]
            }
        }
    }

    pub fn ui_loaded(&mut self, result: Result<UiSettings, AppError>) -> Vec<Effect> {
        let old_fields = self.rule_fields();
        self.ui = match result {
            Ok(s) => Loadable::Ready(s),
            Err(e) => {
                tracing::debug!(error = %e, "UI settings unavailable");
                Loadable::Failed
            }
        };
        // New rule fields (here, or picked up from another machine) need their matches.
        if self.vocabulary.ready().is_some() && self.rule_fields() != old_fields {
            self.vocabulary = Loadable::NotLoaded;
            return self.want(Need::Vocabulary);
        }
        Vec::new()
    }

    fn rule_fields(&self) -> Vec<String> {
        let mut fields: Vec<String> = self.colour_rules().into_iter().map(|r| r.field).collect();
        fields.sort();
        fields
    }

    pub fn saved_loaded(&mut self, result: Result<Vec<SavedFilter>, AppError>) {
        self.saved = match result {
            Ok(list) => Loadable::Ready(list),
            Err(_) => Loadable::Failed,
        };
    }

    pub fn choices_loaded(&mut self, kind: ChoiceKind, result: Result<ChoiceList, AppError>) {
        let value = match result {
            Ok(list) => {
                let folded = list.folded();
                Loadable::Ready(Arc::new(Choices { list, folded }))
            }
            Err(e) => {
                tracing::debug!(error = %e, kind = kind.key(), "choice list unavailable");
                Loadable::Failed
            }
        };
        self.choices.insert(kind, value);
    }

    pub fn vocabulary_loaded(&mut self, result: Result<SemanticVocabulary, AppError>) {
        self.vocabulary = match result {
            Ok(v) => Loadable::Ready(Arc::new(v)),
            Err(_) => Loadable::Failed,
        };
    }

    /// A cache key changed (a quiet refresh found something new): read it again from the cache.
    pub fn cache_changed(&mut self, key: &str) -> Vec<Effect> {
        let mut effects = Vec::new();
        if key == "*" || key == stash_core::cache::keys::UI_SETTINGS {
            if self.ui.ready().is_some() {
                self.ui = Loadable::NotLoaded;
            }
            effects.extend(self.want_ui());
        }
        if (key == "*" || key == stash_core::cache::keys::SAVED_SCENE_FILTERS)
            && self.saved.ready().is_some()
        {
            self.saved = Loadable::NotLoaded;
            effects.extend(self.want(Need::SavedFilters));
        }
        if (key == "*" || key.starts_with(stash_core::cache::keys::SEMANTIC_VOCABULARY))
            && self.vocabulary.ready().is_some()
        {
            self.vocabulary = Loadable::NotLoaded;
            effects.extend(self.want(Need::Vocabulary));
        }
        for kind in ChoiceKind::ALL {
            if (key == "*" || key == stash_core::cache::keys::choices(kind.key()))
                && self.choices.get(&kind).is_some_and(|c| c.ready().is_some())
            {
                self.choices.insert(kind, Loadable::NotLoaded);
                effects.extend(self.want(Need::Choices(kind)));
            }
        }
        effects
    }
}
