//! 011 T002–T005: the queue's playlist state machine (data-model.md's transition table), its
//! shuffle order, and the once-per-round property, without a player or server.

use std::collections::HashSet;

use stash_core::playback::playlist::{
    order, position_of, step, Command, Event, Playlist, Repeat, Round, Settings,
};

fn settings(autoplay: bool, repeat: Repeat, shuffle: bool) -> Settings {
    Settings {
        autoplay,
        repeat,
        shuffle,
    }
}

/// A playlist of `count` scenes playing `playing`, in list order.
fn list(s: Settings, count: u32, playing: Option<u32>) -> Playlist {
    Playlist {
        playing,
        live: playing.is_some(),
        ..Playlist::new(
            Settings {
                shuffle: false,
                ..s
            },
            count,
        )
    }
}

/// A shuffled playlist of `count` scenes whose round starts at `first` and is at `pos`.
fn shuffled(s: Settings, count: u32, first: u32, pos: u32) -> Playlist {
    let round = Round {
        seed: 42,
        first,
        pos,
    };
    Playlist {
        settings: Settings { shuffle: true, ..s },
        count,
        playing: Some(order(&round, count, pos)),
        live: true,
        round: Some(round),
        skips: 0,
    }
}

fn on() -> Settings {
    settings(true, Repeat::Off, false)
}

fn cmd(p: &Playlist, e: Event) -> Command {
    step(p, e).1
}

const ALL_SETTINGS: [Settings; 12] = {
    let mut out = [Settings {
        autoplay: true,
        repeat: Repeat::Off,
        shuffle: false,
    }; 12];
    let repeats = [Repeat::Off, Repeat::All, Repeat::One];
    let mut i = 0;
    while i < 12 {
        out[i] = Settings {
            autoplay: i % 2 == 0,
            repeat: repeats[(i / 2) % 3],
            shuffle: i >= 6,
        };
        i += 1;
    }
    out
};

// ---- T003: the shuffle order ----

#[test]
fn a_rounds_order_visits_each_index_once() {
    for count in [1u32, 2, 3, 7, 50, 1000] {
        for seed in [0u64, 1, 42, 0xDEAD_BEEF, u64::MAX] {
            for first in [0, count / 2, count - 1] {
                let round = Round {
                    seed,
                    first,
                    pos: 0,
                };
                let seen: HashSet<u32> = (0..count).map(|k| order(&round, count, k)).collect();
                assert_eq!(seen.len() as u32, count, "count {count} seed {seed}");
                assert!(seen.iter().all(|&i| i < count));
                assert_eq!(order(&round, count, 0), first);
                for k in 0..count {
                    assert_eq!(position_of(&round, count, order(&round, count, k)), k);
                }
            }
        }
    }
}

#[test]
fn seeds_give_different_orders() {
    let a = Round {
        seed: 1,
        first: 0,
        pos: 0,
    };
    let b = Round {
        seed: 2,
        first: 0,
        pos: 0,
    };
    let oa: Vec<u32> = (0..50).map(|k| order(&a, 50, k)).collect();
    let ob: Vec<u32> = (0..50).map(|k| order(&b, 50, k)).collect();
    assert_ne!(oa, ob);
    assert_ne!(oa, (0..50).collect::<Vec<_>>(), "not list order");
}

#[test]
fn a_million_scene_round_is_computed_in_place() {
    let count = 1_000_000;
    let round = Round {
        seed: 7,
        first: 123_456,
        pos: 0,
    };
    let i = order(&round, count, 999_999);
    assert!(i < count);
    assert_eq!(position_of(&round, count, i), 999_999);
    let p = shuffled(on(), count, 123_456, 999_998);
    let last = order(&p.round.expect("a round"), count, 999_999);
    let (next, c) = step(&p, Event::Next);
    assert_eq!(c, Command::Play(last));
    assert_eq!(next.round.expect("a round").pos, 999_999);
}

// ---- T004: the transition table ----

#[test]
fn ended_with_repeat_one_replays() {
    for s in ALL_SETTINGS.iter().filter(|s| s.repeat == Repeat::One) {
        let p = if s.shuffle {
            shuffled(*s, 10, 3, 4)
        } else {
            list(*s, 10, Some(4))
        };
        assert_eq!(cmd(&p, Event::Ended), Command::Replay, "{s:?}");
    }
}

#[test]
fn ended_with_autoplay_off_stops() {
    for s in ALL_SETTINGS
        .iter()
        .filter(|s| !s.autoplay && s.repeat != Repeat::One)
    {
        let p = if s.shuffle {
            shuffled(*s, 10, 3, 4)
        } else {
            list(*s, 10, Some(4))
        };
        let (next, c) = step(&p, Event::Ended);
        assert_eq!(c, Command::Stop, "{s:?}");
        assert_eq!(next, p, "nothing moves");
    }
}

