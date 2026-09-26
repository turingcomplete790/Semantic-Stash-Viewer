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
