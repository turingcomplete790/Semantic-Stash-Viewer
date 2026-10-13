//! 012 T029: the viewer's rules (data model "Viewer"), each as a pure step.

use stash_core::images::viewer::{step, Command, Event, Slideshow, View, Viewer, MAX_ZOOM};

/// The viewer open on `at` of `count`, already shown.
fn at(at: u32, count: u32) -> Viewer {
    let (v, _) = step(&Viewer::default(), Event::Open { at, count });
    v
}

fn shows(command: &Command) -> Option<(u32, Vec<u32>)> {
    match command {
        Command::Show { at, prefetch } => Some((*at, prefetch.clone())),
        _ => None,
    }
}

#[test]
fn opening_shows_the_image_with_both_neighbours_prefetched() {
    let (v, c) = step(&Viewer::default(), Event::Open { at: 7, count: 214 });
    assert_eq!(v.at, 7);
    assert_eq!(shows(&c), Some((7, vec![8, 6])));
    // At the ends only the neighbour that exists.
    let (_, c) = step(&Viewer::default(), Event::Open { at: 0, count: 3 });
    assert_eq!(shows(&c), Some((0, vec![1])));
    let (_, c) = step(&Viewer::default(), Event::Open { at: 2, count: 3 });
    assert_eq!(shows(&c), Some((2, vec![1])));
    // Past the end: clamped.
    let (v, _) = step(&Viewer::default(), Event::Open { at: 9, count: 3 });
    assert_eq!(v.at, 2);
}

#[test]
fn next_and_previous_step_with_prefetch_of_the_new_neighbours() {
    let v = at(7, 214);
    let (v, c) = step(&v, Event::Next);
    assert_eq!(shows(&c), Some((8, vec![9, 7])));
    let (v, c) = step(&v, Event::Previous);
    assert_eq!(v.at, 7);
    assert_eq!(shows(&c), Some((7, vec![8, 6])));
}

#[test]
fn next_at_the_last_does_nothing() {
    let v = at(213, 214);
    let (after, c) = step(&v, Event::Next);
    assert_eq!(c, Command::Nothing);
    assert_eq!(after.at, 213);
}

#[test]
fn previous_at_the_first_does_nothing_no_wrap() {
    let v = at(0, 214);
    let (after, c) = step(&v, Event::Previous);
    assert_eq!(c, Command::Nothing);
    assert_eq!(after.at, 0);
}

#[test]
fn first_and_last_jump_to_the_results_ends() {
    let v = at(7, 214);
    let (v, c) = step(&v, Event::Last);
    assert_eq!(shows(&c).map(|s| s.0), Some(213));
    let (v, c) = step(&v, Event::Last);
    assert_eq!(c, Command::Nothing);
    let (_, c) = step(&v, Event::First);
    assert_eq!(shows(&c).map(|s| s.0), Some(0));
}

#[test]
fn a_new_image_resets_the_view() {
    let v = at(3, 10);
    let (v, _) = step(
        &v,
        Event::Zoom {
            delta: 1.0,
            anchor: (0.0, 0.0),
        },
    );
    let (v, _) = step(&v, Event::Rotate { clockwise: true });
    let (v, _) = step(&v, Event::Pan { dx: 0.2, dy: 0.0 });
    assert_ne!(v.view, View::default());
    for event in [Event::Next, Event::Previous, Event::First, Event::Last] {
        let (after, c) = step(&v, event);
        if shows(&c).is_some() {
            assert_eq!(after.view, View::default(), "{event:?}");
        }
    }
    let (after, _) = step(&v, Event::Open { at: 5, count: 10 });
    assert_eq!(after.view, View::default());
}

#[test]
fn zoom_clamps_to_an_eighth_and_eight_times() {
    let mut v = at(0, 1);
    for _ in 0..20 {
        v = step(
            &v,
            Event::Zoom {
                delta: 0.5,
                anchor: (0.0, 0.0),
            },
        )
        .0;
    }
    assert_eq!(v.view.zoom, MAX_ZOOM);
    let (same, c) = step(
        &v,
        Event::Zoom {
            delta: 0.5,
            anchor: (0.0, 0.0),
        },
    );
    assert_eq!(c, Command::Nothing, "already at the most");
    assert_eq!(same.view.zoom, MAX_ZOOM);
    for _ in 0..40 {
        v = step(
            &v,
            Event::Zoom {
                delta: -0.5,
                anchor: (0.0, 0.0),
            },
        )
        .0;
    }
    assert_eq!(v.view.zoom, -MAX_ZOOM);
}

