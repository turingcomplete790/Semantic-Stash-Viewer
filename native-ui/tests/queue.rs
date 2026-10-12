//! 010: the queue view's transitions (spec US1–US3; data model "QueueState"; contracts/queue-ui.md).

use iced::keyboard::{Key, Modifiers};
use semantic_stash_viewer_native::effects::{Effect, Reply};
use semantic_stash_viewer_native::screens::scene::Details;
use semantic_stash_viewer_native::screens::scenes::keyboard::GridAction;
use semantic_stash_viewer_native::screens::scenes::layout::Layout;
use semantic_stash_viewer_native::screens::scenes::queue::QUEUE_REST;
use semantic_stash_viewer_native::screens::scenes::{ScenesMsg, ScenesState, ScenesUp, Thumbs};
use semantic_stash_viewer_native::screens::{Context, Mode, Screen, Section};
use semantic_stash_viewer_native::session::connection::Connection;
use semantic_stash_viewer_native::session::{Msg, Session};
use semantic_stash_viewer_native::shell::ShellMsg;
use stash_core::playback::playlist::Repeat;
use stash_core::profiles::ServerProfile;
use stash_core::scenes::query::SceneSort;
use stash_core::scenes::{SceneCard, ScenePage};
use stash_core::AppError;

const TAB: u64 = 1;

fn layout() -> Layout {
    Layout::new(1400.0, 900.0)
}

fn page(p: u32, size: u32, count: u32) -> ScenePage {
    let first = (p - 1) * size;
    ScenePage {
        count,
        page: p,
        page_size: size,
        items: (first..(first + size).min(count))
            .map(|i| SceneCard {
                id: (i + 1).to_string(),
                title: format!("Scene {}", i + 1),
                date: Some("2026-01-01".into()),
                duration_seconds: Some(90.0),
                resolution: None,
                studio: None,
                thumb: Some(format!("ssv-thumb://localhost/scene/{}?v=1", i + 1)),
                has_preview: false,
            })
            .collect(),
    }
}

fn load(effects: &[Effect]) -> Option<(u64, u32, u32)> {
    effects.iter().find_map(|e| match e {
        Effect::LoadScenesPage {
            generation,
            page,
            size,
            ..
        } => Some((*generation, *page, *size)),
        _ => None,
    })
}

/// The scene ids whose details `effects` ask for.
fn details(effects: &[Effect]) -> Vec<String> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::LoadSceneDetails { id, .. } => Some(id.clone()),
            _ => None,
        })
        .collect()
}

fn rest(effects: &[Effect]) -> Option<u64> {
    effects.iter().find_map(|e| match e {
        Effect::QueueRestDue {
            generation, after, ..
        } => {
            assert_eq!(*after, QUEUE_REST);
            Some(*generation)
        }
        _ => None,
    })
}

/// Answer the page request in `effects`.
fn arrive(s: &mut ScenesState, effects: &[Effect], count: u32) -> Vec<Effect> {
    let (generation, p, size) = load(effects).expect("a page request");
    s.page_loaded(generation, Ok(page(p, size, count)), false)
}

/// Page `p` of a `count`-scene library, 50 per page.
fn ready(p: u32, count: u32) -> ScenesState {
    let mut s = ScenesState::at(p, 50);
    let effects = s.enter(TAB);
    let _ = arrive(&mut s, &effects, count);
    s
}

fn queue(count: u32) -> ScenesState {
    let mut s = ready(1, count);
    let _ = s.update(ScenesMsg::Mode(Mode::Queue), TAB, &layout());
    s
}

fn key_move(s: &mut ScenesState, index: usize) -> Vec<Effect> {
    s.update(ScenesMsg::Grid(GridAction::Move(index)), TAB, &layout())
        .effects
}

fn panel_id(s: &ScenesState) -> Option<&str> {
    s.queue.panel.as_deref().map(|p| p.scene_id.as_str())
}

// US1 (T009)

#[test]
fn choosing_queue_selects_the_first_scene_and_loads_its_details() {
    let mut s = ready(1, 120);
    let step = s.update(ScenesMsg::Mode(Mode::Queue), TAB, &layout());
    assert_eq!(
        details(&step.effects),
        ["1"],
        "one details request, at once"
    );
    assert_eq!(
        step.up,
        Some(ScenesUp::Play("1".into())),
        "011: choosing Queue plays"
    );
    assert_eq!(s.queue.selected, Some(0));
    assert_eq!(s.queue_selected.as_deref(), Some("1"));
    assert_eq!(panel_id(&s), Some("1"));
}

#[test]
fn a_keyboard_selection_shows_at_once_and_loads_after_a_rest() {
    let mut s = queue(120);
    let first = key_move(&mut s, 3);
    assert!(details(&first).is_empty(), "nothing loads while passing");
    let stale = rest(&first).expect("a rest timer");
    let panel = s.queue.panel.as_deref().expect("a panel");
    assert_eq!(
        (panel.scene_id.as_str(), panel.title.as_str()),
        ("4", "Scene 4"),
        "the card's title shows at once"
    );
    let current = rest(&key_move(&mut s, 4)).expect("a rest timer");
    assert!(
        s.queue_rest_due(stale, TAB).0.is_empty(),
        "an older rest does nothing"
    );
    assert_eq!(details(&s.queue_rest_due(current, TAB).0), ["5"]);
    assert_eq!(s.focused, Some(4), "the keyboard's card is the selection");
}

#[test]
fn a_click_loads_at_once() {
    let mut s = queue(120);
    let step = s.update(ScenesMsg::QueueSelect(6), TAB, &layout());
    assert_eq!(details(&step.effects), ["7"]);
    assert!(rest(&step.effects).is_none());
    assert_eq!(
        step.up,
        Some(ScenesUp::Play("7".into())),
        "011: a click plays"
    );
    assert_eq!(s.queue.selected, Some(6));
}

#[test]
fn the_selection_stays_on_the_page() {
    let mut s = queue(20);
    let _ = s.update(ScenesMsg::QueueSelect(999), TAB, &layout());
    assert_eq!(s.queue.selected, Some(19));
    assert_eq!(panel_id(&s), Some("20"));
}

