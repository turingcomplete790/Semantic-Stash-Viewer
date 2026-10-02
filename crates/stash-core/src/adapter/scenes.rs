//! Read-only scene queries for the player (contracts/player-commands.md, "Stash GraphQL").

use graphql_client::GraphQLQuery;

use super::{endpoint, StashClient};
use crate::error::AppError;
use crate::scenes::paging::{last_page, validate_page_size};
use crate::scenes::query::{SceneQuery, SortDirection};
use crate::scenes::{
    self, direct_stream_url, display_title, resolution, screenshot_version, thumb_url, SceneCard,
    SceneFile, SceneGroup, SceneListItem, ScenePage,
};

/// Stash's custom `Int64` scalar (file sizes).
type Int64 = i64;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.json",
    query_path = "graphql/recent_scenes.graphql",
    response_derives = "Debug"
)]
pub struct RecentScenes;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.json",
    query_path = "graphql/playable_scene.graphql",
    response_derives = "Debug"
)]
pub struct PlayableScene;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.json",
    query_path = "graphql/test_scenes.graphql",
    response_derives = "Debug"
)]
pub struct TestScenes;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.json",
    query_path = "graphql/find_scenes_page.graphql",
    response_derives = "Debug"
)]
pub struct FindScenesPage;

/// One page of scene cards (005 research R1): `page_size` cards (one of `PAGE_SIZES`) in the
/// query's search and sort, with the total count. One request per page, card fields only
/// (constitution Principle IV). A page past the end (the library shrank, or the size grew) is
/// answered with the last page that exists.
pub async fn find_scenes_page(
    client: &StashClient,
    query: &SceneQuery,
    page: u32,
    page_size: u32,
) -> Result<ScenePage, AppError> {
    validate_page_size(page_size)?;
    let page = page.max(1);
    let (count, scenes) = fetch_scenes_page(client, query, page, page_size).await?;
    if scenes.is_empty() && count > 0 && page > last_page(count, page_size) {
        let last = last_page(count, page_size);
        let (count, scenes) = fetch_scenes_page(client, query, last, page_size).await?;
        return Ok(scene_page(count, last, page_size, scenes));
    }
    Ok(scene_page(count, page, page_size, scenes))
}

type PageScene = find_scenes_page::FindScenesPageFindScenesScenes;

async fn fetch_scenes_page(
    client: &StashClient,
    query: &SceneQuery,
    page: u32,
    page_size: u32,
) -> Result<(u32, Vec<PageScene>), AppError> {
    use find_scenes_page::{FindFilterType, SortDirectionEnum};
    let q = query.normalized();
    let filter = FindFilterType {
        q: (!q.search.is_empty()).then(|| q.search.clone()),
        page: Some(i64::from(page)),
        per_page: Some(i64::from(page_size)),
        sort: Some(q.stash_sort()?),
        direction: Some(match q.direction {
            SortDirection::Asc => SortDirectionEnum::ASC,
            SortDirection::Desc => SortDirectionEnum::DESC,
        }),
    };
    let body = FindScenesPage::build_query(find_scenes_page::Variables {
        filter: Some(filter),
    });
    let data: find_scenes_page::ResponseData = client.graphql(&body).await?;
    let found = data.find_scenes;
    Ok((u32::try_from(found.count).unwrap_or(u32::MAX), found.scenes))
}

fn scene_page(count: u32, page: u32, page_size: u32, scenes: Vec<PageScene>) -> ScenePage {
    ScenePage {
        count,
        page,
        page_size,
        items: scenes
            .into_iter()
            .map(|s| {
                let file = s.files.into_iter().next();
                SceneCard {
                    title: display_title(
                        s.title.as_deref(),
                        file.as_ref().map(|f| f.basename.as_str()),
                    ),
                    date: s.date.filter(|d| !d.is_empty()),
                    duration_seconds: file.as_ref().map(|f| f.duration),
                    resolution: file
                        .as_ref()
                        .and_then(|f| resolution(Some(f.width), Some(f.height))),
                    studio: s.studio.map(|st| st.name),
                    thumb: Some(thumb_url(
                        "scene",
                        &s.id,
                        &s.paths
                            .screenshot
                            .as_deref()
                            .map_or_else(|| "0".to_owned(), screenshot_version),
                    )),
                    has_preview: false,
                    id: s.id,
                }
            })
            .collect(),
    }
}

