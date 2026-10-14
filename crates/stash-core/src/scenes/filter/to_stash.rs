//! Criteria to Stash's `SceneFilterType` (research R3) and `ImageFilterType` (012): the same input
//! the web UI sends for the same criteria (`toCriterionInput`), typed and checked against the
//! pinned schema at build time. Semantic placeholders, invalid criteria, and kinds the entity
//! doesn't offer are never sent.

use super::{Criterion, CriterionKind, CriterionValue, Modifier, Orientation, Resolution};
use crate::adapter::gql::{
    CriterionModifier, CustomFieldCriterionInput, DateCriterionInput, DuplicationCriterionInput,
    FileFilterType, HierarchicalMultiCriterionInput, ImageFilterType, IntCriterionInput, JsonAny,
    MultiCriterionInput, OrientationCriterionInput, OrientationEnum, PhashDistanceCriterionInput,
    ResolutionCriterionInput, ResolutionEnum, SceneFilterType, StashIdCriterionInput,
    StringCriterionInput, TimestampCriterionInput,
};

/// The `SceneFilterType` for `criteria`, or `None` when none can be sent.
pub fn scene_filter(criteria: &[Criterion]) -> Option<SceneFilterType> {
    let mut filter = SceneFilterType::default();
    let mut any = false;
    for c in criteria {
        if c.info().is_placeholder() || c.validate().is_err() {
            continue;
        }
        any |= apply(&mut filter, c);
    }
    any.then_some(filter)
}

/// The `ImageFilterType` for `criteria`, or `None` when none can be sent.
pub fn image_filter(criteria: &[Criterion]) -> Option<ImageFilterType> {
    use crate::browse::entity::Entity;
    let mut filter = ImageFilterType::default();
    let mut any = false;
    for c in criteria {
        if c.info().is_placeholder()
            || !c.kind.applies_to(Entity::Images)
            || c.validate_for(Entity::Images).is_err()
        {
            continue;
        }
        any |= apply_image(&mut filter, c);
    }
    any.then_some(filter)
}

/// The image filter as JSON, as it goes in the request's variables (for tests and logs).
pub fn image_filter_json(criteria: &[Criterion]) -> serde_json::Value {
    image_filter(criteria).map_or(serde_json::Value::Null, |f| {
        serde_json::to_value(f).unwrap_or(serde_json::Value::Null)
    })
}

/// The filter as JSON, as it goes in the request's variables (for tests and logs).
pub fn scene_filter_json(criteria: &[Criterion]) -> serde_json::Value {
    scene_filter(criteria).map_or(serde_json::Value::Null, |f| {
        serde_json::to_value(f).unwrap_or(serde_json::Value::Null)
    })
}

fn modifier(m: Modifier) -> CriterionModifier {
    match m {
        Modifier::Equals => CriterionModifier::Equals,
        Modifier::NotEquals => CriterionModifier::NotEquals,
        Modifier::GreaterThan => CriterionModifier::GreaterThan,
        Modifier::LessThan => CriterionModifier::LessThan,
        Modifier::Between => CriterionModifier::Between,
        Modifier::NotBetween => CriterionModifier::NotBetween,
        Modifier::Includes => CriterionModifier::Includes,
        Modifier::IncludesAll => CriterionModifier::IncludesAll,
        Modifier::Excludes => CriterionModifier::Excludes,
        Modifier::IsNull => CriterionModifier::IsNull,
        Modifier::NotNull => CriterionModifier::NotNull,
        Modifier::MatchesRegex => CriterionModifier::MatchesRegex,
        Modifier::NotMatchesRegex => CriterionModifier::NotMatchesRegex,
    }
}

pub(crate) fn resolution(r: Resolution) -> ResolutionEnum {
    match r {
        Resolution::VeryLow => ResolutionEnum::VeryLow,
        Resolution::Low => ResolutionEnum::Low,
        Resolution::R360p => ResolutionEnum::R360p,
        Resolution::Standard => ResolutionEnum::Standard,
        Resolution::WebHd => ResolutionEnum::WebHd,
        Resolution::StandardHd => ResolutionEnum::StandardHd,
        Resolution::FullHd => ResolutionEnum::FullHd,
        Resolution::QuadHd => ResolutionEnum::QuadHd,
        Resolution::FourK => ResolutionEnum::FourK,
        Resolution::FiveK => ResolutionEnum::FiveK,
        Resolution::SixK => ResolutionEnum::SixK,
        Resolution::SevenK => ResolutionEnum::SevenK,
        Resolution::EightK => ResolutionEnum::EightK,
        Resolution::Huge => ResolutionEnum::Huge,
    }
}

