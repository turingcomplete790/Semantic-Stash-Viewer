//! Chip details: one small card per performer, tag, or studio, fetched when the pointer or focus
//! rests on its chip, once per item per session (spec FR-011b; contracts/query-budget.md
//! `PerformerCard`, `TagCard`, `StudioCard`).

use cynic::QueryBuilder;
use serde::{Deserialize, Serialize};

use super::gql::schema;
use super::StashClient;
use crate::error::AppError;

/// Which kind of chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChipKind {
    Performer,
    Tag,
    Studio,
}

impl ChipKind {
    /// The cache key's kind (`chip:<key>:<id>`).
    pub fn key(self) -> &'static str {
        match self {
            ChipKind::Performer => "performer",
            ChipKind::Tag => "tag",
            ChipKind::Studio => "studio",
        }
    }
}

/// What a chip's card shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChipDetails {
    Performer {
        name: String,
        disambiguation: Option<String>,
        gender: Option<String>,
        birthdate: Option<String>,
        country: Option<String>,
        favorite: bool,
        scene_count: u32,
    },
    Tag {
        name: String,
        description: Option<String>,
        scene_count: u32,
    },
    Studio {
        name: String,
        parent: Option<String>,
        scene_count: u32,
    },
}

#[derive(cynic::QueryVariables, Debug)]
pub(crate) struct ChipVariables {
    pub(crate) id: cynic::Id,
}

#[derive(cynic::Enum, Clone, Copy, Debug)]
#[cynic(graphql_type = "GenderEnum")]
enum GenderEnum {
    Male,
    Female,
    TransgenderMale,
    TransgenderFemale,
    Intersex,
    NonBinary,
}

impl GenderEnum {
    fn label(self) -> &'static str {
        match self {
            GenderEnum::Male => "Male",
            GenderEnum::Female => "Female",
            GenderEnum::TransgenderMale => "Transgender male",
            GenderEnum::TransgenderFemale => "Transgender female",
            GenderEnum::Intersex => "Intersex",
            GenderEnum::NonBinary => "Non-binary",
        }
    }
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Performer")]
struct PerformerCardFields {
    id: cynic::Id,
    name: String,
    disambiguation: Option<String>,
    gender: Option<GenderEnum>,
    birthdate: Option<String>,
    country: Option<String>,
    favorite: bool,
    #[cynic(rename = "scene_count")]
    scene_count: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "ChipVariables")]
pub(crate) struct PerformerCard {
    #[arguments(id: $id)]
    find_performer: Option<PerformerCardFields>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Tag")]
struct TagCardFields {
    id: cynic::Id,
    name: String,
    description: Option<String>,
    #[cynic(rename = "scene_count")]
    scene_count: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "ChipVariables")]
pub(crate) struct TagCard {
    #[arguments(id: $id)]
    find_tag: Option<TagCardFields>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Studio")]
struct ParentStudio {
    name: String,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Studio")]
struct StudioCardFields {
    id: cynic::Id,
    name: String,
    #[cynic(rename = "parent_studio")]
    parent_studio: Option<ParentStudio>,
    #[cynic(rename = "scene_count")]
    scene_count: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "ChipVariables")]
pub(crate) struct StudioCard {
    #[arguments(id: $id)]
    find_studio: Option<StudioCardFields>,
}

fn count(n: i32) -> u32 {
    u32::try_from(n).unwrap_or(0)
}

fn text(s: Option<String>) -> Option<String> {
    s.filter(|s| !s.trim().is_empty())
}

fn missing(kind: ChipKind, id: &str) -> AppError {
    AppError::Internal {
        message: format!("{} {id} no longer exists", kind.key()),
    }
}

/// One card request for one chip (cached as `chip:<kind>:<id>`).
pub async fn chip_details(
    client: &StashClient,
    kind: ChipKind,
    id: &str,
) -> Result<ChipDetails, AppError> {
    let vars = || ChipVariables {
        id: cynic::Id::new(id),
    };
    match kind {
        ChipKind::Performer => {
            let p = client
                .graphql(&PerformerCard::build(vars()))
                .await?
                .find_performer
                .ok_or_else(|| missing(kind, id))?;
            debug_assert_eq!(p.id.inner(), id, "Stash answered for the chip asked");
            Ok(ChipDetails::Performer {
                name: p.name,
                disambiguation: text(p.disambiguation),
                gender: p.gender.map(|g| g.label().to_owned()),
                birthdate: text(p.birthdate),
                country: text(p.country),
                favorite: p.favorite,
                scene_count: count(p.scene_count),
            })
        }
        ChipKind::Tag => {
            let t = client
                .graphql(&TagCard::build(vars()))
                .await?
                .find_tag
                .ok_or_else(|| missing(kind, id))?;
            debug_assert_eq!(t.id.inner(), id, "Stash answered for the chip asked");
            Ok(ChipDetails::Tag {
                name: t.name,
                description: text(t.description),
                scene_count: count(t.scene_count),
            })
        }
        ChipKind::Studio => {
            let s = client
                .graphql(&StudioCard::build(vars()))
                .await?
                .find_studio
                .ok_or_else(|| missing(kind, id))?;
            debug_assert_eq!(s.id.inner(), id, "Stash answered for the chip asked");
            Ok(ChipDetails::Studio {
                name: s.name,
                parent: s.parent_studio.map(|p| p.name),
                scene_count: count(s.scene_count),
            })
        }
    }
}