#[test]
fn moving_on_plays_the_next_in_list_order() {
    let p = list(on(), 10, Some(4));
    for e in [Event::Ended, Event::Next, Event::Skipped] {
        let (next, c) = step(&p, e);
        assert_eq!(c, Command::Play(5), "{e:?}");
        assert_eq!(next.playing, Some(5));
    }
}

#[test]
fn moving_on_plays_the_next_in_round_order() {
    let p = shuffled(on(), 10, 3, 4);
    let r = p.round.expect("a round");
    for e in [Event::Ended, Event::Next, Event::Skipped] {
        let (next, c) = step(&p, e);
        assert_eq!(c, Command::Play(order(&r, 10, 5)), "{e:?}");
        assert_eq!(next.round.expect("a round").pos, 5);
        assert_eq!(next.playing, Some(order(&r, 10, 5)));
    }
}

#[test]
fn at_the_end_repeat_all_wraps() {
    let all = settings(true, Repeat::All, false);
    for e in [Event::Ended, Event::Next] {
        let (next, c) = step(&list(all, 10, Some(9)), e);
        assert_eq!(c, Command::Play(0));
        assert_eq!(next.playing, Some(0));
        let p = shuffled(all, 10, 3, 9);
        let (next, c) = step(&p, e);
        let r = next.round.expect("a round");
        assert_eq!(r.pos, 0);
        assert_ne!(r.seed, 42, "a new round");
        assert_eq!(c, Command::Play(r.first));
        assert_ne!(
            Some(r.first),
            p.playing,
            "a new round doesn't repeat the last scene"
        );
    }
}

#[test]
fn at_the_end_repeat_off_stops() {
    for e in [Event::Ended, Event::Next, Event::Skipped] {
        assert_eq!(cmd(&list(on(), 10, Some(9)), e), Command::Stop, "{e:?}");
        assert_eq!(cmd(&shuffled(on(), 10, 3, 9), e), Command::Stop, "{e:?}");
    }
}

#[test]
fn previous_steps_back_and_never_wraps() {
    let (next, c) = step(&list(on(), 10, Some(4)), Event::Previous);
    assert_eq!((c, next.playing), (Command::Play(3), Some(3)));
    assert_eq!(
        cmd(&list(on(), 10, Some(0)), Event::Previous),
        Command::Nothing
    );
    let all = settings(true, Repeat::All, false);
    assert_eq!(
        cmd(&list(all, 10, Some(0)), Event::Previous),
        Command::Nothing
    );

    let p = shuffled(on(), 10, 3, 4);
    let r = p.round.expect("a round");
    let (next, c) = step(&p, Event::Previous);
    assert_eq!(c, Command::Play(order(&r, 10, 3)));
    assert_eq!(next.round.expect("a round").pos, 3);
    assert_eq!(
        cmd(&shuffled(on(), 10, 3, 0), Event::Previous),
        Command::Nothing
    );
}

#[test]
fn choosing_plays_unless_already_playing() {
    let p = list(on(), 10, Some(4));
    let (next, c) = step(&p, Event::Chosen(7));
    assert_eq!((c, next.playing), (Command::Play(7), Some(7)));
    assert_eq!(step(&p, Event::Chosen(4)), (p, Command::Nothing));
    assert_eq!(cmd(&p, Event::Chosen(10)), Command::Nothing, "out of range");
    let off = list(settings(false, Repeat::Off, false), 10, Some(4));
    assert_eq!(
        cmd(&off, Event::Chosen(7)),
        Command::Play(7),
        "autoplay off"
    );
}

#[test]
fn choosing_while_shuffling_starts_a_round_from_it() {
    let p = shuffled(on(), 10, 3, 4);
    let (next, c) = step(&p, Event::Chosen(8));
    assert_eq!(c, Command::Play(8));
    let r = next.round.expect("a round");
    assert_eq!((r.first, r.pos), (8, 0));
}

#[test]
fn autostart_needs_autoplay() {
    let p = list(on(), 10, None);
    assert_eq!(cmd(&p, Event::Autostart(0)), Command::Play(0));
    let off = list(settings(false, Repeat::All, false), 10, None);
    assert_eq!(step(&off, Event::Autostart(0)), (off, Command::Nothing));
    let p = shuffled(on(), 10, 3, 4);
    let (next, c) = step(&Playlist { playing: None, ..p }, Event::Autostart(0));
    assert_eq!(c, Command::Play(0));
    assert_eq!(next.round.expect("a round").first, 0);
}

