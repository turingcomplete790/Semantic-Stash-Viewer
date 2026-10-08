//! The scene page's requests (008 US2; contracts/query-budget.md `SceneDetails`, `SceneFiles`,
//! `SceneGalleries`): one detail-tier request for what the page shows by default, and one each for
//! the sections opened on demand. Never a request per performer, tag, marker, or file.

use cynic::QueryBuilder;

use super::gql::{schema, Int64, PlayableFileFields};
use super::StashClient;
use crate::error::AppError;
use crate::scenes::{
    display_title, picture_version, screenshot_version, FileFacts, GalleryChip, GroupRef, Marker,
    Named, PerformerChip, SceneDetails as Details,
};

#[derive(cynic::QueryVariables, Debug)]
pub(crate) struct SceneIdVariables {
    pub(crate) id: cynic::Id,
}

// ---- SceneDetails (detail tier) ---------------------------------------------------------------

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
#[cynic(graphql_type = "Group")]
struct GroupName {
    id: cynic::Id,
    name: String,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "SceneGroup")]
struct SceneGroupFields {
    group: GroupName,
    #[cynic(rename = "scene_index")]
    scene_index: Option<i32>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Tag")]
struct TagLabel {
    name: String,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "SceneMarker")]
struct MarkerFields {
    id: cynic::Id,
    title: String,
    seconds: f64,
    #[cynic(rename = "end_seconds")]
    end_seconds: Option<f64>,
    #[cynic(rename = "primary_tag")]
    primary_tag: TagLabel,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "ScenePathsType")]
struct DetailPaths {
    screenshot: Option<String>,
    vtt: Option<String>,
    sprite: Option<String>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Scene")]
struct SceneDetailFields {
    id: cynic::Id,
    title: Option<String>,
    code: Option<String>,
    details: Option<String>,
    director: Option<String>,
    date: Option<String>,
    rating100: Option<i32>,
    organized: bool,
    #[cynic(rename = "o_counter")]
    o_counter: Option<i32>,
    #[cynic(rename = "play_count")]
    play_count: Option<i32>,
    #[cynic(rename = "resume_time")]
    resume_time: Option<f64>,
    #[cynic(rename = "play_duration")]
    play_duration: Option<f64>,
    urls: Vec<String>,
    studio: Option<StudioRef>,
    performers: Vec<PerformerRef>,
    tags: Vec<TagRef>,
    groups: Vec<SceneGroupFields>,
    #[cynic(rename = "scene_markers")]
    scene_markers: Vec<MarkerFields>,
    files: Vec<PlayableFileFields>,
    paths: DetailPaths,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "SceneIdVariables")]
pub(crate) struct SceneDetails {
    #[arguments(id: $id)]
    find_scene: Option<SceneDetailFields>,
}

/// The last path segment without its query (a generated file's name; never the address or key).
fn file_name(path: Option<&str>) -> Option<String> {
    let path = path?;
    let without_query = path.split(['?', '#']).next().unwrap_or(path);
    without_query
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// One `SceneDetails` request: everything the scene page shows by default (cached as
/// `scene:details:<id>`).
pub async fn scene_details(client: &StashClient, id: &str) -> Result<Details, AppError> {
    let data = client
        .graphql(&SceneDetails::build(SceneIdVariables {
            id: cynic::Id::new(id),
        }))
        .await?;
    let scene = data
        .find_scene
        .ok_or_else(|| AppError::SceneNotFound { id: id.to_owned() })?;
    let file = scene.files.first();
    let text = |s: Option<String>| s.filter(|s| !s.trim().is_empty());
    let count = |n: Option<i32>| n.and_then(|n| u32::try_from(n).ok()).unwrap_or(0);
    let mut markers: Vec<Marker> = scene
        .scene_markers
        .into_iter()
        .map(|m| Marker {
            id: m.id.into_inner(),
            title: m.title,
            seconds: m.seconds,
            end_seconds: m.end_seconds,
            tag: m.primary_tag.name,
        })
        .collect();
    markers.sort_by(|a, b| a.seconds.total_cmp(&b.seconds));
    Ok(Details {
        title: display_title(scene.title.as_deref(), file.map(|f| f.basename.as_str())),
        code: text(scene.code),
        date: text(scene.date),
        details: text(scene.details),
        director: text(scene.director),
        studio: scene.studio.map(|s| Named {
            id: s.id.into_inner(),
            name: s.name,
        }),
        performers: scene
            .performers
            .into_iter()
            .map(|p| PerformerChip {
                image: picture_version(p.image_path.as_deref()),
                id: p.id.into_inner(),
                name: p.name,
            })
            .collect(),
        tags: scene
            .tags
            .into_iter()
            .map(|t| Named {
                id: t.id.into_inner(),
                name: t.name,
            })
            .collect(),
        groups: scene
            .groups
            .into_iter()
            .map(|g| GroupRef {
                id: g.group.id.into_inner(),
                name: g.group.name,
                scene_index: g.scene_index,
            })
            .collect(),
        markers,
        urls: scene
            .urls
            .into_iter()
            .filter(|u| !u.trim().is_empty())
            .collect(),
        organized: scene.organized,
        resume_time: scene.resume_time.filter(|t| *t > 0.0),
        play_duration: scene.play_duration.filter(|t| *t > 0.0),
        file_count: scene.files.len(),
        sprite: file_name(scene.paths.sprite.as_deref()),
        vtt: file_name(scene.paths.vtt.as_deref()),
        rating100: scene.rating100.and_then(|r| u8::try_from(r).ok()),
        play_count: count(scene.play_count),
        o_count: count(scene.o_counter),
        duration_seconds: file.map(|f| f.duration),
        file_name: file.map(|f| f.basename.clone()),
        file: file.map(super::scenes::scene_file),
        cover_version: scene
            .paths
            .screenshot
            .as_deref()
            .map_or_else(|| "0".to_owned(), screenshot_version),
        id: scene.id.into_inner(),
    })
}

// ---- SceneFiles ----------------------------------------------------------------------------

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Fingerprint")]
struct FingerprintFields {
    #[cynic(rename = "type")]
    kind: String,
    value: String,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "VideoFile")]
