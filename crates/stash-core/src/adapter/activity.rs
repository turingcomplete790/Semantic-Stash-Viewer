//! Play history writes: `SaveActivity` and `AddPlay` (spec FR-014 to FR-017; contracts/play-history.md).
//! They're the feature's only writes besides the colour rules (contracts/query-budget.md).

use cynic::MutationBuilder;

use super::gql::schema;
use super::StashClient;
use crate::error::AppError;
use crate::playback::history::PlayHistoryReport;
use crate::scenes::SceneDetails;

#[derive(cynic::QueryVariables, Debug)]
pub(crate) struct SaveActivityVariables {
    pub(crate) id: cynic::Id,
    pub(crate) resume_time: Option<f64>,
    pub(crate) play_duration: Option<f64>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "SaveActivityVariables")]
pub(crate) struct SaveActivity {
    #[arguments(id: $id, resume_time: $resume_time, playDuration: $play_duration)]
    #[cynic(rename = "sceneSaveActivity")]
    scene_save_activity: bool,
}

#[derive(cynic::QueryVariables, Debug)]
pub(crate) struct AddPlayVariables {
    pub(crate) id: cynic::Id,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "HistoryMutationResult")]
struct PlayCount {
    count: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "AddPlayVariables")]
pub(crate) struct AddPlay {
    #[arguments(id: $id)]
    #[cynic(rename = "sceneAddPlay")]
    scene_add_play: PlayCount,
}

/// Send one report: `SaveActivity`, then `AddPlay` when the report counts a play. Returns the new
/// play count when a play was added.
pub async fn send_report(
    client: &StashClient,
    report: &PlayHistoryReport,
) -> Result<Option<u32>, AppError> {
    let id = || cynic::Id::new(&report.scene_id);
    let saved = client
        .graphql(&SaveActivity::build(SaveActivityVariables {
            id: id(),
            resume_time: Some(report.resume_time),
            play_duration: Some(report.play_duration),
        }))
        .await?
        .scene_save_activity;
    if !saved {
        return Err(AppError::Internal {
            message: "Stash didn't save the play history".into(),
        });
    }
    if !report.add_play {
        return Ok(None);
    }
    let count = client
        .graphql(&AddPlay::build(AddPlayVariables { id: id() }))
        .await?
        .scene_add_play
        .count;
    Ok(Some(u32::try_from(count).unwrap_or(0)))
}

/// A cached scene page after a confirmed report (contracts/play-history.md "Cache"): the new
/// resume point, the added playing time, and the play count Stash returned.
pub fn patch_details(
    details: &mut SceneDetails,
    report: &PlayHistoryReport,
    play_count: Option<u32>,
) {
    details.resume_time = Some(report.resume_time).filter(|t| *t > 0.0);
    let total = details.play_duration.unwrap_or(0.0) + report.play_duration;
    details.play_duration = Some(total).filter(|t| *t > 0.0);
    if let Some(count) = play_count {
        details.play_count = count;
    }
}
