//! Choice lists and the semantic vocabulary (008 FR-003a, FR-003c to FR-003e; research R4, R12).

use serde_json::{json, Value};
use stash_core::adapter::choices::{choice_list, ChoiceKind};
use stash_core::adapter::semantic::semantic_vocabulary;
use stash_core::adapter::StashClient;
use stash_core::scenes::filter::search::search;
use stash_core::semantic::{
    build_vocabulary, default_colour_rules, flag_is_set, vocabulary_query, ColourRule, Rgb,
    SemanticKind,
};
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn server(body: Value) -> (MockServer, StashClient) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    let client =
        StashClient::new(Url::parse(&server.uri()).expect("url"), false, None).expect("client");
    (server, client)
}

async fn first_body(server: &MockServer) -> Value {
    let requests = server.received_requests().await.expect("recording");
    serde_json::from_slice(&requests[0].body).expect("json")
}

#[tokio::test]
async fn the_tag_list_asks_stash_for_ordinary_tags_only() {
    let (server, client) = server(json!({"data": {"findTags": {"tags": [
        {"id": "6", "name": "Ordinary", "aliases": ["Plain"]}
    ]}}}))
    .await;
    let list = choice_list(&client, ChoiceKind::Tags).await.expect("tags");
    assert_eq!(list.entries.len(), 1);
    let body = first_body(&server).await;
    let fields = body["variables"]["tagFilter"]["custom_fields"]
        .as_array()
        .expect("one custom_fields list");
    let mut names: Vec<&str> = fields.iter().filter_map(|f| f["field"].as_str()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        vec![
            "isAction",
            "isLabel",
            "isMiseEnScene",
            "isRole",
            "isTrait",
            "isVariation",
            "plugtag",
            "variantOfTagId"
        ]
    );
    assert!(fields.iter().all(|f| f["modifier"] == "IS_NULL"));
    assert_eq!(body["variables"]["filter"]["per_page"], -1);
}

#[tokio::test]
async fn group_aliases_are_split_on_commas() {
    let (_s, client) = server(json!({"data": {"findGroups": {"groups": [
        {"id": "1", "name": "G", "aliases": "One, Two ,,Three"}
    ]}}}))
    .await;
    let list = choice_list(&client, ChoiceKind::Groups)
        .await
        .expect("groups");
    assert_eq!(list.entries[0].aliases, vec!["One", "Two", "Three"]);
}

#[test]
fn flag_values_one_and_true_are_set_zero_is_not() {
    for set in [json!(1), json!("1"), json!(true), json!("true"), json!(1.0)] {
        assert!(flag_is_set(Some(&set)), "{set}");
    }
    for unset in [
        json!(0),
        json!("0"),
        json!(false),
        json!("false"),
        Value::Null,
    ] {
        assert!(!flag_is_set(Some(&unset)), "{unset}");
    }
    assert!(!flag_is_set(None));
}

fn tag(id: &str, fields: Value) -> Value {
    json!({"id": id, "name": format!("tag-{id}"), "aliases": [], "custom_fields": fields})
}

fn sample() -> Value {
    json!({
        "isTrait": {"tags": [tag("9", json!({"isTrait": 1})), tag("12", json!({"isTrait": 0})), tag("20", json!({"isTrait": 1, "isLabel": 1}))]},
        "isLabel": {"tags": [tag("20", json!({"isTrait": 1, "isLabel": 1}))]},
        "plugtag": {"tags": [tag("10", json!({"plugtag": "1"}))]},
        "variant": {"tags": [
            tag("11", json!({"variantOfTagId": "9"})),
            tag("30", json!({"variantOfTagId": "999"}))
        ]},
        "rule_0": {"tags": [tag("13", json!({"ssvTestFlag": 1})), tag("14", json!({"ssvTestFlag": 0}))]},
    })
}

fn rules_with_extra() -> Vec<ColourRule> {
    let mut rules = default_colour_rules();
    rules.push(ColourRule {
        field: "ssvTestFlag".into(),
        colour: Rgb(1, 2, 3),
    });
    rules
}

#[test]
fn types_follow_the_fixed_priority_and_variants_borrow_their_canonical_type() {
    let v = build_vocabulary(&sample(), &rules_with_extra());
    assert_eq!(v.kind_of("9"), Some(SemanticKind::Trait));
    assert_eq!(
        v.kind_of("10"),
        Some(SemanticKind::Pluggable),
        "\"1\" counts as set"
    );
    assert_eq!(
        v.kind_of("20"),
        Some(SemanticKind::Label),
        "Label outranks Trait"
    );
    assert_eq!(
        v.kind_of("11"),
        Some(SemanticKind::Trait),
        "a variant takes its canonical type"
    );
    assert_eq!(v.kind_of("12"), None, "a flag of 0 isn't set");
    assert_eq!(
        v.kind_of("30"),
        None,
        "a variant of a missing tag has no type"
    );
}

