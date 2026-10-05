//! 007 T046: playback inside the shell (spec US4; capabilities C5, C6; behaviour that stays B5).

use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use player::{PlayerError, PlayerSnapshot, PlayerStateKind};
use semantic_stash_viewer_native::effects::{Effect, PlayerAction, Reply};
use semantic_stash_viewer_native::player::view::Msg as Controls;
use semantic_stash_viewer_native::screens::Screen;
use semantic_stash_viewer_native::session::{playback, Msg, Session};
use semantic_stash_viewer_native::shell::ShellMsg;
use stash_core::profiles::ServerProfile;

fn profile() -> ServerProfile {
    ServerProfile {
        id: uuid::Uuid::new_v4(),
        display_name: "Testing".into(),
        base_url: "http://localhost:9998/".parse().expect("url"),
        strict_tls: false,
        api_key: None,
        created_at: chrono::Utc::now(),
        last_used_at: None,
    }
}

fn snapshot(state: PlayerStateKind, fullscreen: bool) -> PlayerSnapshot {
    PlayerSnapshot {
        scene_id: Some("7".into()),
        title: Some("Seven".into()),
        state,
        fullscreen,
        paused: state == PlayerStateKind::Paused,
        ..PlayerSnapshot::default()
    }
}

fn observe(s: &mut Session, snap: PlayerSnapshot) -> Vec<Effect> {
    s.update(Msg::Playback(playback::Msg::Snapshot(snap)))
        .effects
}

/// A session showing scene 7 in its first tab, playing.
fn playing() -> Session {
    let p = profile();
    let (mut s, _) = Session::enter(p.clone(), true);
    let _ = s.update(Msg::Reply(Reply::SessionLoaded {
        profile: p.id,
        saved: None,
    }));
    let _ = s.update(Msg::Shell(ShellMsg::Open(Screen::scene("7", "Seven"))));
    let effects = s.update(Msg::Shell(ShellMsg::Play("7".into()))).effects;
    assert!(effects.contains(&Effect::OpenScene("7".into())));
    let _ = observe(&mut s, snapshot(PlayerStateKind::Loading, false));
    let _ = observe(&mut s, snapshot(PlayerStateKind::Playing, false));
    s
}

fn key(s: &mut Session, k: Key) -> Vec<Effect> {
    s.update(Msg::Key(k, Modifiers::empty())).effects
}

#[test]
fn opening_records_the_owner_tab_and_shows_the_player_there() {
    let s = playing();
    assert_eq!(s.playback.owner, Some(s.shell.active().id));
    assert!(s.player_visible());
    assert!(!s.now_playing());
}

#[test]
fn switching_tabs_keeps_playing_and_shows_the_now_playing_bar() {
    let mut s = playing();
    let owner = s.shell.active().id;
    let effects = s.update(Msg::Shell(ShellMsg::NewTab)).effects;
    assert!(!effects
        .iter()
        .any(|e| matches!(e, Effect::Player(PlayerAction::Close))));
    assert!(!s.player_visible());
    assert!(s.now_playing());
    let _ = s.update(Msg::Shell(ShellMsg::Select(owner)));
    assert!(s.player_visible());
}

#[test]
fn leaving_the_scene_in_its_tab_shows_the_bar_too() {
    let mut s = playing();
    let _ = s.update(Msg::Shell(ShellMsg::Back));
    assert!(s.now_playing());
    let _ = s.update(Msg::Shell(ShellMsg::Forward));
    assert!(s.player_visible());
}

#[test]
fn back_to_scene_selects_the_owner_tab() {
    let mut s = playing();
    let owner = s.shell.active().id;
    let _ = s.update(Msg::Shell(ShellMsg::NewTab));
    let _ = s.update(Msg::BackToScene);
    assert_eq!(s.shell.active().id, owner);
    assert!(s.player_visible());
}

#[test]
fn closing_the_owner_tab_closes_the_player() {
    let mut s = playing();
    let owner = s.shell.active().id;
    let _ = s.update(Msg::Shell(ShellMsg::NewTab));
    let effects = s.update(Msg::Shell(ShellMsg::Close(owner))).effects;
    assert!(effects.contains(&Effect::Player(PlayerAction::Close)));
}

#[test]
fn escape_leaves_fullscreen_before_closing() {
    let mut s = playing();
    let _ = observe(&mut s, snapshot(PlayerStateKind::Playing, true));
    assert_eq!(
        key(&mut s, Key::Named(Named::Escape)),
        vec![Effect::SetFullscreen(false)]
    );
    let _ = observe(&mut s, snapshot(PlayerStateKind::Playing, false));
    assert!(key(&mut s, Key::Named(Named::Escape)).contains(&Effect::Player(PlayerAction::Close)));
}

#[test]
fn player_keys_only_reach_a_visible_player() {
    let mut s = playing();
    assert!(
        key(&mut s, Key::Named(Named::Space)).contains(&Effect::Player(PlayerAction::TogglePause))
    );
    let _ = s.update(Msg::Shell(ShellMsg::NewTab));
    assert!(
        !key(&mut s, Key::Named(Named::Space)).contains(&Effect::Player(PlayerAction::TogglePause))
    );
}

#[test]
fn the_end_offers_replay() {
    let mut s = playing();
    let _ = observe(&mut s, snapshot(PlayerStateKind::Ended, false));
    assert!(s.player_visible(), "the last frame stays up");
    let effects = s
        .update(Msg::Playback(playback::Msg::Controls(Controls::Replay)))
        .effects;
    assert!(effects.contains(&Effect::Player(PlayerAction::Replay)));
}

#[test]
fn a_failure_is_shown_and_posted() {
    let mut s = playing();
    let mut failed = snapshot(PlayerStateKind::Error, false);
    failed.error = Some(PlayerError::PlaybackFailed {
        detail: "the file can't be decoded".into(),
    });
    let effects = observe(&mut s, failed.clone());
    assert!(
        effects.iter().any(|e| matches!(e, Effect::Notify { .. })),
        "{effects:?}"
    );
    // The same failure isn't posted twice.
    assert!(!observe(&mut s, failed)
        .iter()
        .any(|e| matches!(e, Effect::Notify { .. })));
}

#[test]
fn leaving_the_session_closes_playback() {
    let s = playing();
    assert!(s.leave().contains(&Effect::Player(PlayerAction::Close)));
}
