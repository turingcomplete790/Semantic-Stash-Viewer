---

description: "Task list for 010 Queue View"
---

# Tasks: Queue View

**Input**: Design documents from `specs/010-queue-view/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/queue-ui.md](contracts/queue-ui.md),
[quickstart.md](quickstart.md)

**Tests**: included. The plan names `native-ui/tests/queue.rs` (state-machine transition tests and
an `iced_test` click flow) and new perf-harness rows; the constitution's budgets (Principles IV, VI)
need them. Within each story, tests are written first and fail before the implementation.

**Organization**: by user story. US1 and US2 are P1; US2 builds on US1's view and selection. US3
(P2) adds the focus split and the remaining keys on top of both.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1, US2, US3 (spec.md)

## Path Conventions

The 007/008 workspace: `native-ui/src/…`, `native-ui/tests/…`, `crates/perf-harness/src/…`. No
`stash-core` or `player` changes are expected (plan, Structure Decision).

---

## Phase 1: Setup

**Purpose**: the new module and test file, registered and compiling.

- [X] T001 Create `native-ui/src/screens/scenes/queue.rs` with a module doc comment (010: the queue
      view, research R1–R4) and register it in `native-ui/src/screens/scenes/mod.rs`
- [X] T002 [P] Create `native-ui/tests/queue.rs` with the shared test helpers the other Scenes tests
      use (a `ScenesState` with a page of cards from `native-ui/tests/scenes.rs`'s fixtures), and one
      placeholder test that builds the state

---

## Phase 2: Foundational (blocks every story)

**Purpose**: the Queue mode, the queue state, and its saved fields. Nothing is visible yet beyond
the mode existing.

- [X] T003 Add `Mode::Queue` to `enum Mode` in `native-ui/src/screens/mod.rs` (doc comment: "List
      beside the selected scene's panel (010 FR-001)"); fix every exhaustive `match` on `Mode` it
      breaks (`native-ui/src/screens/scenes/layout.rs` `cols`/gaps/row height: one column at List's
      row height for now; `native-ui/src/screens/scenes/grid.rs`: List rows for now)
- [X] T004 Make Stash saved filters never select Queue: in `native-ui/src/screens/scenes/state.rs`
      (the `DisplayMode` mapping near `Some(DisplayMode::Wall) => self.mode = Mode::Wall`), leave
      `mode` unchanged when the tab is in Queue and the filter carries no mappable mode, and never map
      any `DisplayMode` to `Mode::Queue` (R6); and when saving a filter from a Queue tab, write List
      *(done: a tab in Queue stays in Queue when a saved filter is applied, whatever its mode; the
      viewer doesn't save filters to Stash yet, so the "write List" part has nothing to change)*
- [X] T005 Define `QueueState` in `native-ui/src/screens/scenes/queue.rs` per data-model.md:
      `selected: Option<usize>` ("index into the page's cards"), `rest: u64` ("generation of the
      pending rest timer (250 ms); a timer for an older generation does nothing"),
      `panel: Option<Box<SceneState>>`, `focus: QueueFocus { List, Panel }` (default `List`),
      `pending_play: PendingPlay { None, First, Last }`; `#[derive(Default)]`, not serialized
- [X] T006 Add to `ScenesState` in `native-ui/src/screens/scenes/state.rs`: `queue_selected:
      Option<String>` (`#[serde(default)]`), `play_next: bool` (`#[serde(default = "yes")]`, default
      `true`), and `#[serde(skip)] queue: QueueState`; session version stays 2
- [X] T007 [P] Snapshot test in `native-ui/tests/snapshot.rs`: a 008 session file (no
      `queue_selected`/`play_next`, no Queue mode) loads with `play_next == true` and
      `queue_selected == None`; a Queue tab round-trips its mode, `queue_selected`, and `play_next`
- [X] T008 Add `Effect::QueueRestDue { tab, generation }` and `Reply::QueueRestDue { tab, generation
      }` to `native-ui/src/effects.rs`, scheduled exactly like `ChipRestDue` (a 250 ms sleep that
      replies with its generation); route the reply to the Scenes tab in
      `native-ui/src/session/mod.rs` as `ChipRestDue` is routed to the Scene tab

