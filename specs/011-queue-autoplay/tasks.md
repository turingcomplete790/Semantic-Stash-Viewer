---

description: "Task list for 011 Queue Autoplay and Playlist Controls (revised plan)"
---

# Tasks: Queue Autoplay and Playlist Controls

**Input**: Design documents from `specs/011-queue-autoplay/` (revised 2026-10-11 after the playlist
clarification)

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/queue-autoplay.md](contracts/queue-autoplay.md),
[quickstart.md](quickstart.md); the 010 queue view on `main`.

**Starting point**: the uncommitted first 011 pass (click and rest play, autoplay on choosing
Queue, results change, and tab switch, a Loop switch on `L`, a loop branch in
`ScenesState::advance`, a skip counter in the session). Tasks below keep what still holds and
replace the rest with the state machine.

**Tests**: included. The state machine is tested exhaustively in `stash-core` (the owner's design
constraint: every Autoplay × Repeat × Shuffle × event combination without a player or server);
`native-ui/tests/queue.rs` covers the UI's wiring. Tests come first in each phase.

**Organization**: the state machine and its wiring are foundational (every story's behaviour goes
through it). Then US1 (click to play), US2 (Autoplay), US3 (Shuffle and Repeat), all P1.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1, US2, US3 (spec.md)

---

## Phase 1: Setup

- [X] T001 Create `crates/stash-core/src/playback/playlist.rs` (module doc: 011 research R1, "the
      queue's playback rules as data and pure functions; no clock, randomness, I/O, or UI") and
      register it in `crates/stash-core/src/playback/mod.rs`
- [X] T002 [P] Create `crates/stash-core/tests/playlist.rs` with a helper that builds a
      `Playlist` from settings, a count, and a playing index

---

## Phase 2: Foundational (blocks every story)

**Purpose**: the pure state machine, the saved settings, and the UI wiring that turns its commands
into 010's actions. After this phase the queue behaves as before (Autoplay on, Repeat Off,
Shuffle off) with every decision made by the machine.

### Tests for the state machine

- [X] T003 [P] Shuffle order tests in `crates/stash-core/tests/playlist.rs`: for counts 1, 2, 3,
      7, 50, 1000 and several seeds, `order(round, k)` for `k in 0..count` visits each index of
      `0..count` exactly once ("A round's order visits each index of `0..count` exactly once"), the
      round's first is the given `first`, and `position_of` inverts `order`; a 1,000,000-scene
      round computes its 999,999th position without allocating a list (O(1), research R9)