#[test]
fn an_empty_page_has_no_selection_or_panel() {
    let mut s = ScenesState::at(1, 50);
    s.mode = Mode::Queue;
    let effects = s.enter(TAB);
    let _ = arrive(&mut s, &effects, 0);
    assert_eq!(s.queue.selected, None);
    assert!(s.queue.panel.is_none());
}

#[test]
fn a_narrow_window_opens_the_scene_instead() {
    let mut s = queue(120);
    let narrow = Layout::new(500.0, 700.0);
    let step = s.update(ScenesMsg::QueueSelect(2), TAB, &narrow);
    assert!(
        matches!(&step.up, Some(semantic_stash_viewer_native::screens::scenes::ScenesUp::Open { id, new_tab: false, .. }) if id == "3"),
        "{:?}",
        step.up
    );
}

#[test]
fn leaving_queue_drops_the_panel() {
    let mut s = queue(120);
    let _ = s.update(ScenesMsg::QueueSelect(5), TAB, &layout());
    let _ = s.update(ScenesMsg::Mode(Mode::List), TAB, &layout());
    assert!(s.queue.panel.is_none());
    assert!(
        s.update(ScenesMsg::QueueSelect(9), TAB, &layout())
            .effects
            .is_empty(),
        "no queue outside Queue"
    );
    // Coming back selects the remembered scene.
    let effects = s
        .update(ScenesMsg::Mode(Mode::Queue), TAB, &layout())
        .effects;
    assert_eq!(details(&effects), ["6"]);
}

// US1 (T010)

#[test]
fn changing_the_results_selects_the_first_scene() {
    let changes: Vec<ScenesMsg> = vec![
        ScenesMsg::Sort(SceneSort::Title),
        ScenesMsg::ToggleDirection,
        ScenesMsg::Page(2),
        ScenesMsg::PageSize(20),
        ScenesMsg::FilterTo(stash_core::scenes::filter::Criterion::new(
            stash_core::scenes::filter::kinds::CriterionKind::Organized,
            stash_core::scenes::filter::Modifier::Equals,
            stash_core::scenes::filter::CriterionValue::Bool(true),
        )),
    ];
    for change in changes {
        let label = format!("{change:?}");
        let mut s = queue(500);
        let _ = s.update(ScenesMsg::QueueSelect(7), TAB, &layout());
        let effects = s.update(change, TAB, &layout()).effects;
        assert_eq!(s.queue.selected, None, "{label}: cleared while loading");
        assert_eq!(s.queue_selected, None, "{label}");
        let effects = arrive(&mut s, &effects, 500);
        assert_eq!(s.queue.selected, Some(0), "{label}: the first scene");
        let first = s.cards()[0].id.clone();
        // 011 R3: the new results' first scene loads and plays at once.
        assert_eq!(details(&effects), std::slice::from_ref(&first), "{label}");
        assert_eq!(s.take_play_due(), Some(first), "{label}: it plays");
    }
}

#[test]
fn a_search_selects_the_first_scene() {
    let mut s = queue(500);
    let _ = s.update(ScenesMsg::QueueSelect(7), TAB, &layout());
    let _ = s.update(ScenesMsg::SearchInput("abc".into()), TAB, &layout());
    let effects = s.update(ScenesMsg::SearchSubmit, TAB, &layout()).effects;
    let _ = arrive(&mut s, &effects, 500);
    assert_eq!(s.queue.selected, Some(0));
}

#[test]
fn a_restored_tab_selects_its_saved_scene() {
    let mut s = queue(120);
    let _ = s.update(ScenesMsg::QueueSelect(4), TAB, &layout());
    let json = serde_json::to_string(&s.persistent()).expect("serialise");
    let mut back: ScenesState = serde_json::from_str(&json).expect("parse");
    assert_eq!(back.mode, Mode::Queue);
    let effects = back.enter(TAB);
    let _ = arrive(&mut back, &effects, 120);
    assert_eq!(back.queue.selected, Some(4));
    assert_eq!(panel_id(&back), Some("5"));

    // Not on the page any more: the first.
    let mut gone = back.persistent();
    gone.queue_selected = Some("9999".into());
    gone.focused = None;
    let effects = gone.enter(TAB);
    let _ = arrive(&mut gone, &effects, 120);
    assert_eq!(gone.queue.selected, Some(0));
}

#[test]
fn a_refreshed_page_keeps_the_selection_and_its_panel() {
    let mut s = queue(120);
    let _ = s.update(ScenesMsg::QueueSelect(4), TAB, &layout());
    let effects = s.cache_changed("*", TAB);
    let effects = arrive(&mut s, &effects, 120);
    assert_eq!(s.queue.selected, Some(4));
    assert!(details(&effects).is_empty(), "the panel is still loaded");
    assert!(rest(&effects).is_none());
}

// FR-005, FR-014 (T011)

#[test]
fn sweeping_rows_sends_no_details_and_resting_sends_one() {
    let mut s = queue(120);
    let mut all = Vec::new();
    let mut last = 0;
    for i in 1..=20 {
        let effects = key_move(&mut s, i);
        last = rest(&effects).expect("a rest timer");
        all.extend(effects);
    }
    assert!(
        details(&all).is_empty(),
        "nothing for rows only passed over"
    );
    // Only the last timer is current.
    let (effects, _) = s.queue_rest_due(last, TAB);
    assert_eq!(details(&effects), ["21"]);
    // Choosing it again (a click) asks for nothing more.
    let again = s.update(ScenesMsg::QueueSelect(20), TAB, &layout()).effects;
    assert!(details(&again).is_empty());
    assert!(details(&key_move(&mut s, 20)).is_empty());
}

// The panel in a session: replies reach it, and the list's keys move it.

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

fn session_in_queue() -> Session {
    session_with(500)
}

