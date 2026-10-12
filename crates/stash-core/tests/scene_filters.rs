//! The filter model: criteria to Stash's `SceneFilterType`, validation, and the web UI's saved
//! filters (008 FR-002 to FR-005, research R3).

use serde_json::{json, Value};
use stash_core::browse::entity::Entity;
use stash_core::scenes::filter::kinds::{CriterionKind, ValueType};
use stash_core::scenes::filter::saved::{decode_default_filter, decode_saved_filter, DisplayMode};
use stash_core::scenes::filter::to_stash::scene_filter_json;
use stash_core::scenes::filter::{
    Criterion, CriterionValue, Duplication, Invalid, Item, Modifier, Orientation, Resolution,
    SemanticFilter,
};
use stash_core::scenes::query::{SceneQuery, SceneSort, SortDirection};

fn item(id: &str) -> Item {
    Item {
        id: id.into(),
        label: format!("Item {id}"),
    }
}

/// A valid value for `kind` under `modifier`.
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
    match kind.info().value_type {
        ValueType::Text { .. } | ValueType::Captions => CriterionValue::Text("abc".into()),
        ValueType::Number { .. } | ValueType::Duration { .. } => {
            CriterionValue::Int(10, range.then_some(20))
        }
        ValueType::Date => {
            CriterionValue::Date("2024-01-02".into(), range.then(|| "2024-03-04".into()))
        }
        ValueType::Timestamp { .. } => CriterionValue::Date(
            "2024-01-02 10:30".into(),
            range.then(|| "2024-03-04 11:00".into()),
        ),
        ValueType::Bool | ValueType::StringBool => CriterionValue::Bool(true),
        ValueType::Choice(options) => CriterionValue::Text(options[0].into()),
        ValueType::Resolution => CriterionValue::Resolution(Resolution::FullHd),
        ValueType::Orientation => CriterionValue::Orientation(vec![Orientation::Portrait]),
        ValueType::Items { .. } => CriterionValue::Items {
            items: vec![item("3")],
            excluded: vec![],
            depth: Some(-1),
        },
        ValueType::StashId => CriterionValue::StashId {
            endpoint: "https://stashdb.org/graphql".into(),
            id: "abc-123".into(),
        },
        ValueType::Phash => CriterionValue::Phash {
            hash: "ffeeddccbbaa9988".into(),
            distance: Some(4),
        },
        ValueType::Duplicated => CriterionValue::Duplicated(Duplication {
            phash: true,
            ..Duplication::default()
        }),
        ValueType::CustomField => CriterionValue::CustomField {
            field: "f".into(),
            values: if range {
                vec![json!(1), json!(5)]
            } else {
                vec![json!("x")]
            },
        },
        ValueType::Semantic => CriterionValue::None,
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
fn every_kind_and_modifier_converts_to_its_stash_field() {
    // 012: the kinds Scenes offers (Images' own kinds are tested in image_filters.rs).
    for kind in Entity::Scenes.criteria().collect::<Vec<_>>().iter() {
        for modifier in modifiers_of(*kind) {
            let c = Criterion::new(*kind, modifier, sample_value(*kind, modifier));
            assert_eq!(
                c.validate(),
                Ok(()),
                "{kind:?} {modifier:?} sample is valid"
            );
            let json = scene_filter_json(std::slice::from_ref(&c));
            let key = kind.info().key;
            let field = &json[key];
            assert!(
                !field.is_null(),
                "{kind:?} {modifier:?} sets `{key}`: {json}"
            );
            // Fields with a modifier send Stash's name for it.
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
fn values_take_stash_shapes() {
    let tags = Criterion::new(
        CriterionKind::Tags,
        Modifier::IncludesAll,
        CriterionValue::Items {
            items: vec![item("7")],
            excluded: vec![item("9")],
            depth: Some(-1),
        },
    );
    let perf = Criterion::new(
        CriterionKind::Performers,
        Modifier::Includes,
        CriterionValue::Items {
            items: vec![item("2"), item("3")],
            excluded: vec![],
            depth: None,
        },
    );
    let between = Criterion::new(
        CriterionKind::Duration,
        Modifier::Between,
        CriterionValue::Int(600, Some(1800)),
    );
    let ts = Criterion::new(
        CriterionKind::CreatedAt,
        Modifier::GreaterThan,
        CriterionValue::Date("2024-01-02 10:30".into(), Some("ignored".into())),
    );
    let res = Criterion::new(
        CriterionKind::Resolution,
        Modifier::Equals,
        CriterionValue::Resolution(Resolution::FullHd),
    );
    let markers = Criterion::new(
        CriterionKind::HasMarkers,
        Modifier::Equals,
        CriterionValue::Bool(false),
    );
    let json = scene_filter_json(&[tags, perf, between, ts, res, markers]);
    assert_eq!(
        json["tags"],
        json!({"value": ["7"], "modifier": "INCLUDES_ALL", "depth": -1, "excludes": ["9"]})
    );
    assert_eq!(
        json["performers"],
        json!({"value": ["2", "3"], "modifier": "INCLUDES"})
    );
    assert_eq!(
        json["duration"],
        json!({"value": 600, "value2": 1800, "modifier": "BETWEEN"})
    );
    assert_eq!(
        json["created_at"],
        json!({"value": "2024-01-02T10:30", "modifier": "GREATER_THAN"}),
        "timestamps use T; a second value is only sent for ranges"
    );
    assert_eq!(
        json["resolution"],
        json!({"value": "FULL_HD", "modifier": "EQUALS"})
    );
    assert_eq!(json["has_markers"], json!("false"));
}

#[test]
fn equals_on_a_hierarchy_means_no_sub_levels() {
    let c = Criterion::new(
        CriterionKind::Tags,
        Modifier::Equals,
        CriterionValue::Items {
            items: vec![item("7")],
            excluded: vec![],
            depth: Some(-1),
        },
    );
    assert_eq!(scene_filter_json(&[c])["tags"]["depth"], json!(0));
}

#[test]
fn validation_follows_the_rules() {
    let kind = CriterionKind::Duration;
    assert_eq!(
        Criterion::new(kind, Modifier::Includes, CriterionValue::Int(1, None)).validate(),
        Err(Invalid::Modifier),
        "the modifier is one the kind allows"
    );
    assert_eq!(
        Criterion::new(kind, Modifier::Between, CriterionValue::Int(1, None)).validate(),
        Err(Invalid::Value),
        "Between has two values"
    );
    assert_eq!(
        Criterion::new(kind, Modifier::IsNull, CriterionValue::Int(1, None)).validate(),
        Err(Invalid::Value),
        "IsNull/NotNull carry no value"
    );
    let empty = CriterionValue::Items {
        items: vec![],
        excluded: vec![],
        depth: None,
    };
    assert_eq!(
        Criterion::new(CriterionKind::Tags, Modifier::Includes, empty).validate(),
        Err(Invalid::Value),
        "Items are non-empty unless the modifier is a null check"
    );
    assert_eq!(
        Criterion::new(CriterionKind::Tags, Modifier::IsNull, CriterionValue::None).validate(),
        Ok(())
    );
}

#[test]
fn invalid_criteria_and_semantic_placeholders_are_never_sent() {
    let placeholder = Criterion::new(
        CriterionKind::Semantic(SemanticFilter::Role),
        Modifier::Equals,
        CriterionValue::None,
    );
    let invalid = Criterion::new(
        CriterionKind::Title,
        Modifier::Includes,
        CriterionValue::Text("  ".into()),
    );
    assert_eq!(scene_filter_json(&[placeholder, invalid]), Value::Null);
}

#[test]
fn custom_fields_may_repeat_and_go_out_as_a_list() {
    let mut q = SceneQuery::default();
    for field in ["a", "b"] {
        q.set_criterion(Criterion::new(
            CriterionKind::CustomField,
            Modifier::NotNull,
            CriterionValue::CustomField {
                field: field.into(),
                values: vec![],
            },
        ));
    }
    assert_eq!(q.criteria.len(), 2);
    let json = scene_filter_json(&q.criteria);
    assert_eq!(json["custom_fields"].as_array().map(Vec::len), Some(2));
    assert!(
        json["custom_fields"][0].get("value").is_none(),
        "null checks send no value"
    );
}

#[test]
fn each_kind_appears_once_and_a_replaced_criterion_keeps_its_place() {
    let mut q = SceneQuery::default();
    let title = |t: &str| {
        Criterion::new(
            CriterionKind::Title,
            Modifier::Includes,
            CriterionValue::Text(t.into()),
        )
    };
    q.set_criterion(title("a"));
    q.set_criterion(Criterion::new(
        CriterionKind::Organized,
        Modifier::Equals,
        CriterionValue::Bool(true),
    ));
    q.set_criterion(title("b"));
    assert_eq!(q.criteria.len(), 2);
    assert_eq!(q.criteria[0], title("b"));
    q.remove_criterion("title");
    assert_eq!(q.criteria.len(), 1);
}

#[test]
fn criteria_order_doesnt_change_the_cache_key_but_their_content_does() {
    let a = Criterion::new(
        CriterionKind::Organized,
        Modifier::Equals,
        CriterionValue::Bool(true),
    );
    let b = Criterion::new(
        CriterionKind::Resolution,
        Modifier::Equals,
        CriterionValue::Resolution(Resolution::FourK),
    );
    let one = SceneQuery {
        criteria: vec![a.clone(), b.clone()],
        ..SceneQuery::default()
    };
    let two = SceneQuery {
        criteria: vec![b, a],
        ..SceneQuery::default()
    };
    assert_eq!(one.cache_hash(), two.cache_hash());
    assert_ne!(one.cache_hash(), SceneQuery::default().cache_hash());
}

// ---- Saved filters ---------------------------------------------------------------------------

fn testing_filters() -> Vec<Value> {
    let doc: Value = serde_json::from_str(include_str!(
        "fixtures/saved_filters/testing-saved-filters.json"
    ))
    .expect("fixture");
    doc["data"]["findSavedFilters"]
        .as_array()
        .expect("filters")
        .clone()
}

fn decode(v: &Value) -> stash_core::scenes::filter::saved::SavedFilter {
    decode_saved_filter(
        v["id"].as_str().unwrap_or_default(),
        v["name"].as_str().unwrap_or_default(),
        v.get("find_filter"),
        v.get("object_filter"),
        v.get("ui_options"),
    )
}

#[test]
fn testings_saved_filters_decode_completely() {
    let filters = testing_filters();
    assert_eq!(filters.len(), 3);
    for f in &filters {
        let saved = decode(f);
        assert!(
            saved.left_out.is_empty(),
            "{}: left out {:?}",
            saved.name,
            saved.left_out
        );
        assert_eq!(saved.page_size, Some(40));
        assert_eq!(saved.mode, Some(DisplayMode::Grid));
        assert_eq!(saved.query.sort, SceneSort::Date);
        assert_eq!(saved.query.direction, SortDirection::Desc);
    }
    let by_name = |n: &str| decode(filters.iter().find(|f| f["name"] == n).expect("filter"));
    let perf = by_name("SSV performer A");
    assert_eq!(
        perf.query.criteria,
        vec![Criterion::new(
            CriterionKind::Performers,
            Modifier::IncludesAll,
            CriterionValue::Items {
                items: vec![Item {
                    id: "2".into(),
                    label: "SSV Test Performer A".into()
                }],
                excluded: vec![],
                depth: None,
            },
        )],
        "labels come with the saved filter, so chips need no lookup"
    );
    let tags = by_name("SSV parent tag + 1080p");
    assert_eq!(tags.query.criteria.len(), 2);
    let json = scene_filter_json(&tags.query.criteria);
    assert_eq!(json["tags"]["depth"], json!(-1));
    assert_eq!(json["resolution"]["value"], json!("FULL_HD"));
    let custom = by_name("SSV scene custom field");
    assert_eq!(
        scene_filter_json(&custom.query.criteria)["custom_fields"][0]["field"],
        json!("ssvSceneFlag")
    );
}

#[test]
fn the_default_filter_decodes_from_the_ui_configuration() {
    let value: Value = serde_json::from_str(include_str!(
        "fixtures/saved_filters/testing-default-filter.json"
    ))
    .expect("fixture");
    let d = decode_default_filter(&value);
    assert!(d.left_out.is_empty());
    assert_eq!(d.query.sort, SceneSort::Title);
    assert_eq!(d.query.direction, SortDirection::Asc);
    assert_eq!(d.query.criteria[0].kind, CriterionKind::Performers);
}

#[test]
fn unknown_keys_and_unreadable_values_are_left_out_not_dropped() {
    let saved = decode_saved_filter(
        "9",
        "odd",
        Some(&json!({"sort": "some_future_sort", "per_page": 25})),
        Some(&json!({
            "future_field": {"value": 1, "modifier": "EQUALS"},
            "rating100": {"value": "not a number", "modifier": "EQUALS"},
            "organized": {"value": "true", "modifier": "EQUALS"},
        })),
        None,
    );
    assert_eq!(saved.query.criteria.len(), 1, "organized still applies");
    let mut left = saved.left_out.clone();
    left.sort();
    assert_eq!(
        left,
        vec!["future_field", "rating100", "sort: some_future_sort"]
    );
}

#[test]
fn old_shapes_still_decode() {
    let (criteria, left) = stash_core::scenes::filter::saved::decode_criteria(&json!({
        // A flat list (ILabeledIdCriterion), a bare number with value2 beside it, and an
        // excludes that tags now express as includes-with-excluded.
        "galleries": {"value": [{"id": "4", "label": "G"}], "modifier": "INCLUDES"},
        "duration": {"value": 60, "value2": 120, "modifier": "BETWEEN"},
        "studios": {"value": {"items": [{"id": "5", "label": "S"}], "excluded": [], "depth": 0}, "modifier": "EXCLUDES"},
        "semantic_role": {"value": "x", "modifier": "EQUALS"},
    }));
    assert!(left.is_empty(), "{left:?}");
    assert_eq!(criteria.len(), 4);
    let studios = criteria
        .iter()
        .find(|c| c.kind == CriterionKind::Studios)
        .expect("studios");
    assert_eq!(studios.modifier, Modifier::Includes);
    assert!(
        matches!(&studios.value, CriterionValue::Items { items, excluded, .. } if items.is_empty() && excluded.len() == 1)
    );
    assert!(criteria
        .iter()
        .any(|c| c.kind == CriterionKind::Semantic(SemanticFilter::Role)));
}

#[test]
fn a_random_sort_keeps_a_seed() {
    let saved = decode_saved_filter(
        "1",
        "r",
        Some(&json!({"sort": "random_123456789"})),
        None,
        None,
    );
    assert_eq!(saved.query.sort, SceneSort::Random);
    assert!(saved.query.seed.is_some());
    assert!(saved.query.stash_sort().is_ok());
}

/// 012 T011: an images saved filter decodes with images' criteria and sorts; a scene filter leaves
/// image-only criteria out, and an image filter leaves scene-only ones out.
#[test]
fn saved_filters_decode_per_entity() {
    use stash_core::images::ImageSort;
    use stash_core::scenes::filter::saved::{decode_saved_filter_for, SavedFilterOf};
    let find = json!({"per_page": 40, "sort": "o_counter", "direction": "ASC"});
    let object = json!({
        "performers": {"modifier": "INCLUDES_ALL", "value": {"items": [{"id": "2", "label": "A"}], "excluded": []}},
        "photographer": {"modifier": "INCLUDES", "value": "Ann"},
        "is_missing": {"modifier": "EQUALS", "value": "photographer"},
        "director": {"modifier": "INCLUDES", "value": "Bo"}
    });
    let images: SavedFilterOf<ImageSort> =
        decode_saved_filter_for(Entity::Images, "4", "SSV", Some(&find), Some(&object), None);
    assert_eq!(images.query.sort, ImageSort::OCounter);
    assert_eq!(images.page_size, Some(40));
    let kinds: Vec<CriterionKind> = images.query.criteria.iter().map(|c| c.kind).collect();
    assert!(kinds.contains(&CriterionKind::Photographer));
    assert!(
        kinds.contains(&CriterionKind::IsMissing),
        "images' own is-missing value"
    );
    assert_eq!(images.left_out, vec!["director".to_owned()]);

    let scenes = decode_saved_filter("1", "S", Some(&find), Some(&object), None);
    assert!(scenes.left_out.contains(&"photographer".to_owned()));
    assert!(
        scenes.left_out.contains(&"sort: o_counter".to_owned())
            || scenes.query.sort == SceneSort::default()
    );
}
