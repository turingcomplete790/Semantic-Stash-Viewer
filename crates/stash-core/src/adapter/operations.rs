//! Every named operation in the query budget, as the exact text sent to Stash (008 research R2).
//!
//! `tests/graphql_operations.rs` compares each with its golden file in `graphql/operations/` and
//! checks the tier and writes rules (contracts/query-budget.md). Variables don't change the text,
//! so placeholder values are used to build each one.

use cynic::{MutationBuilder, QueryBuilder};

use super::activity::{AddPlay, AddPlayVariables, SaveActivity, SaveActivityVariables};
use super::chips::{ChipVariables, PerformerCard, StudioCard, TagCard};
use super::choices::{
    all_by_name, ordinary_tags_filter, ChoiceListGalleries, ChoiceListGroups, ChoiceListPerformers,
    ChoiceListStudios, ChoiceListTags, ChoiceVariables, TagChoiceVariables,
};
use super::filters::{
    SaveTagColours, SaveTagColoursVariables, SavedFiltersVariables, SavedSceneFilters, UiConfig,
};
use super::gql::FindFilterType;
use super::images::{ImageDetailsQuery, ImageIdVariables, ImagesPage, ImagesPageVariables};
use super::scene_page::{SceneDetails, SceneFiles, SceneGalleries, SceneIdVariables};
use super::scenes::{ScenesPage, ScenesPageVariables};

/// Which budget tier an operation belongs to; the tier decides which fields it may select.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// Lists: one entry per card or choice (`ScenesPage`, `ChoiceList*`).
    Card,
    /// One small card per chip (`PerformerCard`, `TagCard`, `StudioCard`).
    Chip,
    /// The scene page's default request (`SceneDetails`).
    Detail,
    /// Everything else: lazy sections, settings, vocabulary, writes.
    Other,
}

/// One named operation's document.
#[derive(Debug, Clone)]
pub struct OperationDocument {
    pub name: &'static str,
    pub tier: Tier,
    pub query: String,
}

/// Every operation under the query budget, in the contract's order.
pub fn documents() -> Vec<OperationDocument> {
    vec![
        OperationDocument {
            name: "ScenesPage",
            tier: Tier::Card,
            query: ScenesPage::build(ScenesPageVariables {
                filter: FindFilterType::default(),
                scene_filter: None,
            })
            .query,
        },
        OperationDocument {
            name: "ImagesPage",
            tier: Tier::Card,
            query: ImagesPage::build(ImagesPageVariables {
                filter: FindFilterType::default(),
                image_filter: None,
            })
            .query,
        },
        OperationDocument {
            name: "ImageDetails",
            tier: Tier::Detail,
            query: ImageDetailsQuery::build(ImageIdVariables {
                id: cynic::Id::new("0"),
            })
            .query,
        },
        OperationDocument {
            name: "SceneDetails",
            tier: Tier::Detail,
            query: SceneDetails::build(scene_id()).query,
        },
        OperationDocument {
            name: "SceneFiles",
            tier: Tier::Other,
            query: SceneFiles::build(scene_id()).query,
        },
        OperationDocument {
            name: "SceneGalleries",
            tier: Tier::Other,
            query: SceneGalleries::build(scene_id()).query,
        },
        OperationDocument {
            name: "PerformerCard",
            tier: Tier::Chip,
            query: PerformerCard::build(chip_id()).query,
        },
        OperationDocument {
            name: "TagCard",
            tier: Tier::Chip,
            query: TagCard::build(chip_id()).query,
        },
        OperationDocument {
            name: "StudioCard",
            tier: Tier::Chip,
            query: StudioCard::build(chip_id()).query,
        },
        OperationDocument {
            name: "ChoiceListPerformers",
            tier: Tier::Card,
            query: ChoiceListPerformers::build(choice()).query,
        },
        OperationDocument {
            name: "ChoiceListTags",
            tier: Tier::Card,
            query: ChoiceListTags::build(TagChoiceVariables {
                filter: all_by_name(),
                tag_filter: ordinary_tags_filter(),
            })
            .query,
        },
        OperationDocument {
            name: "ChoiceListStudios",
            tier: Tier::Card,
            query: ChoiceListStudios::build(choice()).query,
        },
        OperationDocument {
            name: "ChoiceListGroups",
            tier: Tier::Card,
            query: ChoiceListGroups::build(choice()).query,
        },
        OperationDocument {
            name: "ChoiceListGalleries",
            tier: Tier::Card,
            query: ChoiceListGalleries::build(choice()).query,
        },
        OperationDocument {
            name: "SemanticVocabulary",
            tier: Tier::Other,
            query: crate::semantic::vocabulary_query(&crate::semantic::default_colour_rules()),
        },
        OperationDocument {
            name: "SavedSceneFilters",
            tier: Tier::Other,
            query: SavedSceneFilters::build(SavedFiltersVariables { mode: None }).query,
        },
        OperationDocument {
            name: "UiConfig",
            tier: Tier::Other,
            query: UiConfig::build(()).query,
        },
        OperationDocument {
            name: "SaveActivity",
            tier: Tier::Other,
            query: SaveActivity::build(SaveActivityVariables {
                id: cynic::Id::new("0"),
                resume_time: None,
                play_duration: None,
            })
            .query,
        },
        OperationDocument {
            name: "AddPlay",
            tier: Tier::Other,
            query: AddPlay::build(AddPlayVariables {
                id: cynic::Id::new("0"),
            })
            .query,
        },
        OperationDocument {
            name: "SaveTagColours",
            tier: Tier::Other,
            query: SaveTagColours::build(SaveTagColoursVariables {
                key: String::new(),
                value: None,
            })
            .query,
        },
    ]
}

fn choice() -> ChoiceVariables {
    ChoiceVariables {
        filter: all_by_name(),
    }
}

fn scene_id() -> SceneIdVariables {
    SceneIdVariables {
        id: cynic::Id::new("0"),
    }
}

fn chip_id() -> ChipVariables {
    ChipVariables {
        id: cynic::Id::new("0"),
    }
}