/// A session showing a `count`-scene library in Queue, 50 per page.
fn session_with(count: u32) -> Session {
    let p = profile();
    let (mut s, _) = Session::enter(p.clone(), true);
    let _ = s.update(Msg::Reply(Reply::SessionLoaded {
        profile: p.id,
        saved: None,
    }));
    s.shell.layout = layout();
    let effects = s
        .update(Msg::Shell(ShellMsg::Section(Section::Scenes)))
        .effects;
    let (generation, p, size) = load(&effects).expect("a page request");
    let tab = s.shell.active().id;
    let _ = s.update(Msg::Reply(Reply::ScenesPage {
        tab,
        generation,
        result: Ok(page(p, size, count)),
    }));
    let _ = s.update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::Mode(Mode::Queue))));
    s
}

fn scenes(s: &Session) -> &ScenesState {
    match s.shell.active().current() {
        Screen::Scenes(state) => state,
        other => panic!("expected Scenes, got {}", other.title()),
    }
}

#[test]
fn the_panels_replies_land_in_it() {
    let mut s = session_in_queue();
    let tab = s.shell.active().id;
    let _ = s.update(Msg::Reply(Reply::SceneDetails {
        tab,
        id: "1".into(),
        result: Box::new(Err(AppError::SceneNotFound { id: "1".into() })),
    }));
    let panel = scenes(&s).queue.panel.as_deref().expect("a panel");
    assert!(
        matches!(&panel.details, Details::Failed(m) if m.contains("no longer exists")),
        "{:?}",
        panel.details
    );
}

#[test]
fn the_lists_arrows_move_the_selection_and_the_rest_loads_it() {
    let mut s = session_in_queue();
    let mut effects = Vec::new();
    for _ in 0..3 {
        effects = s
            .update(Msg::Key(
                Key::Named(iced::keyboard::key::Named::ArrowDown),
                Modifiers::empty(),
            ))
            .effects;
        assert!(details(&effects).is_empty());
    }
    assert_eq!(scenes(&s).queue.selected, Some(3));
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::PreviewDue { .. })),
        "queue rows don't preview"
    );
    let generation = rest(&effects).expect("a rest timer");
    let tab = s.shell.active().id;
    let effects = s
        .update(Msg::Reply(Reply::QueueRestDue { tab, generation }))
        .effects;
    assert_eq!(details(&effects), ["4"]);
}

// T021: clicking a row in the rendered view.

#[test]
fn clicking_a_row_shows_it_in_the_panel() {
    let mut s = session_in_queue();
    let messages: Vec<ShellMsg> = {
        let thumbs = Thumbs::default();
        let connection = Connection::default();
        let ctx = Context {
            profile: &s.profile,
            connection: &connection,
            thumbs: &thumbs,
            library: &s.library,
            layout: layout(),
            playback: Default::default(),
            previews: &Default::default(),
        };
        let mut ui = iced_test::simulator::Simulator::with_size(
            iced::Settings::default(),
            (1400.0, 900.0),
            s.shell.content(&ctx),
        );
        let _ = ui.simulate([iced::Event::Mouse(iced::mouse::Event::CursorMoved {
            position: iced::Point::new(10.0, 10.0),
        })]);
        ui.click("Scene 3").expect("the row is on screen");
        ui.into_messages().collect()
    };
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, ShellMsg::Scenes(ScenesMsg::QueueSelect(2)))),
        "{messages:?}"
    );
    let mut effects = Vec::new();
    for m in messages {
        effects.extend(s.update(Msg::Shell(m)).effects);
    }
    assert_eq!(details(&effects), ["3"]);
    assert_eq!(
        opened(&effects).as_deref(),
        Some("3"),
        "011: the click plays it"
    );
    let state = scenes(&s);
    assert_eq!(state.focused, Some(2), "the row is highlighted");
    assert_eq!(
        state.queue.panel.as_deref().map(|p| p.title.as_str()),
        Some("Scene 3")
    );
    assert_eq!(
        s.shell.active().current().title(),
        "Scenes",
        "still the list"
    );
}

// US2 (T022, T023): playing a run.

use player::{PlayerSnapshot, PlayerStateKind};
use semantic_stash_viewer_native::session::playback;

fn opened(effects: &[Effect]) -> Option<String> {
    effects.iter().find_map(|e| match e {
        Effect::OpenScene { id, .. } => Some(id.clone()),
        _ => None,
    })
}

fn observe(s: &mut Session, id: &str, state: PlayerStateKind) -> Vec<Effect> {
    s.update(Msg::Playback(playback::Msg::Snapshot(PlayerSnapshot {
        scene_id: Some(id.into()),
        title: Some(format!("Scene {id}")),
        state,
        ..PlayerSnapshot::default()
    })))
    .effects
}

/// Scene `id` plays, then reaches its end: what the session does next.
fn plays_to_the_end(s: &mut Session, id: &str) -> Vec<Effect> {
    let _ = observe(s, id, PlayerStateKind::Loading);
    let _ = observe(s, id, PlayerStateKind::Playing);
    observe(s, id, PlayerStateKind::Ended)
}

fn press(s: &mut Session, key: Key, mods: Modifiers) -> Vec<Effect> {
    s.update(Msg::Key(key, mods)).effects
}

fn named(n: iced::keyboard::key::Named) -> Key {
    Key::Named(n)
}

fn ch(c: &str) -> Key {
    Key::Character(c.into())
}

/// Answer the page request in `effects` (500 scenes, 50 a page).
fn answer(s: &mut Session, effects: &[Effect]) -> Vec<Effect> {
    let (generation, p, size) = load(effects).expect("a page request");
    let tab = s.shell.active().id;
    s.update(Msg::Reply(Reply::ScenesPage {
        tab,
        generation,
        result: Ok(page(p, size, 500)),
    }))
    .effects
}

#[test]
fn enter_plays_the_selection_in_the_panel() {
    let mut s = session_in_queue();
    let effects = press(
        &mut s,
        named(iced::keyboard::key::Named::Enter),
        Modifiers::empty(),
    );
    assert_eq!(opened(&effects).as_deref(), Some("1"));
    let _ = observe(&mut s, "1", PlayerStateKind::Playing);
    assert!(s.player_visible(), "the player shows in the panel");
    assert!(!s.now_playing());
    assert_eq!(scenes(&s).queue.playing.as_deref(), Some("1"));
}

