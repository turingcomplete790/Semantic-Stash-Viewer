//! 012 T003 spike: how long mpv takes from opening a local image to its first frame being ready
//! (research R3), headless (no window: decode only; drawing adds a little on screen).
//!
//! Usage: `cargo run -p player --release --example image_open -- <label>=<path> ...`
//! Prints each label's median, min, and max over 10 opens (paths are never printed).

use std::time::{Duration, Instant};

use player::{MediaKind, OpenRequest, Player, PlayerConfig, PlayerStateKind};

const RUNS: usize = 10;

fn open(player: &Player, path: &str, looped: bool) {
    player.open(OpenRequest {
        source: path.to_owned(),
        scene_id: None,
        title: None,
        api_key: None,
        strict_tls: false,
        cache: None,
        start_seconds: None,
        transcoded: false,
        kind: MediaKind::Image { looped },
    });
}

fn main() {
    let player = Player::new(PlayerConfig::headless()).expect("mpv initialises");
    let rx = player.subscribe();
    let files: Vec<(String, String)> = std::env::args()
        .skip(1)
        .filter_map(|a| a.split_once('=').map(|(l, p)| (l.to_owned(), p.to_owned())))
        .collect();
    for (label, path) in &files {
        let looped = label.contains("gif") || label.contains("webp");
        let mut times = Vec::new();
        for _ in 0..RUNS {
            player.close();
            std::thread::sleep(Duration::from_millis(150));
            let started = Instant::now();
            open(&player, path, looped);
            loop {
                let s = rx.borrow().clone();
                if s.state == PlayerStateKind::Playing
                    && player.stats().open_to_first_frame_ms.is_some()
                {
                    break;
                }
                if s.state == PlayerStateKind::Error || started.elapsed() > Duration::from_secs(10)
                {
                    println!("{label}: failed ({:?})", s.error);
                    break;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            if let Some(ms) = player.stats().open_to_first_frame_ms {
                times.push(ms);
            }
        }
        times.sort_by(f64::total_cmp);
        if let (Some(min), Some(max)) = (times.first(), times.last()) {
            let picture = player.snapshot().picture;
            println!(
                "{label}: median {:.0} ms, min {min:.0}, max {max:.0} ({} runs; picture {picture:?})",
                times[times.len() / 2],
                times.len()
            );
        }
    }
}
