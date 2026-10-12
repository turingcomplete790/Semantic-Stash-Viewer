//! Images' requests (012 research R2): one card-tier request per page, one detail-tier request
//! per image whose details are open, and the full file for the viewer. Never a request per card.

use cynic::QueryBuilder;

use super::gql::{schema, FindFilterType, ImageFilterType, Int64, SortDirectionEnum};
use super::StashClient;
use crate::browse::paging::{last_page, validate_page_size};
use crate::error::AppError;
use crate::images::{
    media_for_format, ImageCard, ImageDetails, ImageFile, ImagePage, ImageQuery, Media,
};
use crate::scenes::query::SortDirection;
use crate::scenes::{
    display_title, picture_version, screenshot_version, thumb_url, GalleryChip, Named,
    PerformerChip,
};

// ---- ImagesPage (card tier) -----------------------------------------------------------------

#[derive(cynic::QueryVariables, Debug)]
pub(crate) struct ImagesPageVariables {
    pub(crate) filter: FindFilterType,
    pub(crate) image_filter: Option<ImageFilterType>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "ImageFile")]
struct ImageFileCard {
    basename: String,
    width: i32,
    height: i32,
    format: String,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "VideoFile")]
struct VideoFileCard {
    basename: String,
    width: i32,
    height: i32,
    duration: f64,
}

#[derive(cynic::InlineFragments, Debug)]
#[cynic(graphql_type = "VisualFile")]
enum VisualFileCard {
    ImageFile(ImageFileCard),
    VideoFile(VideoFileCard),
    #[cynic(fallback)]
    Other,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "ImagePathsType")]