#[test]
fn new_results_clear_the_position() {
    let p = list(on(), 10, Some(4));
    let (next, c) = step(&p, Event::ResultsChanged { count: 3, seed: 9 });
    assert_eq!(c, Command::Nothing);
    assert_eq!((next.count, next.playing, next.round), (3, None, None));

    let p = shuffled(on(), 10, 3, 4);
    let (next, _) = step(&p, Event::ResultsChanged { count: 30, seed: 9 });
    assert_eq!(next.playing, None);
    let r = next.round.expect("a round");
    assert_eq!((r.seed, r.pos), (9, 0), "a new round");
}

#[test]
fn toggling_shuffle_roots_a_round_at_the_playing_scene() {
    let p = list(on(), 10, Some(4));
    let (next, c) = step(&p, Event::ToggleShuffle { seed: 5 });
    assert_eq!(c, Command::Nothing);
    assert!(next.settings.shuffle);
    let r = next.round.expect("a round");
    assert_eq!((r.seed, r.first, r.pos), (5, 4, 0));

    let (off, _) = step(&next, Event::ToggleShuffle { seed: 6 });
    assert!(!off.settings.shuffle);
    assert_eq!(off.round, None);
    assert_eq!(
        cmd(&off, Event::Next),
        Command::Play(5),
        "list order continues"
    );
}

#[test]
fn repeat_cycles_off_all_one() {
    let mut p = list(on(), 10, Some(4));
    let mut seen = vec![];
    for _ in 0..4 {
        let (next, c) = step(&p, Event::CycleRepeat);
        assert_eq!(c, Command::Nothing);
        seen.push(next.settings.repeat);
        p = next;
    }
    assert_eq!(seen, [Repeat::All, Repeat::One, Repeat::Off, Repeat::All]);
}

#[test]
fn set_autoplay_changes_only_the_setting() {
    let p = list(on(), 10, Some(4));
    let (next, c) = step(&p, Event::SetAutoplay(false));
    assert_eq!(c, Command::Nothing);
    assert_eq!(next, list(settings(false, Repeat::Off, false), 10, Some(4)));
}

#[test]
fn a_failure_stops_in_every_state() {
    for s in ALL_SETTINGS {
        for p in [
            list(s, 10, Some(9)),
            shuffled(s, 10, 3, 4),
            list(s, 0, None),
        ] {
            assert_eq!(step(&p, Event::Failed), (p, Command::Stop), "{s:?}");
        }
    }
}

#[test]
fn skips_stop_after_the_whole_result_and_reset_when_a_scene_starts() {
    let all = settings(true, Repeat::All, false);
    let mut p = list(all, 3, Some(0));
    let mut cmds = vec![];
    for _ in 0..3 {
        let (next, c) = step(&p, Event::Skipped);
        cmds.push(c);
        p = next;
    }
    assert_eq!(cmds, [Command::Play(1), Command::Play(2), Command::Stop]);
    let (p, _) = step(&p, Event::Started(2));
    assert_eq!(p.skips, 0);
    assert_eq!(cmd(&p, Event::Skipped), Command::Play(0));
}

#[test]
fn an_empty_result_does_nothing() {
    let events = [
        Event::Ended,
        Event::Failed,
        Event::Skipped,
        Event::Next,
        Event::Previous,
        Event::Chosen(0),
        Event::Autostart(0),
        Event::Started(0),
        Event::Idle(Some(0)),
    ];
    for s in ALL_SETTINGS {
        let p = list(s, 0, None);
        for e in events {
            let c = cmd(&p, e);
            assert!(
                matches!(c, Command::Nothing | Command::Stop),
                "{s:?} {e:?} → {c:?}"
            );
        }
    }
}

#[test]
fn repeat_one_never_holds_a_move_on_the_same_scene() {
    let one = settings(true, Repeat::One, false);
    assert_eq!(cmd(&list(one, 10, Some(4)), Event::Next), Command::Play(5));
    assert_eq!(
        cmd(&list(one, 10, Some(4)), Event::Previous),
        Command::Play(3)
    );
    assert_eq!(cmd(&list(one, 10, Some(9)), Event::Next), Command::Play(0));
    let (next, _) = step(&list(one, 10, Some(4)), Event::Next);
    assert_eq!(next.settings.repeat, Repeat::One, "stays on");
}

