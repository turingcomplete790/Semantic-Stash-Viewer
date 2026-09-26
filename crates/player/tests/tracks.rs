//! Embedded tracks are visible (FR-014): the approach must keep subtitle and audio track
//! selection possible later. Generates a small multi-track MKV with ffmpeg; skips without it.

use std::process::Command;
use std::time::Duration;

use player::{OpenRequest, Player, PlayerConfig, PlayerStateKind, TrackKind};

fn make_multitrack_mkv(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        eprintln!("ffmpeg not found; skipping tracks test");
        return None;
    }
    let srt = dir.join("subs.srt");
    std::fs::write(&srt, "1\n00:00:00,000 --> 00:00:02,000\nhello\n").expect("write srt");
    let out = dir.join("tracks.mkv");
    let status = Command::new("ffmpeg")
        .args(["-v", "error", "-y"])
        .args([
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=3:size=320x240:rate=30",
        ])
        .args(["-f", "lavfi", "-i", "sine=f=440:d=3"])
        .args(["-f", "lavfi", "-i", "sine=f=880:d=3"])
        .arg("-i")
        .arg(&srt)
        .args(["-map", "0", "-map", "1", "-map", "2", "-map", "3"])
        .args([
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-c:a",
            "aac",
            "-c:s",
            "srt",
        ])
        .args([
            "-metadata:s:a:0",
            "language=eng",
            "-metadata:s:a:1",
            "language=jpn",
        ])
        .arg(&out)
        .status()
        .expect("run ffmpeg");
    assert!(status.success(), "ffmpeg failed to build the test file");
    Some(out)
}

#[tokio::test]
async fn lists_embedded_audio_and_subtitle_tracks() {
    let dir = tempfile::tempdir().expect("tempdir");
    let Some(file) = make_multitrack_mkv(dir.path()) else {
        return;
    };

    let player = Player::new(PlayerConfig::headless()).expect("mpv");
    let mut rx = player.subscribe();
    player.open(OpenRequest {
        source: file.to_string_lossy().into_owned(),
        scene_id: None,
        title: None,
        api_key: None,
        strict_tls: false,
        cache: None,
    });

    let snapshot = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            {
                let s = rx.borrow_and_update();
                if s.tracks.len() == 4 && s.state == PlayerStateKind::Playing {
                    return s.clone();
                }
            }
            rx.changed().await.expect("channel open");
        }
    })
    .await
    .expect("tracks within 10 s");

    let of = |kind: TrackKind| snapshot.tracks.iter().filter(move |t| t.kind == kind);
    assert_eq!(of(TrackKind::Video).count(), 1);
    let audio: Vec<_> = of(TrackKind::Audio).collect();
    assert_eq!(audio.len(), 2);
    assert_eq!(audio[0].language.as_deref(), Some("eng"));
    assert_eq!(audio[1].language.as_deref(), Some("jpn"));
    assert_eq!(of(TrackKind::Subtitle).count(), 1);
    assert!(snapshot.tracks.iter().all(|t| !t.external));
}