fn text(c: &Criterion) -> StringCriterionInput {
    StringCriterionInput {
        value: match &c.value {
            CriterionValue::Text(t) => t.trim().to_owned(),
            _ => String::new(),
        },
        modifier: modifier(c.modifier),
    }
}

fn clamp(n: i64) -> i32 {
    i32::try_from(n).unwrap_or(if n < 0 { i32::MIN } else { i32::MAX })
}

fn int(c: &Criterion) -> IntCriterionInput {
    let (value, value2) = match &c.value {
        CriterionValue::Int(a, b) => (clamp(*a), b.map(clamp)),
        _ => (0, None),
    };
    IntCriterionInput {
        value,
        value2: if c.modifier.is_range() { value2 } else { None },
        modifier: modifier(c.modifier),
    }
}

fn date_pair(c: &Criterion, timestamp: bool) -> (String, Option<String>) {
    let shape = |v: &str| {
        let v = v.trim();
        if timestamp {
            v.replacen(' ', "T", 1)
        } else {
            v.to_owned()
        }
    };
    match &c.value {
        CriterionValue::Date(a, b) => (
            shape(a),
            if c.modifier.is_range() {
                b.as_deref().map(shape)
            } else {
                None
            },
        ),
        _ => (String::new(), None),
    }
}

fn date(c: &Criterion) -> DateCriterionInput {
    let (value, value2) = date_pair(c, false);
    DateCriterionInput {
        value,
        value2,
        modifier: modifier(c.modifier),
    }
}

fn timestamp(c: &Criterion) -> TimestampCriterionInput {
    let (value, value2) = date_pair(c, true);
    TimestampCriterionInput {
        value,
        value2,
        modifier: modifier(c.modifier),
    }
}

fn ids(items: &[super::Item]) -> Vec<cynic::Id> {
    items.iter().map(|i| cynic::Id::new(&i.id)).collect()
}

fn multi(c: &Criterion) -> MultiCriterionInput {
    match &c.value {
        CriterionValue::Items {
            items, excluded, ..
        } if !c.modifier.is_null_check() => MultiCriterionInput {
            value: Some(ids(items)),
            modifier: modifier(c.modifier),
            excludes: (!excluded.is_empty()).then(|| ids(excluded)),
        },
        _ => MultiCriterionInput {
            value: None,
            modifier: modifier(c.modifier),
            excludes: None,
        },
    }
}

fn hierarchical(c: &Criterion) -> HierarchicalMultiCriterionInput {
    match &c.value {
        CriterionValue::Items {
            items,
            excluded,
            depth,
        } if !c.modifier.is_null_check() => HierarchicalMultiCriterionInput {
            value: Some(ids(items)),
            modifier: modifier(c.modifier),
            // "Equals" means exactly these: no sub-levels (as the web UI).
            depth: Some(if c.modifier == Modifier::Equals {
                0
            } else {
                depth.unwrap_or(0)
            }),
            excludes: Some(ids(excluded)),
        },
        _ => HierarchicalMultiCriterionInput {
            value: None,
            modifier: modifier(c.modifier),
            depth: None,
            excludes: None,
        },
    }
}

fn bool_value(c: &Criterion) -> bool {
    matches!(c.value, CriterionValue::Bool(true))
}

