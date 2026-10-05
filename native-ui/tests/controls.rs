//! 006 T011: the player's control logic, matching 002's player (002 FR-008–FR-010; capability C6;
//! behaviour that stays B4: the player keeps 002's shortcuts).

use std::time::{Duration, Instant};

use iced::keyboard::key::Named;
use iced::keyboard::Key;
use semantic_stash_viewer_native::player::controls::{
    hover_time, key_action, next_speed, Action, AutoHide, Direction, SeekBar, SPEEDS,
};

fn ch(c: &str) -> Key {
    Key::Character(c.into())
}

#[test]
fn space_toggles_pause() {
    assert_eq!(
        key_action(&Key::Named(Named::Space), false),
        Some(Action::TogglePause)
    );
}

#[test]
fn arrows_seek_ten_seconds_and_change_volume_by_five() {
    assert_eq!(
        key_action(&Key::Named(Named::ArrowLeft), false),
        Some(Action::SeekRelative(-10.0))
    );
    assert_eq!(
        key_action(&Key::Named(Named::ArrowRight), false),
        Some(Action::SeekRelative(10.0))
    );
    assert_eq!(
        key_action(&Key::Named(Named::ArrowUp), false),
        Some(Action::Volume(5.0))
    );
    assert_eq!(
        key_action(&Key::Named(Named::ArrowDown), false),
        Some(Action::Volume(-5.0))
    );
}

#[test]
fn brackets_step_speed_and_backslash_resets() {
    assert_eq!(
        key_action(&ch("["), false),
        Some(Action::Speed(Direction::Back))
    );
    assert_eq!(
        key_action(&ch("]"), false),
        Some(Action::Speed(Direction::Forward))
    );
    assert_eq!(key_action(&ch("\\"), false), Some(Action::NormalSpeed));
}

#[test]
fn speed_steps_through_the_list_and_stops_at_the_ends() {
    assert_eq!(SPEEDS, [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0]);
    assert_eq!(next_speed(1.0, Direction::Forward), 1.25);
    assert_eq!(next_speed(1.0, Direction::Back), 0.75);
    assert_eq!(next_speed(4.0, Direction::Forward), 4.0);
    assert_eq!(next_speed(0.25, Direction::Back), 0.25);
    // Between steps (set elsewhere): the next one in that direction.
    assert_eq!(next_speed(1.1, Direction::Forward), 1.25);
    assert_eq!(next_speed(1.1, Direction::Back), 1.0);
}

#[test]
fn period_and_comma_step_frames_only_while_paused() {
    assert_eq!(
        key_action(&ch("."), true),
        Some(Action::FrameStep(Direction::Forward))
    );
    assert_eq!(
        key_action(&ch(","), true),
        Some(Action::FrameStep(Direction::Back))
    );
    assert_eq!(key_action(&ch("."), false), None);
    assert_eq!(key_action(&ch(","), false), None);
}

#[test]
fn f_m_and_escape() {
    assert_eq!(key_action(&ch("f"), false), Some(Action::ToggleFullscreen));
    assert_eq!(key_action(&ch("m"), false), Some(Action::ToggleMute));
    assert_eq!(
        key_action(&Key::Named(Named::Escape), false),
        Some(Action::Escape)
    );
}

#[test]
fn unrelated_keys_are_left_alone() {
    assert_eq!(key_action(&ch("x"), false), None);
    assert_eq!(key_action(&Key::Named(Named::Tab), false), None);
}

#[test]
fn controls_hide_after_three_seconds_without_movement_while_playing() {
    let t0 = Instant::now();
    let mut hide = AutoHide::new(t0);
    assert!(hide.visible(t0 + Duration::from_millis(2999), true));
    assert!(!hide.visible(t0 + Duration::from_millis(3000), true));
    // Paused, ended, or loading: they stay up.
    assert!(hide.visible(t0 + Duration::from_secs(60), false));
    // Movement or a key shows them again.
    hide.activity(t0 + Duration::from_secs(10));
    assert!(hide.visible(t0 + Duration::from_secs(12), true));
}

#[test]
fn hover_position_maps_to_time() {
    assert_eq!(hover_time(0.0, 400.0, 100.0), 0.0);
    assert_eq!(hover_time(200.0, 400.0, 100.0), 50.0);
    assert_eq!(hover_time(500.0, 400.0, 100.0), 100.0);
    assert_eq!(hover_time(-5.0, 400.0, 100.0), 0.0);
    assert_eq!(hover_time(10.0, 0.0, 100.0), 0.0);
}

#[test]
fn the_seek_bar_shows_the_target_until_mpv_arrives() {
    let t0 = Instant::now();
    let mut bar = SeekBar::default();
    bar.seek_to(300.0, t0);
    assert_eq!(bar.shown(30.0, t0), 300.0);
    // Within a second of the target: arrived.
    assert_eq!(bar.shown(300.4, t0 + Duration::from_millis(200)), 300.4);
    assert_eq!(bar.shown(305.0, t0 + Duration::from_millis(400)), 305.0);
}

#[test]
fn the_seek_bar_gives_up_on_a_target_after_five_seconds() {
    let t0 = Instant::now();
    let mut bar = SeekBar::default();
    bar.seek_to(300.0, t0);
    assert_eq!(bar.shown(31.0, t0 + Duration::from_millis(4999)), 300.0);
    assert_eq!(bar.shown(31.0, t0 + Duration::from_secs(5)), 31.0);
}
