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
    assert!(effects.contains(&Effect::OpenScene {
        id: "7".into(),
        start: None,
        resume: true,
    }));
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
    let mut s = playing();
    assert!(s.leave().contains(&Effect::Player(PlayerAction::Close)));
}

#[test]
fn opening_at_a_position_carries_the_start_to_the_player() {
    let mut p = playback::Playback::default();
    assert_eq!(
        p.open_at("7".into(), 1, Some(90.0)),
        vec![Effect::OpenScene {
            id: "7".into(),
            start: Some(90.0),
            resume: false,
        }],
        "a marker plays from its point, not the resume point"
    );
    assert_eq!(
        p.open("8".into(), 1),
        vec![Effect::OpenScene {
            id: "8".into(),
            start: None,
            resume: true,
        }],
        "a plain Play resumes when resume is on"
    );
    p.resume = false;
    assert_eq!(
        p.open("9".into(), 1),
        vec![Effect::OpenScene {
            id: "9".into(),
            start: None,
            resume: false,
        }],
        "resume off starts at the beginning"
    );
}

// ---- 008 US3 (T060): resume, viewings, chapters, play history ------------------------------

fn history_reports(effects: &[Effect]) -> Vec<&stash_core::playback::history::PlayHistoryReport> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::SavePlayHistory { report, .. } => Some(report),
            _ => None,
        })
        .collect()
}

/// `playing()`, with a moment of playing time on the clock.
fn watched() -> Session {
    let mut s = playing();
    std::thread::sleep(std::time::Duration::from_millis(5));
    let _ = observe(&mut s, snapshot(PlayerStateKind::Playing, false));
    s
}

#[test]
fn a_resumed_scene_offers_to_start_from_the_beginning() {
    let mut s = playing();
    assert!(!s.playback.screen.restart_offered());
    let _ = s.update(Msg::SceneOpened(Ok(Some(120.0))));
    assert!(s.playback.screen.restart_offered(), "shown after a resume");
    assert!(s.playback.screen.needs_tick(), "a timer hides it later");
    let effects = s.update(Msg::Playback(playback::Msg::Controls(Controls::StartOver)));
    assert_eq!(
        effects.effects,
        vec![Effect::Player(PlayerAction::Seek {
            seconds: 0.0,
            exact: true
        })]
    );
    assert!(!s.playback.screen.restart_offered(), "gone once used");

    // A scene opened without a resume point offers nothing.
    let mut s = playing();
    let _ = s.update(Msg::SceneOpened(Ok(None)));
    assert!(!s.playback.screen.restart_offered());
}

#[test]
fn stopping_ends_the_viewing_with_one_final_report() {
    let mut s = watched();
    let effects = observe(&mut s, PlayerSnapshot::default());
    let reports = history_reports(&effects);
    assert_eq!(reports.len(), 1, "{effects:?}");
    assert!(reports[0].ended);
    assert!(
        reports[0].add_play,
        "minimumPlayPercent unset: the first report counts the play"
    );
    assert!(history_reports(&observe(&mut s, PlayerSnapshot::default())).is_empty());
}

#[test]
fn another_scene_ends_the_viewing_first() {
    let mut s = watched();
    let effects = s.update(Msg::Shell(ShellMsg::Play("8".into()))).effects;
    let reports = history_reports(&effects);
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].scene_id, "7");
    let open = effects
        .iter()
        .position(|e| matches!(e, Effect::OpenScene { .. }))
        .expect("opens 8");
    let save = effects
        .iter()
        .position(|e| matches!(e, Effect::SavePlayHistory { .. }))
        .expect("saves 7");
    assert!(
        save < open,
        "the old viewing is saved before the new scene opens"
    );
}

#[test]
fn leaving_the_session_sends_the_final_report_with_the_close() {
    // Quit and server switch both leave the session.
    let mut s = watched();
    let effects = s.leave();
    assert_eq!(history_reports(&effects).len(), 1);
    assert!(effects.contains(&Effect::Player(PlayerAction::Close)));
    // The close's idle snapshot doesn't send a second one.
    assert!(history_reports(&observe(&mut s, PlayerSnapshot::default())).is_empty());
}

#[test]
fn a_failure_or_a_lost_stream_ends_the_viewing() {
    let mut s = watched();
    let mut failed = snapshot(PlayerStateKind::Error, false);
    failed.error = Some(PlayerError::StreamUnreachable);
    assert_eq!(history_reports(&observe(&mut s, failed)).len(), 1);
}

#[test]
fn closing_the_owner_tab_ends_the_viewing() {
    let mut s = watched();
    let owner = s.shell.active().id;
    let _ = s.update(Msg::Shell(ShellMsg::NewTab));
    let effects = s.update(Msg::Shell(ShellMsg::Close(owner))).effects;
    assert!(effects.contains(&Effect::Player(PlayerAction::Close)));
    assert_eq!(
        history_reports(&observe(&mut s, PlayerSnapshot::default())).len(),
        1
    );
}

#[test]
fn with_history_off_nothing_is_reported() {
    let mut s = playing();
    let _ = s.update(Msg::Reply(Reply::PlaybackPreferences(
        semantic_stash_viewer_native::services::settings::PlaybackPreferences {
            resume: true,
            save_history: false,
            ..Default::default()
        },
    )));
    let _ = s.update(Msg::Shell(ShellMsg::Play("7".into())));
    let _ = observe(&mut s, snapshot(PlayerStateKind::Playing, false));
    std::thread::sleep(std::time::Duration::from_millis(5));
    let _ = observe(&mut s, snapshot(PlayerStateKind::Playing, false));
    assert!(history_reports(&observe(&mut s, PlayerSnapshot::default())).is_empty());
}

