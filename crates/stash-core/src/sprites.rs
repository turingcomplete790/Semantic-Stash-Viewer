//! Seek-bar sprite thumbnails: the VTT index and the sprite sheet (spec FR-021, research R9).
//!
//! Stash's sprite sheet is one JPEG of small frames; its WebVTT index maps each time range to a
//! region of the sheet (`start --> end`, then `sprite.jpg#xywh=x,y,w,h`). Both are fetched once
//! per scene, and the hovered time's region is drawn by cropping the decoded sheet.

use crate::adapter::scenes::resource_bytes;
use crate::adapter::StashClient;
use crate::error::AppError;

const MAX_VTT_BYTES: usize = 1024 * 1024;
const MAX_SHEET_BYTES: usize = 16 * 1024 * 1024;

/// A region of the sprite sheet, in its pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// One cue: from `start` until `end` seconds, show `rect`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpriteCue {
    pub start: f64,
    pub end: f64,
    pub rect: Rect,
}

/// The decoded sheet and its index.
#[derive(Debug, Clone, PartialEq)]
pub struct SpriteSheet {
    pub cues: Vec<SpriteCue>,
    /// RGBA, `width × height × 4` bytes.
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// `hh:mm:ss.mmm` or `mm:ss.mmm` in seconds.
fn timestamp(s: &str) -> Option<f64> {
    let mut total = 0.0;
    for part in s.trim().split(':') {
        total = total * 60.0 + part.trim().parse::<f64>().ok()?;
    }
    Some(total)
}

/// The cues of a sprite VTT (malformed cues are skipped), in time order.
pub fn parse_vtt(text: &str) -> Vec<SpriteCue> {
    let mut cues = Vec::new();
    let mut lines = text.lines().map(str::trim);
    while let Some(line) = lines.next() {
        let Some((from, to)) = line.split_once("-->") else {
            continue;
        };
        let (Some(start), Some(end)) = (
            timestamp(from),
            timestamp(to.split_whitespace().next().unwrap_or("")),
        ) else {
            continue;
        };
        let Some(target) = lines.next() else {
            break;
        };
        let Some((_, xywh)) = target.split_once("#xywh=") else {
            continue;
        };
        let n: Vec<u32> = xywh
            .split(',')
            .filter_map(|v| v.trim().parse().ok())
            .collect();
        if let [x, y, w, h] = n[..] {
            cues.push(SpriteCue {
                start,
                end,
                rect: Rect { x, y, w, h },
            });
        }
    }
    cues.sort_by(|a, b| a.start.total_cmp(&b.start));
    cues
}

/// The region for `time`: the cue it falls in, or the last cue for a time past the end.
pub fn lookup(cues: &[SpriteCue], time: f64) -> Option<Rect> {
    let after = cues.partition_point(|c| c.start <= time);
    let cue = cues.get(after.checked_sub(1)?)?;
    (time < cue.end || after == cues.len()).then_some(cue.rect)
}

/// Fetch the index and the sheet (`sprite:<id>` in the cache), or `None` when Stash hasn't
/// generated them. Two GETs.
pub async fn sprite_sources(
    client: &StashClient,
    id: &str,
) -> Result<Option<(String, Vec<u8>)>, AppError> {
    let Some((vtt, _)) = resource_bytes(
        client,
        "scene",
        id,
        "vtt/thumbs",
        "sprite-vtt",
        MAX_VTT_BYTES,
    )
    .await?
    else {
        return Ok(None);
    };
    let Some((sheet, _)) =
        resource_bytes(client, "scene", id, "vtt/sprite", "sprite", MAX_SHEET_BYTES).await?
    else {
        return Ok(None);
    };
    Ok(Some((String::from_utf8_lossy(&vtt).into_owned(), sheet)))
}

/// Parse the index and decode the sheet (off the UI thread).
pub fn decode_sheet(vtt: &str, sheet: &[u8]) -> Result<SpriteSheet, String> {
    let cues = parse_vtt(vtt);
    if cues.is_empty() {
        return Err("the sprite index has no cues".into());
    }
    // Stash's sheets are JPEG (libjpeg-turbo, as thumbnails); anything else through `image`.
    let (rgba, width, height) = match turbojpeg::decompress(sheet, turbojpeg::PixelFormat::RGBA) {
        Ok(img) => {
            let w = u32::try_from(img.width).map_err(|e| e.to_string())?;
            let h = u32::try_from(img.height).map_err(|e| e.to_string())?;
            (img.pixels, w, h)
        }
        Err(_) => {
            let rgba = image::load_from_memory(sheet)
                .map_err(|e| e.to_string())?
                .to_rgba8();
            let (w, h) = rgba.dimensions();
            (rgba.into_raw(), w, h)
        }
    };
    Ok(SpriteSheet {
        cues,
        rgba,
        width,
        height,
    })
}