#[test]
fn unset_flags_and_orphan_variants_are_shown_as_ordinary_tags() {
    let v = build_vocabulary(&sample(), &rules_with_extra());
    let mut extra: Vec<&str> = v.ordinary_extras().iter().map(|t| t.id.as_str()).collect();
    extra.sort_unstable();
    assert_eq!(extra, vec!["12", "30"]);
}

#[test]
fn colours_come_from_the_first_matching_rule() {
    let rules = rules_with_extra();
    let v = build_vocabulary(&sample(), &rules);
    let colour = |field: &str| rules.iter().find(|r| r.field == field).map(|r| r.colour);
    assert_eq!(
        v.colour_of("20", &rules),
        colour("isLabel"),
        "isLabel comes first"
    );
    assert_eq!(v.colour_of("9", &rules), colour("isTrait"));
    assert_eq!(
        v.colour_of("11", &rules),
        colour("isTrait"),
        "variant: canonical's colour"
    );
    assert_eq!(
        v.colour_of("13", &rules),
        Some(Rgb(1, 2, 3)),
        "an added rule colours its tags"
    );
    assert_eq!(
        v.colour_of("14", &rules),
        None,
        "a flag of 0 matches no rule"
    );
    assert_eq!(v.colour_of("12", &rules), None);
    assert_eq!(
        v.colour_of("6", &rules),
        None,
        "ordinary tags have no colour"
    );

    let reordered: Vec<ColourRule> = rules.iter().rev().cloned().collect();
    assert_eq!(
        v.colour_of("20", &reordered),
        colour("isTrait"),
        "order decides"
    );
}

#[test]
fn the_request_has_one_alias_per_flag_plus_variants_and_extra_rules() {
    let q = vocabulary_query(&rules_with_extra());
    for kind in SemanticKind::ALL {
        assert!(
            q.contains(&format!("  {}: findTags", kind.flag())),
            "{kind:?}"
        );
    }
    assert!(q.contains("  variant: findTags"));
    assert!(q.contains("rule_0: findTags"));
    assert!(q.contains("field: \"ssvTestFlag\", modifier: NOT_NULL"));
    assert_eq!(q.matches("NOT_NULL").count(), 9);
    // A bad field name never reaches the text.
    let mut bad = default_colour_rules();
    bad.push(ColourRule {
        field: "x\") { evil }".into(),
        colour: Rgb(0, 0, 0),
    });
    assert!(!vocabulary_query(&bad).contains("evil"));
}

#[tokio::test]
async fn the_vocabulary_is_one_request() {
    let (server, client) = server(json!({"data": sample()})).await;
    let v = semantic_vocabulary(&client, &rules_with_extra())
        .await
        .expect("vocabulary");
    assert_eq!(v.kind_of("9"), Some(SemanticKind::Trait));
    assert_eq!(server.received_requests().await.expect("rec").len(), 1);
}

#[test]
fn the_production_fixture_parses_with_every_flagged_tag_typed() {
    let doc: Value =
        serde_json::from_str(include_str!("fixtures/semantic/vocabulary.json")).expect("fixture");
    let data = &doc["data"];
    let v = build_vocabulary(data, &default_colour_rules());
    for kind in SemanticKind::ALL {
        for t in data[kind.flag()]["tags"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            let id = t["id"].as_str().expect("id");
            let set = flag_is_set(t["custom_fields"].get(kind.flag()));
            if set {
                assert!(
                    v.kind_of(id).is_some(),
                    "{id} flagged {} has a type",
                    kind.flag()
                );
            }
        }
    }
    assert!(!v.tags.is_empty());
    assert!(!v.variants.is_empty());
    // The real data unmarks a type with 0 while setting another (a re-typed tag): the tag takes
    // the type whose flag is set, never the cleared one.
    assert_eq!(
        v.kind_of("7"),
        Some(SemanticKind::Pluggable),
        "isTrait 0, plugtag 1"
    );
    assert_eq!(
        v.kind_of("1625"),
        Some(SemanticKind::Role),
        "isAction 0, isRole 1"
    );
    assert_eq!(
        v.kind_of("19"),
        Some(SemanticKind::Label),
        "isTrait 0, plugtag 0, isLabel 1"
    );
    assert!(v
        .ordinary_extras()
        .iter()
        .all(|t| v.kind_of(&t.id).is_none()));
}

#[test]
fn local_search_folds_case_and_accents_over_names_and_aliases() {
    let list = stash_core::adapter::choices::ChoiceList {
        kind: ChoiceKind::Performers,
        entries: vec![
            stash_core::adapter::choices::Choice {
                id: "1".into(),
                name: "Émilie".into(),
                aliases: vec![],
            },
            stash_core::adapter::choices::Choice {
                id: "2".into(),
                name: "Zoe".into(),
                aliases: vec!["Émma".into()],
            },
        ],
    };
    let folded = list.folded();
    assert_eq!(search(&folded, "emi"), vec![0]);
    assert_eq!(search(&folded, "EMMA"), vec![1]);
}
