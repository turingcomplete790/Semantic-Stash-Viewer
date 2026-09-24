//! `profiles.json` persistence (research R11, data-model.md "ProfilesFile").
//!
//! Writes are atomic (temp file in the same directory, then rename). A file with an unknown
//! future schema version is refused rather than overwritten; a corrupt file is moved aside to
//! `profiles.json.bak` and the store starts empty with a notice.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use super::model::ServerProfile;
use crate::error::AppError;

/// The schema version this build reads and writes.
pub const PROFILES_FILE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfilesFile {
    pub version: u32,
    /// Order = user order in the profile list.
    #[serde(default)]
    pub profiles: Vec<ServerProfile>,
    /// Used for auto-connect on launch (FR-014).
    #[serde(default)]
    pub last_used_profile_id: Option<Uuid>,
}

impl Default for ProfilesFile {
    fn default() -> Self {
        Self {
            version: PROFILES_FILE_VERSION,
            profiles: Vec::new(),
            last_used_profile_id: None,
        }
    }
}

/// Something the user should be told about after opening the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreNotice {
    /// The file couldn't be parsed; it was moved to this path and the list starts empty.
    CorruptFileMovedAside { backup: PathBuf },
}

#[derive(Debug)]
pub struct ProfileStore {
    path: PathBuf,
    file: ProfilesFile,
    notice: Option<StoreNotice>,
}

impl ProfileStore {
    /// Open (or start) the store at `path`. A missing file means no profiles yet.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, AppError> {
        let path = path.into();
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self {
                    path,
                    file: ProfilesFile::default(),
                    notice: None,
                })
            }
            Err(e) => return Err(e.into()),
        };

        // Check the version before trusting the shape, so a newer file is never clobbered.
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(found) = value.get("version").and_then(serde_json::Value::as_u64) {
                if found > u64::from(PROFILES_FILE_VERSION) {
                    return Err(AppError::UnsupportedProfilesVersion {
                        found: u32::try_from(found).unwrap_or(u32::MAX),
                    });
                }
            }
        }

        match serde_json::from_str::<ProfilesFile>(&text) {
            Ok(file) => Ok(Self {
                path,
                file,
                notice: None,
            }),
            Err(e) => {
                let backup = path.with_extension("json.bak");
                tracing::warn!(error = %e, backup = %backup.display(), "profiles.json is corrupt; moving it aside");
                fs::rename(&path, &backup)?;
                Ok(Self {
                    path,
                    file: ProfilesFile::default(),
                    notice: Some(StoreNotice::CorruptFileMovedAside { backup }),
                })
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Take the notice produced by `open`, if any (reported once).
    pub fn take_notice(&mut self) -> Option<StoreNotice> {
        self.notice.take()
    }

    pub fn list(&self) -> &[ServerProfile] {
        &self.file.profiles
    }

    pub fn get(&self, id: Uuid) -> Option<&ServerProfile> {
        self.file.profiles.iter().find(|p| p.id == id)
    }

    pub fn last_used_profile_id(&self) -> Option<Uuid> {
        self.file.last_used_profile_id
    }

    /// Profile whose base URL matches `url` (case-insensitive on scheme and host, ignoring a
    /// trailing slash).
    pub fn find_by_base_url(&self, url: &Url) -> Option<&ServerProfile> {
        let key = url_key(url);
        self.file
            .profiles
            .iter()
            .find(|p| url_key(&p.base_url) == key)
    }

    pub fn insert(&mut self, profile: ServerProfile) -> Result<(), AppError> {
        self.file.profiles.push(profile);
        self.save()
    }

    /// Replace the profile with the same id.
    pub fn replace(&mut self, profile: ServerProfile) -> Result<(), AppError> {
        let slot = self
            .file
            .profiles
            .iter_mut()
            .find(|p| p.id == profile.id)
            .ok_or(AppError::ProfileNotFound { id: profile.id })?;
        *slot = profile;
        self.save()
    }

    /// Remove a profile; clears `last_used_profile_id` if it pointed at it.
    pub fn remove(&mut self, id: Uuid) -> Result<ServerProfile, AppError> {
        let index = self
            .file
            .profiles
            .iter()
            .position(|p| p.id == id)
            .ok_or(AppError::ProfileNotFound { id })?;
        let removed = self.file.profiles.remove(index);
        if self.file.last_used_profile_id == Some(id) {
            self.file.last_used_profile_id = None;
        }
        self.save()?;
        Ok(removed)
    }

    /// Reorder to match `ids`. Profiles not listed keep their relative order at the end.
    pub fn reorder(&mut self, ids: &[Uuid]) -> Result<(), AppError> {
        let mut rest = std::mem::take(&mut self.file.profiles);
        let mut ordered = Vec::with_capacity(rest.len());
        for id in ids {
            if let Some(i) = rest.iter().position(|p| p.id == *id) {
                ordered.push(rest.remove(i));
            }
        }
        ordered.extend(rest);
        self.file.profiles = ordered;
        self.save()
    }

    pub fn set_last_used(&mut self, id: Option<Uuid>) -> Result<(), AppError> {
        self.file.last_used_profile_id = id;
        self.save()
    }

    /// Atomically write the file: temp file in the same directory, fsync, rename.
    fn save(&self) -> Result<(), AppError> {
        let dir = self.path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(dir)?;
        let tmp = dir.join(format!(".profiles.json.{}.tmp", Uuid::new_v4()));
        let json = serde_json::to_vec_pretty(&self.file).map_err(|e| AppError::Internal {
            message: e.to_string(),
        })?;
        let result = (|| -> std::io::Result<()> {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(&json)?;
            f.write_all(b"\n")?;
            f.sync_all()?;
            fs::rename(&tmp, &self.path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result.map_err(AppError::from)
    }
}

/// Comparison key for base URLs. `Url` already lowercases scheme and host.
fn url_key(url: &Url) -> String {
    format!(
        "{}://{}:{}{}",
        url.scheme(),
        url.host_str().unwrap_or_default().to_ascii_lowercase(),
        url.port_or_known_default().unwrap_or_default(),
        url.path().trim_end_matches('/')
    )
}
