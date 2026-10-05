//! 006 T010: the frame-slot ring shared by mpv's render thread and the iced video widget
//! (data-model "VideoSurface"; capability C5; behaviour that stays B5: every frame shown).

use std::time::{Duration, Instant};

use semantic_stash_viewer_native::player::video::slots::{Ring, SlotState, SLOTS};

fn ready_ring() -> (Ring, Instant) {
    let t0 = Instant::now();
    let mut ring = Ring::new();
    ring.request_size((1280, 720), t0);
    assert_eq!(
        ring.settle(t0),
        Some((1280, 720)),
        "the first size applies at once"
    );
    (ring, t0)
}

#[test]
fn four_slots_start_free() {
    let (ring, _) = ready_ring();
    assert_eq!(ring.states(), [SlotState::Free; SLOTS]);
    assert_eq!(ring.on_screen(), None);
}

#[test]
fn a_rendered_frame_becomes_ready_then_on_screen() {
    let (mut ring, _) = ready_ring();
    let a = ring.begin_render().expect("a free slot");
    assert_eq!(ring.states()[a], SlotState::Rendering);
    ring.finish_render(a);
    assert_eq!(ring.states()[a], SlotState::Ready);
    assert_eq!(ring.take_for_display(), Some(a));
    assert_eq!(ring.states()[a], SlotState::OnScreen);
    // Drawing again without a new frame keeps the same slot.
    assert_eq!(ring.take_for_display(), Some(a));
}

#[test]
fn only_one_slot_is_ready_at_a_time_newer_replaces_older() {
    let (mut ring, _) = ready_ring();
    let a = ring.begin_render().expect("slot");
    ring.finish_render(a);
    let b = ring.begin_render().expect("slot");
    assert_ne!(a, b);
    ring.finish_render(b);
    let ready: Vec<_> = (0..3)
        .filter(|&i| ring.states()[i] == SlotState::Ready)
        .collect();
    assert_eq!(ready, vec![b]);
    assert_eq!(ring.states()[a], SlotState::Free);
    assert_eq!(ring.take_for_display(), Some(b));
}

#[test]
fn an_on_screen_slot_is_never_rendered_into() {
    let (mut ring, _) = ready_ring();
    for _ in 0..20 {
        let shown = ring.take_for_display();
        let next = ring
            .begin_render()
            .expect("there is always a slot to render into");
        assert_ne!(Some(next), shown);
        // The slot shown just before may still be sampled by the GPU, so it isn't reused yet.
        assert_ne!(ring.states()[next], SlotState::Retiring);
        ring.finish_render(next);
    }
}

#[test]
fn a_ready_frame_survives_until_the_ui_draws_it() {
    // The slideshow bug: with one slot on screen and one retiring, rendering the next frame must
    // not take the ready frame the UI hasn't drawn yet.
    let (mut ring, _) = ready_ring();
    for _ in 0..2 {
        let f = ring.begin_render().expect("slot");
        ring.finish_render(f);
        ring.take_for_display();
    }
    let ready = ring.begin_render().expect("slot");
    ring.finish_render(ready);
    let next = ring.begin_render().expect("a free slot for the next frame");
    assert_ne!(next, ready);
    assert_eq!(ring.take_for_display(), Some(ready));
}

#[test]
fn an_undisplayed_frame_is_replaced_by_a_newer_one() {
    let (mut ring, _) = ready_ring();
    for _ in 0..2 {
        let f = ring.begin_render().expect("slot");
        ring.finish_render(f);
        ring.take_for_display();
    }
    let older = ring.begin_render().expect("slot");
    ring.finish_render(older);
    let newer = ring.begin_render().expect("slot");
    ring.finish_render(newer);
    // The UI fell behind: it shows the newest frame and the older one is freed.
    assert_eq!(ring.take_for_display(), Some(newer));
    assert_eq!(ring.states()[older], SlotState::Free);
}

#[test]
fn a_resize_applies_after_100_ms_without_further_change() {
    let (mut ring, t0) = ready_ring();
    let generation = ring.generation();
    ring.request_size((1920, 1080), t0);
    assert_eq!(ring.settle(t0 + Duration::from_millis(50)), None);
    ring.request_size((1900, 1060), t0 + Duration::from_millis(60));
    assert_eq!(
        ring.settle(t0 + Duration::from_millis(150)),
        None,
        "the clock restarts"
    );
    assert_eq!(
        ring.settle(t0 + Duration::from_millis(161)),
        Some((1900, 1060))
    );
    assert_eq!(ring.size(), (1900, 1060));
    assert_eq!(ring.generation(), generation + 1);
}

#[test]
fn all_slots_reallocate_together() {
    let (mut ring, t0) = ready_ring();
    let a = ring.begin_render().expect("slot");
    ring.finish_render(a);
    ring.take_for_display();
    ring.request_size((640, 360), t0);
    ring.settle(t0 + Duration::from_millis(100));
    assert_eq!(ring.states(), [SlotState::Free; SLOTS]);
    assert_eq!(ring.on_screen(), None);
}

#[test]
fn the_same_size_is_not_a_resize() {
    let (mut ring, t0) = ready_ring();
    let generation = ring.generation();
    ring.request_size((1280, 720), t0);
    assert_eq!(ring.settle(t0 + Duration::from_secs(1)), None);
    assert_eq!(ring.generation(), generation);
}

#[test]
fn a_frame_from_before_a_resize_is_ignored() {
    let (mut ring, t0) = ready_ring();
    let a = ring.begin_render().expect("slot");
    ring.request_size((640, 360), t0);
    ring.settle(t0 + Duration::from_millis(100));
    // The render thread finishes into a slot of the old generation: it doesn't become ready.
    ring.finish_render(a);
    assert_eq!(ring.take_for_display(), None);
}
