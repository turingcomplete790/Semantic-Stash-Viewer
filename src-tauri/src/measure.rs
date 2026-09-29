//! Debug-only measurement mode for the spike's decision record (T033, quickstart V1–V6).
//!
//! `SSV_MEASURE=<id>,<id>,…` makes a debug build open each scene through the real in-window
//! render path, muted, once connected, then:
//! - record open → first rendered frame, `hwdec`, tracks, or the error (SC-001, SC-006, FR-014);
//! - after 3 s of playback, do 10 exact seeks to spread-out positions and time each to the next
//!   rendered frame (SC-002).
//!
//! `SSV_MEASURE_LONG=<id>` (optional) then plays that scene for `SSV_MEASURE_LONG_SECS`
//! (default 300) and records dropped frames and main-thread CPU (SC-003, research R5).
//!
//! `auto` in either picks scenes from the library's recently added list (the performance
//! harness, 003 research R8): the first three for open/seek, and the first 1080p one for the
//! long run. That list only changes when the library does, so runs are comparable (SC-007).
//!
//! Invalid runs say so instead of reporting numbers (003 FR-016): `"invalid": "not connected"`
//! when there's no connection within 30 s, and `"invalid": "window not drawn"` when the main
//! thread stops drawing during the long run.
//!
//! Results are printed as `MEASURE {json}` lines. Nothing is written to Stash.

use std::sync::Arc;
use std::time::{Duration, Instant};

use player::{Player, PlayerStateKind};
use serde_json::json;
use tauri::{AppHandle, Manager};

use stash_core::adapter::scenes::recent_scenes;
use stash_core::cache::refresh::RefreshPolicy;
use stash_core::connection::snapshot::SessionState;
use stash_core::scenes::SceneListItem;

use crate::cache_commands::read_cached;
use crate::harness::{self, Finish};
use crate::player_commands::{open_scene, player};
use crate::state::AppState;

const SEEKS_PER_SCENE: usize = 10;

/// Start the measurement run if `SSV_MEASURE` is set.
pub fn start_if_requested(app: &AppHandle) {
    let Ok(ids) = std::env::var("SSV_MEASURE") else {
        return;
    };
    let ids: Vec<String> = ids
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    let long = std::env::var("SSV_MEASURE_LONG").ok();
    let long_secs = std::env::var("SSV_MEASURE_LONG_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        run(&app, &ids, long.as_deref(), long_secs).await;
    });
}

fn emit(value: &serde_json::Value) {
    println!("MEASURE {value}");
    tracing::info!(target: "measure", "{value}");
}

async fn run(app: &AppHandle, ids: &[String], long: Option<&str>, long_secs: u64) {
    measure_all(app, ids, long, long_secs).await;
    emit(&json!({"done": true}));
    harness::finished(Finish::Measure);
}

async fn measure_all(app: &AppHandle, ids: &[String], long: Option<&str>, long_secs: u64) {
    let state = app.state::<AppState>();
    let connected = wait_for(Duration::from_secs(30), || {
        matches!(state.manager.snapshot().state, SessionState::Connected)
    })
    .await;
    if !connected {
        emit(&json!({"invalid": "not connected"}));
        return;
    }
    // Let the UI settle after connecting.
    tokio::time::sleep(Duration::from_secs(3)).await;
    let Ok(player) = player(&state).map(Arc::clone) else {
        emit(&json!({"invalid": "player unavailable"}));
        return;
    };
    player.set_muted(true);

    let wants_auto = ids.iter().any(|id| id == "auto") || long == Some("auto");
    let recent = if wants_auto {
        match read_cached(
            &state,
            "scenes:recent",
            RefreshPolicy::Auto,
            false,
            |c| async move { recent_scenes(&c).await },
        )
        .await
        {
            Ok(r) => r.data,
            Err(e) => {
                emit(&json!({"invalid": format!("couldn't list scenes: {e}")}));
                return;
            }
        }
    } else {
        Vec::new()
    };
    let ids: Vec<String> = if ids.iter().any(|id| id == "auto") {
        recent.iter().take(3).map(|s| s.id.clone()).collect()
    } else {
        ids.to_vec()
    };
    let long = match long {
        Some("auto") => pick_1080p(&recent),
        other => other.map(str::to_owned),
    };

    for id in &ids {
        measure_scene(&state, &player, id).await;
    }
    if let Some(id) = long {
        measure_long(&state, &player, &id, long_secs).await;
    }
    player.close();
}

/// The first 1080p scene, else the first scene at all.
fn pick_1080p(scenes: &[SceneListItem]) -> Option<String> {
    scenes
        .iter()
        .find(|s| {
            s.resolution
                .as_deref()
                .is_some_and(|r| r.ends_with("×1080"))
        })
        .or_else(|| scenes.first())
        .map(|s| s.id.clone())
}

/// Poll `pred` every 20 ms until it holds or `timeout` passes.
async fn wait_for(timeout: Duration, mut pred: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if pred() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    pred()
}

