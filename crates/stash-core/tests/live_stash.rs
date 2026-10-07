//! Opt-in test against a real Stash server. Ignored by default:
//!
//! ```sh
//! STASH_TEST_URL=http://localhost:9998 cargo test -p stash-core --test live_stash -- --ignored
//! ```
//!
//! `STASH_TEST_API_KEY` is optional. Read-only: only runs the connection check.

use std::time::Duration;

use stash_core::connection::connect::{test_connection, ConnectOptions};
use stash_core::connection::manager::{
    ConnectRequest, ConnectionManager, ManagerConfig, StashProber, Target,
};
use stash_core::connection::snapshot::SessionState;
use stash_core::profiles::ProfileDraft;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

fn env_draft() -> ProfileDraft {
    let address = std::env::var("STASH_TEST_URL")
        .expect("set STASH_TEST_URL, e.g. http://localhost:9998 (the disposable test instance)");
    ProfileDraft {
        address,
        api_key: std::env::var("STASH_TEST_API_KEY").ok(),
        ..Default::default()
    }
}

#[tokio::test]
#[ignore = "needs a real Stash; set STASH_TEST_URL"]
async fn connects_to_a_real_stash() {
    let outcome = test_connection(
        &env_draft(),
        &CancellationToken::new(),
        ConnectOptions::default(),
    )
    .await
    .expect("connect to STASH_TEST_URL");

    let version = outcome.server.version.trim_start_matches('v');
    let parsed = semver::Version::parse(version.split('-').next().unwrap_or(version))
        .expect("server reports a semver version");
    assert!(
        parsed >= semver::Version::new(0, 31, 1),
        "server {version} is below the minimum v0.31.1"
    );
    // Counts are read successfully; an empty test instance legitimately reports zeros.
    let c = outcome.server.counts;
    println!(
        "connected to {} ({}) security={:?} scenes={} images={} galleries={} performers={}",
        outcome.base_url,
        outcome.server.version,
        outcome.security,
        c.scenes,
        c.images,
        c.galleries,
        c.performers
    );
}

#[tokio::test]
#[ignore = "needs a real Stash; set STASH_TEST_URL"]
async fn connection_manager_reaches_connected() {
    let draft = env_draft();
    let outcome = test_connection(&draft, &CancellationToken::new(), ConnectOptions::default())
        .await
        .expect("connect to STASH_TEST_URL");

    let manager = ConnectionManager::new(
        StashProber::default(),
        ManagerConfig::default(),
        tokio::runtime::Handle::current(),
    );
    let mut rx = manager.subscribe();
    manager.connect(ConnectRequest {
        target: Target {
            profile_id: Uuid::new_v4(),
            base_url: outcome.base_url,
            strict_tls: false,
            api_key: draft.normalized_api_key(),
        },
        is_launch: true,
        has_connected_before: true,
    });

    let connected = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let s = rx.recv().await.expect("snapshot");
            if s.state == SessionState::Connected {
                return s;
            }
        }
    })
    .await
    .expect("Connected within 20 s");
    assert!(connected.server.is_some());
    assert!(connected.security.is_some());
}

// ---- SC-001: the viewer's filters give the web UI's results (008 T038) -------------------------

use stash_core::adapter::StashClient;
use stash_core::scenes::filter::kinds::CriterionKind;
use stash_core::scenes::filter::{Criterion, CriterionValue, Item, Modifier, Resolution as Res};
use stash_core::scenes::query::{SceneQuery, SceneSort, SortDirection};

fn live_client() -> StashClient {
    let url = std::env::var("STASH_TEST_URL").expect("set STASH_TEST_URL");
    StashClient::new(
        url::Url::parse(&url).expect("url"),
        false,
        std::env::var("STASH_TEST_API_KEY").ok(),
    )
    .expect("client")
}

fn items(kind: CriterionKind, modifier: Modifier, id: &str, depth: Option<i32>) -> Criterion {
    Criterion::new(
        kind,
        modifier,
        CriterionValue::Items {
            items: vec![Item {
                id: id.into(),
                label: String::new(),
            }],
            excluded: vec![],
            depth,
        },
    )
}

