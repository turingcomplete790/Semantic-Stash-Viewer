//! The view cache (003 US1): a discardable, per-profile copy of what the viewer reads from
//! Stash, so screens appear instantly and refresh in the background (constitution Principles I,
//! IV, VI; research R1–R3).
//!
//! One SQLite file per server profile, in the platform cache directory. Entries are keyed per
//! view's data (`server:info`, `scenes:recent`, `scene:<id>`, …) and hold the domain type's JSON.
//! The file is never authoritative: a damaged or newer-schema file is deleted and recreated, a
//! different server behind the profile wipes it, and it stays under a share of free disk space,
//! dropping the least recently used entries first.

pub mod identity;
pub mod refresh;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// The cache file schema this build reads and writes.
const SCHEMA_VERSION: i64 = 1;
/// Share of free disk space the cache may use (research R1b).
const FREE_SPACE_SHARE: u64 = 20; // 1/20 = 5%
const MIN_LIMIT: u64 = 64 * 1024 * 1024;
const MAX_LIMIT: u64 = 10 * 1024 * 1024 * 1024;
/// Where free space can't be read (non-Unix).
const FALLBACK_LIMIT: u64 = 1024 * 1024 * 1024;
/// Re-read free space after this many writes.
const LIMIT_RECHECK_WRITES: u32 = 50;

/// Data served to the UI, with where it came from (contracts "Cached<T>").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Cached<T> {
    pub data: T,
    /// True when served from the cache (a background refresh may follow).
    pub from_cache: bool,
    /// When the data came from the server (ISO 8601).
    pub fetched_at: String,
}

