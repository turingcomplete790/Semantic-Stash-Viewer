//! The playback run (006 T020), a port of `src-tauri/src/measure.rs` with the same `MEASURE`
//! lines: open each scene in `SSV_MEASURE` through the real in-window player, time first frame and
//! seeks, count dropped frames, then optionally play `SSV_MEASURE_LONG` for dropped frames per
//! minute and main-thread CPU. Nothing is written to Stash.
//!
//! The app shows the player screen while this runs, so frames are really drawn.

use std::sync::Arc;
use std::time::{Duration, Instant};

use player::{Player, PlayerStateKind};
use serde_json::json;
use stash_core::adapter::scenes::{recent_scenes, test_scenes};
use stash_core::cache::refresh::RefreshPolicy;
use stash_core::connection::snapshot::SessionState;
use stash_core::scenes::SceneListItem;

use super::emit;
use crate::services::Services;

const SEEKS_PER_SCENE: usize = 10;

/// What the app should show while the run plays a scene.
pub type OnScene = Arc<dyn Fn(String) + Send + Sync>;

/// Run the playback measurements; returns when done (the caller exits if the harness asked).
pub async fn run(
    services: Arc<Services>,
    ids: Vec<String>,
    long: Option<(String, u64)>,
    on_scene: OnScene,
) {
    measure_all(&services, &ids, long, &on_scene).await;
    emit(&json!({"done": true}));
}

async fn measure_all(
    services: &Arc<Services>,
    ids: &[String],
    long: Option<(String, u64)>,
    on_scene: &OnScene,
) {
    let connected = wait_for(Duration::from_secs(30), || {
        matches!(services.snapshot().state, SessionState::Connected)
    })
    .await;
    if !connected {
        emit(&json!({"invalid": "not connected"}));
        return;
    }
    tokio::time::sleep(Duration::from_secs(3)).await;
    let Ok(player) = services.player().map(Arc::clone) else {
        emit(&json!({"invalid": "player unavailable"}));
        return;
    };
    player.set_muted(true);

    let wants_auto =
        ids.iter().any(|id| id == "auto") || long.as_ref().is_some_and(|(id, _)| id == "auto");
    let recent = if wants_auto {
        match services
            .read_cached(
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
    let wants_testset = ids.iter().any(|id| id == "testset");
    let mut ids: Vec<String> = if ids.iter().any(|id| id == "auto") {
        recent.iter().take(3).map(|s| s.id.clone()).collect()
    } else {
        ids.iter().filter(|id| *id != "testset").cloned().collect()
    };
    // `testset`: one scene from each of the 002 spike's hard-to-play groups (4K, HEVC, AV1, …),
    // for SC-001–SC-003's codec and 4K checks. Native build only.
    if wants_testset {
        match services
            .read_cached(
                "scenes:test-set",
                RefreshPolicy::Manual,
                false,
                |c| async move { test_scenes(&c).await },
            )
            .await
            .map(|c| c.data)
        {
            Ok(groups) => {
                for g in groups {
                    if let Some(s) = g.scenes.first() {
                        emit(&json!({"testGroup": g.label, "scene": s.id}));
                        ids.push(s.id.clone());
                    }
                }
            }
            Err(e) => emit(&json!({"invalid": format!("couldn't list the test set: {e}")})),
        }
    }
    let long = long.and_then(|(id, secs)| {
        let id = if id == "auto" {
            pick_1080p(&recent)?
        } else {
            id
        };
        Some((id, secs))
    });

    for id in &ids {
        on_scene(id.clone());
        measure_scene(services, &player, id).await;
    }
    if let Some((id, secs)) = long {
        on_scene(id.clone());
        measure_long(services, &player, &id, secs).await;
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

async fn measure_scene(services: &Services, player: &Player, id: &str) {
    let scene = match services.open_scene(id).await {
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
    tokio::time::sleep(Duration::from_millis(200)).await;
    let opened = player.stats();

    tokio::time::sleep(Duration::from_secs(3)).await;
    let duration = snapshot.duration_seconds.unwrap_or(scene.duration_seconds);
    let mut seeks = Vec::new();
    for i in 0..SEEKS_PER_SCENE {
        // Spread over 5–90% of the file, alternating forward and back (as the web build).
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

async fn measure_long(services: &Services, player: &Player, id: &str, secs: u64) {
    if let Err(e) = services.open_scene(id).await {
        emit(&json!({"long": id, "error": e.to_string()}));
        return;
    }
    wait_for(Duration::from_secs(20), || {
        player.snapshot().state == PlayerStateKind::Playing
    })
    .await;
    let start_drops = player.stats().dropped_frames;
    let start_frames = crate::player::video::frame_counts();
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
    let end_frames = crate::player::video::frame_counts();
    let rendered_fps = (end_frames.0 - start_frames.0) as f64 / elapsed;
    let displayed_fps = (end_frames.1 - start_frames.1) as f64 / elapsed;
    // No main-thread CPU at all means nothing was drawn (a hidden window, found in 002).
    let not_drawn = samples.contains(&0.0);
    emit(&json!({
        "long": id,
        "invalid": not_drawn.then_some("window not drawn"),
        "seconds": elapsed.round(),
        "state": player.snapshot().state,
        "hwdec": stats.hwdec,
        "dropped": stats.dropped_frames - start_drops,
        "rendered_fps": (rendered_fps * 10.0).round() / 10.0,
        "displayed_fps": (displayed_fps * 10.0).round() / 10.0,
        "main_thread_cpu_percent": cpu,
        "main_thread_cpu_samples": samples,
        "dropped_samples": drop_samples,
    }));
    player.close();
}

/// Linux `USER_HZ`, fixed at 100 on every supported architecture.
const CLOCK_TICKS: f64 = 100.0;

/// User + system CPU ticks of the main (event loop) thread, whose TID equals the PID.
fn main_thread_cpu_ticks() -> Option<u64> {
    let pid = std::process::id();
    let stat = std::fs::read_to_string(format!("/proc/self/task/{pid}/stat")).ok()?;
    let rest = stat.rsplit_once(')')?.1;
    let fields: Vec<&str> = rest.split_whitespace().collect();
    let utime: u64 = fields.get(11)?.parse().ok()?;
    let stime: u64 = fields.get(12)?.parse().ok()?;
    Some(utime + stime)
}
