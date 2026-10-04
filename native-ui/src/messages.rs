//! Plain-language explanations of failures (007 research R8; behaviour that stays B6: distinct,
//! plain messages for every connection failure, 001 FR-005 / SC-003).
//!
//! Both functions are exhaustive `match`es, so a new failure kind in the core doesn't compile
//! until it has a message here.

use stash_core::connection::failure::ConnectFailure;
use stash_core::AppError;

/// A title, what happened, and what to do about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureMessage {
    pub title: String,
    pub detail: String,
    pub hint: Option<String>,
}

fn msg(title: impl Into<String>, detail: impl Into<String>, hint: Option<&str>) -> FailureMessage {
    FailureMessage {
        title: title.into(),
        detail: detail.into(),
        hint: hint.map(str::to_owned),
    }
}

/// Why a connection didn't work.
pub fn for_connect(failure: &ConnectFailure) -> FailureMessage {
    match failure {
        ConnectFailure::InvalidAddress { reason } => msg(
            "That address can't be used",
            format!("The address isn't valid: {reason}."),
            Some("Enter something like 192.168.1.10:9999 or https://stash.example.com."),
        ),
        ConnectFailure::Unreachable { tried } => msg(
            "Nothing answered at this address",
            match tried.as_slice() {
                [] => "Nothing responded.".to_owned(),
                [one] => format!("Tried {one}, but nothing responded."),
                many => format!("Tried {}, but nothing responded.", many.join(" and ")),
            },
            Some("Check the address and port, that Stash is running, and whether it uses http or https."),
        ),
        ConnectFailure::Timeout => msg(
            "The server didn't respond in time",
            "No response arrived within 15 seconds.",
            Some("Check that the server is reachable from this computer, then try again."),
        ),
        ConnectFailure::NotStash { status } => msg(
            "Something answered, but it isn't Stash",
            match status {
                Some(code) => format!("The server replied with HTTP {code}, not a Stash response."),
                None => "The server's reply didn't look like Stash.".to_owned(),
            },
            Some("Check the port, and remove any extra path from the address."),
        ),
        ConnectFailure::ApiKeyRequired => msg(
            "This server needs an API key",
            "Stash has authentication turned on, so an API key is needed to connect.",
            Some("In Stash's web UI, open Settings → Security and copy the API key."),
        ),
        ConnectFailure::ApiKeyRejected => msg(
            "The API key was rejected",
            "Stash didn't accept this API key.",
            Some("The key may have been regenerated. Copy the current one from Settings → Security."),
        ),
        ConnectFailure::ApiKeyInvalidButNotRequired => msg(
            "This server doesn't need a key, and the one entered is wrong",
            "Stash has no authentication turned on, but it still rejects an incorrect key.",
            Some("Connect without a key, or correct it."),
        ),
        ConnectFailure::UnsupportedVersion { found, minimum } => msg(
            format!("Stash {found} is older than {minimum}"),
            format!("This viewer needs Stash {minimum} or later; the server runs {found}."),
            Some("Update Stash, then connect again."),
        ),
        ConnectFailure::ServerNotReady { status } => msg(
            "Stash isn't ready yet",
            format!("Stash reports status \"{status}\"."),
            Some("Finish setup or the database migration in Stash's web UI first."),
        ),
        ConnectFailure::CertificateNotVerified => msg(
            "The server's certificate couldn't be verified",
            "Strict certificate checking is on, and this server's certificate isn't trusted.",
            Some("Turn off strict certificate checking for this server to connect anyway."),
        ),
        ConnectFailure::DuplicateProfile { .. } => msg(
            "This server is already saved",
            "A saved server already uses this address.",
            Some("Switch to it from the server menu instead."),
        ),
    }
}

/// Any app error, connection failures included.
pub fn for_error(error: &AppError) -> FailureMessage {
    match error {
        AppError::Connect { failure } => for_connect(failure),
        AppError::InvalidDisplayName { reason } => msg(
            "That name can't be used",
            format!("The display name {reason}."),
            None,
        ),
        AppError::ProfileNotFound { .. } => msg(
            "That server is no longer saved",
            "It was removed while this was open.",
            None,
        ),
        AppError::SceneNotFound { id } => msg(
            "That scene doesn't exist",
            format!("The server has no scene with ID {id}."),
            None,
        ),
        AppError::NoPlayableFile { id } => msg(
            "This scene has no video file",
            format!("Scene {id} has no file the player can open."),
            None,
        ),
        AppError::UnsupportedProfilesVersion { found } => msg(
            "Saved servers are from a newer version",
            format!("The saved servers file has version {found}, which this version can't read. It was left untouched."),
            Some("Update Semantic Stash Viewer."),
        ),
        AppError::Storage { message } => msg("Couldn't save settings", message.clone(), None),
        AppError::TabSetInvalid { reason } => msg(
            "Couldn't save your tabs",
            format!("The tabs couldn't be saved ({reason})."),
            None,
        ),
        AppError::OpenFailed { detail } => msg("Couldn't open that", detail.clone(), None),
        AppError::NotConnected => msg(
            "Can't reach the server",
            "This needs the server. The viewer reconnects on its own as soon as it's back.",
            None,
        ),
        AppError::Cancelled => msg("Cancelled", "The attempt was cancelled.", None),
        AppError::Internal { message } => msg("Something went wrong", message.clone(), None),
    }
}