#[test]
fn a_one_scene_queue_with_repeat_all_replays() {
    let all = settings(true, Repeat::All, false);
    assert_eq!(cmd(&list(all, 1, Some(0)), Event::Ended), Command::Replay);
    let p = shuffled(all, 1, 0, 0);
    assert_eq!(cmd(&p, Event::Ended), Command::Replay);
}

#[test]
fn started_follows_the_playing_scene() {
    let p = shuffled(on(), 10, 3, 4);
    let r = p.round.expect("a round");
    let (same, _) = step(&p, Event::Started(order(&r, 10, 4)));
    assert_eq!(same.round, Some(r), "the expected scene keeps the round");
    let i = order(&r, 10, 7);
    let (next, c) = step(&p, Event::Started(i));
    assert_eq!(c, Command::Nothing);
    assert_eq!(next.playing, Some(i));
    let n = next.round.expect("a round");
    assert_eq!(
        (n.first, n.pos),
        (i, 0),
        "a scene from elsewhere starts a round from it"
    );
}

#[test]
fn step_leaves_its_input_untouched() {
    let p = shuffled(on(), 10, 3, 4);
    let copy = p;
    let _ = step(&p, Event::Next);
    assert_eq!(p, copy);
}

// ---- T005: once per round ----

#[test]
fn shuffle_with_repeat_all_plays_every_scene_once_per_round() {
    let all = settings(true, Repeat::All, true);
    for count in [1u32, 2, 5, 37, 200] {
        for start in [0, count - 1, count / 3] {
            let (mut p, _) = step(
                &Playlist::new(all, count),
                Event::ResultsChanged {
                    count,
                    seed: u64::from(start) * 31 + 7,
                },
            );
            p = step(&p, Event::Chosen(start)).0;
            if count == 1 {
                assert_eq!(cmd(&p, Event::Ended), Command::Replay);
                continue;
            }
            let mut seen = HashSet::from([start]);
            for _ in 1..count {
                let (next, c) = step(&p, Event::Ended);
                let Command::Play(i) = c else {
                    panic!("count {count}: {c:?}")
                };
                assert!(seen.insert(i), "count {count}: {i} repeated within a round");
                p = next;
            }
            assert_eq!(seen.len() as u32, count);
            // The next round plays them all again.
            let mut again = HashSet::new();
            for _ in 0..count {
                let (next, c) = step(&p, Event::Ended);
                let Command::Play(i) = c else {
                    panic!("count {count}: {c:?}")
                };
                assert!(again.insert(i), "count {count}: {i} repeated in round two");
                p = next;
            }
        }
    }
}

#[test]
fn shuffle_with_repeat_off_stops_after_every_scene() {
    let off = settings(true, Repeat::Off, true);
    for count in [1u32, 2, 9, 100] {
        let (p, _) = step(
            &Playlist::new(off, count),
            Event::ResultsChanged { count, seed: 3 },
        );
        let (mut p, c) = step(&p, Event::Autostart(0));
        assert_eq!(c, Command::Play(0));
        let mut played = 1;
        loop {
            let (next, c) = step(&p, Event::Ended);
            match c {
                Command::Play(_) => played += 1,
                Command::Stop => break,
                other => panic!("{other:?}"),
            }
            p = next;
        }
        assert_eq!(played, count);
    }
}

#[test]
fn idle_lets_the_last_scene_play_again_and_moves_the_position() {
    let p = list(on(), 10, Some(4));
    assert_eq!(cmd(&p, Event::Chosen(4)), Command::Nothing, "in the player");
    let (idle, c) = step(&p, Event::Idle(None));
    assert_eq!(c, Command::Nothing);
    assert_eq!((idle.playing, idle.live), (Some(4), false));
    let (again, c) = step(&idle, Event::Chosen(4));
    assert_eq!(
        c,
        Command::Play(4),
        "nothing plays, so choosing it plays it"
    );
    assert!(again.live);
    assert_eq!(cmd(&idle, Event::Autostart(4)), Command::Play(4));

    // Continuing from the selection.
    let (at, _) = step(&p, Event::Idle(Some(7)));
    assert_eq!(at.playing, Some(7));
    assert_eq!(cmd(&at, Event::Next), Command::Play(8));
    assert_eq!(cmd(&at, Event::Previous), Command::Play(6));
    let (out, _) = step(&p, Event::Idle(Some(99)));
    assert_eq!(out.playing, Some(4), "out of range keeps the position");

    // In a round, a new round from the selection.
    let p = shuffled(on(), 10, 3, 4);
    let r = p.round.expect("a round");
    let i = order(&r, 10, 6);
    let (at, _) = step(&p, Event::Idle(Some(i)));
    let n = at.round.expect("a round");
    assert_eq!((n.first, n.pos), (i, 0));
}
