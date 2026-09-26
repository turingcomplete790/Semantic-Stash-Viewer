use stash_core::connection::address::candidates;
use stash_core::connection::ConnectFailure;

fn strs(input: &str) -> Vec<String> {
    candidates(input)
        .expect("valid address")
        .iter()
        .map(|u| u.as_str().trim_end_matches('/').to_owned())
        .collect()
}

#[test]
fn no_scheme_tries_https_then_http() {
    assert_eq!(
        strs("localhost:9999"),
        vec!["https://localhost:9999", "http://localhost:9999"]
    );
    assert_eq!(
        strs("  192.168.1.10:9999  "),
        vec!["https://192.168.1.10:9999", "http://192.168.1.10:9999"]
    );
}

#[test]
fn explicit_scheme_is_the_only_candidate() {
    assert_eq!(strs("http://localhost:9999"), vec!["http://localhost:9999"]);
    assert_eq!(
        strs("https://stash.example.com"),
        vec!["https://stash.example.com"]
    );
}

#[test]
fn api_paths_and_trailing_slashes_are_stripped() {
    for input in [
        "http://localhost:9999/",
        "http://localhost:9999/graphql",
        "http://localhost:9999/graphql/",
        "http://localhost:9999/playground",
        "http://localhost:9999//",
    ] {
        assert_eq!(strs(input), vec!["http://localhost:9999"], "input {input}");
    }
}

#[test]
fn reverse_proxy_sub_path_is_kept() {
    assert_eq!(
        strs("https://host/stash/graphql"),
        vec!["https://host/stash"]
    );
    assert_eq!(strs("https://host/stash/"), vec!["https://host/stash"]);
}

#[test]
fn invalid_addresses_fail_before_any_network_call() {
    for input in ["", "   ", "ftp://x", "http://", "https://"] {
        assert!(
            matches!(
                candidates(input),
                Err(ConnectFailure::InvalidAddress { .. })
            ),
            "input {input:?}"
        );
    }
}
