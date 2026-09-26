//! `Player`: one mpv core, its options, commands, and property observation
//! (data-model.md "PlayerSession"; research R5).
//!
//! A single event thread reads mpv events and turns them into `PlayerSnapshot`s on a
//! `watch` channel. Position updates are throttled to ≥ 250 ms apart while playing; everything
//! else (and any position change while paused) is sent immediately.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use libmpv2::events::{Event, PropertyData};
use libmpv2::{mpv_end_file_reason, Format, Mpv};
use tokio::sync::watch;

use crate::commands::{spawn_worker, FrameDirection, PlayerCommand};
use crate::error::PlayerError;
use crate::snapshot::{PlayerSnapshot, PlayerStateKind, PlayerStats};
use crate::tracks::read_tracks;

/// Minimum spacing between position-only updates while playing.
const POSITION_THROTTLE: Duration = Duration::from_millis(250);
/// Playback speed limits (FR-008).
pub const MIN_SPEED: f64 = 0.25;
pub const MAX_SPEED: f64 = 4.0;

#[derive(Debug, Clone, Copy)]
pub struct PlayerConfig {
    /// `vo=null`/`ao=null` for tests; otherwise `vo=libmpv` (render API) and normal audio.
    pub headless: bool,
}

impl PlayerConfig {
    /// No window and no audio, for tests.
    pub fn headless() -> Self {
        Self { headless: true }
    }

    /// Render through the libmpv render API (see `render.rs`).
    pub fn render() -> Self {
        Self { headless: false }
    }
}

/// What to play and how to reach it.
#[derive(Debug, Clone)]
pub struct OpenRequest {
    /// URL or path. The Tauri layer only ever passes Stash's direct stream (FR-003).
    pub source: String,
    pub scene_id: Option<String>,
    pub title: Option<String>,
    /// Sent as an `ApiKey` HTTP header, never in the URL.
    pub api_key: Option<String>,
    /// Profile's strict certificate checking (`tls-verify`).
    pub strict_tls: bool,
    /// Demuxer cache for this file, or `None` for mpv's defaults (research R5c).
    pub cache: Option<CacheLimits>,
}

/// Demuxer cache limits: `demuxer-max-bytes` and `demuxer-max-back-bytes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheLimits {
    pub forward_bytes: u64,
    pub back_bytes: u64,
}

/// mpv's defaults for `demuxer-max-bytes` / `demuxer-max-back-bytes`.
const DEFAULT_CACHE: (&str, &str) = ("150MiB", "50MiB");

pub(crate) struct Inner {
    pub(crate) mpv: Mpv,
    state: Mutex<PlayerSnapshot>,
    tx: watch::Sender<PlayerSnapshot>,
    shutdown: AtomicBool,
    last_position_emit: Mutex<Instant>,
    /// A seek is in flight (until mpv's `playback-restart`). mpv drops frame steps sent during
    /// a seek, so they're queued in `pending_step` and sent once it settles.
    pub(crate) seeking: AtomicBool,
    pub(crate) pending_step: Mutex<Option<&'static str>>,
    /// Unpause again once the in-flight seek settles (replay). An unpause sent together with the
    /// seek can race it: at end of file with `keep-open`, mpv would pause itself again.
    pub(crate) resume_after_seek: AtomicBool,
    /// Latest drag seek waiting for the in-flight one to settle, and when that one started.
    /// Coalescing drag seeks stops scrubbing from flooding mpv (visible flicker on high-res
    /// files, where each seek restarts decoding from a keyframe).
    pub(crate) pending_seek: Mutex<Option<f64>>,
    pub(crate) seek_started: Mutex<Instant>,
    /// Target of the last seek sent to mpv, so a release on the same spot isn't re-decoded.
    pub(crate) last_seek_target: Mutex<Option<f64>>,
}