#[test]
fn when_a_scene_ends_the_next_plays_and_is_selected() {
    let mut s = session_in_queue();
    let _ = s.update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::QueuePlay(0))));
    let effects = plays_to_the_end(&mut s, "1");
    assert_eq!(opened(&effects).as_deref(), Some("2"));
    assert_eq!(scenes(&s).queue.selected, Some(1));
    assert_eq!(details(&effects), ["2"], "its details load with it");
    assert_eq!(scenes(&s).queue.playing.as_deref(), Some("2"));
}

#[test]
fn after_the_pages_last_scene_the_next_page_plays_from_its_first() {
    let mut s = session_in_queue();
    let _ = s.update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::QueuePlay(49))));
    let effects = plays_to_the_end(&mut s, "50");
    assert_eq!(load(&effects).map(|l| l.1), Some(2), "the next page loads");
    assert!(opened(&effects).is_none());
    let effects = answer(&mut s, &effects);
    assert_eq!(opened(&effects).as_deref(), Some("51"));
    assert_eq!(scenes(&s).queue.selected, Some(0));
}

#[test]
fn repeat_off_stops_at_the_end() {
    let mut s = session_in_queue();
    let effects = s
        .update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::Page(10))))
        .effects;
    let _ = answer(&mut s, &effects);
    let _ = s.update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::QueuePlay(49))));
    let effects = plays_to_the_end(&mut s, "500");
    assert!(opened(&effects).is_none());
    assert!(load(&effects).is_none());
}

#[test]
fn with_autoplay_off_a_scene_stops_at_its_end() {
    let mut s = session_in_queue();
    let _ = s.update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::ToggleAutoplay)));
    assert!(!scenes(&s).autoplay);
    let _ = s.update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::QueuePlay(0))));
    assert!(opened(&plays_to_the_end(&mut s, "1")).is_none());
    // The panel's Next button moves on once.
    let effects = s.update(Msg::Shell(ShellMsg::QueueStep(true))).effects;
    assert_eq!(opened(&effects).as_deref(), Some("2"));
}

#[test]
fn a_failure_never_advances() {
    let mut s = session_in_queue();
    let _ = s.update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::QueuePlay(0))));
    let _ = observe(&mut s, "1", PlayerStateKind::Playing);
    let effects = observe(&mut s, "1", PlayerStateKind::Error);
    assert!(opened(&effects).is_none());
}

#[test]
fn outside_the_queue_view_nothing_advances() {
    let mut s = session_in_queue();
    let _ = s.update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::QueuePlay(0))));
    let _ = s.update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::Mode(Mode::Grid))));
    assert!(opened(&plays_to_the_end(&mut s, "1")).is_none());
}

#[test]
fn next_and_previous_play_the_neighbours() {
    let mut s = session_in_queue();
    // Nothing playing: from the selection.
    let effects = press(&mut s, ch("n"), Modifiers::empty());
    assert_eq!(opened(&effects).as_deref(), Some("2"));
    let _ = observe(&mut s, "2", PlayerStateKind::Playing);
    let effects = press(&mut s, ch("n"), Modifiers::empty());
    assert_eq!(opened(&effects).as_deref(), Some("3"));
    let _ = observe(&mut s, "3", PlayerStateKind::Playing);
    let effects = press(&mut s, ch("p"), Modifiers::empty());
    assert_eq!(opened(&effects).as_deref(), Some("2"));
    let _ = observe(&mut s, "2", PlayerStateKind::Playing);
    let _ = press(&mut s, ch("p"), Modifiers::empty());
    let _ = observe(&mut s, "1", PlayerStateKind::Playing);
    let effects = press(&mut s, ch("p"), Modifiers::empty());
    assert!(
        opened(&effects).is_none() && load(&effects).is_none(),
        "nothing before the first scene"
    );
}

#[test]
fn previous_from_a_pages_first_scene_plays_the_previous_pages_last() {
    let mut s = session_in_queue();
    let effects = press(&mut s, ch("]"), Modifiers::empty());
    let _ = answer(&mut s, &effects);
    assert_eq!(scenes(&s).page, 2);
    let effects = press(&mut s, ch("p"), Modifiers::empty());
    assert_eq!(load(&effects).map(|l| l.1), Some(1));
    let effects = answer(&mut s, &effects);
    assert_eq!(opened(&effects).as_deref(), Some("50"));
    assert_eq!(scenes(&s).queue.selected, Some(49));
}

#[test]
fn a_page_change_plays_the_new_pages_first() {
    let mut s = session_in_queue();
    let _ = s.update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::QueuePlay(49))));
    let _ = plays_to_the_end(&mut s, "50");
    // The user goes elsewhere before Play next's page arrives: their page wins (011 R3).
    let effects = s
        .update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::Page(5))))
        .effects;
    let effects = answer(&mut s, &effects);
    assert_eq!(opened(&effects).as_deref(), Some("201"));
    assert_eq!(scenes(&s).queue.selected, Some(0));
}

#[test]
fn a_deleted_scene_is_skipped() {
    let mut s = session_in_queue();
    let _ = s.update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::QueuePlay(1))));
    let effects = s
        .update(Msg::SceneOpened(Err(AppError::SceneNotFound {
            id: "2".into(),
        })))
        .effects;
    assert_eq!(opened(&effects).as_deref(), Some("3"));
}

#[test]
fn moving_the_selection_keeps_playing_and_marks_the_playing_row() {
    let mut s = session_in_queue();
    let _ = s.update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::QueuePlay(0))));
    let _ = observe(&mut s, "1", PlayerStateKind::Playing);
    let effects = press(
        &mut s,
        named(iced::keyboard::key::Named::ArrowDown),
        Modifiers::empty(),
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::Player(_) | Effect::OpenScene { .. })),
        "the list's arrow moves the selection, not the volume: {effects:?}"
    );
    let state = scenes(&s);
    assert_eq!(state.queue.selected, Some(1));
    assert_eq!(state.queue.playing.as_deref(), Some("1"));
    assert!(s.playback.active(), "still playing");
    assert!(
        s.now_playing(),
        "the panel shows the selection; the bar the playing scene"
    );
}

