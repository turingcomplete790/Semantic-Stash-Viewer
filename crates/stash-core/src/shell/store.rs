//! Versioned, atomic JSON files for the app shell's discardable state (004 research R3).
//!
//! Tabs and notifications are UI state, not library data (constitution Principle I). Losing
//! them costs open tabs or old alerts, never data, so loading never fails. A missing file is
//! empty. A damaged file, or one written by a newer viewer, is moved aside to `<name>.bak`, and
//! the store starts empty. Writes go to a temp file in the same directory, are fsynced, and are
//! then renamed over the old file, so a crash never leaves a half-written file.

use std::fs;
use std::io::Write;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

use crate::error::AppError;

/// A file shape with a top-level `version` field.
pub trait Versioned {
    /// The version this build reads and writes.
    const VERSION: u32;
}

#[derive(Debug, Clone)]
pub struct JsonStore<T> {
    path: PathBuf,
    _shape: PhantomData<fn() -> T>,
}

impl<T: Serialize + DeserializeOwned + Default + Versioned> JsonStore<T> {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            _shape: PhantomData,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Read the file. Never fails: problems give an empty (default) value.
    pub fn load(&self) -> T {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!(error = %e, path = %self.path.display(), "couldn't read shell state; starting empty");
                }
                return T::default();
            }
        };
        let newer = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("version").and_then(serde_json::Value::as_u64))
            .is_some_and(|found| found > u64::from(T::VERSION));
        if newer {
            self.move_aside("written by a newer viewer");
            return T::default();
        }
        match serde_json::from_str::<T>(&text) {
            Ok(value) => value,
            Err(e) => {
                self.move_aside(&e.to_string());
                T::default()
            }
        }
    }

    /// Atomically replace the file with `value`.
    pub fn save(&self, value: &T) -> Result<(), AppError> {
        let dir = self.path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(dir)?;
        let name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "state.json".into());
        let tmp = dir.join(format!(".{name}.{}.tmp", Uuid::new_v4()));
        let json = serde_json::to_vec(value).map_err(|e| AppError::Internal {
            message: e.to_string(),
        })?;
        let result = (|| -> std::io::Result<()> {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(&json)?;
            f.sync_all()?;
            fs::rename(&tmp, &self.path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result.map_err(Into::into)
    }

    fn move_aside(&self, reason: &str) {
        let mut backup = self.path.clone().into_os_string();
        backup.push(".bak");
        let backup = PathBuf::from(backup);
        tracing::warn!(reason, backup = %backup.display(), "shell state unusable; moving it aside");
        if let Err(e) = fs::rename(&self.path, &backup) {
            tracing::warn!(error = %e, "couldn't move shell state aside");
        }
    }
}