/// Set `c`'s field on `f`; whether anything was set.
fn apply(f: &mut SceneFilterType, c: &Criterion) -> bool {
    use CriterionKind as K;
    match c.kind {
        K::Title => f.title = Some(text(c)),
        K::Code => f.code = Some(text(c)),
        K::Details => f.details = Some(text(c)),
        K::Director => f.director = Some(text(c)),
        K::Url => f.url = Some(text(c)),
        K::Path => f.path = Some(text(c)),
        K::Oshash => f.oshash = Some(text(c)),
        K::Checksum => f.checksum = Some(text(c)),
        K::VideoCodec => f.video_codec = Some(text(c)),
        K::AudioCodec => f.audio_codec = Some(text(c)),
        K::Captions => f.captions = Some(text(c)),
        K::FileCount => f.file_count = Some(int(c)),
        K::Rating => f.rating100 = Some(int(c)),
        K::OCounter => f.o_counter = Some(int(c)),
        K::Framerate => f.framerate = Some(int(c)),
        K::Bitrate => f.bitrate = Some(int(c)),
        K::Duration => f.duration = Some(int(c)),
        K::ResumeTime => f.resume_time = Some(int(c)),
        K::PlayDuration => f.play_duration = Some(int(c)),
        K::PlayCount => f.play_count = Some(int(c)),
        K::TagCount => f.tag_count = Some(int(c)),
        K::PerformerCount => f.performer_count = Some(int(c)),
        K::PerformerAge => f.performer_age = Some(int(c)),
        K::StashIdCount => f.stash_id_count = Some(int(c)),
        K::InteractiveSpeed => f.interactive_speed = Some(int(c)),
        K::Date => f.date = Some(date(c)),
        K::LastPlayedAt => f.last_played_at = Some(timestamp(c)),
        K::CreatedAt => f.created_at = Some(timestamp(c)),
        K::UpdatedAt => f.updated_at = Some(timestamp(c)),
        K::Organized => f.organized = Some(bool_value(c)),
        K::Interactive => f.interactive = Some(bool_value(c)),
        K::PerformerFavorite => f.performer_favorite = Some(bool_value(c)),
        K::HasMarkers => {
            f.has_markers = Some(if bool_value(c) { "true" } else { "false" }.to_owned());
        }
        K::IsMissing => {
            if let CriterionValue::Text(t) = &c.value {
                f.is_missing = Some(t.clone());
            }
        }
        K::Resolution => {
            if let CriterionValue::Resolution(r) = c.value {
                f.resolution = Some(ResolutionCriterionInput {
                    value: resolution(r),
                    modifier: modifier(c.modifier),
                });
            }
        }
        K::Orientation => {
            if let CriterionValue::Orientation(o) = &c.value {
                f.orientation = Some(OrientationCriterionInput {
                    value: o
                        .iter()
                        .map(|o| match o {
                            Orientation::Landscape => OrientationEnum::Landscape,
                            Orientation::Portrait => OrientationEnum::Portrait,
                            Orientation::Square => OrientationEnum::Square,
                        })
                        .collect(),
                });
            }
        }
        K::Performers => f.performers = Some(multi(c)),
        K::Galleries => f.galleries = Some(multi(c)),
        K::Tags => f.tags = Some(hierarchical(c)),
        K::PerformerTags => f.performer_tags = Some(hierarchical(c)),
        K::Studios => f.studios = Some(hierarchical(c)),
        K::Groups => f.groups = Some(hierarchical(c)),
        K::StashId => {
            let (endpoint, id) = match &c.value {
                CriterionValue::StashId { endpoint, id } => (endpoint.clone(), id.clone()),
                _ => (String::new(), String::new()),
            };
            f.stash_id_endpoint = Some(StashIdCriterionInput {
                endpoint: (!endpoint.trim().is_empty()).then(|| endpoint.trim().to_owned()),
                stash_id: (!id.trim().is_empty()).then(|| id.trim().to_owned()),
                modifier: modifier(c.modifier),
            });
        }
        K::PhashDistance => {
            let (hash, distance) = match &c.value {
                CriterionValue::Phash { hash, distance } => (hash.trim().to_owned(), *distance),
                _ => (String::new(), None),
            };
            f.phash_distance = Some(PhashDistanceCriterionInput {
                value: hash,
                modifier: modifier(c.modifier),
                distance,
            });
        }
        K::Duplicated => {
            if let CriterionValue::Duplicated(d) = c.value {
                let flag = |b: bool| b.then_some(true);
                f.duplicated = Some(DuplicationCriterionInput {
                    phash: flag(d.phash),
                    url: flag(d.url),
                    stash_id: flag(d.stash_id),
                    title: flag(d.title),
                });
            }
        }
        K::CustomField => {
            if let CriterionValue::CustomField { field, values } = &c.value {
                f.custom_fields
                    .get_or_insert_with(Vec::new)
                    .push(CustomFieldCriterionInput {
                        field: field.clone(),
                        value: (!c.modifier.is_null_check())
                            .then(|| values.iter().cloned().map(JsonAny).collect()),
                        modifier: modifier(c.modifier),
                    });
            }
        }
        // Images only (012).
        K::Photographer | K::Folder | K::Semantic(_) => return false,
    }
    true
}