**Checkpoint**: `cargo test --workspace` passes; a tab can hold `Mode::Queue` and survives a
restart.

---

## Phase 3: User Story 1 - Click through scenes beside the list (P1) 🎯 MVP

**Goal**: Switching to Queue shows the page as a compact list beside a panel with the selected
scene: cover, title, Play at once, then the scene page's details once the selection rests.

**Independent Test**: quickstart V1: switch to Queue on performer A's scenes, click several rows
(each shows at once, details within a second), hold ↓ (only the resting row loads details), change
the sort (first scene selected), restart (same selection).

### Tests for User Story 1

- [X] T009 [P] [US1] Transition tests in `native-ui/tests/queue.rs`: entering Queue selects card 0
      and enters its panel (one `SceneDetails` effect); `select(i)` by keyboard emits only a
      `QueueRestDue` (no request) and the panel shows card i's title; a stale `QueueRestDue` (older
      generation) does nothing; the current one enters `SceneState` for card i; a click enters at
      once; "`selected` is always within the current page's cards, or `None` on an empty page"
- [X] T010 [P] [US1] Results-change tests in `native-ui/tests/queue.rs`: search, filter, sort, page
      size, and page changes select card 0 and clear `pending_play`; a restored tab with
      `queue_selected` on the page selects that card, otherwise card 0; an empty page has no panel