/// One mpv core. At most one scene plays at a time.
pub struct Player {
    pub(crate) inner: Arc<Inner>,
    events: Option<JoinHandle<()>>,
    /// Queue for `dispatch`; `None` once dropped (stops the worker).
    commands: Option<std::sync::mpsc::Sender<PlayerCommand>>,
    command_worker: Option<JoinHandle<()>>,
}

// Observed property ids.
const P_TIME_POS: u64 = 1;
const P_DURATION: u64 = 2;
const P_PAUSE: u64 = 3;
const P_SPEED: u64 = 4;
const P_VOLUME: u64 = 5;
const P_MUTE: u64 = 6;
const P_EOF: u64 = 7;
const P_HWDEC: u64 = 8;
const P_TRACKS: u64 = 9;

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// libmpv refuses to start (`mpv_create` returns NULL) unless `LC_NUMERIC` is `"C"`, so that
/// numbers in options parse consistently. GTK sets the process locale from the environment at
/// startup, so restore `LC_NUMERIC` right before creating mpv. Other locale categories are left
/// alone.
fn force_c_numeric_locale() {
    // SAFETY: setlocale with a static NUL-terminated string. Called on the thread that creates
    // mpv, before mpv or its threads exist.
    unsafe {
        libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr());
    }
}

fn mpv_error(context: &str, e: &libmpv2::Error) -> PlayerError {
    PlayerError::PlaybackFailed {
        detail: format!("{context}: {e}"),
    }
}

impl Player {
    pub fn new(config: PlayerConfig) -> Result<Self, PlayerError> {
        force_c_numeric_locale();
        let mpv = Mpv::with_initializer(|init| {
            init.set_option("config", false)?;
            init.set_option("terminal", false)?;
            init.set_option("idle", "yes")?;
            init.set_option("load-scripts", false)?;
            init.set_option("input-default-bindings", false)?;
            init.set_option("osd-level", 0i64)?;
            init.set_option("ytdl", false)?;
            // Keep the last frame at the end so the UI can offer Replay (FR-015).
            init.set_option("keep-open", "yes")?;
            init.set_option("audio-pitch-correction", true)?;
            init.set_option("hwdec", "auto-safe")?;
            if config.headless {
                init.set_option("vo", "null")?;
                init.set_option("ao", "null")?;
            } else {
                init.set_option("vo", "libmpv")?;
            }
            Ok(())
        })
        .map_err(|e| mpv_error("mpv failed to start", &e))?;

        for (name, format, id) in [
            ("time-pos", Format::Double, P_TIME_POS),
            ("duration", Format::Double, P_DURATION),
            ("pause", Format::Flag, P_PAUSE),
            ("speed", Format::Double, P_SPEED),
            ("volume", Format::Double, P_VOLUME),
            ("mute", Format::Flag, P_MUTE),
            ("eof-reached", Format::Flag, P_EOF),
            ("hwdec-current", Format::String, P_HWDEC),
            ("track-list/count", Format::Int64, P_TRACKS),
        ] {
            mpv.observe_property(name, format, id)
                .map_err(|e| mpv_error("observe property", &e))?;
        }

        let (tx, _) = watch::channel(PlayerSnapshot::default());
        let inner = Arc::new(Inner {
            mpv,
            state: Mutex::new(PlayerSnapshot::default()),
            tx,
            shutdown: AtomicBool::new(false),
            last_position_emit: Mutex::new(Instant::now()),
            seeking: AtomicBool::new(false),
            pending_step: Mutex::new(None),
            resume_after_seek: AtomicBool::new(false),
            pending_seek: Mutex::new(None),
            seek_started: Mutex::new(Instant::now()),
            last_seek_target: Mutex::new(None),
        });

        let for_thread = Arc::clone(&inner);
        let events = std::thread::Builder::new()
            .name("mpv-events".into())
            .spawn(move || for_thread.event_loop())
            .map_err(|e| PlayerError::PlaybackFailed {
                detail: format!("could not start the mpv event thread: {e}"),
            })?;

        let (commands, command_worker) =
            spawn_worker(Arc::clone(&inner)).map_err(|e| PlayerError::PlaybackFailed {
                detail: format!("could not start the mpv command thread: {e}"),
            })?;

        Ok(Self {
            inner,
            events: Some(events),
            commands: Some(commands),
            command_worker: Some(command_worker),
        })
    }