// US3 (T033): the keyboard alone.

#[test]
fn f6_moves_the_keyboard_to_the_panel_and_back() {
    use semantic_stash_viewer_native::screens::scenes::queue::QueueFocus;
    let mut s = session_in_queue();
    let _ = s.update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::QueuePlay(0))));
    let _ = observe(&mut s, "1", PlayerStateKind::Playing);
    let f6 = named(iced::keyboard::key::Named::F6);
    let _ = press(&mut s, f6.clone(), Modifiers::empty());
    assert_eq!(scenes(&s).queue.focus, QueueFocus::Panel);
    // The arrows now reach the player.
    let effects = press(
        &mut s,
        named(iced::keyboard::key::Named::ArrowDown),
        Modifiers::empty(),
    );
    assert_eq!(scenes(&s).queue.selected, Some(0), "the selection stays");
    assert!(!effects.is_empty(), "the player took it");
    let _ = press(&mut s, f6, Modifiers::empty());
    assert_eq!(scenes(&s).queue.focus, QueueFocus::List);
    let _ = press(
        &mut s,
        named(iced::keyboard::key::Named::ArrowDown),
        Modifiers::empty(),
    );
    assert_eq!(scenes(&s).queue.selected, Some(1));
}

#[test]
fn a_toggles_autoplay_and_o_opens_the_page() {
    let mut s = session_in_queue();
    let _ = press(&mut s, ch("a"), Modifiers::empty());
    assert!(!scenes(&s).autoplay);
    let _ = press(&mut s, ch("a"), Modifiers::empty());
    assert!(scenes(&s).autoplay);
    let _ = press(&mut s, ch("o"), Modifiers::CTRL);
    assert_eq!(s.shell.tabs.len(), 2, "Ctrl+O: a new tab");
    assert_eq!(s.shell.active().current().title(), "Scene 1");
    let queue_tab = s.shell.tabs[0].id;
    let _ = s.update(Msg::Shell(ShellMsg::Select(queue_tab)));
    let _ = press(&mut s, ch("o"), Modifiers::empty());
    assert_eq!(s.shell.active().current().title(), "Scene 1", "in this tab");
}

// T024: where the player shows.

#[test]
fn leaving_the_queue_view_shows_the_now_playing_bar() {
    let mut s = session_in_queue();
    let _ = s.update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::QueuePlay(0))));
    let _ = observe(&mut s, "1", PlayerStateKind::Playing);
    assert!(s.player_visible());
    let _ = s.update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::Mode(Mode::List))));
    assert!(!s.player_visible());
    assert!(s.now_playing());
    // Back to the scene: the queue's selection returns to it, and the player to the panel.
    let _ = s.update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::Mode(Mode::Queue))));
    let _ = press(
        &mut s,
        named(iced::keyboard::key::Named::ArrowDown),
        Modifiers::empty(),
    );
    assert!(s.now_playing());
    let _ = s.update(Msg::BackToScene);
    assert!(s.player_visible(), "the playing scene is selected again");
    assert_eq!(scenes(&s).queue.selected, Some(0));
}

#[test]
fn another_tab_playing_clears_the_queues_mark() {
    let mut s = session_in_queue();
    let _ = s.update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::QueuePlay(0))));
    let _ = observe(&mut s, "1", PlayerStateKind::Playing);
    let _ = s.update(Msg::Shell(ShellMsg::OpenInNewTab(Screen::scene(
        "77", "Other",
    ))));
    let _ = s.update(Msg::Shell(ShellMsg::Play("77".into())));
    let queue = match s.shell.tabs[0].current() {
        Screen::Scenes(state) => state,
        _ => panic!("the queue tab"),
    };
    assert_eq!(queue.queue.playing, None);
}

// 011 US1 (T005): a click plays; the keyboard plays where it rests.

fn scenes_msg(s: &mut Session, m: ScenesMsg) -> Vec<Effect> {
    s.update(Msg::Shell(ShellMsg::Scenes(m))).effects
}

/// Answer `effects`' rest timer for the active tab.
fn rest_reply(s: &mut Session, effects: &[Effect]) -> Vec<Effect> {
    let generation = rest(effects).expect("a rest timer");
    let tab = s.shell.active().id;
    s.update(Msg::Reply(Reply::QueueRestDue { tab, generation }))
        .effects
}

#[test]
fn clicking_another_row_switches_playback() {
    let mut s = session_in_queue();
    let _ = observe(&mut s, "1", PlayerStateKind::Playing);
    let effects = scenes_msg(&mut s, ScenesMsg::QueueSelect(3));
    assert_eq!(opened(&effects).as_deref(), Some("4"));
    assert_eq!(scenes(&s).queue.selected, Some(3));
}

#[test]
fn clicking_the_playing_row_doesnt_restart() {
    let mut s = session_in_queue();
    let _ = observe(&mut s, "1", PlayerStateKind::Playing);
    let effects = scenes_msg(&mut s, ScenesMsg::QueueSelect(0));
    assert!(opened(&effects).is_none(), "{effects:?}");
}

#[test]
fn the_keyboard_plays_where_it_rests() {
    let mut s = session_in_queue();
    let _ = observe(&mut s, "1", PlayerStateKind::Playing);
    let mut effects = Vec::new();
    for _ in 0..3 {
        effects = press(
            &mut s,
            named(iced::keyboard::key::Named::ArrowDown),
            Modifiers::empty(),
        );
        assert!(opened(&effects).is_none(), "nothing plays while it moves");
    }
    let effects = rest_reply(&mut s, &effects);
    assert_eq!(opened(&effects).as_deref(), Some("4"));
}

#[test]
fn passing_rows_plays_nothing() {
    let mut s = queue(120);
    for i in 1..=20 {
        let step = s.update(ScenesMsg::Grid(GridAction::Move(i)), TAB, &layout());
        assert!(step.up.is_none(), "row {i}");
        assert!(details(&step.effects).is_empty(), "row {i}");
    }
}

