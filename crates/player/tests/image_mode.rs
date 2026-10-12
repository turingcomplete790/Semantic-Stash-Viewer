//! 012 T016: the player's image mode (contracts/player-image-mode.md), run headless on small
//! files ffmpeg makes: a still stays, an animated file loops, a video after an image is a video
//! again, and the view commands are mpv properties.

use std::time::Duration;

use player::{
    source_allowed_for, MediaKind, OpenRequest, Player, PlayerCommand, PlayerConfig,
    PlayerSnapshot, PlayerStateKind,
};
use tokio::sync::watch;

/// `ffmpeg … <args> <dir>/<name>`, or `None` (test skipped) without ffmpeg.
fn make(dir: &std::path::Path, name: &str, args: &[&str]) -> Option<String> {
    let out = dir.join(name);
    let status = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-f", "lavfi"])
        .args(args)
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

fn still(dir: &std::path::Path) -> Option<String> {
    make(
        dir,
        "still.jpg",
        &["-i", "testsrc=size=320x240", "-frames:v", "1"],
    )
}

fn animated(dir: &std::path::Path) -> Option<String> {
    make(
        dir,
        "loop.gif",
        &[
            "-i",
            "testsrc=duration=0.5:size=160x120:rate=10",
            "-loop",
            "0",
        ],
    )
}

fn clip(dir: &std::path::Path) -> Option<String> {
    make(
        dir,
        "clip.mp4",
        &[
            "-i",
            "testsrc=duration=1:size=160x120:rate=10",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
        ],
    )
}

fn headless() -> Player {
    Player::new(PlayerConfig::headless()).expect("mpv initialises")
}

fn request(source: &str, kind: MediaKind) -> OpenRequest {
    OpenRequest {
        source: source.into(),
        scene_id: Some("1".into()),
        title: Some("Test".into()),
        api_key: None,
        strict_tls: false,
        cache: None,
        start_seconds: None,
        transcoded: false,
        kind,
    }
}

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

/// mpv's property as text, polled briefly (commands apply on the player's worker thread).
async fn property_becomes(player: &Player, name: &str, want: &str) {
    for _ in 0..50 {
        if player.debug_property(name).as_deref() == Some(want) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("{name} = {:?}, wanted {want}", player.debug_property(name));
}

#[tokio::test]
async fn a_still_image_plays_and_stays() {
    let dir = tempfile::tempdir().expect("tempdir");
    let Some(file) = still(dir.path()) else {
        return;
    };
    let player = headless();
    let mut rx = player.subscribe();
    player.open(request(&file, MediaKind::Image { looped: false }));
    let s = until(&mut rx, "playing", |s| s.state == PlayerStateKind::Playing).await;
    assert_eq!(s.picture, Some((320, 240)));
    property_becomes(&player, "image-display-duration", "inf").await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(
        player.snapshot().state,
        PlayerStateKind::Playing,
        "a still never ends on its own"
    );
}

#[tokio::test]
async fn an_animated_image_loops_silently_and_a_video_after_it_is_a_video_again() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (Some(gif), Some(video)) = (animated(dir.path()), clip(dir.path())) else {
        return;
    };
    let player = headless();
    let mut rx = player.subscribe();
    player.open(request(&gif, MediaKind::Image { looped: true }));
    until(&mut rx, "playing", |s| s.state == PlayerStateKind::Playing).await;
    property_becomes(&player, "loop-file", "inf").await;
    property_becomes(&player, "options/aid", "no").await;
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert_eq!(
        player.snapshot().state,
        PlayerStateKind::Playing,
        "still looping"
    );

    player.open(request(&video, MediaKind::Video));
    until(&mut rx, "the video ends", |s| {
        s.state == PlayerStateKind::Ended
    })
    .await;
    property_becomes(&player, "loop-file", "no").await;
    property_becomes(&player, "image-display-duration", "1.000000").await;
    property_becomes(&player, "options/aid", "auto").await;
}

#[tokio::test]
async fn view_commands_set_mpvs_properties() {
    let dir = tempfile::tempdir().expect("tempdir");
    let Some(file) = still(dir.path()) else {
        return;
    };
    let player = headless();
    let mut rx = player.subscribe();
    player.open(request(&file, MediaKind::Image { looped: false }));
    until(&mut rx, "playing", |s| s.state == PlayerStateKind::Playing).await;

    player.dispatch(PlayerCommand::Zoom(1.5));
    property_becomes(&player, "video-zoom", "1.500000").await;
    player.dispatch(PlayerCommand::Pan { x: 0.25, y: -0.1 });
    property_becomes(&player, "video-pan-x", "0.250000").await;
    property_becomes(&player, "video-pan-y", "-0.100000").await;
    player.dispatch(PlayerCommand::Rotate(1));
    property_becomes(&player, "video-rotate", "90").await;
    player.dispatch(PlayerCommand::ActualSize(true));
    property_becomes(&player, "video-unscaled", "yes").await;
    // Out of range: clamped.
    player.dispatch(PlayerCommand::Zoom(9.0));
    property_becomes(&player, "video-zoom", "3.000000").await;

    player.dispatch(PlayerCommand::ResetView);
    property_becomes(&player, "video-zoom", "0.000000").await;
    property_becomes(&player, "video-pan-x", "0.000000").await;
    property_becomes(&player, "video-rotate", "0").await;
    property_becomes(&player, "video-unscaled", "no").await;
}

#[test]
fn stashs_image_file_is_allowed_only_for_image_opens() {
    let image = "http://localhost:9998/image/12/image";
    assert!(source_allowed_for(image, false, true));
    assert!(!source_allowed_for(image, false, false), "not as a video");
    let stream = "http://localhost:9998/scene/3/stream";
    assert!(source_allowed_for(stream, false, false));
    assert!(
        !source_allowed_for(stream, false, true),
        "a scene stream isn't an image"
    );
    assert!(!source_allowed_for(
        "http://localhost:9998/image/12/image?apikey=x",
        false,
        true
    ));
    assert!(!source_allowed_for(
        "http://localhost:9998/image/12/thumbnail",
        false,
        true
    ));
    assert!(
        source_allowed_for("/tmp/neighbour.jpg", false, true),
        "a local neighbour file"
    );
}
