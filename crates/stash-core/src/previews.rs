//! Animated card previews: fetch and decode to frames (spec FR-019, research R8; images since 012
//! research R7).
//!
//! A scene's preview is Stash's animated WebP (`/scene/<id>/webp`); an animated image's is its own
//! file (GIF, WebP, or APNG, under 16 MB). Both are image formats: they're decoded here, off the UI
//! thread, into RGBA frames at the card's width, never played as video (constitution Principle
//! V). A scene without a generated preview is a quiet `None`.

use std::io::Cursor;

use fast_image_resize as fr;
use image::codecs::gif::GifDecoder;
use image::codecs::png::PngDecoder;
use image::codecs::webp::WebPDecoder;
use image::{AnimationDecoder, Frame, ImageResult};

use crate::adapter::scenes::resource_bytes;
use crate::adapter::StashClient;
use crate::error::AppError;

/// The largest preview fetched (Stash's are around 1–2 MB).
const MAX_PREVIEW_BYTES: usize = 32 * 1024 * 1024;
/// The largest image file decoded as a card preview (012 FR-016): bigger ones keep their still.
pub const MAX_ANIMATION_BYTES: usize = 16 * 1024 * 1024;
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

/// An animated image's own file (`/image/<id>/image`) as its card preview, or `None` when Stash
/// has no such image or it's over [`MAX_ANIMATION_BYTES`].
pub async fn image_animation_bytes(
    client: &StashClient,
    id: &str,
) -> Result<Option<Vec<u8>>, AppError> {
    Ok(resource_bytes(
        client,
        "image",
        id,
        "image",
        "image-preview",
        MAX_ANIMATION_BYTES,
    )
    .await?
    .map(|(bytes, _)| bytes))
}

/// An animation's container, from its first bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationFormat {
    Gif,
    WebP,
    /// A PNG; animated (APNG) when it has an `acTL` chunk, otherwise one frame.
    Png,
}

/// The format of `bytes`, or `None` for anything else (a JPEG, a video).
pub fn animation_format(bytes: &[u8]) -> Option<AnimationFormat> {
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(AnimationFormat::Gif)
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(AnimationFormat::WebP)
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(AnimationFormat::Png)
    } else {
        None
    }
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
    on_frame: impl FnMut(PreviewFrame) -> bool,
) -> Result<usize, String> {
    decode_animation_each(bytes, width, on_frame)
}

/// Decode a GIF, WebP, or APNG (a still one is one frame) into frames `width` pixels wide, a frame
/// at a time as [`decode_preview_each`].
pub fn decode_animation_each(
    bytes: &[u8],
    width: u32,
    mut on_frame: impl FnMut(PreviewFrame) -> bool,
) -> Result<usize, String> {
    let error = |e: image::ImageError| e.to_string();
    let frames: Box<dyn Iterator<Item = ImageResult<Frame>>> = match animation_format(bytes) {
        Some(AnimationFormat::Gif) => Box::new(
            GifDecoder::new(Cursor::new(bytes))
                .map_err(error)?
                .into_frames(),
        ),
        Some(AnimationFormat::WebP) => Box::new(
            WebPDecoder::new(Cursor::new(bytes))
                .map_err(error)?
                .into_frames(),
        ),
        Some(AnimationFormat::Png) => {
            let png = PngDecoder::new(Cursor::new(bytes)).map_err(error)?;
            if png.is_apng().map_err(error)? {
                Box::new(png.apng().map_err(error)?.into_frames())
            } else {
                let image = image::DynamicImage::from_decoder(png).map_err(error)?;
                Box::new(std::iter::once(Ok(Frame::new(image.into_rgba8()))))
            }
        }
        None => return Err("not an animated image format".into()),
    };
    let mut count = 0;
    for frame in frames {
        let frame = frame.map_err(|e| e.to_string())?;
        let (numer, denom) = frame.delay().numer_denom_ms();
        let delay = numer
            .checked_div(denom)
            .map_or(DEFAULT_DELAY_MS, |ms| ms.max(MIN_DELAY_MS));
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
