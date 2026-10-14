//! The viewer's neighbour files (012 research R4, data model "Neighbour files"): the full files of
//! the image shown and the ones either side, on local disk, so a step opens a local path in mpv and
//! never waits on the network.
//!
//! A bounded directory in the profile's cache (`images/<id>.<ext>`): at most 8 files or 512 MB,
//! least recently used out. Files are fetched with the key in a header and written as they arrive
//! (a large file never sits in memory). The directory starts empty each session.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use crate::adapter::{endpoint, StashClient};
use crate::error::AppError;

/// The most files kept.
pub const MAX_FILES: usize = 8;
/// The most bytes kept (also the largest single file).
pub const MAX_BYTES: u64 = 512 * 1024 * 1024;

/// One image's file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Fetching,
    Ready(PathBuf),
    /// Stash said 404: the image was deleted.
    Missing,
    Failed,
}

#[derive(Debug)]
struct Entry {
    state: State,
    size: u64,
    used: u64,
}

#[derive(Debug, Default)]
struct Inner {
    entries: HashMap<String, Entry>,
    tick: u64,
}

#[derive(Debug)]
pub struct Neighbours {
    dir: PathBuf,
    max_files: usize,
    max_bytes: u64,
    inner: Mutex<Inner>,
}

/// The file name's extension for a content type (mpv probes the content; this is for people).
fn extension(content_type: &str) -> &'static str {
    match content_type.split(';').next().unwrap_or("").trim() {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/avif" => "avif",
        "image/apng" => "apng",
        "image/svg+xml" => "svg",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/quicktime" => "mov",
        _ => "bin",
    }
}

/// Stash's image ids are digits; anything else never reaches a file name.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 20 && id.bytes().all(|b| b.is_ascii_digit())
}

impl Neighbours {
    /// The store in `dir` (created, and emptied of an earlier session's files).
    pub fn open(dir: impl Into<PathBuf>) -> Self {
        Self::with_limits(dir, MAX_FILES, MAX_BYTES)
    }

