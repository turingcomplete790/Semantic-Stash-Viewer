//! Player session state machine, run headless (vo=null, ao=null) on mpv's built-in test
//! source, so no media files or window are needed (data-model.md "PlayerState").

use std::time::Duration;

use player::{
    CacheLimits, OpenRequest, Player, PlayerCommand, PlayerConfig, PlayerError, PlayerSnapshot,
    PlayerStateKind,
};
use tokio::sync::watch;

const TEST_SRC: &str = "av://lavfi:testsrc=duration=5:size=320x240:rate=30";

/// A real, seekable 5 s H.264 file (30 fps, keyframe every 10 frames), generated with ffmpeg.
/// Frame-back-step and exact durations need an indexed file; mpv's lavfi source is live.
/// Returns `None` (test skipped) when ffmpeg isn't installed.
fn seekable_file(dir: &std::path::Path) -> Option<String> {
    let out = dir.join("clip.mp4");
    let status = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-f", "lavfi"])
        .args(["-i", "testsrc=duration=5:size=320x240:rate=30"])
        .args(["-c:v", "libx264", "-preset", "ultrafast", "-g", "10"])
        .arg(&out)
        .status();
    match status {
        Ok(s) if s.success() => Some(out.to_string_lossy().into_owned()),
        _ => {
            eprintln!("ffmpeg not available; skipping");
            None
        }
    }
}

fn headless() -> Player {
    Player::new(PlayerConfig::headless()).expect("mpv initialises")
}

fn request(source: &str) -> OpenRequest {
    OpenRequest {
        source: source.into(),
        scene_id: Some("test".into()),
        title: Some("Test source".into()),
        api_key: None,
        strict_tls: false,
        cache: None,
    }
}

fn open(player: &Player, source: &str) {
    player.open(request(source));
}

/// Wait until `pred` holds for the latest snapshot (up to 10 s).
async fn until(
    rx: &mut watch::Receiver<PlayerSnapshot>,
    what: &str,
    pred: impl Fn(&PlayerSnapshot) -> bool,
) -> PlayerSnapshot {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        {
            let s = rx.borrow_and_update();
            if pred(&s) {
                return s.clone();
            }
        }
        match tokio::time::timeout_at(deadline, rx.changed()).await {
            Ok(Ok(())) => {}
            _ => panic!("timed out waiting for {what}; last = {:?}", rx.borrow()),
        }
    }
}

#[tokio::test]
async fn open_plays_pause_resumes_and_close_goes_idle() {
    let player = headless();
    let mut rx = player.subscribe();
    assert_eq!(rx.borrow().state, PlayerStateKind::Idle);

    open(&player, TEST_SRC);
    let playing = until(&mut rx, "playing", |s| s.state == PlayerStateKind::Playing).await;
    assert_eq!(playing.scene_id.as_deref(), Some("test"));
    assert_eq!(playing.title.as_deref(), Some("Test source"));

    player.set_paused(true);
    until(&mut rx, "paused", |s| {
        s.state == PlayerStateKind::Paused && s.paused
    })
    .await;
    player.set_paused(false);
    until(&mut rx, "playing again", |s| {
        s.state == PlayerStateKind::Playing
    })
    .await;

    player.close();
    let idle = until(&mut rx, "idle", |s| s.state == PlayerStateKind::Idle).await;
    assert_eq!(idle.scene_id, None);
}

#[tokio::test]
async fn exact_seek_lands_on_target() {
    let dir = tempfile::tempdir().expect("tempdir");
    let Some(clip) = seekable_file(dir.path()) else {
        return;
    };
    let player = headless();
    let mut rx = player.subscribe();
    open(&player, &clip);
    until(&mut rx, "playing", |s| s.state == PlayerStateKind::Playing).await;
    player.set_paused(true);
    until(&mut rx, "paused", |s| s.paused).await;

    player.seek(2.0, true);
    let s = until(&mut rx, "position ~2.0", |s| {
        (s.position_seconds - 2.0).abs() < 0.1
    })
    .await;
    assert!(s.duration_seconds.is_some_and(|d| (d - 5.0).abs() < 0.2));
}

#[tokio::test]
async fn a_whole_file_cache_is_accepted_and_seeks_still_land() {
    let dir = tempfile::tempdir().expect("tempdir");
    let Some(clip) = seekable_file(dir.path()) else {
        return;
    };
    let player = headless();
    let mut rx = player.subscribe();
    player.open(OpenRequest {
        cache: Some(CacheLimits {
            forward_bytes: 300 << 20,
            back_bytes: 300 << 20,
        }),
        ..request(&clip)
    });
    until(&mut rx, "playing", |s| s.state == PlayerStateKind::Playing).await;
    player.set_paused(true);
    player.seek(3.0, true);
    until(&mut rx, "position ~3.0", |s| {
        (s.position_seconds - 3.0).abs() < 0.1
    })
    .await;
    // The next file goes back to the defaults without error.
    open(&player, &clip);
    until(&mut rx, "playing", |s| s.state == PlayerStateKind::Playing).await;
}

