//! The scene query: sorts, the seeded random order, and the cache key (005 research R2, R7).

use stash_core::scenes::query::{SceneQuery, SceneSort, SortDirection};

fn query(sort: SceneSort) -> SceneQuery {
    SceneQuery {
        sort,
        ..SceneQuery::default()
    }
}

#[test]
fn the_default_is_newest_first_by_date() {
    let q = SceneQuery::default();
    assert_eq!(q.sort, SceneSort::Date);
    assert_eq!(q.direction, SortDirection::Desc);
    assert_eq!(q.search, "");
    assert_eq!(q.seed, None);
}

#[test]
fn every_web_ui_sort_is_offered_with_its_label() {
    let all = SceneSort::all();
    assert_eq!(all.len(), 27);
    assert_eq!(SceneSort::FileModTime.label(), "File Modification Time");
    assert_eq!(
        SceneSort::PerceptualSimilarity.label(),
        "Perceptual Similarity (pHash)"
    );
    assert_eq!(SceneSort::Organized.label(), "Organised");
    // Labels are unique, and so are the values Stash receives.
    let mut labels: Vec<_> = all.iter().map(|s| s.label()).collect();
    labels.sort_unstable();
    labels.dedup();
    assert_eq!(labels.len(), 27);
}

#[test]
fn sorts_map_to_stash_sort_strings() {
    assert_eq!(query(SceneSort::Date).stash_sort().expect("date"), "date");
    assert_eq!(
        query(SceneSort::FileModTime).stash_sort().expect("mod"),
        "file_mod_time"
    );
    assert_eq!(
        query(SceneSort::Filesize).stash_sort().expect("size"),
        "filesize"
    );
    assert_eq!(
        query(SceneSort::CreatedAt).stash_sort().expect("created"),
        "created_at"
    );
}

#[test]
fn random_needs_a_seed_and_sends_it() {
    let mut q = query(SceneSort::Random);
    assert!(q.stash_sort().is_err(), "random without a seed is rejected");
    q.seed = Some(42);
    assert_eq!(q.stash_sort().expect("seeded"), "random_42");
}

#[test]
fn normalization_trims_search_and_drops_a_stray_seed() {
    let q = SceneQuery {
        search: "  beach  ".into(),
        sort: SceneSort::Title,
        direction: SortDirection::Asc,
        seed: Some(9),
    };
    let n = q.normalized();
    assert_eq!(n.search, "beach");
    assert_eq!(
        n.seed, None,
        "a seed means nothing unless the sort is random"
    );
    let r = SceneQuery {
        seed: Some(9),
        ..query(SceneSort::Random)
    }
    .normalized();
    assert_eq!(r.seed, Some(9));
}

#[test]
fn equal_queries_hash_the_same_and_different_ones_differently() {
    let base = SceneQuery::default();
    let same = SceneQuery {
        search: "  ".into(),
        seed: Some(3), // dropped by normalization: date isn't random
        ..SceneQuery::default()
    };
    assert_eq!(base.cache_hash(), same.cache_hash());
    assert_eq!(base.cache_hash().len(), 16);
    let variants = [
        query(SceneSort::Title),
        SceneQuery {
            direction: SortDirection::Asc,
            ..SceneQuery::default()
        },
        SceneQuery {
            search: "beach".into(),
            ..SceneQuery::default()
        },
        SceneQuery {
            seed: Some(1),
            ..query(SceneSort::Random)
        },
        SceneQuery {
            seed: Some(2),
            ..query(SceneSort::Random)
        },
    ];
    let mut hashes: Vec<String> = variants.iter().map(SceneQuery::cache_hash).collect();
    hashes.push(base.cache_hash());
    let count = hashes.len();
    hashes.sort();
    hashes.dedup();
    assert_eq!(hashes.len(), count, "every distinct query has its own key");
}

#[test]
fn the_query_round_trips_as_camel_case_json() {
    let q = SceneQuery {
        search: "x".into(),
        sort: SceneSort::PlayCount,
        direction: SortDirection::Asc,
        seed: None,
    };
    let json = serde_json::to_value(&q).expect("json");
    assert_eq!(json["sort"], "play_count");
    assert_eq!(json["direction"], "asc");
    let back: SceneQuery = serde_json::from_value(json).expect("back");
    assert_eq!(back, q);
}