#[test]
fn chapter_keys_move_between_markers() {
    let mut s = playing();
    s.playback.set_chapters(vec![
        (10.0, "Start".into()),
        (60.0, "Middle".into()),
        (120.0, "End".into()),
    ]);
    let mut at = snapshot(PlayerStateKind::Playing, false);
    at.position_seconds = 30.0;
    at.duration_seconds = Some(200.0);
    let _ = observe(&mut s, at);
    let seeks = |effects: Vec<Effect>| -> Vec<f64> {
        effects
            .into_iter()
            .filter_map(|e| match e {
                Effect::Player(PlayerAction::Seek { seconds, .. }) => Some(seconds),
                _ => None,
            })
            .collect()
    };
    assert_eq!(seeks(key(&mut s, Key::Named(Named::PageDown))), vec![60.0]);
    // The seek target counts as the position until mpv gets there: back goes to 10 (Start).
    assert_eq!(seeks(key(&mut s, Key::Named(Named::PageUp))), vec![10.0]);
}

#[test]
fn repeated_send_failures_post_one_notification_per_server() {
    use stash_core::connection::snapshot::{ConnectionSnapshot, SessionState};
    use stash_core::playback::history::FlushOutcome;
    let mut s = playing();
    let _ = s.update(Msg::Connection(ConnectionSnapshot {
        state: SessionState::Connected,
        ..ConnectionSnapshot::default()
    }));
    let profile = s.profile.id;
    let failed = || {
        Msg::Reply(Reply::PlayHistoryFlushed {
            profile,
            outcome: FlushOutcome {
                sent: 0,
                failed: Some("server error".into()),
                left: 2,
            },
        })
    };
    let mut posted = 0;
    for _ in 0..6 {
        posted += s
            .update(failed())
            .effects
            .iter()
            .filter(|e| matches!(e, Effect::PlayHistoryUnsaved { .. }))
            .count();
    }
    assert_eq!(posted, 1, "one entry per server, after the third failure");
}

#[test]
fn reconnecting_sends_waiting_play_history() {
    use stash_core::connection::snapshot::{ConnectionSnapshot, SessionState};
    let mut s = playing();
    let effects = s
        .update(Msg::Connection(ConnectionSnapshot {
            state: SessionState::Connected,
            ..ConnectionSnapshot::default()
        }))
        .effects;
    assert!(effects.contains(&Effect::FlushPlayHistory {
        profile: s.profile.id,
        deadline: None
    }));
}

// ---- 008 US5 (T083): a visible fallback when a file can't play ----------------------------

fn failed(error: PlayerError, transcoded: bool) -> PlayerSnapshot {
    let mut s = snapshot(PlayerStateKind::Error, false);
    s.error = Some(error);
    s.transcoded = transcoded;
    s
}

fn transcode(s: &mut Session) -> Vec<Effect> {
    s.update(Msg::Playback(playback::Msg::Controls(
        Controls::PlayTranscoded,
    )))
    .effects
}

#[test]
fn a_decode_failure_offers_the_transcode_and_a_network_failure_doesnt() {
    let mut s = playing();
    let _ = observe(&mut s, failed(PlayerError::StreamUnreachable, false));
    assert!(!s.playback.screen.offers_transcode());
    assert!(transcode(&mut s).is_empty(), "nothing to choose");

    let mut s = playing();
    let _ = observe(&mut s, failed(PlayerError::UnsupportedFormat, false));
    assert!(s.playback.screen.offers_transcode());
}

#[test]
fn choosing_the_transcode_opens_it_with_its_reason() {
    let mut s = playing();
    let _ = observe(&mut s, failed(PlayerError::UnsupportedFormat, false));
    let effects = transcode(&mut s);
    assert_eq!(
        effects,
        vec![Effect::OpenTranscoded {
            id: "7".into(),
            reason: PlayerError::UnsupportedFormat.to_string(),
        }]
    );
}

#[test]
fn a_transcode_that_fails_too_shows_both_reasons_and_isnt_retried() {
    let mut s = playing();
    let _ = observe(&mut s, failed(PlayerError::UnsupportedFormat, false));
    let _ = transcode(&mut s);
    let mut loading = snapshot(PlayerStateKind::Loading, false);
    loading.transcoded = true;
    let _ = observe(&mut s, loading);
    let _ = observe(
        &mut s,
        failed(
            PlayerError::PlaybackFailed {
                detail: "server error".into(),
            },
            true,
        ),
    );
    assert!(!s.playback.screen.offers_transcode(), "no retry");
    assert!(transcode(&mut s).is_empty());
    assert!(
        s.playback.screen.direct_failure.is_some(),
        "the first reason is kept"
    );
}

#[test]
fn a_normal_scene_never_opens_a_transcode() {
    let mut s = playing();
    let effects = s.update(Msg::Shell(ShellMsg::Play("8".into()))).effects;
    assert!(effects
        .iter()
        .all(|e| !matches!(e, Effect::OpenTranscoded { .. })));
    assert!(effects
        .iter()
        .any(|e| matches!(e, Effect::OpenScene { id, .. } if id == "8")));
}
