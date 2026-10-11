# Quickstart: validating Finish Browse and Play (008)

Prerequisites: the native app built (`cargo build -p semantic-stash-viewer-native`); the Testing
instance on :9998 (its saved profile is `localhost:9998`) for anything that writes play history;
Production (`localhost:9999`) read-only.

Test data on Testing (seeded 2026-10-06 through its GraphQL API in the web UI's own formats; ids
only):

| What | Id(s) |
|---|---|
| Performers "SSV Test Performer A" / "B" (A on scenes 181–190, B on 185–195) | 2 / 3 |
| Ordinary tag (all of scenes 181–195); parent tag; child tag (even-numbered scenes) | 6; 7; 8 |
| `isTrait: 1` tag; `plugtag: 1` tag (scene 187) | 9; 10 |
| Common variant (`variantOfTagId` → 9); `isTrait: 0` tag; `ssvTestFlag: 1` tag (scenes 185–186) | 11; 12; 13 |
| Scene with markers (10–20 s, 60 s, 120 s) | 185 |
| Scene with several files | 23 |
| Scenes with sprites and animated previews generated | 185, 192 (194 queued) |
| Scenes with the custom field `ssvSceneFlag: 1` | 181, 183 |
| Saved filters: "SSV performer A"; "SSV parent tag + 1080p" (depth: all); "SSV scene custom field" | 1; 2; 3 |
| Default scene filter (`defaultFilters.scenes`): performer B, title ascending | – |

The plugin compatibility fixture is `crates/stash-core/tests/fixtures/semantic/vocabulary.json`:
the `SemanticVocabulary` result captured read-only from Production on 2026-10-06, with names and
aliases replaced by `tag-<id>` and only the semantic flags and `variantOfTagId` kept (it includes
real flags set to 0).

## Query budget (run first, automated)

```bash
cargo test -p stash-core --test graphql_operations   # selection snapshots and tier rules
cargo test -p stash-core --test query_budget         # request counts per action (mock server)
cargo run -p perf-harness -- --profile "localhost:9998" --quick --allow-writes   # gql-* audit rows; --allow-writes (Testing only) adds the colour save and the 10-minute viewing
```

Expected: snapshots and counts pass; every `gql-*` row within its budget
([contracts/query-budget.md](contracts/query-budget.md)): one request per page, one per scene
page, none for cached pickers, none for chips only passed over.

## V1: Find (US1)

