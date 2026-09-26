use stash_core::connection::version::{check, VersionStatus};
use stash_core::connection::ConnectFailure;

fn unsupported(found: &str) -> ConnectFailure {
    ConnectFailure::UnsupportedVersion {
        found: found.into(),
        minimum: "v0.31.1".into(),
    }
}

#[test]
fn releases_at_or_above_minimum_are_supported() {
    assert_eq!(
        check(Some("v0.31.1"), 85, "OK"),
        Ok(VersionStatus::Supported)
    );
    assert_eq!(
        check(Some("v0.32.0"), 90, "OK"),
        Ok(VersionStatus::Supported)
    );
    assert_eq!(
        check(Some("v1.0.0"), 99, "OK"),
        Ok(VersionStatus::Supported)
    );
}

#[test]
fn older_releases_are_refused_with_both_versions() {
    assert_eq!(
        check(Some("v0.30.1"), 80, "OK"),
        Err(unsupported("v0.30.1"))
    );
    assert_eq!(
        check(Some("v0.31.0"), 84, "OK"),
        Err(unsupported("v0.31.0"))
    );
}

#[test]
fn git_describe_dev_build_is_not_refused() {
    // A naive semver comparison ranks this below 0.31.1 because of the pre-release suffix.
    assert_eq!(
        check(Some("v0.31.1-12-gabc1234"), 85, "OK"),
        Ok(VersionStatus::DevelopmentBuild)
    );
    assert_eq!(
        check(Some("v0.30.1-5-gdeadbee"), 80, "OK"),
        Err(unsupported("v0.30.1-5-gdeadbee"))
    );
}

#[test]
fn unparseable_version_falls_back_to_app_schema() {
    assert_eq!(
        check(Some("garbage"), 85, "OK"),
        Ok(VersionStatus::UnknownButCompatible)
    );
    assert_eq!(
        check(None, 86, "OK"),
        Ok(VersionStatus::UnknownButCompatible)
    );
    assert_eq!(
        check(Some("garbage"), 80, "OK"),
        Err(unsupported("garbage"))
    );
    assert_eq!(check(None, 80, "OK"), Err(unsupported("unknown")));
}

#[test]
fn not_ready_status_is_refused_first() {
    for status in ["NEEDS_MIGRATION", "SETUP"] {
        assert_eq!(
            check(Some("v0.31.1"), 85, status),
            Err(ConnectFailure::ServerNotReady {
                status: status.into()
            })
        );
    }
}