#[tokio::test]
async fn speed_and_volume_are_clamped() {
    let player = headless();
    let mut rx = player.subscribe();
    open(&player, TEST_SRC);
    until(&mut rx, "playing", |s| s.state == PlayerStateKind::Playing).await;

    player.set_speed(8.0);
    until(&mut rx, "speed 4", |s| (s.speed - 4.0).abs() < 1e-6).await;
    player.set_speed(0.1);
    until(&mut rx, "speed 0.25", |s| (s.speed - 0.25).abs() < 1e-6).await;
    player.set_volume(150.0);
    until(&mut rx, "volume 100", |s| (s.volume - 100.0).abs() < 1e-6).await;
    player.set_muted(true);
    until(&mut rx, "muted", |s| s.muted).await;
}

#[tokio::test]
async fn frame_step_only_moves_while_paused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let Some(clip) = seekable_file(dir.path()) else {
        return;
    };
    let player = headless();
    let mut rx = player.subscribe();
    open(&player, &clip);
    until(&mut rx, "playing", |s| s.state == PlayerStateKind::Playing).await;

    player.set_paused(true);
    player.seek(1.0, true);
    let before = until(&mut rx, "paused at 1.0", |s| {
        s.paused && (s.position_seconds - 1.0).abs() < 0.05
    })
    .await
    .position_seconds;
    // Sent right after the seek on purpose: the player must queue it until the seek settles.
    player.frame_step_forward();
    let after = until(&mut rx, "one frame later", |s| {
        s.position_seconds > before + 0.01
    })
    .await
    .position_seconds;
    // 30 fps: one frame ≈ 33 ms.
    assert!(after - before < 0.1, "stepped {} s", after - before);
    player.frame_step_back();
    until(&mut rx, "one frame back", |s| {
        s.position_seconds < after - 0.01
    })
    .await;

    // While playing, frame stepping is ignored (it would pause mpv as a side effect).
    player.set_paused(false);
    until(&mut rx, "playing", |s| !s.paused).await;
    player.frame_step_forward();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(
        !player.snapshot().paused,
        "frame step while playing must be ignored"
    );
}

#[tokio::test]
async fn end_of_file_is_ended_and_replay_restarts() {
    let dir = tempfile::tempdir().expect("tempdir");
    let Some(clip) = seekable_file(dir.path()) else {
        return;
    };
    let player = headless();
    let mut rx = player.subscribe();
    open(&player, &clip);
    until(&mut rx, "playing", |s| s.state == PlayerStateKind::Playing).await;

    player.seek(4.5, true);
    until(&mut rx, "ended", |s| s.state == PlayerStateKind::Ended).await;

    player.replay();
    until(&mut rx, "playing from start", |s| {
        s.state == PlayerStateKind::Playing && s.position_seconds < 1.0
    })
    .await;
}

#[tokio::test]
async fn missing_file_is_an_error_not_a_panic() {
    let player = headless();
    let mut rx = player.subscribe();
    open(&player, "file:///does/not/exist.mp4");
    let s = until(&mut rx, "error", |s| s.state == PlayerStateKind::Error).await;
    assert!(
        matches!(
            s.error,
            Some(PlayerError::StreamUnreachable | PlayerError::UnsupportedFormat)
        ),
        "got {:?}",
        s.error
    );
}

#[tokio::test]
async fn reports_whether_hardware_decoding_is_used() {
    let player = headless();
    let mut rx = player.subscribe();
    open(&player, TEST_SRC);
    let s = until(&mut rx, "hwdec known", |s| s.hwdec.is_some()).await;
    // The raw test source is decoded in software.
    assert_eq!(s.hwdec.as_deref(), Some("no"));
}

#[tokio::test]
async fn dispatch_returns_immediately_and_applies_in_order() {
    let player = headless();
    let mut rx = player.subscribe();
    open(&player, TEST_SRC);
    until(&mut rx, "playing", |s| s.state == PlayerStateKind::Playing).await;

    let started = std::time::Instant::now();
    player.dispatch(PlayerCommand::SetPaused(true));
    player.dispatch(PlayerCommand::SetVolume(40.0));
    player.dispatch(PlayerCommand::SetSpeed(2.0));
    // Queuing must not wait on mpv (contract invariant 1).
    assert!(started.elapsed() < std::time::Duration::from_millis(20));

    until(&mut rx, "paused, volume 40, speed 2", |s| {
        s.paused && (s.volume - 40.0).abs() < 1e-6 && (s.speed - 2.0).abs() < 1e-6
    })
    .await;
}

#[tokio::test]
async fn drag_seeks_are_coalesced_and_the_release_seek_wins() {
    let dir = tempfile::tempdir().expect("tempdir");
    let Some(clip) = seekable_file(dir.path()) else {
        return;
    };
    let player = headless();
    let mut rx = player.subscribe();
    open(&player, &clip);
    until(&mut rx, "playing", |s| s.state == PlayerStateKind::Playing).await;
    player.set_paused(true);
    until(&mut rx, "paused", |s| s.paused).await;

    // A burst of drag positions, like scrubbing, then the exact seek on release.
    for i in 0..50 {
        player.dispatch(PlayerCommand::Seek {
            position_seconds: 0.5 + f64::from(i) * 0.05,
            exact: false,
        });
    }
    player.dispatch(PlayerCommand::Seek {
        position_seconds: 3.0,
        exact: true,
    });

    let s = until(&mut rx, "lands on 3.0", |s| {
        (s.position_seconds - 3.0).abs() < 0.05
    })
    .await;
    assert!(s.paused);
    // Nothing queued afterwards moves it off the release position.
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!((player.snapshot().position_seconds - 3.0).abs() < 0.05);
}
