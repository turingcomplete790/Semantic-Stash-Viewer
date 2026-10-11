//! Filter picker choice lists, cached per server (spec FR-003a, research R4;
//! contracts/query-budget.md `ChoiceList<Kind>`).
//!
//! Names and aliases only, plus each performer's picture version (FR-011c); no images or counts. Tags are the ordinary ones (every semantic flag
//! and `variantOfTagId` null, filtered by Stash); semantic tags come from the vocabulary.

use cynic::QueryBuilder;
use serde::{Deserialize, Serialize};

use super::gql::{schema, CriterionModifier, CustomFieldCriterionInput, FindFilterType};
use super::StashClient;
use crate::error::AppError;
use crate::scenes::filter::search::Folded;
use crate::semantic::{SemanticKind, VARIANT_OF_FIELD};

/// Which list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChoiceKind {
    Performers,
    Tags,
    Studios,
    Groups,
    Galleries,
}

impl ChoiceKind {
    pub const ALL: [ChoiceKind; 5] = [
        ChoiceKind::Performers,
        ChoiceKind::Tags,
        ChoiceKind::Studios,
        ChoiceKind::Groups,
        ChoiceKind::Galleries,
    ];

    /// The cache key's kind (`choices:<key>`).
    pub fn key(self) -> &'static str {
        match self {
            ChoiceKind::Performers => "performers",
            ChoiceKind::Tags => "tags",
            ChoiceKind::Studios => "studios",
            ChoiceKind::Groups => "groups",
            ChoiceKind::Galleries => "galleries",
        }
    }
}

/// One choice: id, name, aliases, and for performers their picture's version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    /// Performers only: the picture version from `image_path`'s `t`; `None` when the path says
    /// `default=true` or for other kinds (008 FR-011c). Required when reading, so lists cached
    /// before versions existed don't parse and are fetched again.
    #[serde(deserialize_with = "Option::deserialize")]
    pub image: Option<String>,
}

/// A whole list, sorted by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChoiceList {
    pub kind: ChoiceKind,
    pub entries: Vec<Choice>,
}

impl ChoiceList {
    /// The search index for [`crate::scenes::filter::search::search`].
    pub fn folded(&self) -> Vec<Folded> {
        self.entries
            .iter()
            .map(|c| Folded::new(&c.name, &c.aliases))
            .collect()
    }
}

// ---- Operations -----------------------------------------------------------------------------