    pub fn subscribe(&self) -> watch::Receiver<PlayerSnapshot> {
        self.inner.tx.subscribe()
    }

    pub fn snapshot(&self) -> PlayerSnapshot {
        lock(&self.inner.state).clone()
    }

    /// Start playing `request.source`. Errors arrive as `state: error` snapshots.
    pub fn open(&self, request: OpenRequest) {
        let inner = &self.inner;
        inner.update(|s| {
            *s = PlayerSnapshot {
                scene_id: request.scene_id.clone(),
                title: request.title.clone(),
                state: PlayerStateKind::Loading,
                volume: s.volume,
                muted: s.muted,
                speed: 1.0,
                fullscreen: s.fullscreen,
                ..PlayerSnapshot::default()
            };
        });

        let header = request
            .api_key
            .as_deref()
            .map(|key| format!("ApiKey: {key}"))
            .unwrap_or_default();
        let (forward, back) = request.cache.map_or(
            (DEFAULT_CACHE.0.to_owned(), DEFAULT_CACHE.1.to_owned()),
            |c| (c.forward_bytes.to_string(), c.back_bytes.to_string()),
        );
        let result = inner
            .mpv
            .set_property("http-header-fields", header.as_str())
            .and_then(|()| {
                inner
                    .mpv
                    .set_property("demuxer-max-bytes", forward.as_str())
            })
            .and_then(|()| {
                inner
                    .mpv
                    .set_property("demuxer-max-back-bytes", back.as_str())
            })
            .and_then(|()| inner.mpv.set_property("tls-verify", request.strict_tls))
            .and_then(|()| inner.mpv.set_property("speed", 1.0))
            .and_then(|()| inner.mpv.set_property("pause", false))
            .and_then(|()| inner.mpv.command("loadfile", &[&request.source, "replace"]));
        if let Err(e) = result {
            inner.fail(PlayerError::from_mpv(&e));
        }
    }

    /// Stop playback and release audio/video (FR-007).
    pub fn close(&self) {
        let _ = self.inner.mpv.command("stop", &[]);
        self.inner.update(|s| {
            *s = PlayerSnapshot {
                volume: s.volume,
                muted: s.muted,
                fullscreen: s.fullscreen,
                ..PlayerSnapshot::default()
            };
        });
    }

    /// Queue a command for the `mpv-commands` worker and return immediately (contract
    /// invariant 1: a UI click never waits on mpv).
    pub fn dispatch(&self, command: PlayerCommand) {
        if let Some(tx) = &self.commands {
            let _ = tx.send(command);
        }
    }

    pub fn toggle_pause(&self) {
        self.inner.apply(PlayerCommand::TogglePause);
    }

    pub fn set_paused(&self, paused: bool) {
        self.inner.apply(PlayerCommand::SetPaused(paused));
    }

    /// Absolute seek. `exact` for a precise landing; otherwise keyframe-fast.
    pub fn seek(&self, position_seconds: f64, exact: bool) {
        self.inner.apply(PlayerCommand::Seek {
            position_seconds,
            exact,
        });
    }

    pub fn seek_relative(&self, seconds: f64) {
        self.inner.apply(PlayerCommand::SeekRelative(seconds));
    }

    /// Clamped to 0.25–4.0; pitch is preserved.
    pub fn set_speed(&self, speed: f64) {
        self.inner.apply(PlayerCommand::SetSpeed(speed));
    }

    /// Clamped to 0–100.
    pub fn set_volume(&self, volume: f64) {
        self.inner.apply(PlayerCommand::SetVolume(volume));
    }

    pub fn set_muted(&self, muted: bool) {
        self.inner.apply(PlayerCommand::SetMuted(muted));
    }