impl<T> Cached<T> {
    /// Data just read from the server (when there's no cache to store it in).
    pub fn fresh(data: T) -> Self {
        Self {
            data,
            from_cache: false,
            fetched_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Returns free bytes on the disk holding the given directory, if known.
pub type FreeSpaceFn = Box<dyn Fn(&Path) -> Option<u64> + Send + Sync>;

/// How the size limit is decided.
pub enum Limit {
    /// A share of free disk space, via the given function (production uses `statvfs`).
    FreeSpace(FreeSpaceFn),
    /// A fixed number of bytes (tests).
    Fixed(u64),
}

impl Limit {
    /// 5% of free space on the cache's disk (research R1b).
    pub fn free_space() -> Self {
        Self::FreeSpace(Box::new(free_space_on))
    }
}

/// 5% of `free`, never below 64 MB and never above 10 GB (FR-008).
pub fn limit_for_free_space(free: u64) -> u64 {
    (free / FREE_SPACE_SHARE).clamp(MIN_LIMIT, MAX_LIMIT)
}

#[cfg(unix)]
fn free_space_on(dir: &Path) -> Option<u64> {
    let stat = rustix::fs::statvfs(dir).ok()?;
    Some(stat.f_bavail.saturating_mul(stat.f_frsize))
}

#[cfg(not(unix))]
fn free_space_on(_dir: &Path) -> Option<u64> {
    None
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn iso(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .unwrap_or_default()
        .to_rfc3339()
}

fn storage(e: impl std::fmt::Display) -> AppError {
    AppError::Storage {
        message: format!("cache: {e}"),
    }
}

/// Remove a profile's cache directory (FR-009). Missing is fine.
pub fn delete_profile_cache(dir: &Path) {
    if let Err(e) = std::fs::remove_dir_all(dir) {
        if e.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!(error = %e, dir = %dir.display(), "couldn't delete a profile's cache");
        }
    }
}

pub struct ViewCache {
    conn: Connection,
    path: PathBuf,
    limit_source: Limit,
    limit: u64,
    writes_since_check: u32,
}

impl ViewCache {
    /// Open (or create) the cache at `path`, limited to 5% of free disk space.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, AppError> {
        Self::open_with(path, Limit::free_space())
    }

    /// Open with a chosen limit. A damaged or newer-schema file is deleted and recreated
    /// (FR-010); only a failure to create a fresh file is an error.
    pub fn open_with(path: impl Into<PathBuf>, limit_source: Limit) -> Result<Self, AppError> {
        let path = path.into();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(storage)?;
        }
        let conn = match Self::try_open(&path) {
            Ok(conn) => conn,
            Err(reason) => {
                tracing::warn!(%reason, path = %path.display(), "cache unusable; starting a new one");
                for suffix in ["", "-wal", "-shm", "-journal"] {
                    let mut p = path.clone().into_os_string();
                    p.push(suffix);
                    let _ = std::fs::remove_file(PathBuf::from(p));
                }
                Self::try_open(&path).map_err(storage)?
            }
        };
        let mut cache = Self {
            conn,
            path,
            limit_source,
            limit: 0,
            writes_since_check: 0,
        };
        cache.refresh_limit();
        Ok(cache)
    }

    fn try_open(path: &Path) -> Result<Connection, String> {
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        let ok: String = conn
            .query_row("PRAGMA quick_check", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        if ok != "ok" {
            return Err(format!("integrity check: {ok}"));
        }
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS entries (
                 key TEXT PRIMARY KEY,
                 value BLOB NOT NULL,
                 fetched_at INTEGER NOT NULL,
                 last_used INTEGER NOT NULL,
                 size INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS entries_last_used ON entries (last_used);",
        )
        .map_err(|e| e.to_string())?;
        let version: Option<String> = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        match version.and_then(|v| v.parse::<i64>().ok()) {
            None => {
                conn.execute(
                    "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?1)",
                    params![SCHEMA_VERSION.to_string()],
                )
                .map_err(|e| e.to_string())?;
            }
            Some(v) if v > SCHEMA_VERSION => return Err(format!("schema version {v} is newer")),
            Some(_) => {}
        }
        Ok(conn)
    }

    fn refresh_limit(&mut self) {
        self.limit = match &self.limit_source {
            Limit::Fixed(bytes) => *bytes,
            Limit::FreeSpace(free) => {
                let dir = self.path.parent().unwrap_or_else(|| Path::new("."));
                free(dir).map_or(FALLBACK_LIMIT, limit_for_free_space)
            }
        };
        self.writes_since_check = 0;
    }

    /// The current size limit in bytes.
    pub fn limit(&self) -> u64 {
        self.limit
    }

    /// Read an entry and mark it used. Anything unreadable counts as a miss.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<Cached<T>> {
        let row: Option<(Vec<u8>, i64)> = self
            .conn
            .query_row(
                "SELECT value, fetched_at FROM entries WHERE key = ?1",
                params![key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .ok()
            .flatten();
        let (value, fetched_at) = row?;
        let data = serde_json::from_slice(&value).ok()?;
        let _ = self.conn.execute(
            "UPDATE entries SET last_used = ?2 WHERE key = ?1",
            params![key, now_ms()],
        );
        Some(Cached {
            data,
            from_cache: true,
            fetched_at: iso(fetched_at),
        })
    }

    /// When `key` was fetched (Unix ms), if cached.
    pub fn fetched_at_ms(&self, key: &str) -> Option<i64> {
        self.conn
            .query_row(
                "SELECT fetched_at FROM entries WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .ok()
            .flatten()
    }

    /// The raw bytes stored for `key` (JSON for data, so refreshes can compare it).
    pub fn raw(&self, key: &str) -> Option<Vec<u8>> {
        self.conn
            .query_row(
                "SELECT value FROM entries WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .ok()
            .flatten()
    }

    /// Store `value` as fetched now.
    pub fn put<T: Serialize>(&mut self, key: &str, value: &T) -> Result<(), AppError> {
        self.put_at(key, value, now_ms())
    }

    /// Store `value` as fetched at `fetched_ms` (Unix ms), then evict past the limit.
    pub fn put_at<T: Serialize>(
        &mut self,
        key: &str,
        value: &T,
        fetched_ms: i64,
    ) -> Result<(), AppError> {
        let json = serde_json::to_vec(value).map_err(storage)?;
        self.put_raw(key, &json, fetched_ms)
    }

    /// Store raw bytes, such as a thumbnail (005 research R5). Blobs share the size limit and
    /// least-recently-used eviction with everything else; their keys (`thumb:…`) keep them apart
    /// from JSON entries, which `get` would not parse anyway.
    pub fn put_bytes(&mut self, key: &str, bytes: &[u8]) -> Result<(), AppError> {
        self.put_raw(key, bytes, now_ms())
    }

    /// Read raw bytes stored with `put_bytes`, marking the entry used.
    pub fn get_bytes(&self, key: &str) -> Option<Vec<u8>> {
        let bytes = self.raw(key)?;
        let _ = self.conn.execute(
            "UPDATE entries SET last_used = ?2 WHERE key = ?1",
            params![key, now_ms()],
        );
        Some(bytes)
    }

    fn put_raw(&mut self, key: &str, bytes: &[u8], fetched_ms: i64) -> Result<(), AppError> {
        let size = i64::try_from(bytes.len()).unwrap_or(i64::MAX);
        self.conn
            .execute(
                "INSERT OR REPLACE INTO entries (key, value, fetched_at, last_used, size)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![key, bytes, fetched_ms, now_ms(), size],
            )
            .map_err(storage)?;
        self.writes_since_check += 1;
        if self.writes_since_check >= LIMIT_RECHECK_WRITES {
            self.refresh_limit();
        }
        self.evict()
    }

    /// Overwrite an entry with a write's result (Phase 2). Same as `put`, named for intent.
    pub fn replace<T: Serialize>(&mut self, key: &str, value: &T) -> Result<(), AppError> {
        self.put(key, value)
    }

    /// Drop least-recently-used entries until the total is within the limit.
    fn evict(&mut self) -> Result<(), AppError> {
        let limit = i64::try_from(self.limit).unwrap_or(i64::MAX);
        loop {
            let total: i64 = self
                .conn
                .query_row("SELECT COALESCE(SUM(size), 0) FROM entries", [], |row| {
                    row.get(0)
                })
                .map_err(storage)?;
            if total <= limit {
                return Ok(());
            }
            let removed = self
                .conn
                .execute(
                    "DELETE FROM entries WHERE key =
                       (SELECT key FROM entries ORDER BY last_used ASC, rowid ASC LIMIT 1)",
                    [],
                )
                .map_err(storage)?;
            if removed == 0 {
                return Ok(());
            }
        }
    }

    /// Remove exactly these entries (FR-006). Returns how many were removed.
    pub fn invalidate(&mut self, keys: &[&str]) -> Result<usize, AppError> {
        let mut removed = 0;
        for key in keys {
            removed += self
                .conn
                .execute("DELETE FROM entries WHERE key = ?1", params![key])
                .map_err(storage)?;
        }
        Ok(removed)
    }

    /// Remove every entry whose key starts with `prefix` (FR-006).
    pub fn invalidate_prefix(&mut self, prefix: &str) -> Result<usize, AppError> {
        let escaped = prefix
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        self.conn
            .execute(
                "DELETE FROM entries WHERE key LIKE ?1 ESCAPE '\\'",
                params![format!("{escaped}%")],
            )
            .map_err(storage)
    }

    /// Total size of the cached data in bytes.
    pub fn size_bytes(&self) -> u64 {
        self.conn
            .query_row("SELECT COALESCE(SUM(size), 0) FROM entries", [], |row| {
                row.get::<_, i64>(0)
            })
            .map(|n| u64::try_from(n).unwrap_or(0))
            .unwrap_or(0)
    }

    /// Every cached key (to tell views what changed after a clear).
    pub fn keys(&self) -> Vec<String> {
        let Ok(mut stmt) = self.conn.prepare("SELECT key FROM entries") else {
            return Vec::new();
        };
        stmt.query_map([], |row| row.get(0))
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
    }

    /// Delete everything (FR-005). Returns the bytes freed.
    pub fn clear(&mut self) -> Result<u64, AppError> {
        let freed = self.size_bytes();
        self.conn
            .execute("DELETE FROM entries", [])
            .map_err(storage)?;
        let _ = self.conn.execute_batch("VACUUM");
        Ok(freed)
    }

    /// Record the server behind this profile. A different server than last time wipes the
    /// cache first (FR-007). Returns true when it wiped.
    pub fn check_identity(&mut self, identity: &str) -> Result<bool, AppError> {
        let stored: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'server_identity'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage)?;
        let wipe = stored.as_deref().is_some_and(|s| s != identity);
        if wipe {
            tracing::info!("server behind this profile changed; clearing its cache");
            self.conn
                .execute("DELETE FROM entries", [])
                .map_err(storage)?;
        }
        if stored.as_deref() != Some(identity) {
            self.conn
                .execute(
                    "INSERT OR REPLACE INTO meta (key, value) VALUES ('server_identity', ?1)",
                    params![identity],
                )
                .map_err(storage)?;
        }
        Ok(wipe)
    }
}