#[test]
fn zoom_sets_the_view() {
    let v = at(0, 1);
    let (v, c) = step(
        &v,
        Event::Zoom {
            delta: 1.0,
            anchor: (0.0, 0.0),
        },
    );
    assert_eq!(c, Command::SetView(v.view));
    assert_eq!(v.view.zoom, 1.0);
}

#[test]
fn zooming_about_the_pointer_keeps_that_point_still() {
    // The point a quarter of the fitted picture right of centre stays under the pointer: at zoom
    // z and pan p it shows at (p + u) * 2^z; solve for u before and check after.
    let v = at(0, 1);
    let (v, _) = step(
        &v,
        Event::Zoom {
            delta: 1.0,
            anchor: (0.25, 0.0),
        },
    );
    let (after, _) = step(
        &v,
        Event::Zoom {
            delta: 1.0,
            anchor: (0.25, 0.0),
        },
    );
    let screen = |view: &View, u: f32| (view.pan.0 + u) * view.zoom.exp2();
    let u = 0.25 / v.view.zoom.exp2() - v.view.pan.0;
    assert!((screen(&after.view, u) - 0.25).abs() < 1e-5);
}

#[test]
fn panning_without_zoom_does_nothing() {
    let v = at(0, 1);
    let (after, c) = step(&v, Event::Pan { dx: 0.3, dy: 0.1 });
    assert_eq!(c, Command::Nothing);
    assert_eq!(after.view.pan, (0.0, 0.0));
    // Zoomed out, still nothing; zooming back to fit centres the picture.
    let (zoomed, _) = step(
        &v,
        Event::Zoom {
            delta: 1.0,
            anchor: (0.0, 0.0),
        },
    );
    let (panned, _) = step(&zoomed, Event::Pan { dx: 0.2, dy: 0.0 });
    assert_eq!(panned.view.pan, (0.2, 0.0));
    let (fitted, _) = step(
        &panned,
        Event::Zoom {
            delta: -1.0,
            anchor: (0.0, 0.0),
        },
    );
    assert_eq!(fitted.view.pan, (0.0, 0.0));
}

#[test]
fn pan_keeps_the_picture_edge_from_passing_the_middle() {
    let v = at(0, 1);
    let (v, _) = step(
        &v,
        Event::Zoom {
            delta: 2.0,
            anchor: (0.0, 0.0),
        },
    );
    let (v, _) = step(&v, Event::Pan { dx: 5.0, dy: -5.0 });
    assert_eq!(v.view.pan, (0.5, -0.5));
}

#[test]
fn actual_size_pans_and_fit_resets_all_but_the_rotation() {
    let v = at(0, 1);
    let (v, c) = step(&v, Event::ToggleActual);
    assert!(v.view.actual);
    assert_eq!(c, Command::SetView(v.view));
    let (v, _) = step(&v, Event::Pan { dx: 0.1, dy: 0.0 });
    assert_eq!(
        v.view.pan,
        (0.1, 0.0),
        "an actual-size picture can be panned"
    );
    let (v, _) = step(&v, Event::Rotate { clockwise: false });
    assert_eq!(v.view.quarter_turns, 3);
    let (v, _) = step(&v, Event::Fit);
    assert_eq!(
        v.view,
        View {
            quarter_turns: 3,
            ..View::default()
        }
    );
}

#[test]
fn rotation_steps_in_quarter_turns() {
    let mut v = at(0, 1);
    for want in [1, 2, 3, 0] {
        v = step(&v, Event::Rotate { clockwise: true }).0;
        assert_eq!(v.view.quarter_turns, want);
    }
}

#[test]
fn failed_keeps_the_position_and_next_still_moves() {
    let v = at(4, 10);
    let (v, c) = step(&v, Event::Failed(4));
    assert_eq!((v.at, v.failed), (4, Some(4)));
    assert_eq!(c, Command::Nothing);
    // A late failure for an image no longer shown changes nothing.
    let (same, _) = step(&v, Event::Failed(2));
    assert_eq!(same.failed, Some(4));
    let (v, c) = step(&v, Event::Next);
    assert_eq!(shows(&c).map(|s| s.0), Some(5));
    assert_eq!(v.failed, None);
}