/// Build a list row from a scene's id, title, and primary file fields.
fn list_item(
    id: String,
    title: Option<&str>,
    file: Option<(&str, f64, i64, i64, &str, &str)>,
) -> SceneListItem {
    let (basename, duration, width, height, codec, format) = match file {
        Some(f) => (Some(f.0), f.1, Some(f.2), Some(f.3), Some(f.4), Some(f.5)),
        None => (None, 0.0, None, None, None, None),
    };
    SceneListItem {
        title: display_title(title, basename),
        duration_seconds: duration,
        resolution: resolution(width, height),
        video_codec: codec.filter(|c| !c.is_empty()).map(str::to_owned),
        container: format.filter(|c| !c.is_empty()).map(str::to_owned),
        id,
    }
}

/// The spike's test set (002 research R7): up to 3 random scenes from each hard-to-play group,
/// in one request. Groups keep a fixed order; empty groups are dropped.
pub async fn test_scenes(client: &StashClient) -> Result<Vec<SceneGroup>, AppError> {
    let body = TestScenes::build_query(test_scenes::Variables);
    let data: test_scenes::ResponseData = client.graphql(&body).await?;
    let rows = |scenes: Vec<test_scenes::TestSceneRow>| -> Vec<SceneListItem> {
        scenes
            .into_iter()
            .map(|s| {
                let file = s.files.first().map(|f| {
                    (
                        f.basename.as_str(),
                        f.duration,
                        f.width,
                        f.height,
                        f.video_codec.as_str(),
                        f.format.as_str(),
                    )
                });
                list_item(s.id.clone(), s.title.as_deref(), file)
            })
            .collect()
    };
    let groups = [
        ("4K H.264", rows(data.four_kh264.scenes)),
        ("4K HEVC", rows(data.four_k_hevc.scenes)),
        ("Above 4K", rows(data.above_four_k.scenes)),
        ("WMV above 720p", rows(data.wmv_hd.scenes)),
        ("VP9 WebM above 720p", rows(data.vp9_hd.scenes)),
        ("AV1", rows(data.av1.scenes)),
        ("MPEG-4 Part 2 (AVI/DivX)", rows(data.mpeg4.scenes)),
        ("FLV", rows(data.flv.scenes)),
    ];
    Ok(groups
        .into_iter()
        .filter(|(_, scenes)| !scenes.is_empty())
        .map(|(label, scenes)| SceneGroup {
            label: label.to_owned(),
            scenes,
        })
        .collect())
}

/// The 20 most recently added scenes (FR-001).
pub async fn recent_scenes(client: &StashClient) -> Result<Vec<SceneListItem>, AppError> {
    let body = RecentScenes::build_query(recent_scenes::Variables);
    let data: recent_scenes::ResponseData = client.graphql(&body).await?;
    Ok(data
        .find_scenes
        .scenes
        .into_iter()
        .map(|s| {
            let file = s.files.into_iter().next();
            SceneListItem {
                title: display_title(
                    s.title.as_deref(),
                    file.as_ref().map(|f| f.basename.as_str()),
                ),
                duration_seconds: file.as_ref().map_or(0.0, |f| f.duration),
                resolution: file
                    .as_ref()
                    .and_then(|f| resolution(Some(f.width), Some(f.height))),
                video_codec: file
                    .as_ref()
                    .map(|f| f.video_codec.clone())
                    .filter(|c| !c.is_empty()),
                container: file
                    .as_ref()
                    .map(|f| f.format.clone())
                    .filter(|c| !c.is_empty()),
                id: s.id,
            }
        })
        .collect())
}

