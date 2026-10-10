//! Viewing rules and the play-history queue (spec FR-014 to FR-017; contracts/play-history.md).
//!
//! A [`Viewing`] turns what the player reports (playing or not, position, duration) into the
//! reports the Stash web UI's player would send: a checkpoint after every minute of actual
//! playing, and a final one when the viewing ends. The [`HistoryQueue`] keeps every report on disk
//! until Stash confirms it, so a crash, a quit, or a lost connection never loses play history.

use std::collections::{BTreeMap, HashSet};
use std::future::Future;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::shell::store::{JsonStore, Versioned};

/// Playing time between checkpoints, in seconds (the web UI's interval).
pub const CHECKPOINT_SECONDS: f64 = 60.0;
/// A resume point at or past this share of the duration is saved as 0 (watched to the end).
pub const COMPLETE_SHARE: f64 = 0.98;
/// How long a quit or a server switch waits for the queue to send.
pub const FLUSH_DEADLINE: Duration = Duration::from_secs(2);
/// The longest gap between two player updates counted as playing (a suspended machine, or a
/// stalled event loop, doesn't count as watching).
const MAX_STEP_SECONDS: f64 = 5.0;

/// One write to Stash about a viewing: `sceneSaveActivity` with this resume point and the playing
/// time since the previous report, plus `sceneAddPlay` when `add_play` is set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayHistoryReport {
    pub scene_id: String,
    /// Seconds; 0 when the position was at 98% of the duration or more.
    pub resume_time: f64,
    /// Playing time since the previous report, in seconds (Stash adds it up).
    pub play_duration: f64,
    pub add_play: bool,
    /// The viewing's last report (not stored: a queued report is just a report).
    #[serde(skip)]
    pub ended: bool,
    /// RFC 3339.
    pub recorded_at: String,
}

/// The resume point to save for `position` in a file `duration` seconds long.
pub fn resume_point(position: f64, duration: f64) -> f64 {
    if !position.is_finite() || position <= 0.0 {
        return 0.0;
    }
    if duration > 0.0 && position >= COMPLETE_SHARE * duration {
        0.0
    } else {
        position
    }
}

/// One viewing of one scene, from the moment it starts playing until it ends.
#[derive(Debug, Clone, PartialEq)]
pub struct Viewing {
    pub scene_id: String,
    /// The server's `minimumPlayPercent` (0 when unset, as the web UI).
    minimum_percent: f64,
    duration: f64,
    position: f64,
    /// The clock at the last update, and whether the player was playing then.
    last: Option<(f64, bool)>,
    played_total: f64,
    played_since_save: f64,
    add_play_sent: bool,
    ended: bool,
}

impl Viewing {
    /// A viewing of `scene_id`, or `None` when "Save play history" is off (nothing is recorded).
    pub fn start(scene_id: &str, minimum_percent: Option<f64>, save_history: bool) -> Option<Self> {
        save_history.then(|| Self {
            scene_id: scene_id.to_owned(),
            minimum_percent: minimum_percent
                .filter(|p| p.is_finite())
                .unwrap_or(0.0)
                .max(0.0),
            duration: 0.0,
            position: 0.0,
            last: None,
            played_total: 0.0,
            played_since_save: 0.0,
            add_play_sent: false,
            ended: false,
        })
    }

    /// Total playing time so far, in seconds.
    pub fn played(&self) -> f64 {
        self.played_total
    }

    /// The player at `now` (seconds on a monotonic clock): whether it's playing, where, and how
    /// long the file is. Returns a checkpoint report once a minute of playing has gone by.
    pub fn observe(
        &mut self,
        now: f64,
        playing: bool,
        position: f64,
        duration: Option<f64>,
    ) -> Option<PlayHistoryReport> {
        if self.ended {
            return None;
        }
        self.count_until(now);
        self.last = Some((now, playing));
        if position.is_finite() {
            self.position = position;
        }
        if let Some(d) = duration.filter(|d| d.is_finite() && *d > 0.0) {
            self.duration = d;
        }
        (self.played_since_save >= CHECKPOINT_SECONDS).then(|| self.report(false))
    }

    /// The viewing ended at `now` (stop, close, another scene, tab close, server switch, quit,
    /// disconnect): the final report, or `None` if nothing was played.
    pub fn end(&mut self, now: f64) -> Option<PlayHistoryReport> {
        if self.ended {
            return None;
        }
        self.count_until(now);
        self.ended = true;
        (self.played_total > 0.0).then(|| self.report(true))
    }

    fn count_until(&mut self, now: f64) {
        if let Some((then, true)) = self.last {
            let step = (now - then).clamp(0.0, MAX_STEP_SECONDS);
            self.played_total += step;
            self.played_since_save += step;
        }
        if let Some((then, _)) = self.last.as_mut() {
            *then = now;
        }
    }

