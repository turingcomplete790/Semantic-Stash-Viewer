//! 007 T016: the connection region and server switching (spec US1; capabilities C2, C4; B6).

use semantic_stash_viewer_native::effects::Effect;
use semantic_stash_viewer_native::session::connection::{Connection, Up};
use semantic_stash_viewer_native::session::{Msg, Session, Up as SessionUp};
use stash_core::connection::failure::ConnectFailure;
use stash_core::connection::snapshot::{ConnectionSnapshot, SessionState};
use stash_core::profiles::ServerProfile;

fn snapshot(state: SessionState) -> ConnectionSnapshot {
    ConnectionSnapshot {
        state,
        ..ConnectionSnapshot::default()
    }
}

fn profile(name: &str) -> ServerProfile {
    ServerProfile {
        id: uuid::Uuid::new_v4(),
        display_name: name.into(),
        base_url: "http://localhost:9998/".parse().expect("url"),
        strict_tls: false,
        api_key: None,
        created_at: chrono::Utc::now(),
        last_used_at: None,
    }
}

#[test]
fn snapshots_move_the_region_between_its_states() {
    let mut c = Connection::default();
    assert_eq!(
        c.observe(snapshot(SessionState::Connecting {
            attempt_url: "x".into()
        })),
        None
    );
    assert_eq!(
        c.observe(snapshot(SessionState::Connected)),
        Some(Up::BecameConnected)
    );
    assert!(c.connected());
    assert_eq!(
        c.observe(snapshot(SessionState::Connected)),
        None,
        "only on the change"
    );
    let _ = c.observe(snapshot(SessionState::Offline {
        attempt: 1,
        next_retry_at: "2026-10-04T00:00:00Z".into(),
    }));
    assert!(!c.connected());
    assert_eq!(
        c.observe(snapshot(SessionState::Connected)),
        Some(Up::BecameConnected)
    );
}

#[test]
fn an_auth_failure_asks_for_the_key() {
    let mut c = Connection::default();
    assert_eq!(
        c.observe(snapshot(SessionState::AuthFailed {
            failure: ConnectFailure::ApiKeyRejected
        })),
        Some(Up::KeyNeeded)
    );
}

#[test]
fn entering_a_session_connects_and_loads_the_server_list() {
    let p = profile("Testing");
    let (session, effects) = Session::enter(p.clone(), true);
    assert_eq!(
        effects,
        vec![
            Effect::Connect {
                profile: p.id,
                launch: true
            },
            Effect::LoadProfiles,
            // 007 US2: this server's tabs, and the first Home tab's summary meanwhile.
            Effect::LoadSession { profile: p.id },
            Effect::LoadSummary {
                tab: session.shell.tabs[0].id
            },
        ]
    );
}

#[test]
fn a_new_key_from_the_prompt_is_tested_then_connects() {
    use semantic_stash_viewer_native::effects::Reply;
    use semantic_stash_viewer_native::shell::overlays::{Msg as OverlayMsg, Overlay};
    let p = profile("Testing");
    let (mut session, _) = Session::enter(p.clone(), true);
    let _ = session.update(Msg::Connection(snapshot(SessionState::AuthFailed {
        failure: ConnectFailure::ApiKeyRejected,
    })));
    assert!(matches!(session.overlay, Overlay::KeyPrompt(_)));
    let _ = session.update(Msg::Overlay(OverlayMsg::KeyChanged("new-key".into())));
    let step = session.update(Msg::Overlay(OverlayMsg::SubmitKey));
    let Some(Effect::UpdateProfile { id, draft }) = step.effects.first() else {
        panic!("expected UpdateProfile, got {:?}", step.effects);
    };
    assert_eq!(*id, p.id);
    assert_eq!(draft.api_key.as_deref(), Some("new-key"));
    let updated = ServerProfile {
        api_key: Some("new-key".into()),
        ..p.clone()
    };
    let step = session.update(Msg::Reply(Reply::ProfileUpdated(Ok(updated))));
    assert_eq!(
        step.effects,
        vec![Effect::Connect {
            profile: p.id,
            launch: false
        }]
    );
    assert_eq!(session.overlay, Overlay::None);
}

#[test]
fn escape_closes_an_overlay_before_anything_else() {
    use iced::keyboard::{key::Named, Key};
    use semantic_stash_viewer_native::shell::overlays::Overlay;
    let (mut session, _) = Session::enter(profile("Testing"), true);
    let _ = session.update(Msg::OpenServerMenu);
    assert_eq!(session.overlay, Overlay::ServerMenu);
    let _ = session.update(Msg::Key(
        Key::Named(Named::Escape),
        iced::keyboard::Modifiers::empty(),
    ));
    assert_eq!(session.overlay, Overlay::None);
}

#[test]
fn the_first_connection_is_reported_once() {
    let (mut session, _) = Session::enter(profile("Testing"), true);
    let step = session.update(Msg::Connection(snapshot(SessionState::Connected)));
    assert_eq!(step.up, Some(SessionUp::FirstConnected));
    let _ = session.update(Msg::Connection(snapshot(SessionState::Offline {
        attempt: 1,
        next_retry_at: "2026-10-04T00:00:00Z".into(),
    })));
    let step = session.update(Msg::Connection(snapshot(SessionState::Connected)));
    assert_eq!(
        step.up,
        Some(SessionUp::Reconnected),
        "a reconnect is not the first connection"
    );
}

#[test]
fn switching_servers_asks_the_app_to_change_session() {
    let (mut session, _) = Session::enter(profile("Testing"), true);
    let other = profile("Production");
    let step = session.update(Msg::SwitchServer(other.id));
    assert_eq!(step.up, Some(SessionUp::Switch(other.id)));
    // Switching to the server that's already active does nothing.
    let current = session.profile.id;
    assert_eq!(session.update(Msg::SwitchServer(current)).up, None);
}

#[test]
fn adding_a_server_from_the_menu_saves_it_and_switches_to_it() {
    use semantic_stash_viewer_native::effects::Reply;
    use semantic_stash_viewer_native::onboarding::Msg as FormMsg;
    use semantic_stash_viewer_native::shell::overlays::{Msg as OverlayMsg, Overlay};
    let (mut session, _) = Session::enter(profile("Production"), true);
    let _ = session.update(Msg::OpenServerMenu);
    let _ = session.update(Msg::Overlay(OverlayMsg::AddServer));
    assert!(matches!(session.overlay, Overlay::AddServer(_)));
    let _ = session.update(Msg::AddServer(FormMsg::Address("localhost:9998".into())));
    let step = session.update(Msg::AddServer(FormMsg::Submit));
    assert!(matches!(
        step.effects.first(),
        Some(Effect::CreateProfile(_))
    ));
    let added = profile("Testing");
    let step = session.update(Msg::Reply(Reply::ProfileCreated(Ok(added.clone()))));
    assert_eq!(step.up, Some(SessionUp::Switch(added.id)));
    assert!(session.servers.iter().any(|s| s.id == added.id));
    assert_eq!(session.overlay, Overlay::None);
}
