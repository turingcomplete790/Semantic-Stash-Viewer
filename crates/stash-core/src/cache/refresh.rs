//! "Show cached, then refresh" (003 research R4–R5).
//!
//! A cache hit returns at once; if it's older than 5 seconds, a background refresh follows and
//! the change callback fires only when the data actually differs. Nothing is fetched while
//! offline. Repeated refresh failures become one `background` notification, which pops up only
//! if they last over a minute, and turns into "Refreshed" when a refresh works again.

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

use super::{Cached, ViewCache};
use crate::error::AppError;
use crate::shell::notifications::{
    NewNotification, NotificationCenter, NotificationKind, Severity,
};

/// Don't refresh data younger than this (keeps requests to one per visit; FR-011).
const REFRESH_FLOOR_MS: i64 = 5_000;
/// Failures in a row before telling the user.
const FAILURES_BEFORE_NOTICE: u32 = 3;
/// Failing for longer than this makes the notification pop up.
const TOAST_AFTER_MS: i64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshPolicy {
    /// Refresh stale hits in the background.
    Auto,
    /// Only fetch on a miss or when forced (random data such as the test set).
    Manual,
}

pub type ChangeFn = Arc<dyn Fn(&str) + Send + Sync>;
pub type ClockFn = Arc<dyn Fn() -> i64 + Send + Sync>;

#[derive(Debug, Default)]
struct KeyState {
    in_flight: bool,
    failures: u32,
    first_failure_ms: Option<i64>,
    notified: bool,
}

pub struct Refresher {
    profile: Uuid,
    cache: Arc<Mutex<ViewCache>>,
    center: Arc<NotificationCenter>,
    on_change: ChangeFn,
    clock: ClockFn,
    online: AtomicBool,
    keys: Mutex<HashMap<String, KeyState>>,
}

fn system_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn not_connected() -> AppError {
    AppError::NotConnected
}

/// Plain-language name of what a key holds, for notifications.
fn describe(key: &str) -> &'static str {
    match key {
        "server:info" => "the server summary",
        "scenes:recent" => "recently added scenes",
        "scenes:test-set" => "the test scenes",
        k if k.starts_with("scene:") => "a scene",
        _ => "some data",
    }
}

fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    let mut out = chars
        .next()
        .map(|c| c.to_uppercase().collect::<String>())
        .unwrap_or_default();
    out.push_str(chars.as_str());
    if !out.ends_with('.') {
        out.push('.');
    }
    out
}

impl Refresher {
    pub fn new(
        profile: Uuid,
        cache: Arc<Mutex<ViewCache>>,
        center: Arc<NotificationCenter>,
        on_change: ChangeFn,
    ) -> Arc<Self> {
        Self::with_clock(profile, cache, center, on_change, Arc::new(system_ms))
    }

    /// With an injected clock (Unix ms), for tests.
    pub fn with_clock(
        profile: Uuid,
        cache: Arc<Mutex<ViewCache>>,
        center: Arc<NotificationCenter>,
        on_change: ChangeFn,
        clock: ClockFn,
    ) -> Arc<Self> {
        Arc::new(Self {
            profile,
            cache,
            center,
            on_change,
            clock,
            online: AtomicBool::new(true),
            keys: Mutex::new(HashMap::new()),
        })
    }

    pub fn profile(&self) -> Uuid {
        self.profile
    }

    pub fn cache(&self) -> &Arc<Mutex<ViewCache>> {
        &self.cache
    }

    /// Follow the connection: nothing is fetched while offline.
    pub fn set_online(&self, online: bool) {
        self.online.store(online, Ordering::SeqCst);
    }

    pub fn is_online(&self) -> bool {
        self.online.load(Ordering::SeqCst)
    }

