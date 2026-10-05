//! Thumbnails as image handles (007 T037): one LRU for the session, keyed by scene id and
//! screenshot version, at most 1,200 (a 1000-card page and the next page's warm-up). Handles hold
//! pixels already decoded at the width they're drawn (the core's 480 px JPEGs, decoded and scaled
//! off the UI thread), so iced only uploads them: decoding and scaling 480 px images as rows
//! scrolled in cost frames. Missing ones are asked for once
//! (`LoadThumbnails`, over the core's `ThumbService` and its disk cache, in batches).

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use iced::widget::image::Handle;
use stash_core::scenes::SceneCard;

use crate::effects::Effect;

/// The most handles kept.
pub const MAX_THUMBS: usize = 1200;
/// Thumbnails warmed on the page after the one showing.
pub const WARM_LIMIT: usize = 120;

/// `(scene id, screenshot version)`.
pub type ThumbKey = (String, String);

/// The key in a card's `ssv-thumb://localhost/scene/<id>?v=<version>`.
pub fn thumb_key(card: &SceneCard) -> Option<ThumbKey> {
    let thumb = card.thumb.as_deref()?;
    let rest = thumb.strip_prefix("ssv-thumb://localhost/scene/")?;
    let (id, version) = rest.split_once("?v=")?;
    Some((id.to_owned(), version.to_owned()))
}

/// The one handle for cards without a thumbnail (yet).
pub fn placeholder() -> Handle {
    static PLACEHOLDER: OnceLock<Handle> = OnceLock::new();
    PLACEHOLDER
        .get_or_init(|| Handle::from_bytes(stash_core::thumbs::PLACEHOLDER))
        .clone()
}

/// Revisions are unique across every `Thumbs`, so a replaced set never matches an old one.
static REVISION: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub struct Thumbs {
    handles: HashMap<ThumbKey, (Handle, u64)>,
    pending: HashSet<ThumbKey>,
    tick: u64,
    /// Changes whenever a handle is added or dropped (views that show them rebuild).
    revision: u64,
}

impl Default for Thumbs {
    fn default() -> Self {
        Self {
            handles: HashMap::new(),
            pending: HashSet::new(),
            tick: 0,
            revision: REVISION.fetch_add(1, Ordering::Relaxed),
        }
    }
}

impl Thumbs {
    pub fn get(&self, key: &ThumbKey) -> Option<&Handle> {
        self.handles.get(key).map(|(h, _)| h)
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn len(&self) -> usize {
        self.handles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }

    /// Mark these as in use and ask for the ones not here or on their way.
    /// `width`: the width they're drawn at.
    pub fn want<'a>(
        &mut self,
        keys: impl IntoIterator<Item = &'a ThumbKey>,
        width: u32,
    ) -> Vec<Effect> {
        self.tick += 1;
        let mut missing = Vec::new();
        for key in keys {
            if let Some(entry) = self.handles.get_mut(key) {
                entry.1 = self.tick;
            } else if self.pending.insert(key.clone()) {
                missing.push(key.clone());
            }
        }
        if missing.is_empty() {
            Vec::new()
        } else {
            vec![Effect::LoadThumbnails {
                keys: missing,
                width,
            }]
        }
    }

    /// The thumbnails for a page of cards.
    pub fn want_cards(&mut self, cards: &[SceneCard], width: u32) -> Vec<Effect> {
        let keys: Vec<ThumbKey> = cards.iter().filter_map(thumb_key).collect();
        self.want(&keys, width)
    }

    /// Thumbnails arrived.
    pub fn insert_all(&mut self, batch: Vec<(ThumbKey, Option<Handle>)>) {
        for (key, bytes) in batch {
            self.insert(key, bytes);
        }
    }

    /// A thumbnail arrived (`None`: the core had nothing, so the placeholder shows).
    pub fn insert(&mut self, key: ThumbKey, handle: Option<Handle>) {
        self.pending.remove(&key);
        let handle = handle.unwrap_or_else(placeholder);
        self.tick += 1;
        self.revision = REVISION.fetch_add(1, Ordering::Relaxed);
        self.handles.insert(key, (handle, self.tick));
        if self.handles.len() > MAX_THUMBS {
            self.evict();
        }
    }

    /// Drop the least recently used tenth.
    fn evict(&mut self) {
        let mut ticks: Vec<u64> = self.handles.values().map(|(_, t)| *t).collect();
        ticks.sort_unstable();
        let cutoff = ticks[(MAX_THUMBS / 10).min(ticks.len() - 1)];
        self.handles.retain(|_, (_, t)| *t > cutoff);
    }
}
