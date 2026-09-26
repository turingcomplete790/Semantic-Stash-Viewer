//! Address normalisation (research R9, FR-002).

use url::Url;

use super::failure::ConnectFailure;

/// Path suffixes users paste from elsewhere that point at Stash's API rather than its base.
const API_SUFFIXES: [&str; 2] = ["graphql", "playground"];

/// Turn raw user input into base-URL candidates to try, in order.
///
/// - No `scheme://` → `https://…` then `http://…`.
/// - Trailing slashes and a trailing `/graphql` or `/playground` are removed; any other
///   sub-path (reverse-proxy deployments such as `https://host/stash`) is kept.
/// - No host, or a scheme other than http/https → `InvalidAddress`, before any network call.
pub fn candidates(input: &str) -> Result<Vec<Url>, ConnectFailure> {
    let input = input.trim();
    if input.is_empty() {
        return Err(invalid("enter the server's address"));
    }

    let with_schemes: Vec<String> = if input.contains("://") {
        vec![input.to_owned()]
    } else {
        vec![format!("https://{input}"), format!("http://{input}")]
    };

    with_schemes.iter().map(|s| normalize(s)).collect()
}

/// Parse one absolute URL and reduce it to a Stash base URL.
pub fn normalize(raw: &str) -> Result<Url, ConnectFailure> {
    let mut url = Url::parse(raw).map_err(|e| invalid(&e.to_string()))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(invalid("the address must start with http:// or https://"));
    }
    if url.host_str().map_or(true, str::is_empty) {
        return Err(invalid("the address has no host"));
    }

    let mut segments: Vec<String> = url
        .path_segments()
        .map(|s| s.filter(|seg| !seg.is_empty()).map(str::to_owned).collect())
        .unwrap_or_default();
    if segments
        .last()
        .is_some_and(|last| API_SUFFIXES.contains(&last.as_str()))
    {
        segments.pop();
    }
    url.set_path(&segments.join("/"));
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

/// Base URL from a final request URL whose last path segment is `endpoint` (e.g. `healthz`).
pub fn base_from_endpoint(final_url: &Url, endpoint: &str) -> Option<Url> {
    let path = final_url.path().trim_end_matches('/');
    let base_path = path.strip_suffix(endpoint)?.trim_end_matches('/');
    let mut base = final_url.clone();
    base.set_path(base_path);
    base.set_query(None);
    base.set_fragment(None);
    Some(base)
}

fn invalid(reason: &str) -> ConnectFailure {
    ConnectFailure::InvalidAddress {
        reason: reason.to_owned(),
    }
}