    fn lock_cache(&self) -> std::sync::MutexGuard<'_, ViewCache> {
        self.cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn lock_keys(&self) -> std::sync::MutexGuard<'_, HashMap<String, KeyState>> {
        self.keys
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The cached copy if there is one (refreshing it in the background when stale and
    /// `policy` is `Auto`), otherwise fetch, store, and return it.
    pub async fn get_or_fetch<T, F, Fut>(
        self: &Arc<Self>,
        key: &str,
        policy: RefreshPolicy,
        fetch: F,
    ) -> Result<Cached<T>, AppError>
    where
        T: Serialize + DeserializeOwned + Send + 'static,
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T, AppError>> + Send + 'static,
    {
        let (hit, fetched_ms) = {
            let cache = self.lock_cache();
            (cache.get::<T>(key), cache.fetched_at_ms(key))
        };
        if let Some(hit) = hit {
            let stale = fetched_ms.map_or(true, |t| (self.clock)() - t >= REFRESH_FLOOR_MS);
            if policy == RefreshPolicy::Auto && stale && self.is_online() {
                self.spawn_refresh::<T, F, Fut>(key.to_owned(), fetch);
            }
            return Ok(hit);
        }
        if !self.is_online() {
            return Err(not_connected());
        }
        self.fetch_now(key, fetch).await
    }

    /// Fetch now and store (a miss, or a forced refresh such as Shuffle).
    pub async fn fetch_now<T, F, Fut>(&self, key: &str, fetch: F) -> Result<Cached<T>, AppError>
    where
        T: Serialize,
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, AppError>>,
    {
        let data = fetch().await?;
        let now = (self.clock)();
        if let Err(e) = self.lock_cache().put_at(key, &data, now) {
            tracing::warn!(error = %e, "couldn't store in the cache");
        }
        Ok(Cached {
            data,
            from_cache: false,
            fetched_at: chrono::DateTime::from_timestamp_millis(now)
                .unwrap_or_default()
                .to_rfc3339(),
        })
    }

    fn spawn_refresh<T, F, Fut>(self: &Arc<Self>, key: String, fetch: F)
    where
        T: Serialize + Send + 'static,
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T, AppError>> + Send + 'static,
    {
        {
            let mut keys = self.lock_keys();
            let state = keys.entry(key.clone()).or_default();
            if state.in_flight {
                return;
            }
            state.in_flight = true;
        }
        let this = Arc::clone(self);
        tokio::spawn(async move {
            let result = fetch().await;
            this.finish_refresh(&key, result);
        });
    }

    fn finish_refresh<T: Serialize>(&self, key: &str, result: Result<T, AppError>) {
        let now = (self.clock)();
        match result {
            Ok(data) => {
                let changed = {
                    let mut cache = self.lock_cache();
                    let before = cache.raw(key);
                    let after = serde_json::to_vec(&data).ok();
                    if let Err(e) = cache.put_at(key, &data, now) {
                        tracing::warn!(error = %e, "couldn't store a refresh");
                    }
                    before != after
                };
                let was_notified = {
                    let mut keys = self.lock_keys();
                    let state = keys.entry(key.to_owned()).or_default();
                    let notified = state.notified;
                    *state = KeyState::default();
                    notified
                };
                if was_notified {
                    self.center.post(NewNotification {
                        key: Some(self.notice_key(key)),
                        profile_id: Some(self.profile),
                        kind: NotificationKind::Background,
                        severity: Severity::Info,
                        title: "Refreshed".into(),
                        detail: Some(format!("{} is up to date again.", sentence(describe(key)))),
                        toast: false,
                        job: None,
                    });
                }
                if changed {
                    (self.on_change)(key);
                }
            }
            Err(error) => {
                let notice = {
                    let mut keys = self.lock_keys();
                    let state = keys.entry(key.to_owned()).or_default();
                    state.in_flight = false;
                    state.failures += 1;
                    let first = *state.first_failure_ms.get_or_insert(now);
                    if state.failures >= FAILURES_BEFORE_NOTICE {
                        state.notified = true;
                        Some(now - first > TOAST_AFTER_MS)
                    } else {
                        None
                    }
                };
                if let Some(toast) = notice {
                    self.center.post(NewNotification {
                        key: Some(self.notice_key(key)),
                        profile_id: Some(self.profile),
                        kind: NotificationKind::Background,
                        severity: Severity::Warning,
                        title: format!("Couldn't refresh {}", describe(key)),
                        detail: Some(sentence(&error.to_string())),
                        toast,
                        job: None,
                    });
                }
            }
        }
    }

    fn notice_key(&self, key: &str) -> String {
        format!("background:{}:{key}", self.profile)
    }
}