    pub fn with_limits(dir: impl Into<PathBuf>, max_files: usize, max_bytes: u64) -> Self {
        let dir = dir.into();
        let _ = std::fs::remove_dir_all(&dir);
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!(error = %e, "couldn't create the image neighbours directory");
        }
        Self {
            dir,
            max_files: max_files.max(1),
            max_bytes,
            inner: Mutex::new(Inner::default()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The image's file if it's here (counts as a use).
    pub fn ready(&self, id: &str) -> Option<PathBuf> {
        let mut inner = self.lock();
        inner.tick += 1;
        let tick = inner.tick;
        let entry = inner.entries.get_mut(id)?;
        match &entry.state {
            State::Ready(path) => {
                entry.used = tick;
                Some(path.clone())
            }
            _ => None,
        }
    }

    pub fn state(&self, id: &str) -> Option<State> {
        self.lock().entries.get(id).map(|e| e.state.clone())
    }

    /// The bytes on disk.
    pub fn size_bytes(&self) -> u64 {
        self.lock()
            .entries
            .values()
            .filter(|e| matches!(e.state, State::Ready(_)))
            .map(|e| e.size)
            .sum()
    }

    /// Delete every file; returns the bytes freed.
    pub fn clear(&self) -> u64 {
        let mut inner = self.lock();
        let mut freed = 0;
        for entry in inner.entries.values() {
            if let State::Ready(path) = &entry.state {
                freed += entry.size;
                let _ = std::fs::remove_file(path);
            }
        }
        inner.entries.clear();
        freed
    }

    /// Fetch the image's file unless it's here or on its way. `Ok(Some(path))` when it's ready
    /// (now or already), `Ok(None)` when Stash has no such image (or it's already being fetched).
    pub async fn fetch(&self, client: &StashClient, id: &str) -> Result<Option<PathBuf>, AppError> {
        if !valid_id(id) {
            return Err(AppError::Internal {
                message: "not an image id".into(),
            });
        }
        {
            let mut inner = self.lock();
            inner.tick += 1;
            let tick = inner.tick;
            match inner.entries.get_mut(id) {
                Some(Entry {
                    state: State::Ready(path),
                    used,
                    ..
                }) => {
                    *used = tick;
                    return Ok(Some(path.clone()));
                }
                Some(Entry {
                    state: State::Fetching | State::Missing,
                    ..
                }) => return Ok(None),
                _ => {}
            }
            inner.entries.insert(
                id.to_owned(),
                Entry {
                    state: State::Fetching,
                    size: 0,
                    used: tick,
                },
            );
        }
        let result = self.download(client, id).await;
        let mut inner = self.lock();
        inner.tick += 1;
        let tick = inner.tick;
        let state = match &result {
            Ok(Some((path, size))) => {
                inner.entries.insert(
                    id.to_owned(),
                    Entry {
                        state: State::Ready(path.clone()),
                        size: *size,
                        used: tick,
                    },
                );
                Self::evict(&mut inner, self.max_files, self.max_bytes, id);
                return Ok(Some(path.clone()));
            }
            Ok(None) => State::Missing,
            Err(_) => State::Failed,
        };
        inner.entries.insert(
            id.to_owned(),
            Entry {
                state,
                size: 0,
                used: tick,
            },
        );
        result.map(|_| None)
    }

    /// GET `/image/<id>/image` with the key in a header into `<dir>/<id>.<ext>`; `None` on a 404.
    async fn download(
        &self,
        client: &StashClient,
        id: &str,
    ) -> Result<Option<(PathBuf, u64)>, AppError> {
        let mut url = endpoint(client.base_url(), "image");
        if let Ok(mut segments) = url.path_segments_mut() {
            segments.push(id).push("image");
        }
        let started = std::time::Instant::now();
        let mut response = client
            .get_with_key(url)
            .send()
            .await
            .map_err(|e| AppError::from(client.classify_transport_error(&e)))?;
        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            client.record_get("image", 0, started);
            return Ok(None);
        }
        if !status.is_success() {
            client.record_get("image", 0, started);
            return Err(AppError::Internal {
                message: format!("the image file wasn't served ({status})"),
            });
        }
        let too_big = || AppError::Internal {
            message: "the image file is larger than the viewer keeps".into(),
        };
        if response
            .content_length()
            .is_some_and(|len| len > self.max_bytes)
        {
            client.record_get("image", 0, started);
            return Err(too_big());
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        // A login page (Stash redirects a request without the key) is never an image.
        if content_type.starts_with("text/") {
            client.record_get("image", 0, started);
            return Err(AppError::Internal {
                message: "the server answered with a page, not the image".into(),
            });
        }
        let path = self.dir.join(format!("{id}.{}", extension(&content_type)));
        let partial = self.dir.join(format!("{id}.part"));
        let io = |e: std::io::Error| AppError::Internal {
            message: format!("couldn't write the image file: {e}"),
        };
        let mut file = std::fs::File::create(&partial).map_err(io)?;
        let mut size: u64 = 0;
        let written: Result<(), AppError> = async {
            while let Some(chunk) = response.chunk().await.map_err(|e| AppError::Internal {
                message: format!("couldn't read the image file: {e}"),
            })? {
                size += chunk.len() as u64;
                if size > self.max_bytes {
                    return Err(too_big());
                }
                file.write_all(&chunk).map_err(io)?;
            }
            file.flush().map_err(io)?;
            Ok(())
        }
        .await;
        drop(file);
        client.record_get("image", size as usize, started);
        if let Err(e) = written {
            let _ = std::fs::remove_file(&partial);
            return Err(e);
        }
        std::fs::rename(&partial, &path).map_err(io)?;
        Ok(Some((path, size)))
    }

    /// Drop the least recently used files past the limits, never `keep` (just fetched).
    fn evict(inner: &mut Inner, max_files: usize, max_bytes: u64, keep: &str) {
        loop {
            let ready: Vec<(&String, &Entry)> = inner
                .entries
                .iter()
                .filter(|(_, e)| matches!(e.state, State::Ready(_)))
                .collect();
            let total: u64 = ready.iter().map(|(_, e)| e.size).sum();
            if ready.len() <= max_files && total <= max_bytes {
                return;
            }
            let Some(oldest) = ready
                .iter()
                .filter(|(id, _)| id.as_str() != keep)
                .min_by_key(|(_, e)| e.used)
                .map(|(id, _)| (*id).clone())
            else {
                return;
            };
            if let Some(Entry {
                state: State::Ready(path),
                ..
            }) = inner.entries.remove(&oldest)
            {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_types_name_the_files() {
        assert_eq!(extension("image/jpeg"), "jpg");
        assert_eq!(extension("image/webp; charset=binary"), "webp");
        assert_eq!(extension("video/mp4"), "mp4");
        assert_eq!(extension(""), "bin");
    }

    #[test]
    fn only_digit_ids_become_file_names() {
        assert!(valid_id("1238"));
        assert!(!valid_id("../x"));
        assert!(!valid_id(""));
    }
}
