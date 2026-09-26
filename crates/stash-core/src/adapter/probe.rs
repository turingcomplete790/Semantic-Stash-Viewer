//! The connect probe and its response classification (contracts/stash-http.md §1, research R5).

use graphql_client::GraphQLQuery;
use reqwest::StatusCode;
use url::Url;

use super::StashClient;
use crate::connection::address::base_from_endpoint;
use crate::connection::failure::ConnectFailure;
use crate::connection::LibraryCounts;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.json",
    query_path = "graphql/connect_probe.graphql",
    response_derives = "Debug"
)]
pub struct ConnectProbe;

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
    let body = ConnectProbe::build_query(connect_probe::Variables);
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
    let parsed: graphql_client::Response<connect_probe::ResponseData> =
        serde_json::from_str(&text).map_err(|_| not_stash.clone())?;
    let data = parsed.data.ok_or(not_stash)?;

    Ok(ProbeData {
        final_base_url: client.base_url().clone(),
        version: data.version.version,
        hash: data.version.hash,
        app_schema: data.system_status.app_schema,
        status: status_name(&data.system_status.status),
        counts: LibraryCounts {
            scenes: count(data.stats.scene_count),
            images: count(data.stats.image_count),
            galleries: count(data.stats.gallery_count),
            performers: count(data.stats.performer_count),
        },
    })
}

fn status_name(status: &connect_probe::SystemStatusEnum) -> String {
    use connect_probe::SystemStatusEnum as S;
    match status {
        S::OK => "OK".into(),
        S::NEEDS_MIGRATION => "NEEDS_MIGRATION".into(),
        S::SETUP => "SETUP".into(),
        S::Other(other) => other.clone(),
    }
}

fn count(n: i64) -> u32 {
    u32::try_from(n).unwrap_or(0)
}
