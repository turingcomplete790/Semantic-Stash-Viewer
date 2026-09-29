//! Per-profile tab sets (004 US2; data-model "Tab", "TabSet", "TabsFile").
//!
//! Routes and view state belong to the UI and are stored as opaque JSON; the core only checks
//! shapes and size limits. Tabs are discardable UI state (constitution Principle I).

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::store::{JsonStore, Versioned};
use super::{MAX_HISTORY, MAX_TABS, MAX_VIEW_STATE_BYTES};
use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    /// A UI route, opaque to the core.
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Unknown))]
    pub route: serde_json::Value,
    /// Scroll offsets, filters, selection, typed text. Opaque; at most 16 KB serialised.
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Unknown>))]
    pub view_state: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Tab {
    pub id: String,
    /// 1–50 entries, oldest first.
    pub history: Vec<HistoryEntry>,
    /// The entry shown: `0 ≤ index < history.len()`.
    pub index: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TabSet {
    /// 1–100 tabs, in strip order.
    pub tabs: Vec<Tab>,
    /// Must match a tab.
    pub selected_tab_id: String,
}

impl TabSet {
    /// Check the data-model limits. Returns `TabSetInvalid` with the first problem found.
    pub fn validate(&self) -> Result<(), AppError> {
        let fail = |reason: String| Err(AppError::TabSetInvalid { reason });
        if self.tabs.is_empty() {
            return fail("a tab set needs at least one tab".into());
        }
        if self.tabs.len() > MAX_TABS {
            return fail(format!("more than {MAX_TABS} tabs"));
        }
        if !self.tabs.iter().any(|t| t.id == self.selected_tab_id) {
            return fail("the selected tab isn't in the set".into());
        }
        for tab in &self.tabs {
            if tab.history.is_empty() {
                return fail(format!("tab {} has no history", tab.id));
            }
            if tab.history.len() > MAX_HISTORY {
                return fail(format!(
                    "tab {} has more than {MAX_HISTORY} history entries",
                    tab.id
                ));
            }
            if tab.index as usize >= tab.history.len() {
                return fail(format!("tab {} points past its history", tab.id));
            }
            for entry in &tab.history {
                if let Some(state) = &entry.view_state {
                    let size = serde_json::to_vec(state)
                        .map(|v| v.len())
                        .unwrap_or(usize::MAX);
                    if size > MAX_VIEW_STATE_BYTES {
                        return fail(format!("tab {} has view state over 16 KB", tab.id));
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TabsFile {
    pub version: u32,
    #[serde(default)]
    pub profiles: HashMap<Uuid, TabSet>,
}

impl Default for TabsFile {
    fn default() -> Self {
        Self {
            version: Self::VERSION,
            profiles: HashMap::new(),
        }
    }
}

impl Versioned for TabsFile {
    const VERSION: u32 = 1;
}

/// `shell/tabs.json`. Serialises its own read-modify-write cycles.
#[derive(Debug)]
pub struct TabsStore {
    file: JsonStore<TabsFile>,
    lock: Mutex<()>,
}

impl TabsStore {
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            file: JsonStore::new(path),
            lock: Mutex::new(()),
        }
    }

    fn guard(&self) -> std::sync::MutexGuard<'_, ()> {
        self.lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The saved tab set for `profile`, if any.
    pub fn load(&self, profile: Uuid) -> Option<TabSet> {
        let _g = self.guard();
        self.file.load().profiles.remove(&profile)
    }

    /// Validate and store `set` for `profile`. Nothing is written if validation fails.
    pub fn save(&self, profile: Uuid, set: &TabSet) -> Result<(), AppError> {
        set.validate()?;
        let _g = self.guard();
        let mut file = self.file.load();
        file.profiles.insert(profile, set.clone());
        self.file.save(&file)
    }

    /// Forget a deleted profile's tabs.
    pub fn delete(&self, profile: Uuid) -> Result<(), AppError> {
        let _g = self.guard();
        let mut file = self.file.load();
        if file.profiles.remove(&profile).is_some() {
            self.file.save(&file)?;
        }
        Ok(())
    }

    /// Drop tab sets for profiles that no longer exist (at startup).
    pub fn prune(&self, known: &[Uuid]) -> Result<(), AppError> {
        let _g = self.guard();
        let mut file = self.file.load();
        let before = file.profiles.len();
        file.profiles.retain(|id, _| known.contains(id));
        if file.profiles.len() != before {
            self.file.save(&file)?;
        }
        Ok(())
    }
}