/// The viewer's criteria for each hand-written query (the saved ones come from Stash).
fn viewer_criteria(name: &str) -> Vec<Criterion> {
    use CriterionKind as K;
    match name {
        "search only" => vec![],
        "one performer" => vec![items(K::Performers, Modifier::IncludesAll, "2", None)],
        "tag with sub-tags" => vec![items(K::Tags, Modifier::Includes, "7", Some(-1))],
        "one studio" => vec![items(K::Studios, Modifier::Includes, "1", Some(0))],
        "resolution above 720p" => vec![Criterion::new(
            K::Resolution,
            Modifier::GreaterThan,
            CriterionValue::Resolution(Res::StandardHd),
        )],
        "duration 10 to 30 minutes" => vec![Criterion::new(
            K::Duration,
            Modifier::Between,
            CriterionValue::Int(600, Some(1800)),
        )],
        "not organized" => vec![Criterion::new(
            K::Organized,
            Modifier::Equals,
            CriterionValue::Bool(false),
        )],
        "rating at least 4 stars" => vec![Criterion::new(
            K::Rating,
            Modifier::GreaterThan,
            CriterionValue::Int(79, None),
        )],
        "scene custom field" => vec![Criterion::new(
            K::CustomField,
            Modifier::NotNull,
            CriterionValue::CustomField {
                field: "ssvSceneFlag".into(),
                values: vec![],
            },
        )],
        other => panic!("no viewer criteria for {other}"),
    }
}

/// The web UI's request: `findScenes` with the hand-written filter, sorted by path.
async fn raw_page(q: Option<&str>, scene_filter: &serde_json::Value) -> (u64, Vec<String>) {
    let url = std::env::var("STASH_TEST_URL").expect("url");
    let body = serde_json::json!({
        "query": "query($f: FindFilterType, $s: SceneFilterType) { findScenes(filter: $f, scene_filter: $s) { count scenes { id } } }",
        "variables": {
            "f": {"q": q, "page": 1, "per_page": 120, "sort": "path", "direction": "ASC"},
            "s": scene_filter,
        }
    });
    let mut req = reqwest::Client::new()
        .post(format!("{}/graphql", url.trim_end_matches('/')))
        .json(&body);
    if let Ok(key) = std::env::var("STASH_TEST_API_KEY") {
        req = req.header("ApiKey", key);
    }
    let v: serde_json::Value = req.send().await.expect("send").json().await.expect("json");
    assert!(
        v.get("errors").is_none(),
        "Stash rejected the hand-written filter: {v}"
    );
    let found = &v["data"]["findScenes"];
    let ids = found["scenes"]
        .as_array()
        .expect("scenes")
        .iter()
        .take(100)
        .map(|s| s["id"].as_str().expect("id").to_owned())
        .collect();
    (found["count"].as_u64().expect("count"), ids)
}

#[tokio::test]
#[ignore = "needs the Testing Stash with 008's seeded data; set STASH_TEST_URL (and STASH_TEST_API_KEY)"]
async fn the_viewers_filters_give_the_web_uis_results() {
    let client = live_client();
    let saved = stash_core::adapter::filters::saved_scene_filters(&client)
        .await
        .expect("saved filters");
    for f in &saved {
        assert!(
            f.left_out.is_empty(),
            "{} decodes completely: {:?}",
            f.name,
            f.left_out
        );
    }
    let doc: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/sc001/queries.json")).expect("fixture");
    let queries = doc["queries"].as_array().expect("queries");
    assert!(queries.len() >= 10);
    for q in queries {
        let name = q["name"].as_str().expect("name");
        let search = q.get("q").and_then(serde_json::Value::as_str);
        let criteria = match q.get("saved").and_then(serde_json::Value::as_str) {
            Some(saved_name) => saved
                .iter()
                .find(|f| f.name == saved_name)
                .unwrap_or_else(|| panic!("saved filter {saved_name} on Testing"))
                .query
                .criteria
                .clone(),
            None => viewer_criteria(name),
        };
        let query = SceneQuery {
            search: search.unwrap_or_default().to_owned(),
            sort: SceneSort::Path,
            direction: SortDirection::Asc,
            seed: None,
            criteria,
        };
        let page = stash_core::adapter::scenes::find_scenes_page(&client, &query, 1, 120)
            .await
            .expect("viewer page");
        let viewer_ids: Vec<String> = page.items.iter().take(100).map(|c| c.id.clone()).collect();
        let (count, raw_ids) = raw_page(search, &q["scene_filter"]).await;
        assert_eq!(u64::from(page.count), count, "{name}: count");
        assert_eq!(viewer_ids, raw_ids, "{name}: first 100");
        println!("{name}: {count} scenes, same first {}", raw_ids.len());
    }
}