#[test]
fn a_rest_on_the_playing_scene_plays_nothing() {
    let mut s = session_in_queue();
    let _ = observe(&mut s, "1", PlayerStateKind::Playing);
    let down = named(iced::keyboard::key::Named::ArrowDown);
    let up = named(iced::keyboard::key::Named::ArrowUp);
    let _ = press(&mut s, down, Modifiers::empty());
    let effects = press(&mut s, up, Modifiers::empty());
    let effects = rest_reply(&mut s, &effects);
    assert!(opened(&effects).is_none(), "{effects:?}");
}

// 011 US2 (T011): autoplay.

#[test]
fn switching_to_a_queue_tab_plays_when_idle() {
    let mut s = session_in_queue();
    let queue_tab = s.shell.active().id;
    let _ = s.update(Msg::Shell(ShellMsg::NewTab));
    let effects = s.update(Msg::Shell(ShellMsg::Select(queue_tab))).effects;
    assert_eq!(opened(&effects).as_deref(), Some("1"));
}

#[test]
fn switching_to_a_queue_tab_leaves_other_playback_alone() {
    let mut s = session_in_queue();
    let queue_tab = s.shell.active().id;
    let _ = s.update(Msg::Shell(ShellMsg::OpenInNewTab(Screen::scene(
        "77", "Other",
    ))));
    let _ = s.update(Msg::Shell(ShellMsg::Play("77".into())));
    let _ = observe(&mut s, "77", PlayerStateKind::Playing);
    let effects = s.update(Msg::Shell(ShellMsg::Select(queue_tab))).effects;
    assert!(opened(&effects).is_none(), "{effects:?}");
    assert!(s.now_playing(), "the other scene keeps playing in the bar");
}

#[test]
fn returning_to_the_playing_queue_tab_doesnt_restart() {
    let mut s = session_in_queue();
    let queue_tab = s.shell.active().id;
    let _ = observe(&mut s, "1", PlayerStateKind::Playing);
    let _ = s.update(Msg::Shell(ShellMsg::NewTab));
    let effects = s.update(Msg::Shell(ShellMsg::Select(queue_tab))).effects;
    assert!(opened(&effects).is_none(), "{effects:?}");
    assert!(s.player_visible());
}

/// A new session restored from `s`'s saved tabs.
fn restored(s: &Session) -> Session {
    let saved = s.shell.capture();
    let p = profile();
    let (mut r, _) = Session::enter(p.clone(), true);
    r.shell.layout = layout();
    let effects = r
        .update(Msg::Reply(Reply::SessionLoaded {
            profile: p.id,
            saved: Some(saved),
        }))
        .effects;
    if load(&effects).is_some() {
        let effects = answer(&mut r, &effects);
        assert!(opened(&effects).is_none(), "a restore never plays");
    }
    r
}

#[test]
fn a_restored_queue_tab_doesnt_play_until_switched_to() {
    let s = session_in_queue();
    let r = restored(&s);
    assert_eq!(scenes(&r).queue.selected, Some(0));
    assert!(!r.playback.active());
}

#[test]
fn a_switched_to_tab_plays_once_its_page_arrives() {
    let mut s = session_in_queue();
    let queue_tab_index = s.shell.selected;
    let _ = s.update(Msg::Shell(ShellMsg::NewTab));
    let mut r = restored(&s);
    let effects = r
        .update(Msg::Shell(ShellMsg::SelectIndex(queue_tab_index)))
        .effects;
    assert!(opened(&effects).is_none(), "the page isn't here yet");
    let effects = answer(&mut r, &effects);
    assert_eq!(opened(&effects).as_deref(), Some("1"));
}

#[test]
fn a_refresh_never_plays() {
    let mut s = queue(120);
    let _ = s.update(ScenesMsg::QueueSelect(4), TAB, &layout());
    let effects = s.cache_changed("*", TAB);
    let _ = arrive(&mut s, &effects, 120);
    assert_eq!(s.take_play_due(), None);
    assert!(!s.queue.rest_plays);
}

// 011 US3 (T020): Repeat.

/// Repeat: Off → All.
fn repeat_all(s: &mut Session) {
    let _ = scenes_msg(s, ScenesMsg::CycleRepeat);
    assert_eq!(scenes(s).repeat, Repeat::All);
}

#[test]
fn repeat_all_returns_to_page_one() {
    let mut s = session_in_queue();
    repeat_all(&mut s);
    let effects = scenes_msg(&mut s, ScenesMsg::Page(10));
    let _ = answer(&mut s, &effects);
    let _ = scenes_msg(&mut s, ScenesMsg::QueuePlay(49));
    let effects = plays_to_the_end(&mut s, "500");
    assert_eq!(load(&effects).map(|l| l.1), Some(1), "page 1 loads");
    let effects = answer(&mut s, &effects);
    assert_eq!(opened(&effects).as_deref(), Some("1"));
    assert_eq!(scenes(&s).page, 1);
}

#[test]
fn a_one_page_queue_with_repeat_all_plays_the_first_again() {
    let mut s = session_with(20);
    repeat_all(&mut s);
    let _ = scenes_msg(&mut s, ScenesMsg::QueuePlay(19));
    let effects = plays_to_the_end(&mut s, "20");
    assert!(load(&effects).is_none(), "no page request");
    assert_eq!(opened(&effects).as_deref(), Some("1"));
}

#[test]
fn a_one_scene_queue_with_repeat_all_replays_it() {
    let mut s = session_with(1);
    repeat_all(&mut s);
    let _ = scenes_msg(&mut s, ScenesMsg::QueuePlay(0));
    let effects = plays_to_the_end(&mut s, "1");
    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::Player(semantic_stash_viewer_native::effects::PlayerAction::Replay)
        )),
        "it plays again from 0:00: {effects:?}"
    );
}

