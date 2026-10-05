//! Thumbnails for grids (005 research R5): Stash's full-size scene screenshots (up to 4K) shrunk
//! to 480 px wide JPEGs, cached in the profile's view cache, and served to the webview through the
//! `ssv-thumb://` scheme so it never decodes a 4K image or sees an API key.
//!
//! JPEGs (nearly every screenshot) take the fast path: libjpeg-turbo decodes straight to 1/2, 1/4,
//! or 1/8 size (DCT scaling), SIMD resizing brings that to exactly 480 px, and libjpeg-turbo
//! encodes it (≈ 9 ms for a 4K screenshot, measured). Other formats (uploaded or scraped PNG/WebP
//! covers) are decoded by `image`. At most six thumbnails are fetched and prepared at once, and
//! decoding runs on the blocking pool; the cache lock is held only for the lookup and the write.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use fast_image_resize as fr;
use tokio::sync::Semaphore;

use crate::cache::ViewCache;
use crate::error::AppError;

/// Thumbnail width: enough for a ≈ 240 px card on a 2× display.
pub const THUMB_WIDTH: u32 = 480;
/// JPEG quality of the thumbnails (≈ 15–30 KB each).
const QUALITY: i32 = 80;
/// Thumbnails fetched and prepared at once.
const CONCURRENCY: usize = 6;

/// Shown when there's nothing to show (no screenshot, a failed fetch, an undecodable image).
pub const PLACEHOLDER: &[u8] = include_bytes!("../assets/thumb-placeholder.jpg");

/// Fetches a scene's source image (bytes and content type), or `None` if it has none.
pub type SourceFetch = Arc<
    dyn Fn(
            String,
        )
            -> Pin<Box<dyn Future<Output = Result<Option<(Vec<u8>, String)>, AppError>> + Send>>
        + Send
        + Sync,
>;

pub struct ThumbService {
    cache: Arc<Mutex<ViewCache>>,
    fetch: SourceFetch,
    slots: Semaphore,
}

impl ThumbService {
    pub fn new(cache: Arc<Mutex<ViewCache>>, fetch: SourceFetch) -> Self {
        Self {
            cache,
            fetch,
            slots: Semaphore::new(CONCURRENCY),
        }
    }

