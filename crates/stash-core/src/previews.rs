//! Animated scene previews: fetch and decode to frames (spec FR-019, research R8).
//!
//! The preview is Stash's animated WebP (`/scene/<id>/webp`), an image format: it's decoded here,
//! off the UI thread, into RGBA frames at the card's width, never played as video (constitution
//! Principle V). A scene without a generated preview is a quiet `None`.

use std::io::Cursor;

use fast_image_resize as fr;
use image::codecs::webp::WebPDecoder;
use image::AnimationDecoder;

use crate::adapter::scenes::resource_bytes;
use crate::adapter::StashClient;
use crate::error::AppError;

/// The largest preview fetched (Stash's are around 1–2 MB).
const MAX_PREVIEW_BYTES: usize = 32 * 1024 * 1024;
/// A frame shown for less than this is shown this long (as browsers do for 0 or tiny delays).
const MIN_DELAY_MS: u32 = 20;
/// The delay for frames that don't say.
const DEFAULT_DELAY_MS: u32 = 100;

/// One frame of a preview, ready to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewFrame {
    /// RGBA, `width × height × 4` bytes.
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// How long it shows, in milliseconds.
    pub delay_ms: u32,
}

/// The preview's file (`preview:<id>` in the cache), or `None` when Stash hasn't generated one.
pub async fn preview_bytes(client: &StashClient, id: &str) -> Result<Option<Vec<u8>>, AppError> {
    Ok(
        resource_bytes(client, "scene", id, "webp", "preview", MAX_PREVIEW_BYTES)
            .await?
            .map(|(bytes, _)| bytes),
    )
}

/// Decode an animated WebP into frames `width` pixels wide (aspect kept; never enlarged). A still
/// WebP is one frame.
pub fn decode_preview(bytes: &[u8], width: u32) -> Result<Vec<PreviewFrame>, String> {
    let mut out = Vec::new();
    decode_preview_each(bytes, width, |f| {
        out.push(f);
        true
    })?;
    Ok(out)
}

/// [`decode_preview`] a frame at a time, so a preview can start playing at its first frame while
/// the rest decode. `on_frame` returns `false` to stop (the preview was left). Returns how many
/// frames were decoded.
pub fn decode_preview_each(
    bytes: &[u8],
    width: u32,
    mut on_frame: impl FnMut(PreviewFrame) -> bool,
) -> Result<usize, String> {
    let decoder = WebPDecoder::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut count = 0;
    for frame in decoder.into_frames() {
        let frame = frame.map_err(|e| e.to_string())?;
        let (numer, denom) = frame.delay().numer_denom_ms();
        let delay = if denom == 0 {
            DEFAULT_DELAY_MS
        } else {
            (numer / denom).max(MIN_DELAY_MS)
        };
        let buffer = frame.into_buffer();
        let (w, h) = buffer.dimensions();
        let (rgba, w, h) = scale(buffer.into_raw(), w, h, width)?;
        count += 1;
        if !on_frame(PreviewFrame {
            rgba,
            width: w,
            height: h,
            delay_ms: delay,
        }) {
            return Ok(count);
        }
    }
    if count == 0 {
        return Err("the preview has no frames".into());
    }
    Ok(count)
}

/// RGBA pixels at `target` width, or as they are when they're no wider.
fn scale(
    rgba: Vec<u8>,
    width: u32,
    height: u32,
    target: u32,
) -> Result<(Vec<u8>, u32, u32), String> {
    if target == 0 || width <= target {
        return Ok((rgba, width, height));
    }
    let target_height = u32::try_from(u64::from(height) * u64::from(target) / u64::from(width))
        .map_err(|e| e.to_string())?
        .max(1);
    let source = fr::images::Image::from_vec_u8(width, height, rgba, fr::PixelType::U8x4)
        .map_err(|e| e.to_string())?;
    let mut scaled = fr::images::Image::new(target, target_height, fr::PixelType::U8x4);
    fr::Resizer::new()
        .resize(
            &source,
            &mut scaled,
            &fr::ResizeOptions::new()
                .resize_alg(fr::ResizeAlg::Convolution(fr::FilterType::Bilinear)),
        )
        .map_err(|e| e.to_string())?;
    Ok((scaled.into_vec(), target, target_height))
}