- [X] T004 [P] Transition tests in `crates/stash-core/tests/playlist.rs`, one per row of the
      data model's transition table, for both Shuffle off and on: `Ended` with Repeat One →
      `Replay`; `Ended` with autoplay off → `Stop`; `Ended`/`Next`/`Skipped` not at the end → the
      next index (list or round order); at the end with Repeat All → `Play(0)` or a new round's
      first; at the end with Repeat Off → `Stop`; `Previous` → the previous, `Nothing` at 0 or the
      round's first ("`Previous` never wraps in list order; in shuffle it stops at the round's
      first"); `Chosen(i)` → `Play(i)`, `Nothing` when `i` is playing, and a new round from `i`
      when shuffling; `Autostart(i)` → `Play(i)` only with autoplay ("`Autostart` with
      `autoplay == false` → `Nothing`"); `ResultsChanged` clears `playing` and the round;
      `ToggleShuffle` on starts a round from `playing`, off clears it; `CycleRepeat` cycles Off →
      All → One → Off; `Failed` → `Stop` in every state; `Skipped` stops once `skips` reaches
      `count` and `Started` resets it; `count == 0` → `Nothing`/`Stop` for every event; Repeat one
      never holds `Next`/`Previous` on the same scene
- [X] T005 [P] A property test in `crates/stash-core/tests/playlist.rs`: from any starting state,
      repeated `Ended` with Autoplay on, Shuffle on, Repeat All plays every index once before any
      index repeats (SC-004), and with Repeat Off stops after exactly `count` scenes

### The state machine

- [X] T006 Implement the types in `crates/stash-core/src/playback/playlist.rs` per data-model.md:
      `Repeat { Off, All, One }` (serde, default `Off`), `Settings { autoplay, repeat, shuffle }`,
      `Round { seed: u64, first: u32, pos: u32 }`, `Playlist { settings, count, playing:
      Option<u32>, round: Option<Round>, skips: u32 }`, `Event` (`Ended`, `Failed`, `Skipped`,
      `Next`, `Previous`, `Chosen(u32)`, `Autostart(u32)`, `ResultsChanged { count, seed }`,
      `SetAutoplay(bool)`, `CycleRepeat`, `ToggleShuffle { seed }`, `Started(u32)`), `Command`
      (`Play(u32)`, `Replay`, `Stop`, `Nothing`); all `Clone`, `PartialEq`, `Debug`
- [X] T007 Implement the shuffle order in `crates/stash-core/src/playback/playlist.rs`: a seeded
      Feistel permutation over the next power of two with cycle-walking below `count`, and
      `order(round, count, k)` / `position_of(round, count, i)` putting `first` at position 0 and
      the permutation without `first` after it (research R3)
- [X] T008 Implement `pub fn step(state: &Playlist, event: Event) -> (Playlist, Command)` in
      `crates/stash-core/src/playback/playlist.rs` as one `match` on the event and settings,
      returning a new value (no mutation of the input), exactly per the transition table; a new
      round at Repeat All's end derives its seed from the previous seed deterministically
      (e.g. a splitmix step) so `step` stays pure

### Settings and wiring

- [X] T009 Replace the saved settings in `native-ui/src/screens/scenes/state.rs`: `play_next`
      becomes `autoplay: bool` (`#[serde(default = "yes", alias = "play_next")]`); add `repeat:
      Repeat` (`#[serde(default)]`) and `shuffle: bool` (`#[serde(default)]`); remove the first
      pass's `loop_queue`; add `#[serde(skip)] playlist: Playlist`, kept in step with the saved
      settings and the page's `count()`; update `persistent()` and `Default`
- [X] T010 Snapshot test in `native-ui/tests/snapshot.rs` (replacing the first pass's `loop_queue`
      checks): a 010 session with `play_next: false` loads with `autoplay == false`; a session with
      none of the fields loads with Autoplay on, Repeat Off, Shuffle off; a tab with Repeat One and
      Shuffle on round-trips them
- [X] T011 Index-based pending play in `native-ui/src/screens/scenes/queue.rs` and `state.rs`:
      `PendingPlay::Index(u32)` replaces `First`/`Last` ("play result index `i` when its page
      arrives"); keep `Start`, `Keyboard`, `None`; the arrival path plays card `i - (page-1)*size`
- [X] T012 Command execution in `native-ui/src/screens/scenes/state.rs`: `fn apply(&mut self,
      cmd: Command, layout) -> Advance` maps `Play(i)` to `queue_play(local)` when `i` is on the
      page, else `go_to_page(i / page_size + 1)` with `PendingPlay::Index(i)` (returning
      `Advance::Loading`); `Replay` to a new `Advance::Replay { id }`; `Stop`/`Nothing` to
      `Advance::Stop`; replace `ScenesState::advance` with `fn queue_event(&mut self, event:
      Event, tab, layout) -> Advance` that steps the machine and applies the command (the loop
      branch and `loop_queue` go)
- [X] T013 Session wiring in `native-ui/src/session/mod.rs`: `ended(id)` sends `Event::Ended`
      to the owner's queue; `SceneOpened(Err(SceneNotFound))` sends `Event::Skipped` (the
      session's `queue_skips` field goes: the machine counts); an Error snapshot sends
      `Event::Failed`; a snapshot reaching `Playing` sends `Event::Started(index)`;
      `queue_step(forward)` sends `Next`/`Previous`; `queue_advance` handles `Advance::Replay`
      by `playback.open_at(id, tab, Some(0.0))` (a new viewing from the beginning, never the
      resume point)
- [X] T014 Run the existing 010 and first-pass 011 tests in `native-ui/tests/queue.rs` against the
      machine-backed queue and fix what the wiring broke (expected unchanged: Play next across
      pages, Previous across pages, deleted-scene skip, failure never advances)

**Checkpoint**: `cargo test --workspace` passes; the queue behaves as before with the machine
deciding.

---

## Phase 3: User Story 1 - Click a scene, it plays (P1) 🎯 MVP

**Goal**: a click, the keyboard's rest, and Enter play through `Event::Chosen` (research R6); the
first pass's behaviour, now through the machine.

**Independent Test**: quickstart V1.

- [X] T015 [P] [US1] Keep the first pass's US1 tests in `native-ui/tests/queue.rs` passing
      (`clicking_another_row_switches_playback`, `clicking_the_playing_row_doesnt_restart`,
      `the_keyboard_plays_where_it_rests`, `passing_rows_plays_nothing`,
      `a_rest_on_the_playing_scene_plays_nothing`, `a_click_loads_at_once`,
      `clicking_a_row_shows_it_in_the_panel`); add `a_click_plays_with_autoplay_off`
- [X] T016 [US1] Route `ScenesMsg::QueueSelect`, `QueuePlay`, Enter, and the keyboard rest's play
      through `Event::Chosen(global index)` in `native-ui/src/screens/scenes/state.rs` (replacing
      `queue_play_step`'s own "is it playing" check with the machine's)

**Checkpoint**: quickstart V1; T015 passes.

---

## Phase 4: User Story 2 - Autoplay: the queue plays by itself (P1)

**Goal**: Autoplay (YouTube-style) gates automatic starts and moving on; off, only choices play
(research R5).

**Independent Test**: quickstart V2.

- [X] T017 [P] [US2] Tests in `native-ui/tests/queue.rs`: with Autoplay off, choosing Queue,
      changing results, switching to an idle queue tab, and a pending start on arrival play
      nothing (`autostart_respects_autoplay_off`); a scene ending stops
      (`with_autoplay_off_a_scene_stops_at_its_end`, replacing 010's Play next test); the panel's
      "Next" button moves on once; `A` toggles Autoplay; the first pass's autoplay tests stay
      (with Autoplay on)
- [X] T018 [US2] Send `Event::Autostart(i)` for choosing Queue, results arriving after a change
      (`PendingPlay` after `results_changed`), `PendingPlay::Start`, and the session's
      `autoplay_on_switch`, in `native-ui/src/screens/scenes/state.rs` and
      `native-ui/src/session/mod.rs`; `ScenesMsg::TogglePlayNext` becomes `ToggleAutoplay` (→
      `Event::SetAutoplay`)
- [X] T019 [US2] The panel's end-of-scene button in `native-ui/src/session/mod.rs` (the stack over
      the player): shown when Autoplay is off and the scene has ended, labelled "Next", sending
      `ShellMsg::QueueStep(true)`; remove 010's "Play next: on/off" and the first pass's "Loop"
      buttons from `native-ui/src/screens/scenes/controls.rs`

**Checkpoint**: quickstart V1–V2; T017 passes.

---

## Phase 5: User Story 3 - Shuffle and Repeat (P1)

**Goal**: the playlist strip with Shuffle 🔀, Repeat (Off → All 🔁 → One 🔂), and Autoplay;
`X`, `R`, `A`; Repeat one replays from 0 as a new viewing; Shuffle crosses pages (research R3,
R4, R8).

**Independent Test**: quickstart V3–V4.

### Tests for User Story 3

- [X] T020 [P] [US3] Repeat tests in `native-ui/tests/queue.rs`: `repeat_one_replays_from_the_start`
      (Repeat One, a scene ends → `OpenScene { start: Some(0.0), resume: false }` of the same id, with
      Autoplay on and off); `next_leaves_repeat_one_on`; `repeat_all_returns_to_page_one` (the first
      pass's `the_loop_returns_to_page_one`, now with Repeat All); `repeat_off_stops_at_the_end`
      (replacing `the_last_pages_last_scene_stops`'s Loop toggle); `r_cycles_repeat` (Off → All →
      One → Off); the first pass's `a_one_page_loop…`, `a_one_scene_queue…`,
      `deleted_scenes_stop…` with Repeat All instead of Loop; remove `l_toggles_loop`
- [X] T021 [P] [US3] Shuffle tests in `native-ui/tests/queue.rs`: `shuffle_plays_each_scene_once`
      (500 scenes at 50 a page, Shuffle on, `n` repeatedly answering page requests: 500 distinct
      ids before any repeats, the list's page following each pick); `previous_retraces_the_round`;
      `turning_shuffle_off_continues_in_list_order`; `new_results_start_a_new_round`;
      `choosing_a_scene_while_shuffling_starts_from_it`; `x_toggles_shuffle`

### Implementation for User Story 3

- [X] T022 [P] [US3] Icons in `native-ui/src/widgets/mod.rs`: `Icon::Shuffle`, `Icon::Repeat`,
      `Icon::RepeatOne` (Lucide `shuffle`, `repeat`, `repeat-1` paths, under the file's existing
      Lucide ISC notice), and `Icon::Autoplay` (a switch with a play glyph, in the same stroke
      style)
- [X] T023 [US3] The playlist strip in `native-ui/src/screens/scenes/queue.rs`: a row above the
      list with three icon buttons (tooltip "Shuffle: on/off", "Repeat: off/all/one", "Autoplay:
      on/off"), the active state in `theme::FROSTED_BLUE` and the inactive in `theme::MUTED`;
      messages `ScenesMsg::ToggleShuffle`, `CycleRepeat`, `ToggleAutoplay`; the list's rows and
      pager below it, so the list's scroll and windowing are unchanged
- [X] T024 [US3] `ScenesMsg::ToggleShuffle` and `CycleRepeat` in
      `native-ui/src/screens/scenes/state.rs`: update the saved field and step the machine
      (`ToggleShuffle { seed }` with a seed from `new_seed()`); `results_changed` sends
      `ResultsChanged { count, seed }` once the count is known (page arrival)
- [X] T025 [US3] Keys in `native-ui/src/shell/keymap.rs` and `native-ui/src/session/mod.rs`
      (`queue_key`): `X` → `Action::QueueShuffle` ("Turn Shuffle on or off"), `R` →
      `Action::QueueRepeat` ("Repeat: off, all, one"), `A` stays (now "Turn Autoplay on or off");
      remove `Action::QueueLoop` and `L`; all three work in fullscreen
- [X] T026 [US3] Keep the list following shuffled picks in `native-ui/src/screens/scenes/state.rs`:
      a `Play(i)` on another page shows that page with the card selected and in view (the
      arrival path's reveal, as 010's pending play does for the last card)

**Checkpoint**: quickstart V1–V4; T020–T021 pass.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [X] T027 [P] Keymap test in `native-ui/tests/keymap.rs`: `the_queue_views_keys_are_listed`
      includes `X`, `R`, `A` and not `L`; `x` and `r` don't resolve at `Level::Player` or
      `Level::Scenes`
- [X] T028 [P] Bench rows in `native-ui/src/measure/bench.rs` (play history off, as 010's
      `queue-next`): `queue-click-play` (click a row → playing, 3 samples); `queue-autoplay` (Grid
      → Queue → playing, 3 samples); `queue-repeat` (Repeat All: last page's last scene seeked to
      its end → page 1's first playing; Repeat One: a scene seeked to its end → playing again);
      `queue-shuffle` (Shuffle on, a scene seeked to its end → the pick playing, 3 samples); adjust
      010's `queue-select` for click-to-play (stop playback after it)
- [X] T029 [P] Budgets in `crates/perf-harness/src/main.rs`: the four rows ≤ 1500 ms (median)
- [ ] T030 Run `cargo run -p perf-harness -- --profile "localhost:9998" --quick` on a quiet machine
      (load average under 4): every 011 row and every 007–010 row within budget (SC-005)
- [X] T031 [P] Docs: README (click plays; Autoplay, Shuffle, Repeat and their keys in the queue
      view), `ROADMAP.md` (tick the Phase 1 queue autoplay item, link 011, rename it to playlist
      controls), and a requirement-to-test table (FR-001–FR-011, SC-001–SC-005) in
      `specs/011-queue-autoplay/quickstart.md`
- [X] T032 Gate: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
      `cargo test --workspace`; then quickstart V1–V4 on Testing (the user verifies before any
      commit)

---

## Dependencies & Execution Order

- **Setup (1)** → **Foundational (2)** → **US1 (3)** → **US2 (4)** and **US3 (5)** (either order;
  both touch `state.rs`, `session/mod.rs`, and `tests/queue.rs`, so in sequence) → **Polish (6)**
- Within Phase 2: tests T003–T005 before T006–T008 (the machine), then T009–T014 (wiring), which
  need the machine.

### Parallel opportunities

- Phase 1: T001 ∥ T002.
- Phase 2: T003 ∥ T004 ∥ T005 (test functions in one new file, written together); T010 (snapshot
  test) ∥ T011 once T009 is in.
- US3: T020 ∥ T021; T022 (`widgets/mod.rs`) ∥ the tests.
- Polish: T027 ∥ T028 ∥ T029 ∥ T031.

### Parallel example: Foundational

```text
Together: T003, T004, T005  machine tests in crates/stash-core/tests/playlist.rs
Then:     T006 → T007 → T008  the machine in crates/stash-core/src/playback/playlist.rs
Then:     T009 → T011 → T012 → T013 → T014  wiring in native-ui (T010 alongside)
```

---

## Implementation Strategy

### MVP (Phases 1–3)

The state machine, wired in, with click to play. The queue behaves as the first pass did, but
every decision comes from one tested pure function.

### Incremental delivery

1. Setup + Foundational → the machine decides; behaviour unchanged.
2. US1 → click/rest/Enter through the machine (MVP).
3. US2 → Autoplay as a real switch.
4. US3 → Shuffle, Repeat, the strip and keys.
5. Polish → bench, budgets, docs, gate; commit after the user's check.
