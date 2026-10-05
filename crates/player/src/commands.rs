//! Playback commands (FR-008) and the worker that keeps them off the caller's thread
//! (contracts/player-commands.md, invariant 1).
//!
//! `Player::dispatch` queues a command and returns immediately; a dedicated `mpv-commands`
//! thread applies it. The synchronous `Player` methods (used by tests) apply the same logic
//! directly, so there's one implementation of each command.

use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::session::{lock, Inner, MAX_SPEED, MIN_SPEED};

/// After this long, an unsettled seek no longer delays coalesced drag seeks.
const STALE_SEEK: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FrameDirection {
    Forward,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayerCommand {
    TogglePause,
    SetPaused(bool),
    /// Absolute seek to the exact frame. `exact: false` marks a drag position (coalesced);
    /// `exact: true` is the release, which supersedes queued drag positions.
    Seek {
        position_seconds: f64,
        exact: bool,
    },
    SeekRelative(f64),
    /// Clamped to 0.25–4.0; pitch is preserved (`audio-pitch-correction`).
    SetSpeed(f64),
    /// Clamped to 0–100.
    SetVolume(f64),
    SetMuted(bool),
    /// Only while paused; queued until an in-flight seek settles.
    FrameStep(FrameDirection),
    /// Back to the start and play (FR-015).
    Replay,
}

impl Inner {
    pub(crate) fn apply(&self, command: PlayerCommand) {
        let mpv = &self.mpv;
        match command {
            PlayerCommand::TogglePause => {
                let _ = mpv.command("cycle", &["pause"]);
            }
            PlayerCommand::SetPaused(paused) => {
                let _ = mpv.set_property("pause", paused);
            }
            PlayerCommand::Seek {
                position_seconds,
                exact,
            } => {
                // All seeks land on the exact frame (see `seek_now`). Drag seeks (`exact:
                // false`) are coalesced: while one is settling, keep only the latest position
                // and send it on `playback-restart`. The release seek (`exact: true`) goes now
                // and supersedes anything pending, unless mpv already went to that exact spot.
                // A seek that hasn't settled after `STALE_SEEK` no longer holds others back.
                let in_flight = self.seeking.load(Ordering::SeqCst)
                    && lock(&self.seek_started).elapsed() < STALE_SEEK;
                if exact {
                    lock(&self.pending_seek).take();
                    let same_spot = lock(&self.last_seek_target)
                        .is_some_and(|last| (last - position_seconds.max(0.0)).abs() < 1e-3);
                    if !same_spot {
                        self.seek_now(position_seconds);
                    }
                } else if in_flight {
                    *lock(&self.pending_seek) = Some(position_seconds);
                } else {
                    self.seek_now(position_seconds);
                }
            }
            PlayerCommand::SeekRelative(seconds) => {
                lock(&self.last_seek_target).take();
                let offset = format!("{seconds:.3}");
                self.seeking.store(true, Ordering::SeqCst);
                let _ = mpv.command("seek", &[&offset, "relative+exact"]);
            }
            PlayerCommand::SetSpeed(speed) => {
                let _ = mpv.set_property("speed", speed.clamp(MIN_SPEED, MAX_SPEED));
            }
            PlayerCommand::SetVolume(volume) => {
                let _ = mpv.set_property("volume", volume.clamp(0.0, 100.0));
            }
            PlayerCommand::SetMuted(muted) => {
                let _ = mpv.set_property("mute", muted);
            }
            PlayerCommand::FrameStep(direction) => self.frame_step(match direction {
                FrameDirection::Forward => "frame-step",
                FrameDirection::Back => "frame-back-step",
            }),
            PlayerCommand::Replay => {
                lock(&self.last_seek_target).take();
                // Unpause now, and again once the seek settles: at end of file with
                // `keep-open`, mpv can re-pause if the unpause lands before the seek does. The
                // second unpause is a no-op when the first one stuck.
                self.resume_after_seek.store(true, Ordering::SeqCst);
                self.seeking.store(true, Ordering::SeqCst);
                let _ = mpv.command("seek", &["0", "absolute+exact"]);
                let _ = mpv.set_property("pause", false);
            }
        }
    }
}

/// Start the `mpv-commands` worker. It exits when the returned sender is dropped.
pub(crate) fn spawn_worker(
    inner: Arc<Inner>,
) -> std::io::Result<(mpsc::Sender<PlayerCommand>, JoinHandle<()>)> {
    let (tx, rx) = mpsc::channel::<PlayerCommand>();
    let handle = std::thread::Builder::new()
        .name("mpv-commands".into())
        .spawn(move || {
            for command in rx {
                inner.apply(command);
            }
        })?;
    Ok((tx, handle))
}