#[test]
fn shown_clears_a_failure_and_records_looping() {
    let v = at(4, 10);
    let (v, _) = step(&v, Event::Failed(4));
    let (v, _) = step(
        &v,
        Event::Shown {
            at: 4,
            looping: true,
        },
    );
    assert_eq!(v.failed, None);
    assert!(v.looping);
}

#[test]
fn results_changed_clamps_and_an_empty_result_closes() {
    let v = at(9, 10);
    let (v, c) = step(&v, Event::ResultsChanged { count: 6 });
    assert_eq!((v.at, v.count), (5, 6));
    assert_eq!(shows(&c).map(|s| s.0), Some(5));
    let (same, c) = step(&v, Event::ResultsChanged { count: 20 });
    assert_eq!((same.at, same.count), (5, 20));
    assert_eq!(c, Command::Nothing);
    let (_, c) = step(&v, Event::ResultsChanged { count: 0 });
    assert_eq!(c, Command::Close { at: 0 });
}

#[test]
fn close_reports_where_it_was() {
    let v = at(37, 214);
    let (_, c) = step(&v, Event::Close);
    assert_eq!(c, Command::Close { at: 37 });
}

#[test]
fn details_toggle() {
    let v = at(0, 1);
    let (v, c) = step(&v, Event::ToggleDetails);
    assert!(v.details);
    assert_eq!(c, Command::Nothing);
    let (v, _) = step(&v, Event::ToggleDetails);
    assert!(!v.details);
}

// ---- The slideshow (its rules live here; US3 wires the clock) ---------------------------------

#[test]
fn a_slideshow_looks_two_ahead_and_advances_on_each_tick() {
    let v = at(3, 10);
    let (v, c) = step(&v, Event::ToggleSlideshow { delay_s: 5 });
    assert_eq!(
        v.slideshow,
        Some(Slideshow {
            delay_s: 5,
            paused: false
        })
    );
    assert_eq!(c, Command::Prefetch(vec![4, 5]));
    let (v, c) = step(&v, Event::Tick);
    assert_eq!(shows(&c), Some((4, vec![5, 6])));
    assert_eq!(v.at, 4);
}

#[test]
fn a_step_or_zoom_by_hand_pauses_the_slideshow() {
    let start = step(&at(3, 10), Event::ToggleSlideshow { delay_s: 5 }).0;
    for event in [
        Event::Next,
        Event::Previous,
        Event::Zoom {
            delta: 0.5,
            anchor: (0.0, 0.0),
        },
    ] {
        let (v, _) = step(&start, event);
        assert!(v.slideshow.is_some_and(|s| s.paused), "{event:?}");
        let (same, c) = step(&v, Event::Tick);
        assert_eq!(c, Command::Nothing, "paused: no advance");
        assert_eq!(same.at, v.at);
    }
}

#[test]
fn a_slideshow_stops_after_the_last_image() {
    let v = step(&at(9, 10), Event::ToggleSlideshow { delay_s: 5 }).0;
    let (v, c) = step(&v, Event::Tick);
    assert_eq!(c, Command::Nothing);
    assert_eq!(v.slideshow, None);
    assert_eq!(v.at, 9);
}

#[test]
fn a_looping_image_waits_for_its_loop() {
    let v = step(&at(2, 10), Event::ToggleSlideshow { delay_s: 5 }).0;
    let (v, _) = step(
        &v,
        Event::Shown {
            at: 2,
            looping: true,
        },
    );
    let (v, c) = step(&v, Event::Tick);
    assert_eq!(c, Command::Nothing);
    let (v, c) = step(&v, Event::LoopDone);
    assert_eq!(shows(&c).map(|s| s.0), Some(3));
    assert!(!v.looping, "the next image hasn't said yet");
}

#[test]
fn the_delay_clamps_to_one_to_sixty_seconds() {
    let v = step(&at(0, 10), Event::ToggleSlideshow { delay_s: 0 }).0;
    assert_eq!(v.slideshow.map(|s| s.delay_s), Some(1));
    let v = step(&v, Event::SetDelay(600)).0;
    assert_eq!(v.slideshow.map(|s| s.delay_s), Some(60));
    let (v, c) = step(&v, Event::ToggleSlideshow { delay_s: 5 });
    assert!(v.slideshow.is_some_and(|s| s.paused));
    assert_eq!(c, Command::Nothing);
}
