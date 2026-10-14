//! 012 T009: image criteria to Stash's `ImageFilterType`, the same input the web UI sends
//! (data-model.md "CriterionKind"; the web UI's v0.31.1 `images.ts`).

use serde_json::json;
use stash_core::browse::entity::Entity;
use stash_core::scenes::filter::kinds::{CriterionKind, ValueType};
use stash_core::scenes::filter::to_stash::{image_filter_json, scene_filter_json};
use stash_core::scenes::filter::{
    Criterion, CriterionValue, Item, Modifier, Orientation, Resolution,
};

fn item(id: &str) -> Item {
    Item {
        id: id.into(),
        label: format!("Item {id}"),
    }
}

/// A valid value for `kind` under `modifier` (images use only these value types).
fn sample_value(kind: CriterionKind, modifier: Modifier) -> CriterionValue {
    if modifier.is_null_check() {
        return match kind {
            CriterionKind::CustomField => CriterionValue::CustomField {
                field: "f".into(),
                values: vec![],
            },
            _ => CriterionValue::None,
        };
    }
    let range = modifier.is_range();
    match kind.info_for(Entity::Images).value_type {
        ValueType::Text { .. } => CriterionValue::Text("abc".into()),
        ValueType::Number { .. } => CriterionValue::Int(10, range.then_some(20)),
        ValueType::Date => {
            CriterionValue::Date("2024-01-02".into(), range.then(|| "2024-03-04".into()))
        }
        ValueType::Timestamp { .. } => CriterionValue::Date(
            "2024-01-02 10:30".into(),
            range.then(|| "2024-03-04 11:00".into()),
        ),
        ValueType::Bool => CriterionValue::Bool(true),
        ValueType::Choice(options) => CriterionValue::Text(options[0].into()),
        ValueType::Resolution => CriterionValue::Resolution(Resolution::FullHd),
        ValueType::Orientation => CriterionValue::Orientation(vec![Orientation::Portrait]),
        ValueType::Items { .. } => CriterionValue::Items {
            items: vec![item("3")],
            excluded: vec![],
            depth: Some(-1),
        },
        ValueType::Phash => CriterionValue::Phash {
            hash: "ffeeddccbbaa9988".into(),
            distance: Some(4),
        },
        ValueType::CustomField => CriterionValue::CustomField {
            field: "f".into(),
            values: if range {
                vec![json!(1), json!(5)]
            } else {
                vec![json!("x")]
            },
        },
        ValueType::Semantic => CriterionValue::None,
        other => panic!("{kind:?}: images don't use {other:?}"),
    }
}

fn modifiers_of(kind: CriterionKind) -> Vec<Modifier> {
    let m = kind.info().modifiers();
    if m.is_empty() {
        vec![Modifier::Equals]
    } else {
        m.to_vec()
    }
}

#[test]
fn images_offer_the_web_uis_criteria_in_order() {
    let keys: Vec<&str> = Entity::Images
        .criteria()
        .filter(|k| !matches!(k, CriterionKind::Semantic(_)))
        .map(|k| k.info().key)
        .collect();
    for key in [
        "title",
        "code",
        "details",
        "photographer",
        "checksum",
        "phash_distance",
        "path",
        "folder",
        "galleries",
        "organized",
        "o_counter",
        "resolution",
        "orientation",
        "is_missing",
        "tags",
        "rating100",
        "tag_count",
        "performer_tags",
        "performers",
        "performer_count",
        "performer_age",
        "performer_favorite",
        "studios",
        "url",
        "date",
        "file_count",
        "created_at",
        "updated_at",
        "custom_fields",
    ] {
        assert!(keys.contains(&key), "{key} offered for images: {keys:?}");
    }
    for scene_only in [
        "director",
        "duration",
        "play_count",
        "groups",
        "has_markers",
        "oshash",
    ] {
        assert!(
            !keys.contains(&scene_only),
            "{scene_only} isn't an image criterion"
        );
    }
    assert_eq!(keys.len(), 29);
}

#[test]
fn every_image_kind_and_modifier_converts_to_its_stash_field() {
    for kind in Entity::Images
        .criteria()
        .filter(|k| !matches!(k, CriterionKind::Semantic(_)))
    {
        for modifier in modifiers_of(kind) {
            let c = Criterion::new(kind, modifier, sample_value(kind, modifier));
            let json = image_filter_json(std::slice::from_ref(&c));
            let field = match kind {
                CriterionKind::Folder => json["files_filter"]["parent_folder"].clone(),
                _ => json[kind.info().key].clone(),
            };
            assert!(!field.is_null(), "{kind:?} {modifier:?} is sent: {json}");
            if !kind.info().modifiers().is_empty() {
                let sent = match kind {
                    CriterionKind::CustomField => field[0]["modifier"].clone(),
                    _ => field["modifier"].clone(),
                };
                assert_eq!(sent, json!(modifier.stash_name()), "{kind:?} {modifier:?}");
            }
        }
    }
}

#[test]
fn image_values_are_shaped_as_the_web_ui_sends_them() {
    let performers = Criterion::new(
        CriterionKind::Performers,
        Modifier::IncludesAll,
        CriterionValue::Items {
            items: vec![item("2")],
            excluded: vec![],
            depth: None,
        },
    );
    let folder = Criterion::new(
        CriterionKind::Folder,
        Modifier::Includes,
        CriterionValue::Items {
            items: vec![item("40")],
            excluded: vec![],
            depth: Some(-1),
        },
    );
    let photographer = Criterion::new(
        CriterionKind::Photographer,
        Modifier::Includes,
        CriterionValue::Text(" Ann ".into()),
    );
    let missing = Criterion::new(
        CriterionKind::IsMissing,
        Modifier::Equals,
        CriterionValue::Text("photographer".into()),
    );
    let json = image_filter_json(&[performers, folder, photographer, missing]);
    assert_eq!(
        json["performers"],
        json!({"value": ["2"], "modifier": "INCLUDES_ALL"})
    );
    assert_eq!(
        json["files_filter"],
        json!({"parent_folder": {"value": ["40"], "modifier": "INCLUDES", "depth": -1, "excludes": []}})
    );
    assert_eq!(
        json["photographer"],
        json!({"value": "Ann", "modifier": "INCLUDES"})
    );
    assert_eq!(json["is_missing"], json!("photographer"));
}

#[test]
fn a_scene_only_criterion_is_never_sent_for_images_and_vice_versa() {
    let director = Criterion::new(
        CriterionKind::Director,
        Modifier::Includes,
        CriterionValue::Text("x".into()),
    );
    assert!(image_filter_json(std::slice::from_ref(&director)).is_null());
    let photographer = Criterion::new(
        CriterionKind::Photographer,
        Modifier::Includes,
        CriterionValue::Text("x".into()),
    );
    assert!(scene_filter_json(std::slice::from_ref(&photographer)).is_null());
}

#[test]
fn images_have_their_own_is_missing_choices() {
    let ValueType::Choice(choices) = CriterionKind::IsMissing.info_for(Entity::Images).value_type
    else {
        panic!("a choice")
    };
    assert!(choices.contains(&"photographer"));
    assert!(!choices.contains(&"director"));
}
