//! Where the native app keeps its files (007 research R4): its own directories, named by its app
//! id, under each platform directory. It never opens the demo's `semantic-stash-viewer/` folders
//! (spec FR-005).

use std::path::{Path, PathBuf};

/// The app id: the directory name under each platform directory, and the window's Wayland
/// `app_id` / X11 class.
pub const APP_ID: &str = "dev.semantic-stash-viewer";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub config: PathBuf,
    pub data: PathBuf,
    pub cache: PathBuf,
}

impl Paths {
    /// From the XDG variables, falling back to `$HOME` as the XDG spec says.
    pub fn resolve() -> Option<Self> {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let xdg = |var: &str, fallback: &str| -> Option<PathBuf> {
            std::env::var_os(var)
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .or_else(|| home.as_ref().map(|h| h.join(fallback)))
        };
        Some(Self::under(
            &xdg("XDG_CONFIG_HOME", ".config")?,
            &xdg("XDG_DATA_HOME", ".local/share")?,
            &xdg("XDG_CACHE_HOME", ".cache")?,
        ))
    }

    /// The app's directories under the given platform directories.
    pub fn under(config: &Path, data: &Path, cache: &Path) -> Self {
        Self {
            config: config.join(APP_ID),
            data: data.join(APP_ID),
            cache: cache.join(APP_ID),
        }
    }

    pub fn profiles(&self) -> PathBuf {
        self.config.join("profiles.json")
    }

    /// The saved session (contracts/session-snapshot.md).
    pub fn session(&self) -> PathBuf {
        self.data.join("session.json")
    }

    pub fn notifications(&self) -> PathBuf {
        self.data.join("notifications.json")
    }

    pub fn logs(&self) -> PathBuf {
        self.data.join("logs")
    }

    /// Per-profile view caches (`<cache>/<profile>/cache.sqlite3`).
    pub fn cache_root(&self) -> PathBuf {
        self.cache.clone()
    }
}
