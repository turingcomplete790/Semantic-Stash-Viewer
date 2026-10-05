//! The view cache per server profile, following the connection (003; the demo's
//! `CacheRegistry`, carried over). Cached data changes are broadcast so open screens can re-read in
//! place.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use stash_core::cache::refresh::Refresher;
use stash_core::cache::ViewCache;
use stash_core::connection::snapshot::SessionState;
use stash_core::shell::notifications::NotificationCenter;
use tokio::sync::broadcast;
use uuid::Uuid;

use super::{lock, Manager};

/// A cached entry changed: `(profile, key)`, where key `*` means everything.
pub type CacheChange = (Uuid, String);

/// One refresher per profile, following the connection.
pub struct CacheRegistry {
    root: PathBuf,
    center: Arc<NotificationCenter>,
    open: Mutex<HashMap<Uuid, Arc<Refresher>>>,
    online: Mutex<Option<Uuid>>,
    changes: broadcast::Sender<CacheChange>,
}

impl CacheRegistry {
    pub fn new(root: PathBuf, center: Arc<NotificationCenter>) -> Self {
        Self {
            root,
            center,
            open: Mutex::new(HashMap::new()),
            online: Mutex::new(None),
            changes: broadcast::channel(64).0,
        }
    }

    /// Changes to cached data (a refresh that returned something new, or a clear).
    pub fn changes(&self) -> broadcast::Receiver<CacheChange> {
        self.changes.subscribe()
    }

    pub fn for_profile(&self, profile: Uuid) -> Option<Arc<Refresher>> {
        let mut open = lock(&self.open);
        if let Some(r) = open.get(&profile) {
            return Some(Arc::clone(r));
        }
        let path = self.root.join(profile.to_string()).join("cache.sqlite3");
        let cache = match ViewCache::open(path) {
            Ok(cache) => cache,
            Err(e) => {
                tracing::warn!(error = %e, "view cache unavailable; reading from the server");
                return None;
            }
        };
        let refresher = Refresher::new(
            profile,
            Arc::new(Mutex::new(cache)),
            Arc::clone(&self.center),
            {
                let changes = self.changes.clone();
                Arc::new(move |key: &str| {
                    let _ = changes.send((profile, key.to_owned()));
                })
            },
        );
        refresher.set_online(*lock(&self.online) == Some(profile));
        open.insert(profile, Arc::clone(&refresher));
        Some(refresher)
    }

    fn set_online(&self, active: Option<Uuid>, online: bool) {
        *lock(&self.online) = active.filter(|_| online);
        for (id, r) in lock(&self.open).iter() {
            r.set_online(online && Some(*id) == active);
        }
    }

    /// Clear a profile's cache; returns the bytes freed and tells open screens.
    pub fn clear(&self, profile: Uuid) -> Result<u64, stash_core::AppError> {
        let Some(r) = self.for_profile(profile) else {
            return Ok(0);
        };
        let freed = lock(r.cache()).clear()?;
        let _ = self.changes.send((profile, "*".into()));
        Ok(freed)
    }

    /// The profile's cache size in bytes.
    pub fn size(&self, profile: Uuid) -> u64 {
        self.for_profile(profile)
            .map_or(0, |r| lock(r.cache()).size_bytes())
    }

    /// Follow the connection: online state, the identity check, and the server summary.
    pub fn follow(self: &Arc<Self>, manager: &Manager, runtime: &tokio::runtime::Handle) {
        let mut rx = manager.subscribe();
        let this = Arc::clone(self);
        runtime.spawn(async move {
            loop {
                let snapshot = match rx.recv().await {
                    Ok(s) => s,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                };
                let connected = matches!(snapshot.state, SessionState::Connected);
                if let (true, Some(id), Some(info)) =
                    (connected, snapshot.profile_id, snapshot.server.as_ref())
                {
                    if let Some(r) = this.for_profile(id) {
                        let wiped = {
                            let mut cache = lock(r.cache());
                            let wiped = cache.check_identity(&info.identity).unwrap_or(false);
                            if let Err(e) = cache.put("server:info", info) {
                                tracing::warn!(error = %e, "couldn't cache the server summary");
                            }
                            wiped
                        };
                        if wiped {
                            let _ = this.changes.send((id, "*".into()));
                        }
                    }
                }
                this.set_online(snapshot.profile_id, connected);
            }
        });
    }
}
