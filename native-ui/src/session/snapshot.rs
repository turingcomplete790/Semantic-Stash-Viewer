//! The saved session (007 T028; contracts/session-snapshot.md): each profile's tabs, histories,
//! and screen fields, so a relaunch restores everything (B2). Only persistent fields are written;
//! a file that doesn't parse or has another version is set aside as `session.json.bad`.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::screens::Screen;
use crate::shell::Shell;

/// Bumped whenever the saved shape of the state tree changes. No migrations.
pub const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedSession {
    pub selected: usize,
    pub tabs: Vec<SavedTab>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedTab {
    pub history: Vec<Screen>,
    pub cursor: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct SessionFile {
    version: u32,
    sessions: BTreeMap<Uuid, SavedSession>,
}

/// The shell's saved form.
pub fn capture(shell: &Shell) -> SavedSession {
    shell.capture()
}

fn bad_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".bad");
    path.with_file_name(name)
}

/// The whole file: `Ok(None)` when there's none, `Err(())` when it's unusable.
fn read(path: &Path) -> Result<Option<SessionFile>, ()> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            tracing::warn!(error = %e, "couldn't read the saved session");
            return Err(());
        }
    };
    match serde_json::from_slice::<SessionFile>(&bytes) {
        Ok(file) if file.version == VERSION => Ok(Some(file)),
        Ok(file) => {
            tracing::warn!(version = file.version, "saved session has another version");
            Err(())
        }
        Err(e) => {
            tracing::warn!(error = %e, "saved session doesn't parse");
            Err(())
        }
    }
}

/// `profile`'s saved session, if there's a usable one. An unusable file is set aside so the app
/// starts with a fresh Home tab.
pub fn load(path: &Path, profile: Uuid) -> Option<SavedSession> {
    match read(path) {
        Ok(file) => file.and_then(|mut f| f.sessions.remove(&profile)),
        Err(()) => {
            if let Err(e) = std::fs::rename(path, bad_path(path)) {
                tracing::warn!(error = %e, "couldn't set the saved session aside");
            }
            None
        }
    }
}

/// Save `profile`'s session, keeping the other profiles'. The write is atomic: a temporary file
/// beside it, then a rename.
pub fn save(path: &Path, profile: Uuid, session: &SavedSession) -> io::Result<()> {
    let mut file = read(path).ok().flatten().unwrap_or(SessionFile {
        version: VERSION,
        sessions: BTreeMap::new(),
    });
    file.sessions.insert(profile, session.clone());
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp_name = path.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);
    std::fs::write(&tmp, serde_json::to_vec(&file).map_err(io::Error::other)?)?;
    std::fs::rename(&tmp, path)
}
