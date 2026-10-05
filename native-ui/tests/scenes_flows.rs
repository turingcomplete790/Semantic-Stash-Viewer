//! 007 T035: Scenes flows with fixture pages (spec US3; B1): `]` and `[` change page, and opening
//! a card then going back returns the exact page, mode, and scroll.

use iced::keyboard::{Key, Modifiers};
use iced_test::simulator;
use semantic_stash_viewer_native::effects::{Effect, Reply};
use semantic_stash_viewer_native::screens::scenes::{ScenesMsg, Thumbs};
use semantic_stash_viewer_native::screens::{Context, Mode, Screen, Section};
use semantic_stash_viewer_native::session::connection::Connection;
use semantic_stash_viewer_native::session::{Msg, Session};
use semantic_stash_viewer_native::shell::ShellMsg;
use stash_core::profiles::ServerProfile;
use stash_core::scenes::{SceneCard, ScenePage};

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

fn fixture(page: u32, count: u32) -> ScenePage {
    let first = (page - 1) * 50;
    ScenePage {
        count,
        page,
        page_size: 50,
        items: (first..(first + 50).min(count))
            .map(|i| SceneCard {
                id: (i + 1).to_string(),
                title: format!("Scene {}", i + 1),
                date: Some("2026-01-01".into()),
                duration_seconds: Some(90.0),
                resolution: Some("1920×1080".into()),
                studio: None,
                thumb: Some(format!("ssv-thumb://localhost/scene/{}?v=1", i + 1)),
                has_preview: false,
            })
            .collect(),
    }
}

fn load(effects: &[Effect]) -> Option<(u64, u64, u32)> {
    effects.iter().find_map(|e| match e {
        Effect::LoadScenesPage {
            tab,
            generation,
            page,
            ..
        } => Some((*tab, *generation, *page)),
        _ => None,
    })
}

/// Answer the page request in `effects` with fixture data.
fn answer(session: &mut Session, effects: &[Effect]) -> Vec<Effect> {
    let (tab, generation, page) = load(effects).expect("a page request");
    session
        .update(Msg::Reply(Reply::ScenesPage {
            tab,
            generation,
            result: Ok(fixture(page, 500)),
        }))
        .effects
}

fn session_on_scenes() -> Session {
    let p = profile();
    let (mut s, _) = Session::enter(p.clone(), true);
    let _ = s.update(Msg::Reply(Reply::SessionLoaded {
        profile: p.id,
        saved: None,
    }));
    let effects = s
        .update(Msg::Shell(ShellMsg::Section(Section::Scenes)))
        .effects;
    let _ = answer(&mut s, &effects);
    s
}

fn scenes(s: &Session) -> &semantic_stash_viewer_native::screens::scenes::ScenesState {
    match s.shell.active().current() {
        Screen::Scenes(state) => state,
        other => panic!("expected Scenes, got {}", other.title()),
    }
}

#[test]
fn brackets_change_page() {
    let mut s = session_on_scenes();
    let effects = s
        .update(Msg::Key(Key::Character("]".into()), Modifiers::empty()))
        .effects;
    assert_eq!(load(&effects).map(|l| l.2), Some(2));
    let _ = answer(&mut s, &effects);
    assert_eq!(scenes(&s).page, 2);
    let effects = s
        .update(Msg::Key(Key::Character("[".into()), Modifiers::empty()))
        .effects;
    assert_eq!(load(&effects).map(|l| l.2), Some(1));
}

#[test]
fn cards_request_their_thumbnails_once() {
    let mut s = session_on_scenes();
    let effects = s
        .update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::Scrolled {
            y: 10.0,
        })))
        .effects;
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::LoadThumbnails { .. })),
        "already requested when the page arrived"
    );
}

#[test]
fn opening_a_card_and_going_back_returns_the_exact_place() {
    let mut s = session_on_scenes();
    let _ = s.update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::Mode(Mode::List))));
    let _ = s.update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::Scrolled {
        y: 160.0,
    })));

    // Click the third card in the rendered list.
    let messages: Vec<ShellMsg> = {
        let thumbs = Thumbs::default();
        let connection = Connection::default();
        let ctx = Context {
            profile: &s.profile,
            connection: &connection,
            thumbs: &thumbs,
            layout: semantic_stash_viewer_native::screens::scenes::layout::Layout::new(
                1100.0, 2400.0,
            ),
        };
        let mut ui = simulator(s.shell.content(&ctx));
        ui.click("Scene 3").expect("the card is on screen");
        ui.into_messages().collect()
    };
    for m in messages {
        let _ = s.update(Msg::Shell(m));
    }
    assert_eq!(s.shell.active().current().title(), "Scene 3");

    let effects = s.update(Msg::Shell(ShellMsg::Back)).effects;
    let state = scenes(&s);
    assert_eq!(
        (state.page, state.mode, state.scroll),
        (1, Mode::List, 160.0)
    );
    assert!(load(&effects).is_none(), "the page is still there");
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::ScrollTo { y, .. } if *y == 160.0)),
        "the scroll position comes back: {effects:?}"
    );
}

#[test]
fn ctrl_click_opens_in_a_new_tab() {
    let mut s = session_on_scenes();
    let _ = s.update(Msg::Modifiers(Modifiers::CTRL));
    let _ = s.update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::Open {
        index: 0,
        new_tab: false,
    })));
    assert_eq!(s.shell.tabs.len(), 2);
    assert_eq!(s.shell.active().current().title(), "Scene 1");
    assert_eq!(
        s.shell.tabs[0].current().title(),
        "Scenes",
        "the grid stays in its tab"
    );
}