struct ThumbnailPath {
    thumbnail: Option<String>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Image")]
struct ImageCardFields {
    id: cynic::Id,
    title: Option<String>,
    date: Option<String>,
    paths: ThumbnailPath,
    #[cynic(rename = "visual_files")]
    visual_files: Vec<VisualFileCard>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindImagesResultType")]
struct ImageCards {
    count: i32,
    images: Vec<ImageCardFields>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "ImagesPageVariables")]
pub(crate) struct ImagesPage {
    #[arguments(filter: $filter, image_filter: $image_filter)]
    find_images: ImageCards,
}

/// One page of image cards for `query` (search, criteria, sort), with the total count. One request
/// per page, card fields only (constitution Principle IV). A page past the end is answered with the
/// last page that exists.
pub async fn find_images_page(
    client: &StashClient,
    query: &ImageQuery,
    page: u32,
    page_size: u32,
) -> Result<ImagePage, AppError> {
    validate_page_size(page_size)?;
    let page = page.max(1);
    let (count, images) = fetch_page(client, query, page, page_size).await?;
    if images.is_empty() && count > 0 && page > last_page(count, page_size) {
        let last = last_page(count, page_size);
        let (count, images) = fetch_page(client, query, last, page_size).await?;
        return Ok(image_page(count, last, page_size, images));
    }
    Ok(image_page(count, page, page_size, images))
}

async fn fetch_page(
    client: &StashClient,
    query: &ImageQuery,
    page: u32,
    page_size: u32,
) -> Result<(u32, Vec<ImageCardFields>), AppError> {
    let q = query.normalized();
    let filter = FindFilterType {
        q: (!q.search.is_empty()).then(|| q.search.clone()),
        page: Some(i32::try_from(page).unwrap_or(i32::MAX)),
        per_page: Some(i32::try_from(page_size).unwrap_or(i32::MAX)),
        sort: Some(q.stash_sort()?),
        direction: Some(match q.direction {
            SortDirection::Asc => SortDirectionEnum::Asc,
            SortDirection::Desc => SortDirectionEnum::Desc,
        }),
    };
    let data = client
        .graphql(&ImagesPage::build(ImagesPageVariables {
            filter,
            image_filter: crate::scenes::filter::to_stash::image_filter(&q.criteria),
        }))
        .await?;
    let found = data.find_images;
    Ok((u32::try_from(found.count).unwrap_or(u32::MAX), found.images))
}

fn dimension(n: i32) -> u32 {
    u32::try_from(n).unwrap_or(0)
}

fn image_page(count: u32, page: u32, page_size: u32, images: Vec<ImageCardFields>) -> ImagePage {
    ImagePage {
        count,
        page,
        page_size,
        items: images
            .into_iter()
            .map(|i| {
                let (basename, width, height, media) = match i.visual_files.into_iter().next() {
                    Some(VisualFileCard::ImageFile(f)) => (
                        Some(f.basename),
                        dimension(f.width),
                        dimension(f.height),
                        media_for_format(&f.format),
                    ),
                    Some(VisualFileCard::VideoFile(f)) => (
                        Some(f.basename),
                        dimension(f.width),
                        dimension(f.height),
                        Media::Clip {
                            duration: f.duration,
                        },
                    ),
                    Some(VisualFileCard::Other) | None => (None, 0, 0, Media::Still),
                };
                let version = i
                    .paths
                    .thumbnail
                    .as_deref()
                    .map_or_else(|| "0".to_owned(), screenshot_version);
                ImageCard {
                    title: display_title(i.title.as_deref(), basename.as_deref()),
                    date: i.date.filter(|d| !d.is_empty()),
                    thumb: Some(thumb_url("image", i.id.inner(), &version)),
                    width,
                    height,
                    media,
                    id: i.id.into_inner(),
                }
            })
            .collect(),
    }
}

// ---- ImageDetails (detail tier) -------------------------------------------------------------

#[derive(cynic::QueryVariables, Debug)]
pub(crate) struct ImageIdVariables {
    pub(crate) id: cynic::Id,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Studio")]
struct StudioRef {
    id: cynic::Id,
    name: String,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Performer")]
struct PerformerRef {
    id: cynic::Id,
    name: String,
    #[cynic(rename = "image_path")]
    image_path: Option<String>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Tag")]
struct TagRef {
    id: cynic::Id,
    name: String,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Gallery")]
struct GalleryRef {
    id: cynic::Id,
    title: Option<String>,
    #[cynic(rename = "image_count")]
    image_count: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "ImageFile")]
struct ImageFileDetail {
    basename: String,
    path: String,
    size: Int64,
    format: String,
    width: i32,
    height: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "VideoFile")]
struct VideoFileDetail {
    basename: String,
    path: String,
    size: Int64,
    format: String,
    width: i32,
    height: i32,
    duration: f64,
}

#[derive(cynic::InlineFragments, Debug)]
#[cynic(graphql_type = "VisualFile")]
enum VisualFileDetail {
    ImageFile(ImageFileDetail),
    VideoFile(VideoFileDetail),
    #[cynic(fallback)]
    Other,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Image")]
struct ImageDetailFields {
    id: cynic::Id,
    title: Option<String>,
    code: Option<String>,
    date: Option<String>,
    photographer: Option<String>,
    details: Option<String>,
    rating100: Option<i32>,
    organized: bool,
    #[cynic(rename = "o_counter")]
    o_counter: Option<i32>,
    urls: Vec<String>,
    studio: Option<StudioRef>,
    performers: Vec<PerformerRef>,
    tags: Vec<TagRef>,
    galleries: Vec<GalleryRef>,
    #[cynic(rename = "visual_files")]
    visual_files: Vec<VisualFileDetail>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "ImageIdVariables")]
pub(crate) struct ImageDetailsQuery {
    #[arguments(id: $id)]
    find_image: Option<ImageDetailFields>,
}

/// One `ImageDetails` request (cached as `image:details:<id>`).
pub async fn image_details(client: &StashClient, id: &str) -> Result<ImageDetails, AppError> {
    let data = client
        .graphql(&ImageDetailsQuery::build(ImageIdVariables {
            id: cynic::Id::new(id),
        }))
        .await?;
    let image = data
        .find_image
        .ok_or_else(|| AppError::ImageNotFound { id: id.to_owned() })?;
    let text = |s: Option<String>| s.filter(|s| !s.trim().is_empty());
    let size = |n: Int64| u64::try_from(n.0).unwrap_or(0);
    let (basename, file) = match image.visual_files.into_iter().next() {
        Some(VisualFileDetail::ImageFile(f)) => (
            Some(f.basename),
            Some(ImageFile {
                path: f.path,
                size: size(f.size),
                format: f.format,
                width: dimension(f.width),
                height: dimension(f.height),
                duration: None,
            }),
        ),
        Some(VisualFileDetail::VideoFile(f)) => (
            Some(f.basename),
            Some(ImageFile {
                path: f.path,
                size: size(f.size),
                format: f.format,
                width: dimension(f.width),
                height: dimension(f.height),
                duration: Some(f.duration),
            }),
        ),
        Some(VisualFileDetail::Other) | None => (None, None),
    };
    Ok(ImageDetails {
        title: display_title(image.title.as_deref(), basename.as_deref()),
        code: text(image.code),
        date: text(image.date),
        photographer: text(image.photographer),
        details: text(image.details),
        rating100: image.rating100.and_then(|r| u8::try_from(r).ok()),
        organized: image.organized,
        o_count: image
            .o_counter
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0),
        urls: image
            .urls
            .into_iter()
            .filter(|u| !u.trim().is_empty())
            .collect(),
        studio: image.studio.map(|s| Named {
            id: s.id.into_inner(),
            name: s.name,
        }),
        performers: image
            .performers
            .into_iter()
            .map(|p| PerformerChip {
                image: picture_version(p.image_path.as_deref()),
                id: p.id.into_inner(),
                name: p.name,
            })
            .collect(),
        tags: image
            .tags
            .into_iter()
            .map(|t| Named {
                id: t.id.into_inner(),
                name: t.name,
            })
            .collect(),
        galleries: image
            .galleries
            .into_iter()
            .map(|g| GalleryChip {
                id: g.id.into_inner(),
                title: g.title.filter(|t| !t.trim().is_empty()).unwrap_or_default(),
                image_count: u32::try_from(g.image_count).unwrap_or(0),
            })
            .collect(),
        file,
        id: image.id.into_inner(),
    })
}

// ---- Files ----------------------------------------------------------------------------------

/// The largest image file fetched for the viewer (an 80 MP PNG is well under this).
pub const MAX_IMAGE_BYTES: usize = 512 * 1024 * 1024;

/// The image's full file (`/image/<id>/image`), fetched with the key in a header; `None` when
/// Stash has no such image.
pub async fn image_bytes(
    client: &StashClient,
    id: &str,
) -> Result<Option<(Vec<u8>, String)>, AppError> {
    super::scenes::resource_bytes(client, "image", id, "image", "image", MAX_IMAGE_BYTES).await
}