    /// The thumbnail for `kind`/`id` at `version`: cached, or fetched and prepared now. `None`
    /// for a kind this build doesn't serve (`image` and `gallery` arrive with the galleries
    /// spec); the placeholder when there's nothing to show.
    pub async fn get(&self, kind: &str, id: &str, version: &str) -> Option<Vec<u8>> {
        if kind != "scene" {
            return None;
        }
        let key = format!("thumb:{kind}:{id}:{version}");
        if let Some(bytes) = self.lock().get_bytes(&key) {
            return Some(bytes);
        }
        let Ok(_slot) = self.slots.acquire().await else {
            return Some(PLACEHOLDER.to_vec());
        };
        // Another request may have finished it while this one waited for a slot.
        if let Some(bytes) = self.lock().get_bytes(&key) {
            return Some(bytes);
        }
        let source = match (self.fetch)(id.to_owned()).await {
            Ok(Some(source)) => source,
            Ok(None) => return Some(PLACEHOLDER.to_vec()),
            Err(e) => {
                tracing::debug!(error = %e, id, "thumbnail source unavailable");
                return Some(PLACEHOLDER.to_vec());
            }
        };
        let prepared = tokio::task::spawn_blocking(move || prepare(&source.0, &source.1)).await;
        match prepared {
            Ok(Ok(bytes)) => {
                if let Err(e) = self.lock().put_bytes(&key, &bytes) {
                    tracing::warn!(error = %e, "couldn't cache a thumbnail");
                }
                Some(bytes)
            }
            Ok(Err(reason)) => {
                tracing::debug!(%reason, id, "couldn't make a thumbnail");
                Some(PLACEHOLDER.to_vec())
            }
            Err(e) => {
                tracing::warn!(error = %e, "thumbnail task failed");
                Some(PLACEHOLDER.to_vec())
            }
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ViewCache> {
        self.cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Shrink an image to `THUMB_WIDTH` (never enlarging it) and encode it as JPEG.
pub fn prepare(source: &[u8], content_type: &str) -> Result<Vec<u8>, String> {
    let is_jpeg = source.starts_with(&[0xFF, 0xD8, 0xFF]);
    let (rgb, width, height) = if is_jpeg {
        decode_jpeg_scaled(source, THUMB_WIDTH)?
    } else {
        decode_other(source, content_type)?
    };
    let (rgb, width, height) = if width > THUMB_WIDTH {
        resize(rgb, width, height, THUMB_WIDTH)?
    } else {
        (rgb, width, height)
    };
    encode_jpeg(&rgb, width, height)
}

/// A thumbnail (as `prepare` makes them) decoded to RGBA pixels for a UI to draw about `width`
/// wide without decoding on its own thread. JPEGs decode at the DCT scale (1/8, 1/4, 1/2, 1)
/// closest to `width` from above, allowing a 10% stretch, with no separate resize: that keeps it
/// cheap enough to run while a grid scrolls. `(pixels, width, height)`.
pub fn decode_rgba(source: &[u8], width: u32) -> Result<(Vec<u8>, u32, u32), String> {
    if !source.starts_with(&[0xFF, 0xD8, 0xFF]) {
        let image = image::load_from_memory(source)
            .map_err(|e| e.to_string())?
            .into_rgba8();
        let (w, h) = image.dimensions();
        return Ok((image.into_raw(), w, h));
    }
    let mut decoder = turbojpeg::Decompressor::new().map_err(|e| e.to_string())?;
    let header = decoder.read_header(source).map_err(|e| e.to_string())?;
    let enough = (width as usize * 9 / 10).max(1);
    let factor = [8, 4, 2, 1]
        .into_iter()
        .map(|n| turbojpeg::ScalingFactor::new(1, n))
        .find(|f| f.scale(header.width) >= enough)
        .unwrap_or(turbojpeg::ScalingFactor::ONE);
    decoder
        .set_scaling_factor(factor)
        .map_err(|e| e.to_string())?;
    let (w, h) = (factor.scale(header.width), factor.scale(header.height));
    let mut image = turbojpeg::Image {
        pixels: vec![0u8; w * h * 4],
        width: w,
        pitch: w * 4,
        height: h,
        format: turbojpeg::PixelFormat::RGBA,
    };
    decoder
        .decompress(source, image.as_deref_mut())
        .map_err(|e| e.to_string())?;
    Ok((image.pixels, dim(w)?, dim(h)?))
}

/// Decode at the largest DCT scale (1/8, 1/4, 1/2, 1) that stays at least `min_width` wide.
fn decode_jpeg_scaled(source: &[u8], min_width: u32) -> Result<(Vec<u8>, u32, u32), String> {
    let mut decoder = turbojpeg::Decompressor::new().map_err(|e| e.to_string())?;
    let header = decoder.read_header(source).map_err(|e| e.to_string())?;
    let factor = [8, 4, 2, 1]
        .into_iter()
        .map(|n| turbojpeg::ScalingFactor::new(1, n))
        .find(|f| f.scale(header.width) >= min_width as usize)
        .unwrap_or(turbojpeg::ScalingFactor::ONE);
    decoder
        .set_scaling_factor(factor)
        .map_err(|e| e.to_string())?;
    let (width, height) = (factor.scale(header.width), factor.scale(header.height));
    let mut image = turbojpeg::Image {
        pixels: vec![0u8; width * height * 3],
        width,
        pitch: width * 3,
        height,
        format: turbojpeg::PixelFormat::RGB,
    };
    decoder
        .decompress(source, image.as_deref_mut())
        .map_err(|e| e.to_string())?;
    Ok((image.pixels, dim(width)?, dim(height)?))
}

fn decode_other(source: &[u8], content_type: &str) -> Result<(Vec<u8>, u32, u32), String> {
    let image = image::load_from_memory(source)
        .map_err(|e| format!("{content_type}: {e}"))?
        .into_rgb8();
    let (width, height) = image.dimensions();
    Ok((image.into_raw(), width, height))
}

fn resize(
    rgb: Vec<u8>,
    width: u32,
    height: u32,
    target_width: u32,
) -> Result<(Vec<u8>, u32, u32), String> {
    let target_height =
        u32::try_from(u64::from(height) * u64::from(target_width) / u64::from(width))
            .unwrap_or(1)
            .max(1);
    let source = fr::images::Image::from_vec_u8(width, height, rgb, fr::PixelType::U8x3)
        .map_err(|e| e.to_string())?;
    let mut target = fr::images::Image::new(target_width, target_height, fr::PixelType::U8x3);
    fr::Resizer::new()
        .resize(
            &source,
            &mut target,
            &fr::ResizeOptions::new()
                .resize_alg(fr::ResizeAlg::Convolution(fr::FilterType::Bilinear)),
        )
        .map_err(|e| e.to_string())?;
    Ok((target.into_vec(), target_width, target_height))
}

fn encode_jpeg(rgb: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let (w, h) = (width as usize, height as usize);
    let image = turbojpeg::Image {
        pixels: rgb,
        width: w,
        pitch: w * 3,
        height: h,
        format: turbojpeg::PixelFormat::RGB,
    };
    turbojpeg::compress(image, QUALITY, turbojpeg::Subsamp::Sub2x2)
        .map(|buf| buf.to_vec())
        .map_err(|e| e.to_string())
}

fn dim(n: usize) -> Result<u32, String> {
    u32::try_from(n).map_err(|_| "image too large".to_owned())
}