#[derive(cynic::QueryVariables, Debug)]
pub(crate) struct ChoiceVariables {
    pub(crate) filter: FindFilterType,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Performer")]
struct PerformerChoice {
    id: cynic::Id,
    name: String,
    #[cynic(rename = "alias_list")]
    alias_list: Vec<String>,
    #[cynic(rename = "image_path")]
    image_path: Option<String>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindPerformersResultType")]
struct PerformerChoices {
    performers: Vec<PerformerChoice>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "ChoiceVariables")]
pub(crate) struct ChoiceListPerformers {
    #[arguments(filter: $filter)]
    find_performers: PerformerChoices,
}

#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(graphql_type = "TagFilterType", rename_all = "None")]
pub(crate) struct TagFilterType {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub(crate) custom_fields: Option<Vec<CustomFieldCriterionInput>>,
}

#[derive(cynic::QueryVariables, Debug)]
pub(crate) struct TagChoiceVariables {
    pub(crate) filter: FindFilterType,
    pub(crate) tag_filter: TagFilterType,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Tag")]
struct TagChoice {
    id: cynic::Id,
    name: String,
    aliases: Vec<String>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindTagsResultType")]
struct TagChoices {
    tags: Vec<TagChoice>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "TagChoiceVariables")]
pub(crate) struct ChoiceListTags {
    #[arguments(filter: $filter, tag_filter: $tag_filter)]
    find_tags: TagChoices,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Studio")]
struct StudioChoice {
    id: cynic::Id,
    name: String,
    aliases: Vec<String>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindStudiosResultType")]
struct StudioChoices {
    studios: Vec<StudioChoice>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "ChoiceVariables")]
pub(crate) struct ChoiceListStudios {
    #[arguments(filter: $filter)]
    find_studios: StudioChoices,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Group")]
struct GroupChoice {
    id: cynic::Id,
    name: String,
    aliases: Option<String>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindGroupsResultType")]
struct GroupChoices {
    groups: Vec<GroupChoice>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "ChoiceVariables")]
pub(crate) struct ChoiceListGroups {
    #[arguments(filter: $filter)]
    find_groups: GroupChoices,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Gallery")]
struct GalleryChoice {
    id: cynic::Id,
    title: Option<String>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindGalleriesResultType")]
struct GalleryChoices {
    galleries: Vec<GalleryChoice>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "ChoiceVariables")]
pub(crate) struct ChoiceListGalleries {
    #[arguments(filter: $filter)]
    find_galleries: GalleryChoices,
}

/// Every entry, sorted by name (`per_page: -1`).
pub(crate) fn all_by_name() -> FindFilterType {
    FindFilterType {
        per_page: Some(-1),
        sort: Some("name".into()),
        ..FindFilterType::default()
    }
}

/// Ordinary tags only: every semantic flag and `variantOfTagId` null.
pub(crate) fn ordinary_tags_filter() -> TagFilterType {
    let null = |field: &str| CustomFieldCriterionInput {
        field: field.to_owned(),
        value: None,
        modifier: CriterionModifier::IsNull,
    };
    let mut fields: Vec<CustomFieldCriterionInput> =
        SemanticKind::ALL.iter().map(|k| null(k.flag())).collect();
    fields.push(null(VARIANT_OF_FIELD));
    TagFilterType {
        custom_fields: Some(fields),
    }
}

fn choice(id: cynic::Id, name: String, aliases: Vec<String>) -> Choice {
    Choice {
        id: id.into_inner(),
        name,
        aliases,
        image: None,
    }
}

/// One `ChoiceList<Kind>` request.
pub async fn choice_list(client: &StashClient, kind: ChoiceKind) -> Result<ChoiceList, AppError> {
    let filter = all_by_name();
    let entries = match kind {
        ChoiceKind::Performers => client
            .graphql(&ChoiceListPerformers::build(ChoiceVariables { filter }))
            .await?
            .find_performers
            .performers
            .into_iter()
            .map(|p| Choice {
                image: crate::scenes::picture_version(p.image_path.as_deref()),
                ..choice(p.id, p.name, p.alias_list)
            })
            .collect(),
        ChoiceKind::Tags => client
            .graphql(&ChoiceListTags::build(TagChoiceVariables {
                filter,
                tag_filter: ordinary_tags_filter(),
            }))
            .await?
            .find_tags
            .tags
            .into_iter()
            .map(|t| choice(t.id, t.name, t.aliases))
            .collect(),
        ChoiceKind::Studios => client
            .graphql(&ChoiceListStudios::build(ChoiceVariables { filter }))
            .await?
            .find_studios
            .studios
            .into_iter()
            .map(|s| choice(s.id, s.name, s.aliases))
            .collect(),
        ChoiceKind::Groups => client
            .graphql(&ChoiceListGroups::build(ChoiceVariables { filter }))
            .await?
            .find_groups
            .groups
            .into_iter()
            .map(|g| {
                // A group's aliases are one string; the web UI separates them with commas.
                let aliases = g
                    .aliases
                    .unwrap_or_default()
                    .split(',')
                    .map(str::trim)
                    .filter(|a| !a.is_empty())
                    .map(str::to_owned)
                    .collect();
                choice(g.id, g.name, aliases)
            })
            .collect(),
        ChoiceKind::Galleries => client
            .graphql(&ChoiceListGalleries::build(ChoiceVariables {
                filter: FindFilterType {
                    sort: Some("title".into()),
                    ..filter
                },
            }))
            .await?
            .find_galleries
            .galleries
            .into_iter()
            .map(|g| {
                let name = g
                    .title
                    .filter(|t| !t.trim().is_empty())
                    .unwrap_or_else(|| format!("Gallery {}", g.id.inner()));
                choice(g.id, name, Vec::new())
            })
            .collect(),
    };
    Ok(ChoiceList { kind, entries })
}