/// One scene ready to play. The stream URL is built from the client's base URL (research R6).
pub async fn playable_scene(
    client: &StashClient,
    id: &str,
) -> Result<scenes::PlayableScene, AppError> {
    let body = PlayableScene::build_query(playable_scene::Variables { id: id.to_owned() });
    let data: playable_scene::ResponseData = client.graphql(&body).await?;
    let scene = data
        .find_scene
        .ok_or_else(|| AppError::SceneNotFound { id: id.to_owned() })?;
    let file = scene
        .files
        .into_iter()
        .next()
        .ok_or_else(|| AppError::NoPlayableFile { id: id.to_owned() })?;

    Ok(scenes::PlayableScene {
        title: display_title(scene.title.as_deref(), Some(&file.basename)),
        stream_url: direct_stream_url(client.base_url(), &scene.id),
        duration_seconds: file.duration,
        file: SceneFile {
            container: Some(file.format).filter(|s| !s.is_empty()),
            video_codec: Some(file.video_codec).filter(|s| !s.is_empty()),
            audio_codec: Some(file.audio_codec).filter(|s| !s.is_empty()),
            width: u32::try_from(file.width).ok().filter(|w| *w > 0),
            height: u32::try_from(file.height).ok().filter(|h| *h > 0),
            frame_rate: Some(file.frame_rate).filter(|f| *f > 0.0),
            bit_rate: u64::try_from(file.bit_rate).ok().filter(|b| *b > 0),
            size: u64::try_from(file.size).ok(),
        },
        id: scene.id,
    })
}

/// Largest screenshot passed to the UI; bigger ones are skipped (they cross the IPC bridge as
/// base64).
const MAX_SCREENSHOT_BYTES: usize = 2 * 1024 * 1024;
/// Thumbnail sources are resized right away, so large uploaded covers are fine (005 R5).
const MAX_THUMB_SOURCE_BYTES: usize = 16 * 1024 * 1024;

/// The scene's screenshot as a `data:` URL, or `None` if Stash has none (or it's too big).
///
/// Fetched by the core with the API key, because the UI never talks to Stash itself
/// (constitution Principle III) and an `<img>` can't send the `ApiKey` header. Read-only.
pub async fn scene_screenshot(client: &StashClient, id: &str) -> Result<Option<String>, AppError> {
    use base64::Engine as _;
    let Some((bytes, content_type)) = screenshot_bytes(client, id, MAX_SCREENSHOT_BYTES).await?
    else {
        return Ok(None);
    };
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(Some(format!("data:{content_type};base64,{encoded}")))
}

/// A scene's screenshot (or uploaded cover) as bytes and content type, for thumbnails (005
/// research R5). The key goes in a header; `None` when the scene has none.
pub async fn scene_screenshot_bytes(
    client: &StashClient,
    id: &str,
) -> Result<Option<(Vec<u8>, String)>, AppError> {
    screenshot_bytes(client, id, MAX_THUMB_SOURCE_BYTES).await
}

async fn screenshot_bytes(
    client: &StashClient,
    id: &str,
    max_bytes: usize,
) -> Result<Option<(Vec<u8>, String)>, AppError> {
    let mut url = endpoint(&endpoint(client.base_url(), "scene"), id);
    if let Ok(mut segments) = url.path_segments_mut() {
        segments.push("screenshot");
    }
    let response = client
        .get_with_key(url)
        .send()
        .await
        .map_err(|e| AppError::from(client.classify_transport_error(&e)))?;
    if !response.status().is_success() {
        return Ok(None);
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .filter(|t| t.starts_with("image/"))
        .unwrap_or("image/jpeg")
        .to_owned();
    if response
        .content_length()
        .is_some_and(|len| len > max_bytes as u64)
    {
        return Ok(None);
    }
    let bytes = response.bytes().await.map_err(|e| AppError::Internal {
        message: format!("couldn't read the screenshot: {e}"),
    })?;
    if bytes.len() > max_bytes {
        return Ok(None);
    }
    Ok(Some((bytes.to_vec(), content_type)))
}
