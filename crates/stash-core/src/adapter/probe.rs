//! The connect probe and its response classification (contracts/stash-http.md §1, research R5).

use cynic::QueryBuilder;
use reqwest::StatusCode;
use url::Url;

use super::gql::schema;
use super::StashClient;
use crate::connection::address::base_from_endpoint;
use crate::connection::failure::ConnectFailure;
use crate::connection::LibraryCounts;

/// Stash's readiness; a value this build doesn't know becomes `Other`.
#[derive(cynic::Enum, Clone, Debug, PartialEq, Eq)]
#[cynic(graphql_type = "SystemStatusEnum")]
enum SystemStatusEnum {
    Setup,
    NeedsMigration,
    Ok,
    #[cynic(fallback)]
    Other(String),
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Version")]
struct VersionFields {
    version: Option<String>,
    hash: String,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "SystemStatus")]
struct SystemStatusFields {
    app_schema: i32,
    status: SystemStatusEnum,
    // Tells two Stash instances at the same address apart (003 research R2). Only a hash of these
    // is kept; the paths themselves are never stored.
    database_path: Option<String>,
    config_path: Option<String>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "StatsResultType")]
struct StatsFields {
    #[cynic(rename = "scene_count")]
    scene_count: i32,
    #[cynic(rename = "image_count")]
    image_count: i32,
    #[cynic(rename = "gallery_count")]
    gallery_count: i32,
    #[cynic(rename = "performer_count")]
    performer_count: i32,
}

/// One round trip on connect: identity, version, readiness, and the library summary (001
/// FR-007). Only what the connection screen needs (Principle IV).
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
struct ConnectProbe {
    version: VersionFields,
    system_status: SystemStatusFields,
    stats: StatsFields,
}

/// What the probe learned from a server that answered like Stash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeData {
    /// Base URL after following redirects.
    pub final_base_url: Url,
    /// `version.version`; Stash may leave it null.
    pub version: Option<String>,
    pub hash: String,
    pub app_schema: i64,
    /// `systemStatus.status` as a string (`OK`, `NEEDS_MIGRATION`, `SETUP`, …).
    pub status: String,
    pub counts: LibraryCounts,
    /// `systemStatus.databasePath` / `configPath`, for the server identity (003 R2).
    pub database_path: Option<String>,
    pub config_path: Option<String>,
}

/// Probe one candidate base URL.
///
/// 1. `GET {base}/healthz` follows any redirects to find the final base URL (a 301/302 would
///    turn the GraphQL POST into a GET, so redirects are resolved first).
/// 2. `POST {base}/graphql` with `ConnectProbe`, sending the API key if configured.
/// 3. A `401` with `Www-Authenticate: FormBased` proves it's Stash with an auth problem; if a
///    key was sent, retry once without it to tell a wrong key on an open server apart.
pub async fn probe(client: &StashClient) -> Result<ProbeData, ConnectFailure> {
    let client = resolve_redirects(client).await?;

    let response = send(&client, true).await?;
    match response.status() {
        StatusCode::OK => parse_stash(&client, response).await,
        StatusCode::UNAUTHORIZED if is_form_based(&response) => {
            if !client.has_api_key() {
                return Err(ConnectFailure::ApiKeyRequired);
            }
            let retry = send(&client, false).await?;
            if retry.status() == StatusCode::OK && parse_stash(&client, retry).await.is_ok() {
                Err(ConnectFailure::ApiKeyInvalidButNotRequired)
            } else {
                Err(ConnectFailure::ApiKeyRejected)
            }
        }
        other => Err(ConnectFailure::NotStash {
            status: Some(other.as_u16()),
        }),
    }
}

/// Follow redirects with an unauthenticated `GET /healthz` and rebase the client on the final
/// URL. A non-2xx answer is fine here (it may not be Stash; the POST decides).
async fn resolve_redirects(client: &StashClient) -> Result<StashClient, ConnectFailure> {
    let response = client
        .get("healthz")
        .send()
        .await
        .map_err(|e| client.classify_transport_error(&e))?;
    match base_from_endpoint(response.url(), "healthz") {
        Some(base) if base != *client.base_url() => {
            tracing::debug!(from = %client.base_url(), to = %base, "followed redirect");
            Ok(client.rebased(base))
        }
        _ => Ok(client.clone()),
    }
}

async fn send(client: &StashClient, with_key: bool) -> Result<reqwest::Response, ConnectFailure> {
    let body = ConnectProbe::build(());
    client
        .graphql_request(with_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| client.classify_transport_error(&e))
}

fn is_form_based(response: &reqwest::Response) -> bool {
    response
        .headers()
        .get_all(reqwest::header::WWW_AUTHENTICATE)
        .iter()
        .any(|v| {
            v.to_str()
                .is_ok_and(|s| s.trim().eq_ignore_ascii_case("FormBased"))
        })
}

async fn parse_stash(
    client: &StashClient,
    response: reqwest::Response,
) -> Result<ProbeData, ConnectFailure> {
    let not_stash = ConnectFailure::NotStash {
        status: Some(StatusCode::OK.as_u16()),
    };
    let text = response
        .text()
        .await
        .map_err(|e| client.classify_transport_error(&e))?;
    let parsed: cynic::GraphQlResponse<ConnectProbe> =
        serde_json::from_str(&text).map_err(|_| not_stash.clone())?;
    let data = parsed.data.ok_or(not_stash)?;

    Ok(ProbeData {
        final_base_url: client.base_url().clone(),
        version: data.version.version,
        hash: data.version.hash,
        app_schema: i64::from(data.system_status.app_schema),
        status: status_name(&data.system_status.status),
        counts: LibraryCounts {
            scenes: count(data.stats.scene_count),
            images: count(data.stats.image_count),
            galleries: count(data.stats.gallery_count),
            performers: count(data.stats.performer_count),
        },
        database_path: data.system_status.database_path.clone(),
        config_path: data.system_status.config_path.clone(),
    })
}

fn status_name(status: &SystemStatusEnum) -> String {
    match status {
        SystemStatusEnum::Ok => "OK".into(),
        SystemStatusEnum::NeedsMigration => "NEEDS_MIGRATION".into(),
        SystemStatusEnum::Setup => "SETUP".into(),
        SystemStatusEnum::Other(other) => other.clone(),
    }
}

fn count(n: i32) -> u32 {
    u32::try_from(n).unwrap_or(0)
}