fn resolution_input(c: &Criterion) -> Option<ResolutionCriterionInput> {
    match c.value {
        CriterionValue::Resolution(r) => Some(ResolutionCriterionInput {
            value: resolution(r),
            modifier: modifier(c.modifier),
        }),
        _ => None,
    }
}

fn orientation_input(c: &Criterion) -> Option<OrientationCriterionInput> {
    match &c.value {
        CriterionValue::Orientation(o) => Some(OrientationCriterionInput {
            value: o
                .iter()
                .map(|o| match o {
                    Orientation::Landscape => OrientationEnum::Landscape,
                    Orientation::Portrait => OrientationEnum::Portrait,
                    Orientation::Square => OrientationEnum::Square,
                })
                .collect(),
        }),
        _ => None,
    }
}

fn phash_input(c: &Criterion) -> PhashDistanceCriterionInput {
    let (hash, distance) = match &c.value {
        CriterionValue::Phash { hash, distance } => (hash.trim().to_owned(), *distance),
        _ => (String::new(), None),
    };
    PhashDistanceCriterionInput {
        value: hash,
        modifier: modifier(c.modifier),
        distance,
    }
}

fn custom_field_input(c: &Criterion) -> Option<CustomFieldCriterionInput> {
    match &c.value {
        CriterionValue::CustomField { field, values } => Some(CustomFieldCriterionInput {
            field: field.clone(),
            value: (!c.modifier.is_null_check())
                .then(|| values.iter().cloned().map(JsonAny).collect()),
            modifier: modifier(c.modifier),
        }),
        _ => None,
    }
}

/// Set `c`'s field on an image filter; whether anything was set. Only kinds Images offer reach
/// here ([`image_filter`] checks).
fn apply_image(f: &mut ImageFilterType, c: &Criterion) -> bool {
    use CriterionKind as K;
    match c.kind {
        K::Title => f.title = Some(text(c)),
        K::Code => f.code = Some(text(c)),
        K::Details => f.details = Some(text(c)),
        K::Photographer => f.photographer = Some(text(c)),
        K::Url => f.url = Some(text(c)),
        K::Path => f.path = Some(text(c)),
        K::Checksum => f.checksum = Some(text(c)),
        K::FileCount => f.file_count = Some(int(c)),
        K::Rating => f.rating100 = Some(int(c)),
        K::OCounter => f.o_counter = Some(int(c)),
        K::TagCount => f.tag_count = Some(int(c)),
        K::PerformerCount => f.performer_count = Some(int(c)),
        K::PerformerAge => f.performer_age = Some(int(c)),
        K::Date => f.date = Some(date(c)),
        K::CreatedAt => f.created_at = Some(timestamp(c)),
        K::UpdatedAt => f.updated_at = Some(timestamp(c)),
        K::Organized => f.organized = Some(bool_value(c)),
        K::PerformerFavorite => f.performer_favorite = Some(bool_value(c)),
        K::IsMissing => match &c.value {
            CriterionValue::Text(t) => f.is_missing = Some(t.clone()),
            _ => return false,
        },
        K::Resolution => f.resolution = resolution_input(c),
        K::Orientation => f.orientation = orientation_input(c),
        K::Performers => f.performers = Some(multi(c)),
        K::Galleries => f.galleries = Some(multi(c)),
        K::Tags => f.tags = Some(hierarchical(c)),
        K::PerformerTags => f.performer_tags = Some(hierarchical(c)),
        K::Studios => f.studios = Some(hierarchical(c)),
        K::Folder => {
            f.files_filter = Some(FileFilterType {
                parent_folder: Some(hierarchical(c)),
            });
        }
        K::PhashDistance => f.phash_distance = Some(phash_input(c)),
        K::CustomField => match custom_field_input(c) {
            Some(input) => f.custom_fields.get_or_insert_with(Vec::new).push(input),
            None => return false,
        },
        _ => return false,
    }
    true
}
