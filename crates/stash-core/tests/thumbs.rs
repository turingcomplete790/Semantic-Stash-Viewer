//! Scene thumbnails: fetch, shrink to 480 px, cache (005 research R5).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use stash_core::cache::{Limit, ViewCache};
use stash_core::thumbs::{prepare, SourceFetch, ThumbService, PLACEHOLDER, THUMB_WIDTH};
use stash_core::AppError;

/// A JPEG of the given size (a gradient, so it compresses like a real picture).
fn jpeg(width: usize, height: usize) -> Vec<u8> {
    let mut pixels = vec![0u8; width * height * 3];
    for y in 0..height {
        for x in 0..width {
            let i = (y * width + x) * 3;
            pixels[i] = (x * 255 / width) as u8;
            pixels[i + 1] = (y * 255 / height) as u8;
            pixels[i + 2] = ((x + y) % 256) as u8;
        }
    }
    let image = turbojpeg::Image {
        pixels: pixels.as_slice(),
        width,
        pitch: width * 3,
        height,
        format: turbojpeg::PixelFormat::RGB,
    };
    turbojpeg::compress(image, 85, turbojpeg::Subsamp::Sub2x2)
        .expect("encode")
        .to_vec()
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let img = image::RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 90])
    });
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png)
        .expect("png");
    out.into_inner()
}

fn dimensions(jpeg: &[u8]) -> (usize, usize) {
    let header = turbojpeg::read_header(jpeg).expect("a JPEG");
    (header.width, header.height)
}

fn cache() -> (tempfile::TempDir, Arc<Mutex<ViewCache>>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let c =
        ViewCache::open_with(dir.path().join("c.sqlite3"), Limit::Fixed(64 << 20)).expect("cache");
    (dir, Arc::new(Mutex::new(c)))
}

/// A fetcher that returns `source` and counts calls.
fn fetcher(source: Option<(Vec<u8>, &'static str)>, calls: Arc<AtomicUsize>) -> SourceFetch {
    Arc::new(move |_id: String| {
        calls.fetch_add(1, Ordering::SeqCst);
        let source = source.clone().map(|(b, t)| (b, t.to_owned()));
        Box::pin(async move { Ok(source) })
    })
}

#[test]
fn a_4k_screenshot_becomes_a_480_px_jpeg() {
    let out = prepare(&jpeg(3840, 2160), "image/jpeg").expect("prepare");
    assert_eq!(dimensions(&out), (THUMB_WIDTH as usize, 270));
    assert!(out.len() <= 30 * 1024, "{} bytes", out.len());
}

#[test]
fn small_and_odd_sizes_keep_their_proportions() {
    let out = prepare(&jpeg(720, 540), "image/jpeg").expect("prepare");
    assert_eq!(dimensions(&out), (480, 360));
    // Narrower than the thumbnail: never upscaled.
    let out = prepare(&jpeg(320, 240), "image/jpeg").expect("prepare");
    assert_eq!(dimensions(&out), (320, 240));
}

#[test]
fn a_png_cover_goes_through_the_general_path() {
    let out = prepare(&png(1920, 1080), "image/png").expect("prepare");
    assert_eq!(dimensions(&out), (480, 270));
}

#[test]
fn garbage_is_an_error_not_a_panic() {
    assert!(prepare(b"not an image", "image/jpeg").is_err());
}

#[tokio::test]
async fn a_cached_thumbnail_is_served_without_fetching() {
    let (_dir, cache) = cache();
    let calls = Arc::new(AtomicUsize::new(0));
    let service = ThumbService::new(
        Arc::clone(&cache),
        fetcher(Some((jpeg(1920, 1080), "image/jpeg")), Arc::clone(&calls)),
    );
    let first = service.get("scene", "7", "42").await.expect("scene kind");
    assert_eq!(dimensions(&first), (480, 270));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(cache
        .lock()
        .expect("lock")
        .get_bytes("thumb:scene:7:42")
        .is_some());
    let again = service.get("scene", "7", "42").await.expect("scene kind");
    assert_eq!(again, first);
    assert_eq!(calls.load(Ordering::SeqCst), 1, "served from the cache");
    // A new version fetches again.
    service.get("scene", "7", "43").await.expect("scene kind");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn failures_and_missing_screenshots_return_the_placeholder() {
    let (_dir, cache) = cache();
    let calls = Arc::new(AtomicUsize::new(0));
    let none = ThumbService::new(Arc::clone(&cache), fetcher(None, Arc::clone(&calls)));
    assert_eq!(
        none.get("scene", "1", "0").await.expect("kind"),
        PLACEHOLDER
    );
    let failing: SourceFetch =
        Arc::new(|_id: String| Box::pin(async { Err(AppError::NotConnected) }));
    let service = ThumbService::new(Arc::clone(&cache), failing);
    assert_eq!(
        service.get("scene", "2", "0").await.expect("kind"),
        PLACEHOLDER
    );
    assert!(
        cache
            .lock()
            .expect("lock")
            .get_bytes("thumb:scene:2:0")
            .is_none(),
        "failures aren't cached"
    );
}

#[tokio::test]
async fn unknown_kinds_are_refused() {
    let (_dir, cache) = cache();
    let service = ThumbService::new(cache, fetcher(None, Arc::default()));
    assert!(
        service.get("image", "1", "0").await.is_none(),
        "reserved for the galleries spec"
    );
    assert!(service.get("nonsense", "1", "0").await.is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn at_most_six_thumbnails_are_prepared_at_once() {
    let (_dir, cache) = cache();
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let source = jpeg(640, 360);
    let (a, p) = (Arc::clone(&active), Arc::clone(&peak));
    let slow: SourceFetch = Arc::new(move |_id: String| {
        let (a, p, source) = (Arc::clone(&a), Arc::clone(&p), source.clone());
        Box::pin(async move {
            let now = a.fetch_add(1, Ordering::SeqCst) + 1;
            p.fetch_max(now, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(30)).await;
            a.fetch_sub(1, Ordering::SeqCst);
            Ok(Some((source, "image/jpeg".to_owned())))
        })
    });
    let service = Arc::new(ThumbService::new(cache, slow));
    let tasks: Vec<_> = (0..20)
        .map(|i| {
            let s = Arc::clone(&service);
            tokio::spawn(async move { s.get("scene", &i.to_string(), "1").await })
        })
        .collect();
    for t in tasks {
        t.await.expect("task").expect("kind");
    }
    let seen = peak.load(Ordering::SeqCst);
    assert!(seen <= 6, "peak {seen}");
    assert!(seen >= 2, "they do run concurrently (peak {seen})");
}

/// Release-build timing for a 4K screenshot (budget < 15 ms). Run with
/// `cargo test --release -p stash-core --test thumbs -- --ignored --nocapture`.
#[test]
#[ignore = "timing; run in release"]
fn timing_4k_prepare() {
    let source = jpeg(3840, 2160);
    let mut times: Vec<f64> = (0..15)
        .map(|_| {
            let t = Instant::now();
            prepare(&source, "image/jpeg").expect("prepare");
            t.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    times.sort_by(f64::total_cmp);
    println!("4K prepare, median of 15: {:.1} ms", times[7]);
}
