//! Read-only scene queries for the player (contracts/player-commands.md, "Stash GraphQL").

use graphql_client::GraphQLQuery;

use super::StashClient;
use crate::error::AppError;
use crate::scenes::{
    self, direct_stream_url, display_title, resolution, SceneFile, SceneGroup, SceneListItem,
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
