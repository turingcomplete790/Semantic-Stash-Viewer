//! Images (012): their sorts, cards, pages, and details. The list machinery is the browse layer's
//! (`crate::browse`); this holds what is images' own.

pub mod neighbours;
pub mod viewer;

use serde::{Deserialize, Serialize};

use crate::browse::query::{ListQuery, Sort};

/// Every sort the web UI offers for images (v0.31.1 `images.ts` plus `filter-options.ts`'s media
/// and common options), serialized as Stash's sort names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ImageSort {
    Filesize,
    FileCount,
    Date,
    Resolution,
    Title,
    /// The web UI's default for images.
    #[default]
    Path,
    Rating,
    FileModTime,
    TagCount,
    PerformerCount,
    Random,
    OCounter,
    CreatedAt,
    UpdatedAt,
}

impl Sort for ImageSort {
    const RANDOM: ImageSort = ImageSort::Random;
}

impl ImageSort {
    /// In the web UI's menu order (alphabetical by label, as it shows them).
    pub fn all() -> &'static [ImageSort] {
        use ImageSort::*;
        &[
            CreatedAt,
            Date,
            FileCount,
            FileModTime,
            Filesize,
            OCounter,
            Path,
            PerformerCount,
            Random,
            Rating,
            Resolution,
            TagCount,
            Title,
            UpdatedAt,
        ]
    }

    /// The web UI's label (Stash's default en-GB locale).
    pub fn label(self) -> &'static str {
        use ImageSort::*;
        match self {
            Filesize => "File Size",
            FileCount => "File Count",
            Date => "Date",
            Resolution => "Resolution",
            Title => "Title",
            Path => "Path",
            Rating => "Rating",
            FileModTime => "File Modification Time",
            TagCount => "Tag Count",
            PerformerCount => "Performer Count",
            Random => "Random",
            OCounter => "O-Count",
            CreatedAt => "Created At",
            UpdatedAt => "Updated At",
        }
    }
}

/// What an Images tab is showing. The web UI's default is path, ascending.
pub type ImageQuery = ListQuery<ImageSort>;

impl ImageQuery {
    /// A new Images query in the web UI's default order.
    pub fn images_default() -> ImageQuery {
        ImageQuery {
            sort: ImageSort::Path,
            direction: crate::browse::query::SortDirection::Asc,
            ..ImageQuery::default()
        }
    }
}

/// How an image's file moves (012 data model "ImageCard"): decides looping in the viewer and the
/// animated card preview.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Media {
    Still,
    /// GIF, WebP, or APNG: may be animated (known once decoded).
    MayAnimate,
    /// The file is a short video (an image clip).
    Clip {
        duration: f64,
    },
}

/// One card in the Images grid (012 data model "ImageCard").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageCard {
    pub id: String,
    /// Stash's title, else the file's name (as the web UI).
    pub title: String,
    pub date: Option<String>,
    /// `ssv-thumb://localhost/image/<id>?v=<version>`: served by the core, never carrying a key.
    pub thumb: Option<String>,
    pub width: u32,
    pub height: u32,
    pub media: Media,
}

/// One page of image cards with the total number of matching images.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagePage {
    pub count: u32,
    /// 1-based: the page actually returned (the last page when the request was past the end).
    pub page: u32,
    pub page_size: u32,
    pub items: Vec<ImageCard>,
}

/// The image's file, as the details panel shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageFile {
    pub path: String,
    pub size: u64,
    pub format: String,
    pub width: u32,
    pub height: u32,
    /// For a clip.
    pub duration: Option<f64>,
}

/// What the details panel shows (012 data model "ImageDetails"). Cached per image
/// (`image:details:<id>`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageDetails {
    pub id: String,
    pub title: String,
    pub code: Option<String>,
    pub date: Option<String>,
    pub photographer: Option<String>,
    pub details: Option<String>,
    /// 0–100.
    pub rating100: Option<u8>,
    pub organized: bool,
    pub o_count: u32,
    pub urls: Vec<String>,
    pub studio: Option<crate::scenes::Named>,
    pub performers: Vec<crate::scenes::PerformerChip>,
    pub tags: Vec<crate::scenes::Named>,
    pub galleries: Vec<crate::scenes::GalleryChip>,
    pub file: Option<ImageFile>,
}

/// The media kind for a file's format (Stash's `format`: `jpeg` or `mjpeg`, `png`, `gif`, `webp`,
/// …). With image clips on, Stash makes animated GIFs clips (`VideoFile`), handled by the caller.
pub fn media_for_format(format: &str) -> Media {
    match format.to_ascii_lowercase().as_str() {
        "gif" | "webp" | "apng" => Media::MayAnimate,
        _ => Media::Still,
    }
}