    /// One frame forward. Ignored unless paused (mpv would pause as a side effect).
    pub fn frame_step_forward(&self) {
        self.inner
            .apply(PlayerCommand::FrameStep(FrameDirection::Forward));
    }

    /// One frame back. Ignored unless paused.
    pub fn frame_step_back(&self) {
        self.inner
            .apply(PlayerCommand::FrameStep(FrameDirection::Back));
    }

    /// From Ended (or anywhere): back to the start and play (FR-015).
    pub fn replay(&self) {
        self.inner.apply(PlayerCommand::Replay);
    }

    /// Measurements for the decision record (debug builds expose this to the UI).
    pub fn stats(&self) -> PlayerStats {
        let count = |name: &str| self.inner.mpv.get_property::<i64>(name).unwrap_or(0);
        let dropped = count("frame-drop-count") + count("decoder-frame-drop-count");
        PlayerStats {
            dropped_frames: i32::try_from(dropped).unwrap_or(i32::MAX),
            hwdec: self.snapshot().hwdec,
            ..PlayerStats::default()
        }
    }

    /// Mirror the window's fullscreen state into snapshots (the window itself is toggled by
    /// the host).
    pub fn set_fullscreen_flag(&self, fullscreen: bool) {
        self.inner.update(|s| s.fullscreen = fullscreen);
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        // Closing the queue ends the command worker once it has drained.
        self.commands.take();
        if let Some(handle) = self.command_worker.take() {
            let _ = handle.join();
        }
        self.inner.shutdown.store(true, Ordering::SeqCst);
        if let Some(handle) = self.events.take() {
            let _ = handle.join();
        }
    }
}

impl Inner {
    /// Apply a change and publish it.
    fn update(&self, change: impl FnOnce(&mut PlayerSnapshot)) {
        let snapshot = {
            let mut state = lock(&self.state);
            change(&mut state);
            state.clone()
        };
        *lock(&self.last_position_emit) = Instant::now();
        self.tx.send_replace(snapshot);
    }

    /// Send a frame-step command now, or queue it until an in-flight seek settles.
    /// Issue an exact seek immediately and mark it in flight.
    ///
    /// Always exact, including while dragging: keyframe seeks show the nearest keyframe (often
    /// seconds away on high-res files), and the exact seek on release then visibly jumps to a
    /// different frame. Coalescing keeps exact drag seeks affordable: measured on the dev
    /// machine, exact seeks take ~110 ms (4K H.264), ~150 ms (1080p), ~300 ms (4K HEVC).
    pub(crate) fn seek_now(&self, position_seconds: f64) {
        let position = position_seconds.max(0.0);
        let target = format!("{position:.3}");
        self.seeking.store(true, Ordering::SeqCst);
        *lock(&self.seek_started) = Instant::now();
        *lock(&self.last_seek_target) = Some(position);
        let _ = self.mpv.command("seek", &[&target, "absolute+exact"]);
    }

    pub(crate) fn frame_step(&self, command: &'static str) {
        if !lock(&self.state).paused {
            return;
        }
        if self.seeking.load(Ordering::SeqCst) {
            *lock(&self.pending_step) = Some(command);
        } else {
            let _ = self.mpv.command(command, &[]);
        }
    }

    fn fail(&self, error: PlayerError) {
        tracing::warn!(%error, "playback failed");
        self.update(|s| {
            s.state = PlayerStateKind::Error;
            s.error = Some(error);
        });
    }

    /// Recompute Playing/Paused/Ended from pause + eof, unless idle, loading, or failed.
    fn settle_state(s: &mut PlayerSnapshot, eof: Option<bool>) {
        if matches!(
            s.state,
            PlayerStateKind::Idle | PlayerStateKind::Loading | PlayerStateKind::Error
        ) {
            return;
        }
        s.state = match eof {
            Some(true) => PlayerStateKind::Ended,
            _ if s.state == PlayerStateKind::Ended && eof.is_none() => PlayerStateKind::Ended,
            _ if s.paused => PlayerStateKind::Paused,
            _ => PlayerStateKind::Playing,
        };
    }

