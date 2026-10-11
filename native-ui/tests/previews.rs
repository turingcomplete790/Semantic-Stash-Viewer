//! 008 T073: card previews and the wall (spec FR-019, FR-020; research R8).

use std::time::{Duration, Instant};

use iced::widget::image::Handle;
use semantic_stash_viewer_native::effects::Effect;
use semantic_stash_viewer_native::screens::scenes::layout::Layout;
use semantic_stash_viewer_native::screens::scenes::preview::{
    Frames, PreviewState, Previews, PREVIEW_REST,
};
use semantic_stash_viewer_native::screens::scenes::ScenesState;
use semantic_stash_viewer_native::screens::Mode;

fn frames(n: usize, ms: u64) -> Frames {
    (0..n)
        .map(|i| {
            (
                Handle::from_rgba(1, 1, vec![i as u8, 0, 0, 255]),
                Duration::from_millis(ms),
            )
        })
        .collect()
}

/// The rest timer the effects start: its generation.
fn due(effects: &[Effect]) -> u64 {
    let timers: Vec<u64> = effects
        .iter()
        .filter_map(|e| match e {
            Effect::PreviewDue { generation, after } => {
                assert_eq!(*after, PREVIEW_REST);
                assert_eq!(PREVIEW_REST, Duration::from_millis(200));
                Some(*generation)
            }
            Effect::StopPreview => None,
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    match timers[..] {
        [g] => g,
        _ => panic!("one rest timer: {effects:?}"),
    }
}

#[test]
fn resting_on_a_card_loads_then_plays_its_preview_and_leaving_drops_it() {
    let mut p = Previews::default();
    let g = due(&p.enter("7".into(), true));
    assert!(matches!(p.state, PreviewState::Waiting { .. }));
    assert!(!p.playing(), "no frame timer while waiting");
    assert_eq!(
        p.due(g, 320, Instant::now()),
        vec![Effect::LoadPreview {
            scene_id: "7".into(),
            generation: g,
            width: 320
        }]
    );
    assert!(matches!(p.state, PreviewState::Loading { .. }));
    let now = Instant::now();
    p.frames(g, frames(3, 100), true, now);
    assert!(p.playing(), "the frame timer runs while it plays");
    assert!(p.frame_for("7").is_some(), "the frame replaces the still");
    assert!(p.frame_for("8").is_none());

    assert_eq!(p.left("7"), vec![Effect::StopPreview], "its decoding stops");
    assert!(matches!(p.state, PreviewState::Idle), "frames dropped");
    assert!(!p.playing());
}

#[test]
fn passing_over_cards_fetches_nothing() {
    let mut p = Previews::default();
    let a = due(&p.enter("1".into(), true));
    let b = due(&p.enter("2".into(), true));
    let c = due(&p.enter("3".into(), true));
    let _ = p.left("3");
    for g in [a, b, c] {
        assert!(
            p.due(g, 320, Instant::now()).is_empty(),
            "timer {g} came due after leaving"
        );
    }
}

#[test]
fn only_one_preview_exists() {
    let mut p = Previews::default();
    let a = due(&p.enter("1".into(), true));
    let _ = p.due(a, 320, Instant::now());
    p.frames(a, frames(2, 50), true, Instant::now());
    let b = due(&p.enter("2".into(), true));
    assert!(p.frame_for("1").is_none(), "the first one stops");
    // A late result for the first is ignored.
    p.frames(a, frames(2, 50), true, Instant::now());
    assert!(p.frame_for("1").is_none());
    let _ = p.due(b, 320, Instant::now());
    p.frames(b, frames(2, 50), true, Instant::now());
    assert!(p.frame_for("2").is_some());
}

#[test]
fn keyboard_focus_starts_a_preview_the_same_way() {
    let mut p = Previews::default();
    let g = due(&p.focus(Some(("5".into(), true))));
    assert_eq!(p.due(g, 200, Instant::now()).len(), 1);
    // The pointer takes over while it's on a card, and focus comes back after.
    let h = due(&p.enter("6".into(), true));
    assert_eq!(p.due(h, 200, Instant::now()).len(), 1);
    let back = p.left("6");
    assert!(
        back.iter().any(|e| matches!(e, Effect::PreviewDue { .. })),
        "back to the focused card: {back:?}"
    );
}

#[test]
fn a_card_without_a_preview_keeps_its_still_and_asks_nothing() {
    let mut p = Previews::default();
    assert!(p.enter("9".into(), false).is_empty());
    assert!(matches!(p.state, PreviewState::Idle));
    // One Stash says has a preview but serves none: not asked again this session.
    let g = due(&p.enter("10".into(), true));
    let _ = p.due(g, 320, Instant::now());
    p.frames(g, Vec::new(), true, Instant::now());
    let _ = p.left("10");
    assert!(p.enter("10".into(), true).is_empty());
}

#[test]
fn frames_advance_on_their_own_delays() {
    let mut p = Previews::default();
    let g = due(&p.enter("7".into(), true));
    let _ = p.due(g, 320, Instant::now());
    let start = Instant::now();
    p.frames(g, frames(3, 100), true, start);
    assert!(!p.tick(start + Duration::from_millis(50)), "not yet");
    assert!(p.tick(start + Duration::from_millis(100)));
    assert_eq!(p.key().map(|(_, i)| i), Some(1));
    assert!(p.tick(start + Duration::from_millis(200)));
    assert!(p.tick(start + Duration::from_millis(300)));
    assert_eq!(p.key().map(|(_, i)| i), Some(0), "loops");
}

#[test]
fn leaving_the_scenes_screen_clears_everything() {
    let mut p = Previews::default();
    let g = due(&p.enter("7".into(), true));
    let _ = p.due(g, 320, Instant::now());
    p.frames(g, frames(2, 50), true, Instant::now());
    p.clear();
    assert!(!p.playing());
    assert!(p.due(g, 320, Instant::now()).is_empty());
}

#[test]
fn the_wall_is_denser_than_the_grid_and_saved_with_the_tab() {
    let layout = Layout::new(1100.0, 900.0);
    assert!(layout.cols(Mode::Wall) > layout.cols(Mode::Grid));
    assert!(layout.row_height(Mode::Wall) < layout.row_height(Mode::Grid));
    assert!(layout.wall_width() >= 160.0);
    let mut s = ScenesState::at(1, 50);
    s.mode = Mode::Wall;
    let saved = serde_json::to_string(&s.persistent()).expect("save");
    let back: ScenesState = serde_json::from_str(&saved).expect("restore");
    assert_eq!(back.mode, Mode::Wall);
}

#[test]
fn arrows_move_through_the_wall_by_its_columns() {
    use iced::keyboard::key::Named;
    use iced::keyboard::Key;
    use semantic_stash_viewer_native::screens::scenes::keyboard::{grid_key, GridAction, GridKeys};
    let layout = Layout::new(1100.0, 900.0);
    let cols = layout.cols(Mode::Wall);
    let action = grid_key(
        &Key::Named(Named::ArrowDown),
        GridKeys {
            index: Some(0),
            count: 100,
            cols,
            ctrl: false,
        },
    );
    assert_eq!(action, Some(GridAction::Move(cols)));
}

#[test]
fn it_plays_from_the_first_frame_and_holds_until_the_rest_arrive() {
    let mut p = Previews::default();
    let g = due(&p.enter("7".into(), true));
    let _ = p.due(g, 320, Instant::now());
    let start = Instant::now();
    p.frames(g, frames(1, 100), false, start);
    assert!(p.playing(), "the first frame plays at once");
    assert!(
        !p.tick(start + Duration::from_millis(500)),
        "holds the last decoded frame instead of looping"
    );
    p.frames(g, frames(2, 100), false, start);
    assert!(p.tick(start + Duration::from_millis(600)));
    assert_eq!(p.key().map(|(_, i)| i), Some(1));
    p.frames(g, Vec::new(), true, start);
    assert!(p.tick(start + Duration::from_millis(700)));
    assert!(p.tick(start + Duration::from_millis(800)));
    assert_eq!(p.key().map(|(_, i)| i), Some(0), "complete: it loops");
}

#[test]
fn scrolling_isnt_resting_so_no_preview_starts_until_it_pauses() {
    let mut p = Previews::default();
    let g = due(&p.enter("7".into(), true));
    let _ = p.due(g, 320, Instant::now());
    p.frames(g, frames(2, 50), true, Instant::now());
    assert!(p.playing());
    // A scroll stops it and waits.
    let start = Instant::now();
    let effects = p.scrolled(start);
    assert!(effects.contains(&Effect::StopPreview));
    assert!(!p.playing());
    let g = due(&effects[1..]);
    // Still scrolling when the timer comes due: it waits again, loading nothing.
    let more = p.scrolled(start + Duration::from_millis(150));
    assert!(more.is_empty(), "no new timer per scroll frame");
    let again = p.due(g, 320, start + Duration::from_millis(200));
    assert!(matches!(again[..], [Effect::PreviewDue { .. }]));
    // Once scrolling has paused, it loads.
    let loads = p.due(g, 320, start + Duration::from_millis(400));
    assert!(matches!(loads[..], [Effect::LoadPreview { .. }]));
}
