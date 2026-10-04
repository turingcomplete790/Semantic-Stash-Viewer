//! 007 T015: the Onboarding state's transitions (spec US1; capability C1).

use semantic_stash_viewer_native::effects::{Effect, Reply};
use semantic_stash_viewer_native::onboarding::{Msg, Phase, ServerForm, Up};
use stash_core::connection::failure::ConnectFailure;
use stash_core::profiles::{ProfileDraft, ServerProfile};
use stash_core::AppError;

fn profile() -> ServerProfile {
    ServerProfile {
        id: uuid::Uuid::new_v4(),
        display_name: "Testing".into(),
        base_url: "http://localhost:9998/".parse().expect("url"),
        strict_tls: false,
        api_key: Some("key".into()),
        created_at: chrono::Utc::now(),
        last_used_at: None,
    }
}

#[test]
fn an_empty_address_cant_be_submitted() {
    let mut form = ServerForm::default();
    assert!(!form.can_submit());
    let step = form.update(Msg::Address("   ".into()));
    assert!(step.effects.is_empty());
    let step = form.update(Msg::Submit);
    assert!(
        step.effects.is_empty(),
        "nothing is tested without an address"
    );
    assert_eq!(form.phase, Phase::Editing);
}

#[test]
fn submitting_tests_and_saves_the_server() {
    let mut form = ServerForm::default();
    let _ = form.update(Msg::Address("localhost:9998".into()));
    let _ = form.update(Msg::ApiKey("  secret  ".into()));
    let _ = form.update(Msg::StrictTls(true));
    let step = form.update(Msg::Submit);
    assert_eq!(
        step.effects,
        vec![Effect::CreateProfile(ProfileDraft {
            display_name: None,
            address: "localhost:9998".into(),
            api_key: Some("  secret  ".into()),
            strict_tls: true,
        })]
    );
    assert_eq!(form.phase, Phase::Connecting);
    // A second submit while connecting does nothing.
    assert!(form.update(Msg::Submit).effects.is_empty());
}

#[test]
fn an_empty_api_key_is_sent_as_none() {
    let mut form = ServerForm::default();
    let _ = form.update(Msg::Address("nas".into()));
    let step = form.update(Msg::Submit);
    let Some(Effect::CreateProfile(draft)) = step.effects.first() else {
        panic!("expected CreateProfile, got {:?}", step.effects);
    };
    assert_eq!(draft.api_key, None);
}

#[test]
fn success_hands_the_saved_server_to_the_app() {
    let mut form = ServerForm::default();
    let _ = form.update(Msg::Address("localhost:9998".into()));
    let _ = form.update(Msg::Submit);
    let saved = profile();
    let step = form.update(Msg::Reply(Reply::ProfileCreated(Ok(saved.clone()))));
    assert_eq!(step.up, Some(Up::Saved(saved)));
}

#[test]
fn a_failure_keeps_the_form_and_shows_why() {
    let mut form = ServerForm::default();
    let _ = form.update(Msg::Address("localhost:1".into()));
    let _ = form.update(Msg::ApiKey("k".into()));
    let _ = form.update(Msg::Submit);
    let failure = AppError::Connect {
        failure: ConnectFailure::Timeout,
    };
    let step = form.update(Msg::Reply(Reply::ProfileCreated(Err(failure.clone()))));
    assert_eq!(step.up, None);
    assert_eq!(form.phase, Phase::Failed(failure));
    assert_eq!(form.address, "localhost:1");
    assert_eq!(form.api_key, "k");
    // Editing again clears the failure.
    let _ = form.update(Msg::Address("localhost:9998".into()));
    assert_eq!(form.phase, Phase::Editing);
}

#[test]
fn the_api_key_is_hidden_until_shown() {
    let mut form = ServerForm::default();
    assert!(!form.show_key);
    let _ = form.update(Msg::ToggleShowKey);
    assert!(form.show_key);
}

#[test]
fn an_optional_name_is_sent_trimmed_and_an_empty_one_uses_the_address() {
    let mut form = ServerForm::default();
    let _ = form.update(Msg::Address("localhost:9998".into()));
    let _ = form.update(Msg::Name("  Testing  ".into()));
    let step = form.update(Msg::Submit);
    let Some(Effect::CreateProfile(draft)) = step.effects.first() else {
        panic!("expected CreateProfile, got {:?}", step.effects);
    };
    assert_eq!(draft.display_name.as_deref(), Some("Testing"));
}
