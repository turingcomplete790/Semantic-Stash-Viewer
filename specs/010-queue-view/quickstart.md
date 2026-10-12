# Quickstart: validating the Queue view (010)

Prerequisites: the native app built (`cargo build -p semantic-stash-viewer-native`); the Testing
instance on :9998 (saved profile `localhost:9998`). Testing's seeded data (008 quickstart): performer
A (id 2) is on scenes 181–190, and scene 24 is 26 seconds long.

## Automated

```bash
cargo test --workspace                       # includes native-ui/tests/queue.rs
cargo run -p perf-harness -- --profile "localhost:9998" --quick   # queue-* and gql-queue-* rows
```

Expected: every queue row within its budget ([contracts/queue-ui.md](contracts/queue-ui.md)), and
every 007/008 row still passing.

## V1: Click through scenes (US1)

Open Scenes, filter to performer A, switch to Queue. The list shows scenes 181–190 and the first is
selected with its details. Click several rows: each shows at once, details arrive within a second.
Hold ↓ through the list: rows highlight as they pass, and only the one you stop on loads details
(the bench's `gql-queue-sweep` row checks this). Change the sort: the first scene is selected.
Restart: the tab comes back in Queue with the same selection.

## V2: Play a run (US2)

Select a scene and press Enter: it plays in the panel with its resume point and chapters. Seek near
the end and let it finish: the next plays and is selected. With a page size smaller than the result
(for example 20 on an unfiltered tab), let the page's last scene end: the next page loads and its
first scene plays. Press `n` and `p`: the next and previous scenes play. Move the selection while
something plays: it keeps playing, the playing row is marked. Turn Play next off (`a`): a scene stops
at its end with Replay. Scene 1022 (the undecodable test file) shows the error and doesn't advance.

## V3: Keyboard only (US3)

Without the pointer: move through the list, play, skip with `n`/`p`, toggle Play next, F6 into the
panel (the arrows now seek and change volume), F6 back, `o` to open the full page, and F1 to see the
queue's keys listed.

## Requirement to test

Tests are in `native-ui/tests/queue.rs` unless another file is named; harness rows run against
Testing.

| Requirement | Tests and harness rows |
|---|---|
| FR-001 Queue mode, saved | `choosing_queue_selects_the_first_scene_and_loads_its_details`, `a_restored_tab_selects_its_saved_scene`, `snapshot.rs::queue_fields_are_additive` |
| FR-002 compact list beside the panel | `clicking_a_row_shows_it_in_the_panel` (iced_test), `layout.rs` unit tests |
| FR-003 panel: still and title at once, then the scene page's details | `a_keyboard_selection_shows_at_once_and_loads_after_a_rest`, `the_panels_replies_land_in_it` |
| FR-004 choose without leaving, row highlighted | `clicking_a_row_shows_it_in_the_panel`, `a_click_loads_at_once` |
| FR-005 details only on rest, once | `sweeping_rows_sends_no_details_and_resting_sends_one`, `a_refreshed_page_keeps_the_selection_and_its_panel`; `gql-queue-sweep`, `gql-queue-rest` |
| FR-006 results change → first scene | `changing_the_results_selects_the_first_scene`, `a_search_selects_the_first_scene` |
| FR-007 restore | `a_restored_tab_selects_its_saved_scene`, `snapshot.rs::queue_fields_are_additive` |
| FR-008 play in the panel with the one player | `enter_plays_the_selection_in_the_panel`, `leaving_the_queue_view_shows_the_now_playing_bar` |
| FR-009 Play next, on by default, saved | `when_a_scene_ends_the_next_plays_and_is_selected`, `with_play_next_off_a_scene_stops_at_its_end`, `snapshot.rs::queue_fields_are_additive` |
| FR-010 across pages; Replay at the end | `after_the_pages_last_scene_the_next_page_plays_from_its_first`, `previous_from_a_pages_first_scene_plays_the_previous_pages_last`, `the_last_pages_last_scene_stops`, `a_page_change_cancels_a_pending_play` |
| FR-011 selection independent of playback, playing mark | `moving_the_selection_keeps_playing_and_marks_the_playing_row`, `another_tab_playing_clears_the_queues_mark` |
| FR-012 no advance after a failure | `a_failure_never_advances`; deleted scenes skipped: `a_deleted_scene_is_skipped` |
| FR-013 keyboard, keys listed | `f6_moves_the_keyboard_to_the_panel_and_back`, `a_toggles_play_next_and_o_opens_the_page`, `next_and_previous_play_the_neighbours`, `keymap.rs::the_queue_views_keys_are_listed` |
| FR-014 request budgets | `sweeping_rows_sends_no_details_and_resting_sends_one`; `gql-queue-sweep` (0), `gql-queue-rest` (1) |
| SC-001 selection < 150 ms | `queue-select` |
| SC-002 next playing < 1.5 s, page boundary included | `queue-next` (two runs on a page, one across) |
| SC-003 no requests passing over; scrolling | `gql-queue-sweep`; `scenes-scroll-1000-queue` (≤ 1% at 144 Hz) |
| SC-004 ten in a row without the pointer | `next_and_previous_play_the_neighbours`, `when_a_scene_ends_the_next_plays_and_is_selected`; manual V3 |
| SC-005 every 007/008 budget still passes | the full harness run |