#[test]
fn previous_never_wraps() {
    let mut s = session_in_queue();
    let _ = observe(&mut s, "1", PlayerStateKind::Playing);
    let effects = press(&mut s, ch("p"), Modifiers::empty());
    assert!(opened(&effects).is_none() && load(&effects).is_none());
}

#[test]
fn n_on_the_last_scene_with_repeat_all_returns_to_page_one() {
    let mut s = session_in_queue();
    repeat_all(&mut s);
    let effects = scenes_msg(&mut s, ScenesMsg::Page(10));
    let _ = answer(&mut s, &effects);
    let _ = scenes_msg(&mut s, ScenesMsg::QueuePlay(49));
    let _ = observe(&mut s, "500", PlayerStateKind::Playing);
    let effects = press(&mut s, ch("n"), Modifiers::empty());
    assert_eq!(load(&effects).map(|l| l.1), Some(1));
}

#[test]
fn a_failure_never_repeats() {
    let mut s = session_with(20);
    repeat_all(&mut s);
    let _ = scenes_msg(&mut s, ScenesMsg::QueuePlay(19));
    let _ = observe(&mut s, "20", PlayerStateKind::Playing);
    let effects = observe(&mut s, "20", PlayerStateKind::Error);
    assert!(opened(&effects).is_none());
}

#[test]
fn deleted_scenes_stop_after_one_round() {
    let mut s = session_with(3);
    repeat_all(&mut s);
    let _ = scenes_msg(&mut s, ScenesMsg::QueuePlay(0));
    let mut id = "1".to_owned();
    let mut opens = 0;
    for _ in 0..5 {
        let effects = s
            .update(Msg::SceneOpened(Err(AppError::SceneNotFound {
                id: id.clone(),
            })))
            .effects;
        match opened(&effects) {
            Some(next) => {
                opens += 1;
                id = next;
            }
            None => break,
        }
    }
    // 1 was deleted: 2, then 3; 3 deleted too makes three in a row, the whole result: it stops.
    assert_eq!(opens, 2, "one round of the three scenes, then it stops");
}

#[test]
fn repeat_one_replays_from_the_start() {
    for autoplay in [true, false] {
        let mut s = session_in_queue();
        if !autoplay {
            let _ = scenes_msg(&mut s, ScenesMsg::ToggleAutoplay);
        }
        let _ = scenes_msg(&mut s, ScenesMsg::CycleRepeat);
        let _ = scenes_msg(&mut s, ScenesMsg::CycleRepeat);
        assert_eq!(scenes(&s).repeat, Repeat::One);
        let _ = scenes_msg(&mut s, ScenesMsg::QueuePlay(3));
        let effects = plays_to_the_end(&mut s, "4");
        // The player's own replay from 0:00 (not a reopen of the file it holds at its end).
        assert!(
            effects.iter().any(|e| matches!(
                e,
                Effect::Player(semantic_stash_viewer_native::effects::PlayerAction::Replay)
            )),
            "autoplay {autoplay}: replayed from 0:00, {effects:?}"
        );
        assert!(opened(&effects).is_none(), "no reopen");
        assert_eq!(scenes(&s).queue.selected, Some(3), "it stays selected");
    }
}

#[test]
fn next_leaves_repeat_one_on() {
    let mut s = session_in_queue();
    let _ = scenes_msg(&mut s, ScenesMsg::CycleRepeat);
    let _ = scenes_msg(&mut s, ScenesMsg::CycleRepeat);
    let _ = observe(&mut s, "1", PlayerStateKind::Playing);
    let effects = press(&mut s, ch("n"), Modifiers::empty());
    assert_eq!(opened(&effects).as_deref(), Some("2"));
    assert_eq!(scenes(&s).repeat, Repeat::One);
}

#[test]
fn r_cycles_repeat() {
    let mut s = session_in_queue();
    let mut seen = vec![scenes(&s).repeat];
    for _ in 0..3 {
        let _ = press(&mut s, ch("r"), Modifiers::empty());
        seen.push(scenes(&s).repeat);
    }
    assert_eq!(seen, [Repeat::Off, Repeat::All, Repeat::One, Repeat::Off]);
}

// 011 US1 (T015).

#[test]
fn a_click_plays_with_autoplay_off() {
    let mut s = session_in_queue();
    let _ = scenes_msg(&mut s, ScenesMsg::ToggleAutoplay);
    let _ = observe(&mut s, "1", PlayerStateKind::Playing);
    let effects = scenes_msg(&mut s, ScenesMsg::QueueSelect(3));
    assert_eq!(opened(&effects).as_deref(), Some("4"));
}

#[test]
fn a_closed_players_scene_plays_again_when_chosen() {
    let mut s = session_in_queue();
    let _ = observe(&mut s, "1", PlayerStateKind::Playing);
    let _ = s.update(Msg::Playback(playback::Msg::Close));
    let _ = observe(&mut s, "1", PlayerStateKind::Idle);
    assert!(!s.playback.active());
    let effects = scenes_msg(&mut s, ScenesMsg::QueueSelect(0));
    assert_eq!(
        opened(&effects).as_deref(),
        Some("1"),
        "nothing plays, so it plays"
    );
}

// 011 US2 (T017): Autoplay.

#[test]
fn autostart_respects_autoplay_off() {
    // Choosing Queue.
    let mut s = ready(1, 120);
    s.autoplay = false;
    let step = s.update(ScenesMsg::Mode(Mode::Queue), TAB, &layout());
    assert_eq!(step.up, None);
    assert_eq!(s.queue.selected, Some(0), "it's still selected");

    // Changing the results.
    let effects = s
        .update(ScenesMsg::Sort(SceneSort::Title), TAB, &layout())
        .effects;
    let _ = arrive(&mut s, &effects, 120);
    assert_eq!(s.queue.selected, Some(0));
    assert_eq!(s.take_play_due(), None);

    // Switching to the tab while nothing plays.
    let mut s = session_in_queue();
    let _ = scenes_msg(&mut s, ScenesMsg::ToggleAutoplay);
    let queue_tab = s.shell.active().id;
    let _ = s.update(Msg::Shell(ShellMsg::NewTab));
    let effects = s.update(Msg::Shell(ShellMsg::Select(queue_tab))).effects;
    assert!(opened(&effects).is_none(), "{effects:?}");

    // A pending start when the page arrives.
    let queue_tab_index = s.shell.selected;
    let _ = s.update(Msg::Shell(ShellMsg::NewTab));
    let mut r = restored(&s);
    let effects = r
        .update(Msg::Shell(ShellMsg::SelectIndex(queue_tab_index)))
        .effects;
    let effects = answer(&mut r, &effects);
    assert!(opened(&effects).is_none(), "{effects:?}");
    assert!(!scenes(&r).autoplay, "Autoplay off was restored");
}