Search a known title; add performer and resolution filters; check the count and the first 100
against Testing's web UI for the same criteria. Apply each saved filter; open a new Scenes tab
and check the default filter applies. From a scene page, choose a performer, a tag, and a studio
(and Ctrl+click one). Restart: the tab's search, chips, page, and scroll come back. Open the
filter menu and check the Semantic group shows its placeholders.
The 12 SC-001 queries (checked automatically by
`STASH_TEST_URL=http://localhost:9998 STASH_TEST_API_KEY=… cargo test -p stash-core --test live_stash -- --ignored`,
and by eye against Testing's web UI): search text only; one performer; one tag with sub-tags
(depth); one studio; resolution above 720p; duration between 10 and 30 minutes; not organized;
rating at least 4 stars; one custom field; and the 3 saved filters. On 2026-10-06 all 12 gave the
same count and first 100 as the web UI's request. Check the Tags picker: semantic tags grouped by type, each type
in its default colour, and the `isTrait: 0` tag among ordinary, uncoloured tags. Do the whole of V1 once with the keyboard only. Open the performer picker on
Production (read-only): it opens at once after the first time and narrows as you type.

## V2: Scene page (US2)

Open scenes with many performers, with markers, with several files, and with almost no metadata;
compare with the web UI's scene page. Hover a performer, tag, and studio chip (details card);
sweep across a row of chips quickly (nothing loads). Performer chips show 40 px round pictures,
sharp on a high-density display, and a performer's card shows their portrait. Open "All files" and
"Galleries". Choose a marker (playback starts there). Tag chips show their type's colour; the
variant tag shows its canonical tag's colour.

On Testing only (it writes Stash's UI configuration): open Settings → Tag colours, check the
seven default rules and their tag counts, change the Role colour, add a rule for another tag
flag, reorder two rules, and save. Chips recolour at once; Testing's `configuration { ui }`
holds `semanticStashViewer.tagColours` and nothing else in the UI configuration changed. Reset to
the defaults. Disconnect and save: the page says the colours weren't saved and keeps the old ones.

## V3: Resume and history (US3, Testing only)

Play a scene for 2 minutes, stop; within 5 s the web UI shows the resume point and time watched;
reopen it (resumes there, "start from the beginning" offered). Watch to the end (resume cleared).
End viewings by tab close, scene change, server switch, quit, and pulling the network (then
reconnect): every one reaches Stash. Kill the app mid-viewing: Stash has the history to within a
minute. Turn "Save play history" off: nothing changes in the web UI. Jump between markers with
the chapter keys.

## V4: Previews (US4)

Hover cards (preview starts within the budget, stops on leave); wall mode on a 250 page (stills;
only the hovered one animates); a scene without previews (still stays); hover the seek bar of a
scene with sprites (thumbnail and time).

## V5: Fallback (US5, Testing)

A file mpv can't decode (prepare one on Testing): the error offers "Play transcoded"; it plays
with a "Transcoded" badge, and the log has the scene and reason. Normal scenes never transcode.

## Requirements to tests

Test files are under `crates/stash-core/tests/` (core) and `native-ui/tests/` (UI); harness rows
come from `perf-harness` (`--allow-writes` on Testing for the write rows).

| Requirement | Tests | Harness rows |
|---|---|---|
| FR-001 search | `scenes_find::typing_waits_then_one_search_runs_from_page_one`, `…enter_searches_at_once_and_results_for_an_older_query_are_dropped`; `live_stash::the_viewers_filters_give_the_web_uis_results` | `gql-filter-apply` |
| FR-002 every criterion | `scene_filters::every_kind_and_modifier_converts_to_its_stash_field`, `…values_take_stash_shapes`, `…validation_follows_the_rules`, `…custom_fields_may_repeat_and_go_out_as_a_list` | |
| FR-003 chips, clear all, empty state | `scenes_find::adding_changing_removing_and_clearing_criteria_each_load_page_one`, `…no_matches_offers_to_clear_the_filters` | |
| FR-003a pickers | `scenes_find::a_picker_kind_asks_the_library_for_its_list`, `…a_picker_list_is_read_once_per_session`; `choices::local_search_folds_case_and_accents_over_names_and_aliases`; `picker::the_tags_picker_survives_the_vocabulary_arriving` | `picker-open`, `picker-narrow`, `gql-picker-cold`, `gql-picker-cached` |
| FR-003b semantic placeholders | `scene_filters::invalid_criteria_and_semantic_placeholders_are_never_sent`; `scenes_find::semantic_placeholders_open_nothing_and_survive_a_restore` | |
| FR-003c two disjoint tag lists | `choices::the_tag_list_asks_stash_for_ordinary_tags_only`, `…the_request_has_one_alias_per_flag_plus_variants_and_extra_rules`, `…the_production_fixture_parses_with_every_flagged_tag_typed`; `graphql_operations::only_the_vocabulary_selects_custom_fields` | `gql-vocabulary-cold`, `gql-tags-picker-cold` |
| FR-003d tag chips by type | `choices::types_follow_the_fixed_priority_and_variants_borrow_their_canonical_type`, `…unset_flags_and_orphan_variants_are_shown_as_ordinary_tags`; `scene::tag_chips_take_their_type_and_rule_colour_from_the_vocabulary` | |
| FR-003e colour rules | `choices::colours_come_from_the_first_matching_rule`; `theme::chip_text_reaches_4_5_to_1_on_every_default_and_some_picked_colours` | |
| FR-003f keyboard | `scenes_find::the_filters_work_without_a_pointer`; `settings::tag_colours::the_keyboard_moves_the_highlight_within_the_rules`; `keymap::shell_shortcuts_resolve` | |
| FR-003g Settings → Tag colours | `settings::tag_colours::changing_a_colour_and_saving_is_one_write_of_every_rule`, `…a_failed_save_says_so_and_keeps_the_edits`, `…rules_can_be_added_reordered_and_only_added_ones_removed`; `query_budget::saving_tag_colours_is_one_write_of_that_key_only` | `gql-tag-colours-save` |
| FR-004 saved and default filters | `scene_filters::testings_saved_filters_decode_completely`, `…the_default_filter_decodes_from_the_ui_configuration`; `scenes_find::applying_a_saved_filter_sets_everything_and_loads_once`, `…a_new_scenes_tab_starts_from_the_servers_default_filter` | |
| FR-005 parts left out | `scene_filters::unknown_keys_and_unreadable_values_are_left_out_not_dropped`; `scenes_find::a_saved_filter_with_parts_left_out_says_so` | |
| FR-006 chips open filtered Scenes | `scenes_find::a_chip_opens_scenes_filtered_to_it_here_or_with_ctrl_in_a_new_tab` | |
| FR-007 per-tab state | `snapshot::a_filtered_scenes_tab_round_trips_without_a_request`; `previews::the_wall_is_denser_than_the_grid_and_saved_with_the_tab` | |
| FR-008 latest results only | `scenes_find::enter_searches_at_once_and_results_for_an_older_query_are_dropped` | |
| FR-009 the scene page | `scene::entering_loads_the_details_and_cover_once`, `…a_section_loads_once_then_opens_and_closes`, `…a_deleted_scene_says_it_no_longer_exists` | `gql-scene-page`, `gql-scene-files`, `gql-scene-galleries` |
| FR-010 instant title, cover, Play | `scene::the_card_title_shows_before_the_details_and_stays_after` | |
| FR-011 markers play | `scene::choosing_a_marker_plays_from_it` | |
| FR-011a chip data and pictures | `query_budget::a_scene_page_is_one_request_even_with_40_performers`; `scene::pictures_are_decoded_at_their_drawn_size_on_this_display` | |
| FR-011b chip cards and portrait | `scene::a_chip_card_loads_only_after_a_rest`, `…a_chip_card_is_fetched_once_per_session`, `…a_portrait_is_wanted_only_for_a_rested_performer_chip_with_an_image`, `…icon_and_portrait_are_separate_handles_from_one_cached_image`; `query_budget::each_chip_card_is_one_request` | `gql-chip-rest`, `gql-chip-sweep`, `gql-chip-portrait`, `chip-portrait` |
| FR-011c picture versions | `choices::performer_choices_carry_their_picture_version`, `…lists_and_chips_cached_before_picture_versions_are_read_again`; `scene::a_changed_picture_is_a_new_key_and_one_load`; `picker::picker_rows_ask_only_for_pictures_that_exist_at_their_version` | `gql-chip-rest-downloads` |
| FR-012 resume | `playback::opening_at_a_position_carries_the_start_to_the_player`, `…a_resumed_scene_offers_to_start_from_the_beginning` | |
| FR-013 chapters | `playback::chapter_keys_move_between_markers`; `keymap::the_player_keeps_002s_shortcuts_exactly` | |
| FR-014 every end reports | `playback::stopping_ends_the_viewing_with_one_final_report`, `…another_scene_ends_the_viewing_first`, `…leaving_the_session_sends_the_final_report_with_the_close`, `…closing_the_owner_tab_ends_the_viewing`, `…a_failure_or_a_lost_stream_ends_the_viewing`; `play_history::a_checkpoint_follows_each_minute_of_playing_with_the_time_since_the_last`, `…paused_time_doesnt_count`, `…a_resume_point_near_the_end_is_cleared` | `gql-viewing-10min-writes` |
| FR-015 play count | `play_history::a_play_is_counted_once_on_the_first_report_past_the_minimum` | |
| FR-016 nothing lost | `play_history::reports_are_kept_until_confirmed_and_survive_a_restart`, `…waiting_reports_for_one_scene_merge_and_profiles_stay_apart`, `…a_flush_sends_in_order_and_stops_at_a_failure`, `…the_flush_deadline_leaves_the_rest_queued`; `playback::reconnecting_sends_waiting_play_history`, `…repeated_send_failures_post_one_notification_per_server` | |
| FR-017 the viewer's own switch | `play_history::nothing_is_recorded_with_history_off_or_nothing_played`; `playback::with_history_off_nothing_is_reported`; `query_budget::with_history_off_a_viewing_writes_nothing`; `services::settings` unit test | |
| FR-018 the only writes | `graphql_operations::only_play_history_and_tag_colours_write`, `…every_operation_matches_its_golden_file` | |
| FR-019 animated previews | `previews` (core) `an_animated_webp_decodes_to_frames_with_their_delays`, `decoding_frame_by_frame_stops_when_asked`; `previews` (UI) `resting_on_a_card_loads_then_plays_its_preview_and_leaving_drops_it`, `…only_one_preview_exists`, `…it_plays_from_the_first_frame_and_holds_until_the_rest_arrive` | `preview-start` |
| FR-020 wall | `previews::the_wall_is_denser_than_the_grid_and_saved_with_the_tab`, `…arrows_move_through_the_wall_by_its_columns` | `scenes-scroll-1000-wall` |
| FR-020a fetched only on rest | `previews::passing_over_cards_fetches_nothing`, `…keyboard_focus_starts_a_preview_the_same_way`, `…scrolling_isnt_resting_so_no_preview_starts_until_it_pauses` | |
| FR-021 seek-bar thumbnails | `previews::the_vtt_index_maps_times_to_sheet_regions` | |
| FR-022 quiet fallback | `previews::a_missing_preview_or_sprite_is_a_quiet_none`, `…a_card_without_a_preview_keeps_its_still_and_asks_nothing` | |
| FR-023 "Play transcoded" | `player` `commands::tests::the_guard_takes_a_transcode_only_with_the_flag`, `…a_decode_failure_isnt_a_network_failure`; `playback::a_decode_failure_offers_the_transcode_and_a_network_failure_doesnt`, `…choosing_the_transcode_opens_it_with_its_reason`, `…a_transcode_that_fails_too_shows_both_reasons_and_isnt_retried`, `…a_normal_scene_never_opens_a_transcode` | |
| FR-024 marked and logged | `playback::choosing_the_transcode_opens_it_with_its_reason` (the reason reaches the log line); the badge is checked by hand (V5, scene 1022) | |
| FR-025 lean lists | `query_budget::showing_a_page_is_one_scenes_page_request`, `…a_page_with_search_and_criteria_is_still_one_request`; `graphql_operations::card_chip_and_detail_operations_select_no_forbidden_field` | `gql-page-50`, `gql-page-1000` |
| FR-026 cached pages | `scene_filters::criteria_order_doesnt_change_the_cache_key_but_their_content_does`; `snapshot::a_filtered_scenes_tab_round_trips_without_a_request` | |
| SC-001 results match the web UI | `live_stash::the_viewers_filters_give_the_web_uis_results` (12 queries on Testing) | |
| SC-002 first page under 1 s | | `gql-filter-apply`, `scenes-page-change` |
| SC-002a drop-down under 150 ms | | `picker-open`, `picker-narrow` |
| SC-003 finding a scene | V1 (by hand) | |
| SC-004 scene page under 150 ms | `scene::the_card_title_shows_before_the_details_and_stays_after` | `nav-first-paint` |
| SC-005 web UI shows the resume point | V3 (by hand) | `gql-viewing-10min-writes` |
| SC-006 no history lost | the FR-014 and FR-016 tests | |
| SC-007 preview start; wall smoothness | | `preview-start`, `scenes-scroll-1000-wall` |
| SC-008 a filtered tab comes back exactly | `scenes_flows::opening_a_card_and_going_back_returns_the_exact_place`; `snapshot::a_filtered_scenes_tab_round_trips_without_a_request` | |
| SC-009 007's budgets still pass | | every 007 row (see the summary below) |

## Harness summary (T090, Testing, 2026-10-10)

Full run with `--allow-writes`, then a quick run with the playback scenes pinned
(`--scenes 1010,1011,1012 --long 1010`), because the automatic choice took the newest scene,
1022, the deliberately undecodable V5 file.

- **Every `gql-*` row is within budget**: page 50 and 1000, filter apply 2, picker cold 1 and
  cached 0, tags picker cold 1, vocabulary, scene page, files, galleries 1 each, chip sweep 0, chip
  rest 1 with 0 downloads, chip portrait 0 and 0, tag colours save 1, 10-minute viewing 12 (11 +
  1).
- **008's new rows pass**: picker open 14 ms and narrow 14 ms, preview start 85 ms (budget
  300 ms after the 200 ms rest), chip portrait 21 ms, wall scrolling 0.3–0.7% missed frames.
- **007's rows pass at the constitution's budgets (5.1.0)**: cold start, navigation first paint
  (14 ms), input acknowledgement (14 ms), page change (21 ms), page jump (29 ms), list scrolling
  (0.4%, budget 1%), grid scrolling on the 1000-scene page (2.4–2.7%, budget 3% at 144 Hz),
  playback open (28 ms) and seek to frame (82 ms).
- **Grid scrolling, for the record**: these runs predate the 3% budget and failed the old 1%; the
  last commit before 008 (`96ba9a5`, built in its own checkout) measured 2.3% on the same machine
  the same day, so 008 didn't change it.
- **T095 (2026-10-10, quick run, scenes pinned, 144 Hz)**: on a quiet machine every budget passed
  and the harness exited 0: grid scrolling 0.6% (budget 3%), list 0% and wall 0.1% (budget 1%),
  page change p95 30 ms, preview start 71 ms, measured at 143–145 Hz (frame-time baseline
  6.9–7.0 ms). A run minutes earlier, while the machine's load average was over 40 (nothing from
  Stash), gave grid 6%, list 3.2%, and wall 2.4%: the earlier grid failures were load, not the
  code.
- **Not measured**: the long-playback rows (dropped frames, displayed fps, main-thread CPU) said
  "window not drawn": the app's window wasn't on screen during that stage. They need a run with
  the window visible.

## Gate

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`, and the harness with every 007 row still passing (SC-009).

## V2a: Performer picture versions (research R13; Testing only, it writes a performer)

1. Open scene 185 and the Performers picker: performers 2 and 3 show their pictures.
2. In Stash's web UI (Testing), replace performer 2's picture.
3. After the scene details and the performer list refresh (or after clearing the cache), the scene
   page's chip, the picker row, and the chip card's portrait show the new picture; the old one
   never comes back.
4. Remove performer 3's picture: its chip and picker row show no picture, and the bench's
   `gql-picker-cold` row lists no `performer-image` download for it.