    fn event_loop(self: Arc<Self>) {
        while !self.shutdown.load(Ordering::SeqCst) {
            let Some(event) = self.mpv.wait_event(0.2) else {
                continue;
            };
            match event {
                Ok(Event::Seek) => self.seeking.store(true, Ordering::SeqCst),
                Ok(Event::PlaybackRestart) => {
                    self.seeking.store(false, Ordering::SeqCst);
                    self.update(|s| {
                        if s.state == PlayerStateKind::Loading {
                            s.state = if s.paused {
                                PlayerStateKind::Paused
                            } else {
                                PlayerStateKind::Playing
                            };
                        }
                    });
                    if let Some(command) = lock(&self.pending_step).take() {
                        let _ = self.mpv.command(command, &[]);
                    }
                    if self.resume_after_seek.swap(false, Ordering::SeqCst) {
                        let _ = self.mpv.set_property("pause", false);
                    }
                    // Send the latest coalesced drag position, if any.
                    let pending = lock(&self.pending_seek).take();
                    if let Some(position) = pending {
                        self.seek_now(position);
                    }
                }
                Ok(Event::EndFile(reason)) => {
                    // Errors surface as `Err` below; a stop leaves the state `close` set.
                    if reason == mpv_end_file_reason::Eof {
                        self.update(|s| s.state = PlayerStateKind::Ended);
                    }
                }
                Ok(Event::PropertyChange {
                    reply_userdata,
                    change,
                    ..
                }) => self.on_property(reply_userdata, change),
                Ok(Event::Shutdown) => break,
                Ok(_) => {}
                Err(e) => {
                    // Event errors come from failed loads (end-file with an error).
                    let state = lock(&self.state).state;
                    if matches!(
                        state,
                        PlayerStateKind::Loading
                            | PlayerStateKind::Playing
                            | PlayerStateKind::Paused
                    ) {
                        self.fail(PlayerError::from_mpv(&e));
                    }
                }
            }
        }
    }

    fn on_property(&self, id: u64, change: PropertyData<'_>) {
        match (id, change) {
            (P_TIME_POS, PropertyData::Double(pos)) => {
                // Once playback has moved away from the last seek target, a release on that
                // spot is a real seek again.
                {
                    let mut last = lock(&self.last_seek_target);
                    if last.is_some_and(|t| (pos - t).abs() > 0.5) {
                        last.take();
                    }
                }
                let paused = lock(&self.state).paused;
                let due = lock(&self.last_position_emit).elapsed() >= POSITION_THROTTLE;
                if paused || due {
                    self.update(|s| s.position_seconds = pos);
                } else {
                    lock(&self.state).position_seconds = pos;
                }
            }
            (P_DURATION, PropertyData::Double(d)) => self.update(|s| s.duration_seconds = Some(d)),
            (P_PAUSE, PropertyData::Flag(paused)) => self.update(|s| {
                s.paused = paused;
                Self::settle_state(s, None);
            }),
            (P_SPEED, PropertyData::Double(speed)) => self.update(|s| s.speed = speed),
            (P_VOLUME, PropertyData::Double(volume)) => self.update(|s| s.volume = volume),
            (P_MUTE, PropertyData::Flag(muted)) => self.update(|s| s.muted = muted),
            (P_EOF, PropertyData::Flag(eof)) => self.update(|s| Self::settle_state(s, Some(eof))),
            (P_HWDEC, PropertyData::Str(hwdec)) => {
                let hwdec = hwdec.to_owned();
                self.update(|s| s.hwdec = Some(hwdec));
            }
            (P_TRACKS, PropertyData::Int64(_)) => {
                let tracks = read_tracks(&self.mpv);
                self.update(|s| s.tracks = tracks);
            }
            _ => {}
        }
    }
}
