# Quickstart: validating queue autoplay and playlist controls (011)

Prerequisites: the native app built (`cargo build -p semantic-stash-viewer-native`); the Testing
instance on :9998 (saved profile `localhost:9998`). Testing's seeded data (008, 010): performer A
(id 2) is on scenes 181–190; scene 24 is 26 seconds long; scene 1022 can't be decoded.

## Automated

```bash
cargo test -p stash-core --test playlist     # every Autoplay × Repeat × Shuffle transition
cargo test --workspace                       # includes native-ui/tests/queue.rs
cargo run -p perf-harness -- --profile "localhost:9998" --quick   # queue-* rows
```

Expected: the four 011 rows within 1.5 s; every 010 row still within budget
([contract](contracts/queue-autoplay.md)).

## V1: Click to play (US1)

Filter to performer A, choose Queue: the first scene plays (Autoplay on). Click three rows: each
plays at once; click the playing row: it carries on. Hold ↓, stop on a row: it plays after a
moment.

## V2: Autoplay (US2)

Seek near the end: the next scene plays. Press `A` (Autoplay off, the switch dims): seek near the
end again: it stops with Replay and "Next". Change the sort with Autoplay off: nothing starts;
with it on: the new first scene plays. Play something in another tab, switch back: it keeps
playing. Restart with a queue tab open: nothing plays until you click.

## V3: Repeat (US3)

Press `R` once (🔁 Repeat all): on performer A's one-page queue, play the last scene to its end:
the first plays. Press `R` again (🔂 Repeat one): let a scene end: it plays again from 0:00, with
Autoplay on or off; `n` still moves on. Press `R` again: Off; the last scene's end stops.

## V4: Shuffle (US3)

Press `X` (🔀): play through ten scenes with `n`: no repeats; the list keeps its order and jumps
to each scene's page with it selected (try an unfiltered tab at 20 per page); `p` retraces them.
Turn Shuffle off: the next is the row after the playing one. Scene 1022 in the queue: its error
shows and nothing moves on.

## Requirements and their tests

`playlist` is `crates/stash-core/tests/playlist.rs`; `queue`, `snapshot`, and `keymap` are in
`native-ui/tests/`; the `queue-*` rows are perf-harness rows.

| Requirement | Tests |
|---|---|
| FR-001 click plays | queue: `clicking_another_row_switches_playback`, `clicking_a_row_shows_it_in_the_panel`, `a_click_loads_at_once`, `a_click_plays_with_autoplay_off`; row `queue-click-play` |
| FR-002 the playing row doesn't restart | queue: `clicking_the_playing_row_doesnt_restart`, `a_closed_players_scene_plays_again_when_chosen`; playlist: `choosing_plays_unless_already_playing`, `idle_lets_the_last_scene_play_again_and_moves_the_position` |
| FR-003 the keyboard plays where it rests | queue: `the_keyboard_plays_where_it_rests`, `passing_rows_plays_nothing`, `a_rest_on_the_playing_scene_plays_nothing`, `enter_plays_the_selection_in_the_panel` |
| FR-004 Autoplay | queue: `choosing_queue_selects_the_first_scene_and_loads_its_details`, `changing_the_results_selects_the_first_scene`, `with_autoplay_on_new_results_play_their_first`, `autostart_respects_autoplay_off`, `with_autoplay_off_a_scene_stops_at_its_end`, `a_toggles_autoplay_and_o_opens_the_page`; playlist: `autostart_needs_autoplay`, `ended_with_autoplay_off_stops`; snapshot: `queue_fields_are_additive`; row `queue-autoplay` |
| FR-005 only the tab on screen starts | queue: `switching_to_a_queue_tab_plays_when_idle`, `switching_to_a_queue_tab_leaves_other_playback_alone`, `returning_to_the_playing_queue_tab_doesnt_restart`, `a_restored_queue_tab_doesnt_play_until_switched_to`, `a_switched_to_tab_plays_once_its_page_arrives`, `a_refresh_never_plays` |
| FR-006 Repeat | queue: `repeat_one_replays_from_the_start`, `repeat_all_returns_to_page_one`, `a_one_page_queue_with_repeat_all_plays_the_first_again`, `a_one_scene_queue_with_repeat_all_replays_it`, `repeat_off_stops_at_the_end`, `r_cycles_repeat`; playlist: `ended_with_repeat_one_replays`, `at_the_end_repeat_all_wraps`, `at_the_end_repeat_off_stops`, `repeat_cycles_off_all_one`; row `queue-repeat` |
| FR-007 Shuffle | queue: `shuffle_plays_each_scene_once`, `previous_retraces_the_round`, `turning_shuffle_off_continues_in_list_order`, `new_results_start_a_new_round`, `choosing_a_scene_while_shuffling_starts_from_it`, `x_toggles_shuffle`; playlist: `a_rounds_order_visits_each_index_once`, `shuffle_with_repeat_all_plays_every_scene_once_per_round`, `shuffle_with_repeat_off_stops_after_every_scene`, `toggling_shuffle_roots_a_round_at_the_playing_scene`, `started_follows_the_playing_scene`; row `queue-shuffle` |
| FR-008 Next and Previous | queue: `next_and_previous_play_the_neighbours`, `previous_from_a_pages_first_scene_plays_the_previous_pages_last`, `previous_never_wraps`, `n_on_the_last_scene_with_repeat_all_returns_to_page_one`, `next_leaves_repeat_one_on`; playlist: `previous_steps_back_and_never_wraps`, `repeat_one_never_holds_a_move_on_the_same_scene` |
| FR-009 controls and keys | keymap: `the_queue_views_keys_are_listed`; queue: `r_cycles_repeat`, `x_toggles_shuffle`, `a_toggles_autoplay_and_o_opens_the_page`; the strip by eye (V3, V4) |
| FR-010 failures and deleted scenes | queue: `a_failure_never_advances`, `a_failure_never_repeats`, `a_deleted_scene_is_skipped`, `deleted_scenes_stop_after_one_round`; playlist: `a_failure_stops_in_every_state`, `skips_stop_after_the_whole_result_and_reset_when_a_scene_starts` |
| FR-011 010 kept | queue: the 010 tests (`moving_the_selection_keeps_playing_and_marks_the_playing_row`, `f6_moves_the_keyboard_to_the_panel_and_back`, `another_tab_playing_clears_the_queues_mark`, …); rows `queue-select`, `queue-next`, `gql-queue-*` |
| SC-001 click < 1.5 s | row `queue-click-play` |
| SC-002 Autoplay start < 1.5 s | row `queue-autoplay` |
| SC-003 next, page 1, pick, replay < 1.5 s | rows `queue-next`, `queue-repeat`, `queue-shuffle`; playlist: `a_million_scene_round_is_computed_in_place` (O(1) steps) |
| SC-004 an hour unattended, no repeats per round | playlist: `shuffle_with_repeat_all_plays_every_scene_once_per_round`; queue: `shuffle_plays_each_scene_once` |
| SC-005 010 budgets | every 010 row in the harness run |