struct FileFields {
    id: cynic::Id,
    basename: String,
    path: String,
    size: Int64,
    duration: f64,
    width: i32,
    height: i32,
    #[cynic(rename = "video_codec")]
    video_codec: String,
    #[cynic(rename = "audio_codec")]
    audio_codec: String,
    format: String,
    #[cynic(rename = "frame_rate")]
    frame_rate: f64,
    #[cynic(rename = "bit_rate")]
    bit_rate: i32,
    fingerprints: Vec<FingerprintFields>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Scene")]
struct SceneFilesFields {
    files: Vec<FileFields>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "SceneIdVariables")]
pub(crate) struct SceneFiles {
    #[arguments(id: $id)]
    find_scene: Option<SceneFilesFields>,
}

/// One `SceneFiles` request: every file's facts, the primary first (cached as
/// `scene:files:<id>`).
pub async fn scene_files(client: &StashClient, id: &str) -> Result<Vec<FileFacts>, AppError> {
    let data = client
        .graphql(&SceneFiles::build(SceneIdVariables {
            id: cynic::Id::new(id),
        }))
        .await?;
    let scene = data
        .find_scene
        .ok_or_else(|| AppError::SceneNotFound { id: id.to_owned() })?;
    Ok(scene
        .files
        .into_iter()
        .map(|f| FileFacts {
            id: f.id.into_inner(),
            basename: f.basename,
            path: f.path,
            size: u64::try_from(f.size.0).unwrap_or(0),
            duration: f.duration,
            width: u32::try_from(f.width).unwrap_or(0),
            height: u32::try_from(f.height).unwrap_or(0),
            video_codec: f.video_codec,
            audio_codec: f.audio_codec,
            format: f.format,
            frame_rate: f.frame_rate,
            bit_rate: u64::try_from(f.bit_rate).unwrap_or(0),
            fingerprints: f
                .fingerprints
                .into_iter()
                .map(|fp| (fp.kind, fp.value))
                .collect(),
        })
        .collect())
}

// ---- SceneGalleries ------------------------------------------------------------------------

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Gallery")]
struct GalleryFields {
    id: cynic::Id,
    title: Option<String>,
    #[cynic(rename = "image_count")]
    image_count: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Scene")]
struct SceneGalleriesFields {
    galleries: Vec<GalleryFields>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "SceneIdVariables")]
pub(crate) struct SceneGalleries {
    #[arguments(id: $id)]
    find_scene: Option<SceneGalleriesFields>,
}

/// One `SceneGalleries` request (cached as `scene:galleries:<id>`).
pub async fn scene_galleries(client: &StashClient, id: &str) -> Result<Vec<GalleryChip>, AppError> {
    let data = client
        .graphql(&SceneGalleries::build(SceneIdVariables {
            id: cynic::Id::new(id),
        }))
        .await?;
    let scene = data
        .find_scene
        .ok_or_else(|| AppError::SceneNotFound { id: id.to_owned() })?;
    Ok(scene
        .galleries
        .into_iter()
        .map(|g| GalleryChip {
            title: g
                .title
                .filter(|t| !t.trim().is_empty())
                .unwrap_or_else(|| format!("Gallery {}", g.id.inner())),
            id: g.id.into_inner(),
            image_count: u32::try_from(g.image_count).unwrap_or(0),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::file_name;

    #[test]
    fn generated_file_names_drop_the_address_and_key() {
        assert_eq!(
            file_name(Some("http://h:9998/scene/abc_sprite.jpg?apikey=secret")),
            Some("abc_sprite.jpg".into())
        );
        assert_eq!(file_name(None), None);
        assert_eq!(file_name(Some("http://h/")), None);
    }
}