    fn report(&mut self, ended: bool) -> PlayHistoryReport {
        let share = if self.duration > 0.0 {
            self.played_total / self.duration * 100.0
        } else {
            0.0
        };
        let add_play = !self.add_play_sent && share >= self.minimum_percent;
        if add_play {
            self.add_play_sent = true;
        }
        let play_duration = std::mem::take(&mut self.played_since_save);
        PlayHistoryReport {
            scene_id: self.scene_id.clone(),
            resume_time: resume_point(self.position, self.duration),
            play_duration,
            add_play,
            ended,
            recorded_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

// ---- The queue ------------------------------------------------------------------------------

/// `play-history.json`: reports not yet confirmed by Stash, by profile id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueFile {
    pub version: u32,
    pub pending: BTreeMap<String, Vec<PlayHistoryReport>>,
}

impl Default for QueueFile {
    fn default() -> Self {
        Self {
            version: Self::VERSION,
            pending: BTreeMap::new(),
        }
    }
}

impl Versioned for QueueFile {
    const VERSION: u32 = 1;
}

/// The reports waiting to be sent, kept on disk (written atomically on every change).
#[derive(Debug)]
pub struct HistoryQueue {
    store: JsonStore<QueueFile>,
    file: QueueFile,
    /// Profiles whose first report is being sent (it isn't merged into while in flight).
    in_flight: HashSet<String>,
}

impl HistoryQueue {
    pub fn open(path: impl Into<PathBuf>) -> Self {
        let store = JsonStore::<QueueFile>::new(path);
        let file = store.load();
        Self {
            store,
            file,
            in_flight: HashSet::new(),
        }
    }

    /// The reports waiting for `profile`, oldest first.
    pub fn pending(&self, profile: &str) -> &[PlayHistoryReport] {
        self.file.pending.get(profile).map_or(&[], Vec::as_slice)
    }

    /// Profiles with something waiting.
    pub fn profiles(&self) -> Vec<String> {
        self.file
            .pending
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Queue `report` for `profile` and write the file (before it's sent). A waiting report for
    /// the same scene takes it in: durations add, the latest resume point wins, and a play counted
    /// by either is kept.
    pub fn push(&mut self, profile: &str, report: PlayHistoryReport) -> Result<(), AppError> {
        self.stage(profile, report);
        self.save()
    }

    /// [`Self::push`] without writing the file: a UI queues in memory at once (so a flush that
    /// follows sees it) and leaves the write to [`Self::persist`] off its thread.
    pub fn stage(&mut self, profile: &str, report: PlayHistoryReport) {
        let busy = self.in_flight.contains(profile);
        let list = self.file.pending.entry(profile.to_owned()).or_default();
        let first_free = usize::from(busy);
        match list
            .iter_mut()
            .skip(first_free)
            .rev()
            .find(|r| r.scene_id == report.scene_id)
        {
            Some(waiting) => {
                waiting.play_duration += report.play_duration;
                waiting.resume_time = report.resume_time;
                waiting.add_play |= report.add_play;
                waiting.recorded_at = report.recorded_at;
            }
            None => list.push(report),
        }
    }

    /// Write the file (atomically).
    pub fn persist(&self) -> Result<(), AppError> {
        self.save()
    }

    /// The next report to send for `profile`, marked in flight.
    pub fn next(&mut self, profile: &str) -> Option<PlayHistoryReport> {
        if self.in_flight.contains(profile) {
            return None;
        }
        let first = self.file.pending.get(profile)?.first()?.clone();
        self.in_flight.insert(profile.to_owned());
        Some(first)
    }

    /// Stash confirmed the report in flight: drop it.
    pub fn confirm(&mut self, profile: &str) -> Result<(), AppError> {
        if self.in_flight.remove(profile) {
            if let Some(list) = self.file.pending.get_mut(profile) {
                if !list.is_empty() {
                    list.remove(0);
                }
                if list.is_empty() {
                    self.file.pending.remove(profile);
                }
            }
        }
        self.save()
    }

    /// Sending the report in flight failed: it stays first in line.
    pub fn release(&mut self, profile: &str) {
        self.in_flight.remove(profile);
    }

    fn save(&self) -> Result<(), AppError> {
        self.store.save(&self.file)
    }
}

/// What a flush did.
#[derive(Debug, Clone, PartialEq)]
pub struct FlushOutcome {
    pub sent: usize,
    /// Why sending stopped early, if it did (the rest stays queued).
    pub failed: Option<String>,
    pub left: usize,
}

/// Send `profile`'s reports in order through `send` until the queue is empty, a send fails, or
/// `deadline` passes. Each confirmed report leaves the queue; the rest stay for next time.
pub async fn flush<F, Fut, T>(
    queue: &Mutex<HistoryQueue>,
    profile: &str,
    deadline: Option<Instant>,
    send: F,
) -> FlushOutcome
where
    F: Fn(PlayHistoryReport) -> Fut,
    Fut: Future<Output = Result<T, AppError>>,
{
    let lock = || {
        queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    };
    let mut sent = 0;
    let mut failed = None;
    loop {
        let remaining = deadline.map(|d| d.saturating_duration_since(Instant::now()));
        if remaining.is_some_and(|r| r.is_zero()) {
            break;
        }
        let Some(report) = lock().next(profile) else {
            break;
        };
        let result = match remaining {
            Some(r) => tokio::time::timeout(r, send(report))
                .await
                .unwrap_or_else(|_| {
                    Err(AppError::Internal {
                        message: "play history flush deadline passed".into(),
                    })
                }),
            None => send(report).await,
        };
        match result {
            Ok(_) => {
                if let Err(e) = lock().confirm(profile) {
                    tracing::warn!(error = %e, "couldn't update the play history queue");
                }
                sent += 1;
            }
            Err(e) => {
                lock().release(profile);
                failed = Some(e.to_string());
                break;
            }
        }
    }
    let left = lock().pending(profile).len();
    FlushOutcome { sent, failed, left }
}
