---

description: "Task list for 007-port-to-iced"
---

# Tasks: Port the Viewer to iced

**Input**: Design documents from `specs/007-port-to-iced/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/session-snapshot.md](contracts/session-snapshot.md),
[contracts/measurements.md](contracts/measurements.md), [quickstart.md](quickstart.md)

**Tests**: Included. The spec requires every capability and every behaviour that stays to be
covered by tests (FR-015, research R12). Write each story's tests first, as transition tests
(pure functions over state and message), and check that they fail.

**Rules that apply to every task**:
- **The UI is a hierarchical state machine in iced's state** (research R1).
  - **Structure**: enums are exclusive states and structs are regions active at once. Each level's
    `update` returns a `Step { effects, up }`; unhandled events bubble to the parent; states have
    `enter` and `exit`.
  - **No routes**: a tab's history holds whole `Screen` states.
- **Effects are data** (R2): no I/O, decoding, or blocking in `update` or `view`. Transitions return
  `Effect` values; `effects.rs` turns them into tasks over the service layer; results come back
  as typed messages carrying a generation, and stale ones are dropped.
- **The native app's own files only** (R4): everything lives under `dev.semantic-stash-viewer/`
  (config, data, cache). Nothing reads, writes, or migrates the demo's `semantic-stash-viewer/`
  files.
- **Behaviours that stay** (spec Overview) hold exactly; everything else is designed fresh.
- **Stash**: no writes added; nothing assumes read-only access (spec FR-002). Measurements use the
  Testing profile (`localhost:9998`); Production is for manual checks only.
- **Unsafe code**: only in the video path, behind owning types; the lints land in US4 (R11).
- **Each story** ends with its capability-checklist rows, a manual check with the user, and the
  gate. Commit only after the user has looked.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependencies on unfinished tasks)
- **[Story]**: the user story the task belongs to (US1–US6)

---

## Phase 1: Setup

**Purpose**: a fair baseline, the harness in its own crate, and the native app's identity and
folders.

- [X] T001 Run `cargo run --bin perf-harness -- --profile "Testing" --app web` (the demo, before
  the harness moves) and record every row in `specs/007-port-to-iced/quickstart.md` under a new
  "Results → Baseline (demo on Testing)" heading. This is SC-008's reference (contracts/measurements.md
  "Baseline").
- [X] T002 [P] Move the harness to its own crate:
  - Copy `src-tauri/src/bin/perf_harness.rs` and `src-tauri/src/bin/perf_harness/report.rs` into
    `crates/perf-harness/src/main.rs` and `crates/perf-harness/src/report.rs`.
  - Add `crates/perf-harness/Cargo.toml` (package `perf-harness`, deps `serde`, `serde_json`,
    `chrono`) and the workspace member.
  - Change `--app` to default to `native`.
  - Delete the old bin from `src-tauri` and update `README.md`'s harness command.
  - `cargo test -p perf-harness` passes (its 9 tests).
- [X] T003 [P] Create `specs/007-port-to-iced/capabilities.md` with one row per capability in the
  spec Overview's table and one per "behaviour that stays". Columns: capability, acceptance
  scenario for the new UI (to fill per story), tests, result, date. End with a sign-off line for
  the user.
- [X] T004 [P] The native app's folders and identity:
  - In `native-ui/src/services/paths.rs` (split from `native-ui/src/services.rs`), resolve
    config, data, and cache under `dev.semantic-stash-viewer/` (research R4): `profiles.json`,
    `session.json`, `notifications.json`, `cache/`, `logs/`.
  - Remove 006's `tabs-native.json`/`notifications-native.json` paths.
  - Set the window's Wayland `app_id` / X11 class to `dev.semantic-stash-viewer` in
    `native-ui/src/app.rs`.
  - Copy the icon from `src-tauri/icons` to `native-ui/assets/`.
  - Add iced's `svg` feature in `native-ui/Cargo.toml`.
  - Update `native-ui/tests/services.rs`: the native paths, and nothing under the demo's
    `semantic-stash-viewer/` folders is opened.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: typed GraphQL through Cynic in the core, and the state machine skeleton every story
builds on.

**⚠️ CRITICAL**: No user story work begins until this phase is complete.

### Cynic in the core (research R5)

- [X] T005 Register Stash v0.31.1's schema for Cynic:
  - Take the SDL from the Stash source at tag `v0.31.1` (`git -C ~/stash-source/stash show
    v0.31.1:graphql/schema/…`, concatenating `schema.graphql` and `types/*.graphql`), or convert
    the committed v0.31.1 introspection `crates/stash-core/graphql/schema.json` if the tag isn't
    available. Save it as `crates/stash-core/graphql/stash-v0.31.1.graphql`.
  - Document the steps in `crates/stash-core/graphql/README.md`.
  - Add `cynic` 3.14 and the build dependency `cynic-codegen`, plus a `build.rs` that registers
    the schema.
  - Set `rust-version = "1.85"` in `crates/stash-core/Cargo.toml`.
- [X] T006 Move every operation in `crates/stash-core/src/adapter/` (`probe.rs`, `health.rs`,
  `scenes.rs`, `jobs.rs`) from `graphql_client` to Cynic query structs:
  - Shared fragments per view tier: `SceneCardFields` (grid card) and `PlayableSceneFields`
    (player).
  - The custom scalars `Map`/`Any` map to `serde_json::Value`.
  - `adapter/mod.rs` sends Cynic's operation as the JSON body through the existing `reqwest`
    client (same headers, API key, and TLS handling) and decodes Cynic's typed response.
  - Remove `graphql_client` and the `.graphql` operation files it used (keep
    `jobs_subscribe.graphql` if T007 keeps the hand-written subscription).
  - **Proof**: `cargo test -p stash-core` passes unchanged, recorded fixtures included.
- [X] T007 Jobs subscription: check whether `graphql-ws-client` (Cynic's companion) accepts the
  lenient-TLS connector the core uses (Principle VII). If yes, move the subscription in
  `crates/stash-core/src/jobs/`; if not, keep the hand-written `graphql-transport-ws` client.
  Record the outcome in `research.md` R5.
- [X] T008 Smoke check after the switch: the demo (`cargo run -p semantic-stash-viewer`) and the
  006 native build still connect, browse, and play on Testing; `cargo clippy --workspace
  --all-targets -- -D warnings` passes.

### The state machine skeleton (research R1–R3)

- [X] T009 Write `native-ui/src/machine.rs`:
  - `Step<Up> { effects: Vec<Effect>, up: Option<Up> }` with helpers (`Step::none()`,
    `Step::effect(…)`, `Step::up(…)`, combining child steps).
  - The `enter`/`exit` convention: a function returning `Vec<Effect>`.
  - A doc comment stating the rules: no I/O in `update`; unhandled events bubble.
- [X] T010 Write `native-ui/src/effects.rs`:
  - The `Effect` enum from data-model.md "Effect (data)", each carrying the address of the state
    that asked for it (tab id, plus a generation where results can go stale).
  - The executor that maps each effect to an iced `Task` over the service layer and wraps the
    result in the right message.
- [X] T011 Split `native-ui/src/services.rs` into `native-ui/src/services/{mod.rs,cache.rs,paths.rs}`
  (paths from T004):
  - Start the core's `JobsWatcher` with the session.
  - Record "last used" on connect (`profiles::service::mark_used`).
  - Add profile management (`create_profile`, `update_profile`, `delete_profile`,
    `reorder_profiles`) and a `NotificationCenter` on the native path.
  - Keep the 006 player start and the video surface.
- [X] T012 Rewrite `native-ui/src/app.rs` as the root of the machine:
  - `App` = `Onboarding(ServerForm)` | `Session(Session)` (stubs for both), with the root
    `Message` tree.
  - Subscriptions: connection snapshots, player snapshots, the notification `watch`, keyboard
    events *with their capture status*, video frames, and window frames only while needed.
  - Run every `Step`'s effects through `effects.rs`.
  - Drop 006's recently-added Home list.
  - 006's playback measurement (`measure/playback.rs`) still works.
- [X] T013 [P] Write `native-ui/src/widgets/`: svg icons (the demo's set, redrawn as needed), a
  one-line text that clips or ellipsizes (whichever iced 0.14 supports, recorded), and shared
  button styles.

**Checkpoint**: the core runs on Cynic with unchanged tests; the native app starts into an empty
Onboarding or Session through the new root, using its own folders.

---

## Phase 3: User Story 1 - Connect and manage servers (Priority: P1) 🎯 MVP

**Goal**: a new user gets from first launch to a connected session, sees the connection's state
and security at all times, and switches between servers (spec US1).

**Independent Test**: quickstart V1, starting with no native files.

### Tests for User Story 1

- [X] T014 [P] [US1] `native-ui/tests/messages.rs`:
  - every `ConnectFailure` and `AppError` variant has a message;
  - titles are distinct per failure kind (001 SC-003);
  - the cases from the demo's `ui/src/__tests__/failures.test.ts`, re-expressed for the new
    wording.
- [X] T015 [P] [US1] `native-ui/tests/onboarding.rs` (transition tests):
  - an empty or invalid address can't be saved;
  - Save emits `SaveProfile` then `Connect`;
  - a failure shows its message and keeps the form;
  - success moves `App` to `Session`;
  - the API key field is hidden by default and can be shown.
- [X] T016 [P] [US1] `native-ui/tests/connection.rs` (transition tests):
  - snapshots move the region between `Connecting`, `Connected`, `Offline`, `AuthFailed`, and
    `Failed`;
  - entering `Connected` re-requests screens whose data isn't ready;
  - `AuthFailed` opens the key prompt overlay;
  - a server switch saves the session, exits the old Session, and enters the new one;
  - removing the last server returns to `Onboarding`.

### Implementation for User Story 1

- [X] T017 [US1] Check iced 0.14 text input on KDE Wayland (typing, paste, IME composition,
  `secure` fields) in a scratch form and record the result in `research.md` R7 (a known gap if
  IME misbehaves).
- [X] T018 [US1] `native-ui/src/onboarding.rs`:
  - `ServerForm` state and view: address, API key (secure, show/hide), strict TLS, connect;
  - plain-language failures from `messages.rs` (T019);
  - initial focus on the address as an entry effect.
- [X] T019 [US1] `native-ui/src/messages.rs`: an exhaustive `match` giving `{ title, detail, hint }`
  for every `ConnectFailure` and `AppError` (research R8).
- [X] T020 [US1] `native-ui/src/session/connection.rs`:
  - the connection region driven by the core's snapshots;
  - the indicator view (state plus unencrypted / unverified / verified, Principle VII);
  - a details panel with the server, version, and security.
- [X] T021 [US1] The key prompt overlay (`AuthFailed`) and the server menu overlay (list, switch,
  "manage servers" which opens Settings → Servers) in `native-ui/src/shell/overlays.rs`, with the
  Session switch transition in `native-ui/src/session/mod.rs`.
- [X] T022 [US1] Write US1's rows in `capabilities.md`, run quickstart V1 with the user, and pass
  the gate.

**Checkpoint**: first launch → connected; switching servers works; failures read plainly.

---

## Phase 4: User Story 2 - The app shell: navigation bar and tabs (Priority: P2)

**Goal**: sections, tabs that never reset, back and forward, overlays, and full restore on
relaunch (spec US2, FR-006, FR-007).

**Independent Test**: quickstart V2.

### Tests for User Story 2

- [X] T023 [P] [US2] `native-ui/tests/tabs.rs` (transition tests):
  - opening a view pushes a `Screen` and drops entries after the cursor;
  - back and forward move the cursor and return the exact earlier state;
  - history keeps at most 50 entries, dropping the oldest;
  - at most 50 tabs;
  - closing a tab selects its neighbour, and the last tab can't be closed;
  - switching tabs exits and enters screens without resetting their state (entering a ready
    screen emits no load);
  - "open in new tab" from a screen bubbles to the shell.
- [X] T024 [P] [US2] `native-ui/tests/snapshot.rs` (contracts/session-snapshot.md):
  - a session with several tabs, histories, scroll positions, and a selected tab round-trips
    exactly;
  - transient fields aren't written;
  - an unknown version or a broken file is renamed `session.json.bad` and a fresh Home tab opens;
  - writes are atomic;
  - each profile keeps its own tabs.
- [X] T025 [P] [US2] `native-ui/tests/keymap.rs`:
  - key events captured by a field never reach the keymap;
  - the innermost active binding wins and unbound keys bubble up;
  - the table lists every binding once with a label;
  - the player's bindings are exactly 002's (space, ←/→, ↑/↓, `[` `]` `\`, `.` `,`, `f`, `m`,
    Escape).
- [X] T026 [P] [US2] `native-ui/tests/shell_flows.rs` (`iced_test`): the navigation bar opens each
  section; the tab strip selects, closes, and middle-click closes; Ctrl+T / Ctrl+W / Ctrl+Tab /
  Ctrl+1–9 work; keyboard help opens and Escape closes it.

### Implementation for User Story 2

- [X] T027 [US2] `native-ui/src/shell/mod.rs` and `native-ui/src/shell/tab.rs`:
  - the Shell (tabs: at most 50; selected; overlay `None | KeyboardHelp | Notifications |
    ServerMenu | KeyPrompt`);
  - the Tab (`history: Vec<Screen>`, at most 50, and `cursor`), with enter/exit on activation;
  - the `Screen` enum in `native-ui/src/screens/mod.rs`, with Home and stubs for the others.
- [X] T028 [US2] `native-ui/src/session/snapshot.rs` per contracts/session-snapshot.md:
  - serde of persistent fields only (`#[serde(skip)]` on transient data), version, per profile;
  - limits of 50 tabs and 50 entries;
  - `SaveSession` debounced 500 ms after a persistent change and on quit;
  - atomic write; bad files set aside.
- [X] T029 [US2] Shell views in `native-ui/src/shell/`: the navigation bar (Home, Scenes, Settings,
  the notification bell placeholder for US5, the server menu, the connection indicator), the tab
  strip (select, close, middle-click close, reorder by drag or keys), and the keyboard help
  overlay.
- [X] T030 [US2] `native-ui/src/shell/keymap.rs`: the table `(chord, level, action, label)` and
  dispatch down the active branch with bubbling (R6), plus Tab / Shift+Tab focus movement.
- [X] T031 [US2] `native-ui/src/screens/home.rs`: server summary `Loading | Ready(info) |
  Unreachable`, cache first (entry effect `LoadSummary`), and connection details.
- [X] T032 [US2] Write US2's rows in `capabilities.md`, run quickstart V2 with the user (including
  relaunch and a corrupted `session.json`), and pass the gate.

**Checkpoint**: tabs never reset; relaunch restores every tab exactly.

---

## Phase 5: User Story 3 - Browse scenes a page at a time (Priority: P3)

**Goal**: the paged grid and list with thumbnails, sorting, sizes, keyboard, and return to place
(spec US3, FR-008).

**Independent Test**: quickstart V3 and the harness's Scenes rows.

### Tests for User Story 3

- [X] T033 [P] [US3] `native-ui/tests/scenes_keyboard.rs`: the grid's keyboard rules (arrows within
  the page, edges to the previous or next page, Home/End, `[` `]`, Enter and Ctrl+Enter, list mode
  as one column), as a pure function in `native-ui/src/screens/scenes/keyboard.rs`.
- [X] T034 [P] [US3] `native-ui/tests/scenes.rs` (transition tests):
  - entering Scenes emits exactly one `LoadScenesPage`, and re-entering a ready page emits none;
  - a result with an old generation is dropped;
  - a page past the end shows the last page;
  - a new sort goes to page 1;
  - a new page size keeps the first visible scene;
  - page sizes are only 20, 40, 50, 60, 120, 250, 500, 1000 (default 50);
  - a ready page emits neighbour prefetch and next-page thumbnail effects;
  - offline with a cached page shows it, without one shows "unreachable".
- [X] T035 [P] [US3] `native-ui/tests/scenes_flows.rs` (`iced_test`, fixture pages): `]` / `[`
  change page; opening a card and going back returns the exact page, mode, and scroll.

### Implementation for User Story 3

- [X] T036 [US3] `native-ui/src/screens/scenes/state.rs`:
  - `ScenesState` (query, page ≥ 1, page_size, mode, scroll, focused; data `Loading { generation }
    | Ready { page, count, cards } | Unreachable`) and its transitions;
  - effects to `read_cached` the core's `find_scenes_page` (the core's cache keys), plus
    neighbour prefetch.
- [X] T037 [US3] `native-ui/src/screens/scenes/thumbs.rs`: the LRU of image handles keyed by scene
  id and screenshot version, "at most 2,000" (1,200 after research R16); the `LoadThumbnail` effect over `ThumbService`; one
  placeholder; next-page warming.
- [X] T038 [US3] `native-ui/src/screens/scenes/grid.rs` and `card.rs`, designed fresh: responsive
  columns with cards at least 240 px wide; a 16:9 thumbnail; one-line title and details; list rows;
  a visible focus ring; click opens, Ctrl+click and middle-click open in a new tab.
- [X] T039 [US3] `native-ui/src/screens/scenes/controls.rs`:
  - page controls (position and total, first/previous/next/last, go to page, page size), above
    and below the grid;
  - the sort menu (the core's `SceneSort` labels, direction, random seed) and the grid/list
    toggle;
  - empty and unreachable states.
- [X] T040 [US3] `native-ui/src/measure/bench.rs`, Scenes and navigation rows per
  contracts/measurements.md:
  - `nav-first-paint`, `control-press`, `scroll-frame-time`;
  - `scenes-page-change`, with a 1 s glance between changes;
  - `scenes-page-jump`;
  - `scenes-scroll-1000-{grid,list}-cold` (thumbnails arriving) then cached;
  - frame timing from iced's per-frame events.
- [X] T041 [US3] *(Only if T040 shows SC-005 failing)* Row windowing in
  `native-ui/src/screens/scenes/grid.rs`: build the rows in view plus one screen each side, with
  spacers; re-measure.
- [X] T042 [US3] Write US3's rows in `capabilities.md`, run quickstart V3 with the user, and pass
  the gate.

**Checkpoint**: the 1000-card scroll meets its budget cold and cached; return-to-place is exact.

---

## Phase 6: User Story 4 - Open a scene and play it (Priority: P4)

**Goal**: the scene view, in-window playback in the shell, and the video path meeting the
unsafe-code gate (spec US4, FR-009, FR-014).

**Independent Test**: quickstart V4; clippy with the unsafe-code lints.

### Hardening first (research R11)

- [X] T043 [US4] `crates/player/src/render.rs` and `session.rs`: give `Mpv` an owner that outlives
  every render context, so `RenderContext` borrows it without the `transmute` to `'static`. Keep
  `create_renderer`'s preconditions documented under `# Safety`. The player's tests pass.
- [X] T044 [US4] `native-ui/src/player/video/`:
  - owning types with `Drop` for the EGL image, GL texture and framebuffer, and exported Vulkan
    image, grouped in a per-generation frame set freed together on the render thread;
  - a `RenderTarget` (side context plus Wayland connection) that `create_renderer` takes by
    reference;
  - `vk_frames.rs`'s large block split into one-purpose functions with one unsafe operation per
    block and a `// SAFETY:` note on each.
- [X] T045 [US4] Lints:
  - `#![forbid(unsafe_code)]` in `crates/stash-core/src/lib.rs`;
  - deny `clippy::undocumented_unsafe_blocks`, `clippy::multiple_unsafe_ops_per_block`, and
    `unsafe_op_in_unsafe_fn` in `native-ui` and `player`;
  - set `WGPU_BACKEND=vulkan` without `set_var` (`.cargo/config.toml` `[env]` and the harness's
    launch environment, or iced's settings if 0.14 exposes a backend choice);
  - `cargo clippy --workspace --all-targets -- -D warnings` passes.

### Tests for User Story 4

- [X] T046 [P] [US4] `native-ui/tests/playback.rs` (transition tests):
  - `OpenScene` → `Opening` → `Playing`, with the owner tab recorded;
  - switching tabs keeps `Playing` and shows the now-playing bar elsewhere;
  - closing the owner tab closes the player;
  - Escape leaves fullscreen before closing;
  - `Ended` offers replay;
  - `Error` shows a plain message and posts a notification effect;
  - a server switch closes playback.

  006's `tests/controls.rs` and `tests/video_slots.rs` stay.
- [X] T047 [P] [US4] `native-ui/tests/scene.rs` (transition tests): entering a Scene emits
  `LoadSceneDetails`; a restored tab shows its title before details arrive; Play emits
  `OpenScene`; a details failure shows its message.

### Implementation for User Story 4

- [X] T048 [US4] `native-ui/src/screens/scene.rs`, designed fresh: title, cover (the core's
  screenshot through the cache), details, Play; the player view embedded when this tab owns
  playback.
- [X] T049 [US4] `native-ui/src/session/playback.rs`: the playback region around 006's
  `PlayerScreen`. The video widget is in the tree only in the owner tab's active screen; the
  now-playing bar (title, play/pause, back to the scene) appears in other tabs; fullscreen is a
  sub-state.
- [X] T050 [US4] `native-ui/src/measure/bench.rs`: `tab-switch-20-tabs` (with `videoSized`),
  `now-playing-appears`, and `back-to-scene`. The playback run (006) still reports `rendered_fps`
  and `displayed_fps`.
- [X] T051 [US4] Write US4's rows in `capabilities.md`, run quickstart V4 with the user (every
  codec, every control and key), and pass the gate with the new lints.

**Checkpoint**: playback works from the grid inside the shell; the video path meets the unsafe-code
gate.

---

## Phase 7: User Story 5 - Notifications and jobs (Priority: P5)

**Goal**: one notification centre with a badge, toasts that never take focus, connection alerts,
failures, and live Stash jobs (spec US5, FR-010).

**Independent Test**: quickstart V5.

- [X] T052 [P] [US5] `native-ui/tests/notifications.rs` (transition tests):
  - a lost connection makes one entry that updates when it returns;
  - the badge counts unread and active entries;
  - toasts expire and never take focus;
  - mark read, dismiss, and dismiss all;
  - a running job shows progress and finishes.
- [X] T053 [US5] `native-ui/src/shell/notifications.rs`: the Notifications overlay, the badge on the
  navigation bar's bell, and the toast layer (a `stack` layer without focus), all fed by the core's
  `NotificationCenter` `watch`; jobs from the `JobsWatcher` (T011); playback failures posted by the
  playback region.
- [X] T054 [US5] Write US5's rows in `capabilities.md`, run quickstart V5 with the user, and pass
  the gate.

---

## Phase 8: User Story 6 - Settings, cache, and retiring the demo (Priority: P6)

**Goal**: Settings, measurement against the demo, the user's sign-off, then the demo's removal
(spec US6, FR-011–FR-013, SC-007–SC-009).

**Independent Test**: quickstart V6.

### Tests for User Story 6

- [ ] T055 [P] [US6] `native-ui/tests/settings.rs` (transition tests):
  - the Servers editor moves `Editing → Testing → Saving` (or `Error` with its message);
  - delete asks for confirmation;
  - reorder emits `ReorderProfiles`;
  - Troubleshooting reads the cache size on entry, and "Clear cache" reports the bytes freed;
  - Keyboard lists the keymap table.

### Implementation for User Story 6

- [ ] T056 [US6] `native-ui/src/screens/settings/`: `servers.rs` (list, edit, test, delete with
  confirmation, reorder), `keyboard.rs` (the keymap table), `troubleshooting.rs` (cache size, clear
  cache, open log folder through `xdg-open`), and `about.rs` (version, build, licence).
- [ ] T057 [US6] Memory and CPU rows for both apps in `crates/perf-harness/src/main.rs`
  (contracts/measurements.md): `VmRSS` and CPU time summed over the process tree from `/proc`,
  sampled after the bench settles and during the long playback.
- [ ] T058 [US6] Run the full harness for the native app and the demo on Testing on the same day;
  record both in `quickstart.md` "Results"; compare every row against T001's baseline (SC-008).
- [ ] T059 [US6] Stability run: 20 launch, browse, play, and quit cycles (scripted with
  `SSV_HARNESS_EXIT` and `SSV_MEASURE`, plus 5 by hand); record crashes, hangs, and lost tabs
  (expected none, SC-007).
- [ ] T060 [US6] Complete `capabilities.md` (every capability and behaviour that stays passes, with
  its tests) and get the **user's sign-off**. Stop here until it's given.
- [ ] T061 [US6] Remove the demo:
  - Delete `ui/`, `src-tauri/`, the root npm files, and CI's Node and bindings steps in
    `.github/workflows/ci.yml`.
  - Delete core code only the demo used: `crates/stash-core/src/shell/tabs.rs` and the screenshot
    data-URL helper with its `base64` dependency in `crates/stash-core/src/adapter/scenes.rs`.
  - Remove `perf-harness`'s `--app` option.
  - Set the workspace members to `stash-core`, `player`, `native-ui`, `perf-harness`, with
    `rust-version = "1.88"`.
- [ ] T062 [US6] Update `README.md` (the native app only; how to run, test, and measure; the demo's
  old `semantic-stash-viewer/` folders can be deleted by hand) and `ROADMAP.md` (Phase 1 continues
  natively).
- [ ] T063 [US6] Fresh-clone check: clone into a temporary directory and run build, the full gate,
  the app, and `perf-harness --quick` with only the Rust toolchain and system libraries (SC-009);
  record the result.

---

## Phase 9: Polish & Cross-Cutting Concerns

- [ ] T064 Run the full gate: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --
  -D warnings`, `cargo test --workspace`. Confirm no `unsafe` outside the video path
  (`grep -rn unsafe native-ui/src crates`).
- [ ] T065 [P] Check every test file names the capability or behaviour it covers, and every row in
  `capabilities.md` lists at least one test (FR-015).

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (Phase 1)**: none. T001 must run before T002 moves the harness (it uses the old bin).
- **Foundational (Phase 2)**: needs Phase 1. T005 → T006 → T007 → T008 (the core switch); T009
  → T010 → T011 → T012 (the skeleton); T013 alongside. It blocks every story.
- **US1 (Phase 3)**: needs Phase 2.
- **US2 (Phase 4)**: needs US1 (a Session exists to hold the shell).
- **US3 (Phase 5)**: needs US2 (Scenes is a Screen in a Tab).
- **US4 (Phase 6)**: needs US3 (a scene opens from the grid). T043–T045 (hardening) can start
  right after Phase 2 if convenient; they touch only `player` and `player/video`.
- **US5 (Phase 7)**: needs US2 (the shell's overlay); independent of US3 and US4.
- **US6 (Phase 8)**: needs US1–US5; T060 (sign-off) gates T061–T063.
- **Polish (Phase 9)**: last.

### Within stories

- Tests first, and they fail.
- Then state and transitions, then views, then the capability rows and the manual check.

### Parallel opportunities

- Phase 1: T002, T003, T004.
- Phase 2: T013 beside the skeleton; the core switch (T005–T008) beside the skeleton (T009–T012),
  since they're different crates.
- Each story's test tasks together (T014–T016, T023–T026, T033–T035, T046–T047).
- US5 (T052–T054) beside US3 or US4 once US2 is done.
- US4's hardening (T043–T045) beside US2 or US3.

## Parallel Example: User Story 2

```text
Task: "T023 [P] [US2] Tab transition tests in native-ui/tests/tabs.rs"
Task: "T024 [P] [US2] Session snapshot tests in native-ui/tests/snapshot.rs"
Task: "T025 [P] [US2] Keymap tests in native-ui/tests/keymap.rs"
Task: "T026 [P] [US2] Shell flow tests in native-ui/tests/shell_flows.rs"
```

## Implementation Strategy

### MVP first

1. Phases 1–2: the baseline, the harness crate, the core on Cynic, the machine skeleton.
2. US1: from first launch to a connected session. **Stop and check with the user.**

### Then incremental

US2 (shell and restore), then US3 (Scenes), then US4 (playback and hardening), then US5
(notifications), then US6 (Settings, measurement, sign-off, removal). Each story is checked by the
user and committed before the next.

### Notes

- Transition tests are the main safety net: they need no window or server and run in
  milliseconds.
- If hand-written transitions grow error-prone (repeated enter/exit bookkeeping), consider
  `statig` (research R1 fallback) before adding more levels; record the decision.