async fn measure_scene(state: &AppState, player: &Player, id: &str) {
    let scene = match open_scene(state, id).await {
        Ok(scene) => scene,
        Err(e) => {
            emit(&json!({"scene": id, "result": "open failed", "error": e.to_string()}));
            return;
        }
    };
    let settled = wait_for(Duration::from_secs(20), || {
        matches!(
            player.snapshot().state,
            PlayerStateKind::Playing | PlayerStateKind::Paused | PlayerStateKind::Error
        )
    })
    .await;
    let snapshot = player.snapshot();
    if !settled || snapshot.state == PlayerStateKind::Error {
        emit(&json!({
            "scene": id,
            "result": if settled { "error" } else { "stuck loading" },
            "error": snapshot.error,
        }));
        player.close();
        return;
    }
    // The rendered-frame refinement lands on the next draw.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let opened = player.stats();

    tokio::time::sleep(Duration::from_secs(3)).await;
    let duration = snapshot.duration_seconds.unwrap_or(scene.duration_seconds);
    let mut seeks = Vec::new();
    for i in 0..SEEKS_PER_SCENE {
        // Spread over 5–90% of the file, alternating forward and back.
        let fraction =
            0.05 + 0.85 * (((i * 7) % SEEKS_PER_SCENE) as f64 + 0.5) / SEEKS_PER_SCENE as f64;
        let target = duration * fraction;
        let before = player.stats().last_seek_to_frame_ms;
        player.seek(target, true);
        let landed = wait_for(Duration::from_secs(10), || {
            let stats = player.stats();
            stats.last_seek_to_frame_ms.is_some()
                && stats.last_seek_to_frame_ms != before
                && (player.snapshot().position_seconds - target).abs() < 1.0
        })
        .await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        seeks.push(if landed {
            player.stats().last_seek_to_frame_ms.map(|ms| ms.round())
        } else {
            None
        });
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
    let end = player.stats();
    let snapshot = player.snapshot();
    let tracks: Vec<_> = snapshot
        .tracks
        .iter()
        .map(|t| json!({"kind": t.kind, "codec": t.codec, "lang": t.language, "title": t.title}))
        .collect();
    emit(&json!({
        "scene": id,
        "result": "played",
        "container": scene.file.container,
        "codec": scene.file.video_codec,
        "size": format!("{}x{}", scene.file.width.unwrap_or(0), scene.file.height.unwrap_or(0)),
        "hwdec": end.hwdec,
        "open_ms": opened.open_to_first_frame_ms.map(f64::round),
        "seek_ms": seeks,
        "dropped": end.dropped_frames,
        "tracks": tracks,
    }));
    player.close();
    tokio::time::sleep(Duration::from_millis(500)).await;
}

async fn measure_long(state: &AppState, player: &Player, id: &str, secs: u64) {
    if let Err(e) = open_scene(state, id).await {
        emit(&json!({"long": id, "error": e.to_string()}));
        return;
    }
    wait_for(Duration::from_secs(20), || {
        player.snapshot().state == PlayerStateKind::Playing
    })
    .await;
    let start_drops = player.stats().dropped_frames;
    let start_cpu = main_thread_cpu_ticks();
    let started = Instant::now();
    let mut samples = Vec::new();
    let mut drop_samples = Vec::new();
    let mut last = (start_cpu, Instant::now());
    while started.elapsed() < Duration::from_secs(secs) {
        tokio::time::sleep(Duration::from_secs(10)).await;
        let now = (main_thread_cpu_ticks(), Instant::now());
        if let (Some(a), Some(b)) = (last.0, now.0) {
            let percent =
                (b - a) as f64 / CLOCK_TICKS / now.1.duration_since(last.1).as_secs_f64() * 100.0;
            samples.push(percent.round());
        }
        drop_samples.push(player.stats().dropped_frames - start_drops);
        last = now;
    }
    let elapsed = started.elapsed().as_secs_f64();
    let cpu = match (start_cpu, main_thread_cpu_ticks()) {
        (Some(a), Some(b)) => Some(((b - a) as f64 / CLOCK_TICKS / elapsed * 100.0).round()),
        _ => None,
    };
    let stats = player.stats();
    // No main-thread CPU at all means nothing was drawn (a hidden window, found in 002), so the
    // frame counts mean nothing.
    let not_drawn = samples.contains(&0.0);
    emit(&json!({
        "long": id,
        "invalid": not_drawn.then_some("window not drawn"),
        "seconds": elapsed.round(),
        "state": player.snapshot().state,
        "hwdec": stats.hwdec,
        "dropped": stats.dropped_frames - start_drops,
        "main_thread_cpu_percent": cpu,
        "main_thread_cpu_samples": samples,
        "dropped_samples": drop_samples,
    }));
    player.close();
}

/// Linux `USER_HZ`, fixed at 100 on every supported architecture.
const CLOCK_TICKS: f64 = 100.0;

/// User + system CPU ticks of the main (GTK) thread, whose TID equals the PID.
fn main_thread_cpu_ticks() -> Option<u64> {
    let pid = std::process::id();
    let stat = std::fs::read_to_string(format!("/proc/self/task/{pid}/stat")).ok()?;
    // Fields after the parenthesised command name; utime and stime are fields 14 and 15.
    let rest = stat.rsplit_once(')')?.1;
    let fields: Vec<&str> = rest.split_whitespace().collect();
    let utime: u64 = fields.get(11)?.parse().ok()?;
    let stime: u64 = fields.get(12)?.parse().ok()?;
    Some(utime + stime)
}
