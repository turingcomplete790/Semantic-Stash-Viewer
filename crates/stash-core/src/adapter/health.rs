//! Health check: `GET {base}/healthz` (contracts/stash-http.md §2, research R8).

use super::StashClient;

/// True when Stash answers `/healthz` with a 2xx. Any other status or a transport error is a
/// failure. Unauthenticated, so it detects reachability only; auth problems are caught by the
/// re-probe and by ordinary requests.
pub async fn health(client: &StashClient) -> bool {
    match client.get("healthz").send().await {
        Ok(response) => response.status().is_success(),
        Err(e) => {
            tracing::debug!(error = %e, url = %client.base_url(), "health check failed");
            false
        }
    }
}