- [X] T011 [P] [US1] Budget test in `native-ui/tests/queue.rs` (as 008's `gql-chip-sweep` test):
      moving the selection over 20 rows within the rest emits 0 `SceneDetails`; resting emits 1; a
      second visit to a cached scene emits 0 (FR-005, FR-014)

### Implementation for User Story 1

- [X] T012 [US1] Implement selection in `native-ui/src/screens/scenes/queue.rs`:
      `QueueState::select(i, Via::Keyboard | Via::Pointer, tab, cards) -> Vec<Effect>`: keyboard bumps
      `rest`, drops a `panel` for another scene ("a selection change drops a panel for another
      scene"), and returns `Effect::QueueRestDue`; pointer builds the panel at once;
      `rest_due(generation)` builds `SceneState::new(card)` and returns its `enter(tab)` effects;
      keep `ScenesState::queue_selected` in step with the selected card's id
- [X] T013 [US1] Wire the queue into `ScenesState` in `native-ui/src/screens/scenes/state.rs`:
      choosing Queue in the mode switch selects `queue_selected`'s card or card 0; every page
      arrival and results change (search, filter, sort, page size, page) re-selects per data-model
      "Page arrives" / "Results change"; forward `SceneState` messages and replies for the panel's
      scene (by scene id and generation, as 008's Scene tab does) to `queue.panel`
- [X] T014 [P] [US1] Make `SceneState::details_view` usable in the panel in
      `native-ui/src/screens/scene.rs`: add a vertical panel layout (media slot across the top at
      16:9 of the panel width, details scrolling below) without changing the scene page's own
      layout; the media slot takes the cover with Play, or the player (US2)
- [X] T015 [P] [US1] Compact queue rows in `native-ui/src/screens/scenes/grid.rs`: reuse List's row
      parts (small still from the thumbnail cache, title, details line) at a compact height; the
      selected row highlighted with the theme's selection colour; `width(Fill)` on row, keyed column
      and scroll column (008's `list_rows_span_the_whole_width` lesson); windowed like List
- [X] T016 [US1] Layout in `native-ui/src/screens/scenes/layout.rs`: queue list width = max(⅓ of the
      window, 300 px), panel = the rest with a 480 px minimum; below both minimums the panel shows
      only the media slot; below the list minimum plus the media minimum, the list alone shows and
      choosing a scene opens its full page (R7); unit tests for the three cases in the same file
- [X] T017 [US1] The queue view in `native-ui/src/screens/scenes/queue.rs`: `view(state, layout,
      pictures)` composing the list (T015) and the panel (T014); the panel before details arrive
      shows the card's still, title, and Play; an empty page shows the existing empty state with
      Clear filters and an empty panel; "This scene no longer exists" when the panel's details come
      back missing (as the scene page shows it)
- [X] T018 [US1] Mode switch and pointer in `native-ui/src/screens/scenes/controls.rs` and
      `native-ui/src/screens/scenes/grid.rs`: add `mode(Mode::Queue, "Queue")` after Wall; a row
      click selects via Pointer; keep the list's scroll position across selection and panel updates
- [X] T019 [US1] Keyboard selection in list focus in `native-ui/src/screens/scenes/state.rs`: in
      Queue, route `grid_key` (`native-ui/src/screens/scenes/keyboard.rs`, one column) so ↑ ↓ Home
      End move the selection via Keyboard (not open), `[` `]` and edges change page; the list scrolls
      to keep the selection in view
- [X] T020 [US1] Previews: the queue's rows don't start hover previews (they're compact stills); make
      sure `Previews` in `native-ui/src/session/mod.rs` treats Queue like List if List has none, or
      skips Queue rows otherwise, so scrolling stays on the light hover path
- [X] T021 [US1] `iced_test` flow in `native-ui/tests/queue.rs`: switch to Queue from the mode
      switch, click the third row, see its title in the panel and the row highlighted

**Checkpoint**: US1 works alone (quickstart V1); tests T009–T011 and T021 pass.

---

## Phase 4: User Story 2 - Play a run of scenes in the panel (P1)

**Goal**: Play in the panel uses the app's one player; Play next advances on Ended, across pages;
Next/Previous skip; moving the selection never stops playback.

**Independent Test**: quickstart V2: play, let it end (next plays and is selected), cross a page
boundary with page size 20, `n`/`p`, move the selection while playing (playing mark), Play next off
(Replay), scene 1022 (error, no advance).

### Tests for User Story 2

- [X] T022 [P] [US2] Ended tests in `native-ui/tests/queue.rs`: with `play_next`, card i's Ended
      plays and selects i+1; the page's last card's Ended asks for page n+1 with `pending_play =
      First`, and the arriving page plays card 0; the last page's last card stops (no effect beyond
      Replay); `play_next == false` never advances; an Error snapshot never advances ("Play next never
      advances after an Error snapshot"); Ended from a scene owned by another tab, or after the tab
      left Queue, does nothing
- [X] T023 [P] [US2] Next/Previous tests in `native-ui/tests/queue.rs`: Next = Ended's rules; Previous
      on card 0 asks for page n−1 with `pending_play = Last` and plays its last card; Previous on page
      1 card 0 does nothing; a user page or filter change clears `pending_play` ("`pending_play` is
      cleared when its page arrives (played or not) and by any other page or filter change the user
      makes"); a missing (deleted) scene is skipped by Play next
- [X] T024 [P] [US2] Visibility tests (written in `native-ui/tests/queue.rs` beside the other queue
      flows): `player_visible()` is true for the owner Scenes tab in Queue while its selection is
      the playing scene (T030's rule); false after the tab leaves
      Queue (now-playing bar shows); another tab starting playback clears the queue's playing mark

### Implementation for User Story 2

- [X] T025 [US2] The ended signal in `native-ui/src/session/playback.rs`: when a snapshot's `state`
      turns to `PlayerStateKind::Ended` from anything else (not `Error`/failed), report it once as
      `ended: Option<String>` (the scene id) for the session to take; reset on the next open
- [X] T026 [US2] Player in the panel in `native-ui/src/session/mod.rs`: extend `player_visible()`
      (line ~183) with the case "owner tab's current screen is `Screen::Scenes` in `Mode::Queue` and
      the playing scene is one of its cards"; in `view` (line ~1112, the `Screen::Scene(scene) if
      self.player_visible()` arm), add the Scenes-in-Queue arm that hands the player view to the
      queue panel's media slot; fullscreen, Escape, close, and the now-playing bar behave as in 007
- [X] T027 [US2] Play from the queue in `native-ui/src/screens/scenes/queue.rs` and
      `native-ui/src/screens/scenes/state.rs`: the panel's Play, Enter in list focus, and a row
      double-click emit an `Up::Play` for the card with `owner = this tab` through the same path the
      scene page uses (resume point, "Start from the beginning", chapters, seek thumbnails, history,
      transcode fallback: 008)
- [X] T028 [US2] Play next in `native-ui/src/session/mod.rs`: on `playback.ended` for a scene owned
      by a Scenes tab in Queue with `play_next`, call the tab's `QueueState::advance(Next)`: next card
      on the page → play and select it (list scrolls to it); else next page → `Page(n+1)` with
      `pending_play = First`; else nothing (Replay shows); skip cards whose details came back missing
- [X] T029 [US2] Pending play in `native-ui/src/screens/scenes/queue.rs`: on page arrival,
      `First` plays card 0, `Last` plays the last card, then clears `pending_play`; any user-made
      page or results change clears it first (T013's path)
- [X] T030 [US2] Playing mark and selection independence in `native-ui/src/screens/scenes/grid.rs`
      and `native-ui/src/screens/scenes/queue.rs`: the playing row carries a playing mark distinct
      from the selection highlight (Principle XI); moving the selection while playing keeps the
      player, and the panel shows the selected scene with Play (the player stays in the panel only
      while the selection is the playing scene, otherwise the now-playing bar shows within the tab)
- [X] T031 [US2] Play next toggle in `native-ui/src/screens/scenes/controls.rs`: a "Play next" toggle
      beside the mode switch, shown only in Queue, bound to `ScenesState::play_next`; with it off, the
      Ended player shows Replay plus a "Play next" button that advances once (contract, Play next)
- [X] T032 [US2] `n` / `p` in `native-ui/src/session/mod.rs` key routing: in a Scenes tab in Queue
      (list or panel focus, fullscreen included), `n` and `p` call `advance(Next | Previous)` and play;
      the player binds neither, so they fall through to the tab while something plays

**Checkpoint**: US1 and US2 work (quickstart V1, V2); T022–T024 pass.

---

## Phase 5: User Story 3 - Work the queue from the keyboard (P2)

**Goal**: Everything without a pointer: focus split, toggle, open full page, and keys listed.

**Independent Test**: quickstart V3.

### Tests for User Story 3

- [X] T033 [P] [US3] Focus tests in `native-ui/tests/queue.rs`: F6 toggles `focus` List ↔ Panel; with
      Panel focus and something playing, ↑ ↓ ← → go to the player (volume/seek) and don't move the
      selection; with List focus they move the selection; `a` toggles `play_next` in both; `o` opens
      the selected scene's page in the tab, Ctrl+O in a new tab
- [X] T034 [P] [US3] Keymap tests in `native-ui/tests/keymap.rs`: the Scenes (queue view) entries
      exist for ↑↓, Home/End, `[` `]`, Enter, `n`, `p`, `a`, `o`, Ctrl+O, F6; the existing uniqueness
      test passes (no duplicate at the same level, none clashing with Player keys)

### Implementation for User Story 3

- [X] T035 [US3] Focus in `native-ui/src/session/mod.rs` key routing: in a Scenes tab in Queue, F6
      toggles `queue.focus`; Panel focus sends keys to the player first when it plays (as on the scene
      page) and to the panel's chips otherwise; List focus sends ↑↓ Home End `[` `]` Enter to the
      queue (T019, T027); draw a focus outline on the focused area in
      `native-ui/src/screens/scenes/queue.rs`
- [X] T036 [US3] `a`, `o`, Ctrl+O in `native-ui/src/screens/scenes/state.rs`: `a` toggles
      `play_next`; `o` opens `Screen::scene(id, title)` for the selection in this tab, Ctrl+O in a new
      tab (as Enter/Ctrl+Enter do in Grid)
- [X] T037 [US3] Key listing in `native-ui/src/shell/keymap.rs`: add a "Scenes (queue view)" group
      (a new `Level::Queue` labelled "Scenes (queue view)", or entries under `Level::Scenes` tagged
      for the queue) with every key from the contract's table; keyboard help (F1) and Settings →
      Keyboard show it

**Checkpoint**: all three stories work (quickstart V1–V3); T033–T034 pass.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [X] T038 [P] Bench rows in `native-ui/src/measure/bench.rs`: `queue-select` (select a row → panel
      shows cover, title, Play), `gql-queue-sweep` (20 rows within the rest: 0 `SceneDetails`),
      `gql-queue-rest` (rest on one: 1 `SceneDetails`), `queue-next` (play scene 24 on Testing, seek
      to 2 s before its end, time to the next scene playing; a second run crossing a page boundary
      with page size 1 or 20), `scenes-scroll-1000-queue-cold` and `scenes-scroll-1000-queue`
- [X] T039 [P] Budgets in `crates/perf-harness/src/`: `queue-select` ≤ 150 ms, `queue-next` ≤
      1500 ms, `gql-queue-sweep` 0, `gql-queue-rest` 1 `SceneDetails`, `scenes-scroll-1000-queue` ≤ 1%
      missed frames at 144 Hz (lists' budget, constitution 5.1.0; not the grid's 3%)
- [X] T040 Run `cargo run -p perf-harness -- --profile "localhost:9998" --quick` on a quiet machine
      (load average under 4) and confirm every queue row and every 007/008 row passes (SC-005)
- [X] T041 [P] Docs: README (Queue mode and its keys), `ROADMAP.md` (check the Phase 1 "Queue view"
      item; the gallery queue view stays open), and a requirement-to-test table in
      `specs/010-queue-view/quickstart.md` mapping FR-001–FR-014 and SC-001–SC-005 to tests and rows
- [X] T042 Gate: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
      `cargo test --workspace`; then the manual quickstart V1–V3 on Testing (the user verifies before
      any commit)

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (1)** → **Foundational (2)** → **US1 (3)** → **US2 (4)** → **US3 (5)** → **Polish (6)**
- US2 needs US1's selection, panel, and view (T012–T017). US3 needs US1's list keys (T019) and US2's
  play paths (T027, T032). Polish needs all three.

### Within each story

- Tests first (they fail), then state (`queue.rs`, `state.rs`), then views, then session wiring.
- `native-ui/src/screens/scenes/queue.rs`, `state.rs`, and `native-ui/src/session/mod.rs` are
  touched by many tasks: those run in order.

### Parallel opportunities

- Phase 1: T001 ∥ T002.
- Phase 2: T007 ∥ T008 after T006.
- US1: T009 ∥ T010 ∥ T011 (one file, but independent test functions; write together); T014 (scene.rs)
  ∥ T015 (grid.rs) after T012.
- US2: T022 ∥ T023 ∥ T024; T025 (playback.rs) ∥ T029–T030 (queue.rs, grid.rs).
- US3: T033 ∥ T034.
- Polish: T038 ∥ T039 ∥ T041.

### Parallel example: User Story 1

```text
Together: T014 vertical panel layout in native-ui/src/screens/scene.rs
          T015 compact queue rows in native-ui/src/screens/scenes/grid.rs
Then:     T016 layout → T017 view → T018 mode switch → T019 keys → T021 iced_test flow
```

### Parallel example: User Story 2

```text
Together: T025 ended signal in native-ui/src/session/playback.rs
          T030 playing mark in native-ui/src/screens/scenes/grid.rs
Then:     T026 player in panel → T027 play → T028 Play next → T029 pending play → T031 → T032
```

---

## Implementation Strategy

### MVP (US1 only)

Phases 1–3: a Queue mode that clicks through scenes with details beside the list. Stop and check
quickstart V1; it's already useful for browsing, playing through the scene page as before.

### Incremental delivery

1. Setup + Foundational → the mode exists and saves.
2. US1 → click through (MVP); user verifies, commit.
3. US2 → play a run with Play next; user verifies, commit.
4. US3 → full keyboard; user verifies, commit.
5. Polish → harness rows, docs, gate.

Commit only after the user verifies each increment.