#[test]
fn with_autoplay_on_new_results_play_their_first() {
    let mut s = session_in_queue();
    let _ = observe(&mut s, "1", PlayerStateKind::Playing);
    let _ = scenes_msg(&mut s, ScenesMsg::QueueSelect(5));
    let _ = observe(&mut s, "6", PlayerStateKind::Playing);
    let effects = scenes_msg(&mut s, ScenesMsg::ToggleDirection);
    let effects = answer(&mut s, &effects);
    assert_eq!(opened(&effects).as_deref(), Some("1"));
}

// 011 US3 (T021): Shuffle.

/// Press `n` (answering a page request) and report the scene that opened, now playing.
fn next_scene(s: &mut Session) -> Option<String> {
    let mut effects = press(s, ch("n"), Modifiers::empty());
    if load(&effects).is_some() {
        effects = answer(s, &effects);
    }
    let id = opened(&effects)?;
    let _ = observe(s, &id, PlayerStateKind::Playing);
    Some(id)
}

/// The scene's page and selection follow it.
fn shows(s: &Session, id: &str) {
    let n: u32 = id.parse().expect("a number");
    let state = scenes(s);
    assert_eq!(state.page, (n - 1) / 50 + 1, "scene {id}'s page");
    let selected = state.queue.selected.and_then(|i| state.cards().get(i));
    assert_eq!(
        selected.map(|c| c.id.as_str()),
        Some(id),
        "scene {id} selected"
    );
}

#[test]
fn x_toggles_shuffle() {
    let mut s = session_in_queue();
    assert!(!scenes(&s).shuffle);
    let _ = press(&mut s, ch("x"), Modifiers::empty());
    assert!(scenes(&s).shuffle);
    let _ = press(&mut s, ch("x"), Modifiers::empty());
    assert!(!scenes(&s).shuffle);
}

#[test]
fn shuffle_plays_each_scene_once() {
    let mut s = session_in_queue();
    let _ = observe(&mut s, "1", PlayerStateKind::Playing);
    let _ = press(&mut s, ch("x"), Modifiers::empty());
    let mut seen = std::collections::HashSet::from(["1".to_owned()]);
    let mut pages = std::collections::HashSet::new();
    for _ in 1..500 {
        let id = next_scene(&mut s).expect("a scene not yet played");
        shows(&s, &id);
        pages.insert(scenes(&s).page);
        assert!(seen.insert(id.clone()), "{id} repeated within the round");
    }
    assert_eq!(seen.len(), 500);
    assert!(pages.len() > 1, "picks come from every page");
    assert_eq!(next_scene(&mut s), None, "Repeat off: the round ends");
}

#[test]
fn previous_retraces_the_round() {
    let mut s = session_in_queue();
    let _ = observe(&mut s, "1", PlayerStateKind::Playing);
    let _ = press(&mut s, ch("x"), Modifiers::empty());
    let mut played = vec!["1".to_owned()];
    for _ in 0..5 {
        played.push(next_scene(&mut s).expect("a pick"));
    }
    for want in played.iter().rev().skip(1) {
        let mut effects = press(&mut s, ch("p"), Modifiers::empty());
        if load(&effects).is_some() {
            effects = answer(&mut s, &effects);
        }
        assert_eq!(opened(&effects).as_ref(), Some(want));
        let _ = observe(&mut s, want, PlayerStateKind::Playing);
        shows(&s, want);
    }
    let effects = press(&mut s, ch("p"), Modifiers::empty());
    assert!(opened(&effects).is_none(), "the round's first");
}

#[test]
fn turning_shuffle_off_continues_in_list_order() {
    let mut s = session_in_queue();
    let _ = observe(&mut s, "1", PlayerStateKind::Playing);
    let _ = press(&mut s, ch("x"), Modifiers::empty());
    let id = next_scene(&mut s).expect("a pick");
    let _ = press(&mut s, ch("x"), Modifiers::empty());
    let n: u32 = id.parse().expect("a number");
    let next = next_scene(&mut s);
    if n < 500 {
        assert_eq!(next, Some((n + 1).to_string()));
    } else {
        assert_eq!(next, None);
    }
}

#[test]
fn new_results_start_a_new_round() {
    let mut s = session_in_queue();
    let _ = observe(&mut s, "1", PlayerStateKind::Playing);
    let _ = press(&mut s, ch("x"), Modifiers::empty());
    let before = scenes(&s).playlist.round.expect("a round");
    let effects = scenes_msg(&mut s, ScenesMsg::Sort(SceneSort::Title));
    let _ = answer(&mut s, &effects);
    let after = scenes(&s).playlist.round.expect("a round");
    assert_ne!(after.seed, before.seed, "a new round");
    // Scene 1 plays on (it's still first): the round starts from it.
    assert_eq!((after.first, after.pos), (0, 0), "from the new first scene");
}

#[test]
fn choosing_a_scene_while_shuffling_starts_from_it() {
    let mut s = session_in_queue();
    let _ = observe(&mut s, "1", PlayerStateKind::Playing);
    let _ = press(&mut s, ch("x"), Modifiers::empty());
    let effects = scenes_msg(&mut s, ScenesMsg::QueueSelect(7));
    assert_eq!(opened(&effects).as_deref(), Some("8"));
    let round = scenes(&s).playlist.round.expect("a round");
    assert_eq!((round.first, round.pos), (7, 0));
}
