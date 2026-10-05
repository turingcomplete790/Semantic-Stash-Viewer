---

description: "Task list for 006-native-ui-spike"
---

# Tasks: Native UI Spike (iced)

**Input**: Design documents from `specs/006-native-ui-spike/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/measurements.md](contracts/measurements.md),
[contracts/decision-record.md](contracts/decision-record.md), [quickstart.md](quickstart.md)

**Tests**: Included. Research R7 calls for pure-logic tests, `iced_test` flows, and the
harness, and the constitution's gate applies to the new crate. Write each story's tests first and
check that they fail.

**Rules that apply to every task**:
- **The web build doesn't change** (FR-001). `src-tauri` is touched only by the additive harness
  tasks (T008, T040); `stash-core` and `player` get no behaviour changes.
- **Read-only** (FR-003): no writes to Stash (no resume, play count, or O-counter sync), and no
  jobs watcher. `profiles.json` is read, never rewritten. Production (`localhost:9999`) is fine
  for manual checks and the harness.
- **Same files, separate shell state** (R5):
  - `profiles.json` under `<config_dir>/semantic-stash-viewer/`.
  - The view cache under `<cache_dir>/semantic-stash-viewer/`.
  - Tabs and notifications in `<local_data_dir>/semantic-stash-viewer/shell/tabs-native.json` and
    `notifications-native.json`. The directories are the ones the web build resolves in
    `src-tauri/src/lib.rs`.
- **Direct calls**: views run core calls as iced `Task`s and receive typed results as messages.
  No JSON, commands, or bindings between the UI and the core.
- **API key in headers only** (Principle VII): the core already does this; never put it in a log
  line or a measurement.
- **Gate order** (R9, FR-015): Phase 3 (US1) ends in a go/no-go check (T022). On a no-go, skip to
  T043 and write the no-go record.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependencies on unfinished tasks)
- **[Story]**: the user story the task belongs to (US1–US4)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: The new crate exists, builds, and is part of the workspace gate.

- [X] T001 Create the `native-ui` crate and add it to the workspace members in `Cargo.toml`.
  - `native-ui/Cargo.toml`: package `semantic-stash-viewer-native`, `rust-version = "1.88"`,
    `[lints] workspace = true`.
  - Dependencies: `iced = { version = "0.14", features = ["wgpu", "image", "tokio", "advanced"] }`,
    `wgpu-hal` 27 (feature `vulkan`), `ash` 0.38, `khronos-egl` (dynamic loading), `glow`, path
    deps `stash-core` and `player`, workspace `tokio`, `serde`, `serde_json`, `uuid`, `tracing`,
    `thiserror`.
  - Dev dependencies: `iced_test` 0.14, `tempfile`.
  - Check the `wgpu-hal` and `ash` versions match `cargo tree -p semantic-stash-viewer-native -i wgpu`.
  - A stub `native-ui/src/main.rs` that opens an empty iced window; `cargo build -p
    semantic-stash-viewer-native` succeeds.
- [X] T002 [P] Make sure CI builds and lints the new crate: in `.github/workflows/ci.yml`, confirm
  the `stable` toolchain (≥ 1.88) and the system packages cover `libmpv` and the Vulkan/EGL
  headers if any crate needs them at build time (they're loaded dynamically, so they shouldn't).
  Run `cargo clippy --workspace --all-targets -- -D warnings` locally.
- [X] T003 [P] Logging in `native-ui/src/logging.rs`, mirroring `src-tauri/src/logging.rs`: the
  same log directory (`<local_data_dir>/semantic-stash-viewer/logs`) with a `native` file prefix,
  and the same redaction of `apikey=`.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The services glue, the app skeleton, and the measurement plumbing every story uses.

**⚠️ CRITICAL**: No user story work begins until this phase is complete.

- [X] T004 Path resolution in `native-ui/src/services.rs`:
  - `config_dir`, `local_data_dir` and `cache_dir`, each joined with `semantic-stash-viewer`,
    giving the same directories as `src-tauri/src/lib.rs` (Tauri's `config_dir()`,
    `local_data_dir()`, `cache_dir()` plus `APP_DIR`).
  - The native file names: `shell/tabs-native.json`, `shell/notifications-native.json`.
- [X] T005 `Services` in `native-ui/src/services.rs` (research R5), with a doc comment marking it
  a copy of the web glue for the decision record. It holds:
  - the `ProfileStore`, opened read-only in practice (no `save` calls);
  - a `ConnectionManager<StashProber>` that connects to `SSV_HARNESS_PROFILE` or the last-used
    profile;
  - a `NotificationCenter` on `notifications-native.json`;
  - copies of `CacheRegistry` and `read_cached` (from `src-tauri/src/cache_commands.rs`,
    including the online-flag sync) and `active_client` (from
    `src-tauri/src/player_commands.rs`);
  - one `ThumbService` per profile (as `src-tauri/src/thumb_scheme.rs`'s `ThumbServices`);
  - the `player::Player` started with `PlayerConfig::render()` (`None` if libmpv fails, the app
    still runs).

  No jobs watcher.
- [X] T006 App skeleton in `native-ui/src/app.rs` and `native-ui/src/main.rs`:
  - a tokio runtime shared with `Services`, and `iced::application(...)` with a dark theme;
  - `State`, `Message`, `update`, `view`;
  - subscriptions: connection snapshots (`ConnectionManager::subscribe`) as messages, window
    events, and the close request. On close: save state (tabs once T035 exists), stop the
    player, then exit.
  - The initial view shows the connection state and a "Scenes" placeholder.
- [X] T007 [P] Measurement plumbing in `native-ui/src/measure/mod.rs`, per
  [contracts/measurements.md](contracts/measurements.md):
  - environment helpers for `SSV_HARNESS_PROFILE`, `SSV_HARNESS_CLEAR_CACHE`, `SSV_HARNESS_EXIT`,
    `SSV_DEBUG_BENCH`, `SSV_MEASURE`, `SSV_MEASURE_LONG`, `SSV_MEASURE_LONG_SECS`;
  - a `measure!` printer for `MEASURE {json}` lines;
  - the `coldStartMs` mark when the first interactive view is drawn;
  - a panic hook that prints `MEASURE {"uiError": …}`;
  - clearing the profile's cache when `SSV_HARNESS_CLEAR_CACHE` is set;
  - exiting after the run when `SSV_HARNESS_EXIT` is set.
- [X] T008 [P] Harness option in `src-tauri/src/bin/perf_harness.rs` (additive):
  - `--app web|native`, default `web`, which behaves exactly as today;
  - `native` builds `semantic-stash-viewer-native` (debug) and launches
    `target/debug/semantic-stash-viewer-native` with the same steps and environment;
  - the report header shows the app.
- [X] T009 [P] Tests in `native-ui/tests/services.rs`: path resolution matches the web build's
  directories; opening services never rewrites `profiles.json` (compare its bytes before and
  after, in a temp dir); the native tab and notification paths differ from the web build's.

**Checkpoint**: `cargo run -p semantic-stash-viewer-native` opens, connects to the saved
profile, and prints `coldStartMs`; the harness runs with `--app native` (cold starts only).

---

## Phase 3: User Story 1 - Play a scene inside the native window (Priority: P1) 🎯 Gate

**Goal**: hardware-decoded mpv video inside the iced window on Wayland, with the full 002
controls on top (research R1).

**Independent Test**: open a scene from the recently added list, use every control by mouse and
keyboard while `perf-harness --app native --quick` measures (quickstart V1).

### Tests for User Story 1

- [X] T010 [P] [US1] Tests in `native-ui/tests/video_slots.rs` for the frame-slot ring in
  `native-ui/src/player/video/slots.rs` (data-model VideoSurface):
  - 3 slots, each "free, rendering, ready, on-screen";
  - "Only one slot is ready at a time (newer replaces older)";
  - "an on-screen slot isn't rendered into";
  - a resize applies "after 100 ms without further change";
  - all slots reallocate together.
- [X] T011 [P] [US1] Tests in `native-ui/tests/controls.rs` for the pure control logic in
  `native-ui/src/player/controls.rs`:
  - the 002 FR-009 key map: space, ←/→ ±10 s, ↑/↓ volume, `[` / `]` slower/faster through
    0.25×–4×, `\` normal speed, `.` / `,` frame step only while paused, `f`, `m`, and Escape
    (leave fullscreen, then close);
  - auto-hide after 3 s without movement, shown again on movement or a key;
  - seek-bar hover position to time.

### Implementation for User Story 1

- [X] T012 [US1] Side GL context in `native-ui/src/player/video/egl.rs`:
  - load EGL dynamically and create a surfaceless display on the render node
    (`EGL_MESA_platform_surfaceless`, or `EGL_EXT_platform_device` on `/dev/dri/renderD128`);
  - a GL 3.3 core (or GLES 3) context, and a `get_proc_address` for mpv;
  - check `EGL_MESA_image_dma_buf_export`; return plain errors naming the missing piece.
- [X] T013 [US1] DMA-BUF framebuffers in `native-ui/src/player/video/gl_frames.rs` (amended: imports the Vulkan-allocated DMA-BUFs with `EGL_EXT_image_dma_buf_import`, research R1 amendment):
  - for each of 3 slots, an RGBA8 texture plus an FBO at the given size;
  - export with `eglCreateImage` + `eglExportDMABUFImageQueryMESA` / `eglExportDMABUFImageMESA`,
    giving fourcc, modifier (require linear), fd, stride, offset;
  - free everything on drop.
- [X] T014 [US1] Vulkan frame images in `native-ui/src/player/video/vk_frames.rs` (amended: allocates linear exportable images and exports DMA-BUFs, since wgpu doesn't enable the DRM modifier extension):
  - check `VK_EXT_external_memory_dma_buf` and `VK_EXT_image_drm_format_modifier` on iced's wgpu
    device, through `Device::as_hal::<wgpu_hal::api::Vulkan>`;
  - create a `VkImage` with `VkExternalMemoryImageCreateInfo` (DMA_BUF) and
    `VkImageDrmFormatModifierExplicitCreateInfoEXT` (the exported modifier and plane layout);
  - import the fd with `VkImportMemoryFdInfoKHR`, bind, and wrap it with
    `create_texture_from_hal` into a `wgpu::Texture` and view;
  - errors are plain and name what's missing.
- [X] T015 [US1] Render thread in `native-ui/src/player/video/render_thread.rs`:
  - owns the EGL context, the slots and `player::Renderer` (created with
    `create_renderer(get_proc_address, None)`; set `vaapi-device` to the render node if hwdec
    reports no interop);
  - `Renderer::set_update_callback` wakes it; it renders into a free slot
    (`Renderer::render(fbo, w, h)`, with the flip chosen so wgpu samples upright), runs
    `glFinish`, marks the slot ready, and notifies the UI through a channel;
  - handles resize and stop requests.
- [X] T016 [US1] iced shader primitive in `native-ui/src/player/video/primitive.rs`:
  - `prepare` imports a slot generation's textures once (T014) and keeps them until the next
    reallocation;
  - `render` draws the ready slot as a quad that keeps the video's aspect ratio (from
    `PlayerSnapshot` width and height), letterboxed in the widget bounds, with a small WGSL
    shader.
- [X] T017 [US1] `VideoSurface` in `native-ui/src/player/video/mod.rs`:
  - starts and stops the render thread;
  - turns frame notifications into an iced subscription message (`Message::FrameReady`), so
    rendering is reactive;
  - feeds widget size × scale factor into the slot resize settling (T010);
  - teardown order (R1, SC-010): stop the thread, free the mpv render context before the mpv
    handle, drop the wgpu textures, destroy the EGL context.
- [X] T018 [US1] Controls view in `native-ui/src/player/controls.rs`, on the T011 logic:
  - play/pause; a seek bar with current time, duration, hover time, click and drag;
  - skip ±10 s; speed menu 0.25×–4× with pitch kept; frame step; volume; mute; fullscreen;
    close;
  - state from `Player::subscribe()` as messages; commands through `Player` methods (`seek`,
    `seek_relative`, `set_speed`, `set_volume`, `set_muted`, `frame_step_forward` /
    `frame_step_back`, `toggle_pause`).
- [X] T019 [US1] Player screen in `native-ui/src/player/view.rs` and its wiring in
  `native-ui/src/app.rs`:
  - `stack![video, controls]`; auto-hide controls and cursor after 3 s; fullscreen through
    `window::set_mode`;
  - the ended state with Replay; a plain-language error state when a scene can't play (FR-006);
  - until US2, a recently added picker (up to 20, from the core adapter) as the entry point;
  - opening plays through `Player::open` with the stream and key the core provides (as
    `src-tauri/src/player_commands.rs` does), and closing calls `Player::close`.
- [X] T020 [US1] Playback measurements in `native-ui/src/measure/playback.rs`, matching
  `src-tauri/src/measure.rs`'s `SSV_MEASURE` run and line format: `open_ms`, `seek_ms` samples,
  dropped frames, `hwdec`, codec, container, plus the long run (`SSV_MEASURE_LONG`) for dropped
  frames per minute and main-thread CPU.
- [X] T021 [US1] *(Not needed: the DMA-BUF path passed every playback criterion; no fallback built.)* *(Only if T022's first measurement fails a playback criterion)* Fallbacks behind
  `SSV_VIDEO_PATH=dmabuf|gl|readback` (research R1):
  - read-back in `native-ui/src/player/video/readback.rs`: the side context copies each frame
    through a pixel buffer into an iced texture;
  - then the wgpu GL backend path (`WGPU_BACKEND=gl`, mpv rendering into a wgpu-owned GL texture
    through `wgpu-hal`'s GLES backend).

  Measure each the same way.
- [X] T022 [US1] **Playback gate** (quickstart V1).
  - Manual: every control by mouse and keyboard; clean drawing over moving video; resize and
    fullscreen; error case; close; 5 quit cycles while playing.
  - Then `cargo run --bin perf-harness -- --profile "Production" --app native --quick`.
  - Record SC-001 to SC-004 (open, seek median including 4K, dropped per minute at 1080p and 4K,
    hwdec per codec, input acknowledgement) in `specs/006-native-ui-spike/quickstart.md` "Results".
  - If they fail after T021, record a **NO-GO** and go to T043 (FR-015).

**Checkpoint**: the playback gate is decided. Continue only on a pass.

---

> **Closed 2026-10-03 after the playback gate.** The user decided GO on US1's evidence
> ([decision.md](decision.md)) and the constitution moved to iced (v4.0.0). The open tasks below
> (US2 grid, US3 tabs, the rest of US4, Polish) are **moved to the port feature** as its starting
> scope; they are left unchecked here on purpose.

## Phase 4: User Story 2 - Browse scenes a page at a time (Priority: P2)

**Goal**: the paged Scenes grid and list of feature 005, with thumbnails straight from the core
(research R3, R4).

**Independent Test**: quickstart V2. Browse at 50 and 1000 per page in both modes, change pages,
jump, open a scene and come back, while the bench measures.

### Tests for User Story 2

- [ ] T023 [P] [US2] Tests in `native-ui/tests/keyboard.rs`: port every case of
  `ui/src/__tests__/scenes/keyboard.test.ts`. That covers arrows within the grid, edges to the
  previous or next page, Home/End, `[` / `]`, Enter and Ctrl+Enter, and list mode as one column,
  against `native-ui/src/scenes/keyboard.rs`.
- [ ] T024 [P] [US2] Tests in `native-ui/tests/page.rs` for the page state machine in
  `native-ui/src/scenes/page.rs` (data-model "Scenes page"):
  - "loading, ready, unreachable";
  - a page past the end shows as the last page, with `requested` ≠ `shown`;
  - "while connecting a failure stays 'loading'"; connecting retries a page that isn't ready;
  - a refresh of the shown page re-reads it in place;
  - a new sort goes to page 1;
  - a new page size goes to "the page containing the first visible scene".

### Implementation for User Story 2

- [ ] T025 [P] [US2] Port `ui/src/scenes/keyboard.ts` to `native-ui/src/scenes/keyboard.rs` as a
  pure function returning move, edge, page, or open.
- [ ] T026 [US2] Page loading in `native-ui/src/scenes/page.rs`:
  - calls `stash_core::adapter::scenes::find_scenes_page` through `read_cached`, with the web
    build's cache key `scenes:q:<hash>:s:<size>:p:<page>` and refresh policy (see
    `src-tauri/src/scenes_commands.rs`), so both builds share cached pages;
  - prefetches the previous and next pages after a page arrives; drops stale answers by
    generation.
- [ ] T027 [US2] Thumbnails in `native-ui/src/scenes/thumbs.rs` (R3):
  - an LRU of `iced::widget::image::Handle` keyed by `(scene id, screenshot version)` with
    "at most 2,000" entries, filled from `ThumbService::get` in tasks (the core keeps its limit
    of 6 at a time);
  - one shared placeholder handle (`stash_core::thumbs::PLACEHOLDER`) while loading or missing;
  - warm the next page's thumbnails after a page arrives.
- [ ] T028 [US2] Grid and list in `native-ui/src/scenes/grid.rs`:
  - columns from the width (`responsive`), cards at least 240 px wide;
  - card: a 16:9 thumbnail, a one-line title, and one-line details ("duration · resolution ·
    studio · date");
  - list rows 44 px high;
  - a visible focus ring and roving focus; click opens, Ctrl+click and middle-click open in a new
    tab (a new tab only once US3 exists).
- [ ] T029 [US2] Scenes view in `native-ui/src/scenes/view.rs`:
  - toolbar: a sort menu from `SceneSort` labels with direction and random seed, and a
    grid/list toggle;
  - page controls above and below: "Page N of M · a–b of total"; first, previous, next and last;
    go to page; per page (20, 40, 50, 60, 120, 250, 500, 1000; default 50);
  - changing page scrolls to the top;
  - changing the page size keeps the first visible scene on screen;
  - loading placeholders, the unreachable state, and the empty state ("No scenes in this library
    yet").
- [ ] T030 [US2] Wire Scenes into `native-ui/src/app.rs`:
  - Scenes becomes the start view; opening a card pushes a Scene entry (the US1 player screen);
  - Back (button, Alt+←, Escape from the player) returns to the Scenes entry with page, size,
    mode, sort, and scroll restored (`scrollable::scroll_to` once the page is ready), per 005
    R13;
  - the scroll offset is saved after 200 ms without scrolling.
- [ ] T031 [US2] UI bench in `native-ui/src/measure/bench.rs` (contract lines), driving the app by
  sending itself messages and timing frames with `window::frames()`:
  - `nav-first-paint` and `control-press`;
  - `scenes-page-change`: a 1 s glance before each of 20 changes, timed until the page and its
    visible thumbnails are drawn;
  - `scenes-page-jump`: 5 jumps, timed until the cards show;
  - at 1000 per page: `scenes-scroll-1000-{grid,list}-cold` first (thumbnails uncached after
    `SSV_HARNESS_CLEAR_CACHE`), then wait for all of the page's thumbnails, then
    `scenes-scroll-1000-{grid,list}`;
  - each scroll is 64 px per frame for 5 s against an idle baseline.
- [ ] T032 [US2] *(Only if T031 shows SC-007 failing)* Windowing in
  `native-ui/src/scenes/grid.rs` (R4): build only the rows in view plus one screen above and
  below, with fixed-height spacers for the rest; re-measure.
- [ ] T033 [US2] Headless flows in `native-ui/tests/flows.rs` with `iced_test`, feeding fixture
  `ScenePage` messages (no server):
  - `]` and `[` change page;
  - opening a card and going back restores page 12, list mode, and the scroll offset;
  - a past-the-end page shows the last page number.

**Checkpoint**: quickstart V2 passes by hand, and the bench prints all Scenes lines.

---

## Phase 5: User Story 3 - Tabs with a scene playing (Priority: P3)

**Goal**: a tab strip where switching keeps each tab's state, and the playing video is correct
on the first frame after every switch (research R6).

**Independent Test**: quickstart V3. Play, switch to Scenes, change pages, switch back 20
times; quit and relaunch.

### Tests for User Story 3

- [ ] T034 [P] [US3] Tests in `native-ui/tests/tabs.rs` for `native-ui/src/shell/tabs.rs`:
  - opening a view pushes an entry; Back moves the cursor; "returning to an equal earlier entry
    reuses it (005 R13)";
  - each tab's view state survives switching;
  - save and restore round-trip through `stash_core::shell::tabs::TabsStore` on a temp
    `tabs-native.json` (view state serialised into `HistoryEntry.view_state`);
  - "A page past the end restores as the last page".

### Implementation for User Story 3

- [ ] T035 [US3] Tabs in `native-ui/src/shell/tabs.rs`:
  - `Tab { id, history, cursor, title }`; entries Scenes `{ query, page (≥ 1), page_size (20,
    40, 50, 60, 120, 250, 500, 1000; default 50), mode (grid or list), scroll, focus }` and Scene
    `{ scene_id, title }` (data-model);
  - persistence through `TabsStore` on `tabs-native.json`, per profile, "500 ms after a change and
    on quit".
- [ ] T036 [US3] Tab strip in `native-ui/src/shell/tab_strip.rs`:
  - select, close, new tab, middle-click close;
  - the 004 shortcuts: Ctrl+T, Ctrl+W, Ctrl+Tab and Ctrl+Shift+Tab, Ctrl+1–9;
  - the playing tab marked.
- [ ] T037 [US3] Wire tabs into `native-ui/src/app.rs`:
  - each tab renders its current entry; one player for the app, owned by one Scene tab;
  - the video widget is in the tree only while its tab is active, and playback continues
    meanwhile;
  - closing the owning tab stops playback;
  - Ctrl+click and middle-click on a card open a new tab;
  - the close request saves tabs, then exits;
  - tabs are restored on launch.
- [ ] T038 [US3] Bench additions in `native-ui/src/measure/bench.rs`:
  - `tab-switch-20-tabs` (20 tabs, one playing; 40 switches), with `videoSized` true when the
    first frame after switching to the playing tab is drawn at the widget's size;
  - `now-playing-appears` and `back-to-scene`, matching the web build's definitions in
    `ui/src/debug/bench.ts`.
- [ ] T039 [US3] Headless flows in `native-ui/tests/flows.rs`: switching tabs keeps each tab's
  page and mode; closing a tab selects its neighbour; tabs saved to a temp file come back on a
  fresh app state.

**Checkpoint**: quickstart V3 passes by hand.

---

## Phase 6: User Story 4 - Decide with evidence (Priority: P4)

**Goal**: both builds measured side by side, a stability run, and the decision record.

**Independent Test**: quickstart V6. `decision.md` states GO or NO-GO with every criterion
measured for both builds.

- [ ] T040 [US4] Memory and CPU rows in `src-tauri/src/bin/perf_harness.rs` (additive, both
  apps):
  - sum `VmRSS` and user + system CPU time over the app's process tree from `/proc`; for the web
    build that includes the WebKit web and network processes;
  - sample them 5 s after the UI bench settles on Scenes (page of 50), and during
    `SSV_MEASURE_LONG` playback;
  - report `memory-idle-scenes-50`, `memory-playing-1080p`, `cpu-idle-scenes-50` and
    `cpu-playing-1080p` as info rows.
- [ ] T041 [US4] Run the full harness for both builds on the same day (quickstart V4):
  - `perf-harness --profile "Production" --app native`, then `--app web`;
  - record every line in `specs/006-native-ui-spike/quickstart.md` "Results", next to the web
    build's.
- [ ] T042 [US4] Stability run (quickstart V5, SC-010):
  - 20 launch, browse, play and quit cycles of the native build (scripted in the session
    scratchpad with `SSV_HARNESS_EXIT` and `SSV_MEASURE`, plus 5 by hand);
  - after each launch, check that tabs come back;
  - record crashes, hangs, and lost state (expected none).
- [X] T043 [US4] Write `specs/006-native-ui-spike/decision.md` per
  [contracts/decision-record.md](contracts/decision-record.md):
  1. GO or NO-GO;
  2. the evidence table for SC-001 to SC-011, both builds;
  3. the playback approach used;
  4. what got simpler or harder;
  5. toolkit gaps;
  6. glue to move;
  7. next steps (on GO, the constitution amendment and an outline of the port);
  8. unknowns.

  GO only if SC-001 to SC-010 pass.

**Checkpoint**: the decision is made and written.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [ ] T044 Run the full gate:
  - `cargo fmt --all --check`;
  - `cargo clippy --workspace --all-targets -- -D warnings` (the native crate included);
  - `cargo test --workspace`;
  - the bindings drift check (`cargo run -p semantic-stash-viewer --bin export-bindings` leaves
    `ui/src/bindings.ts` unchanged);
  - `npm --prefix ui run lint`, `npm --prefix ui run typecheck`, `npm --prefix ui test`.
- [ ] T045 [P] Note the spike in `README.md` (how to run the native build and the harness's
  `--app native`) and in `ROADMAP.md` (the spike, its decision, and what follows), linking to
  `decision.md`.

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (Phase 1)**: none. T002 and T003 can run alongside T001's tail.
- **Foundational (Phase 2)**: needs T001; blocks every story.
- **US1 (Phase 3)**: needs Phase 2. It is **the gate**: US2 and US3 start only after T022 passes.
- **US2 (Phase 4)**: needs T022 passed. It uses the US1 player screen for "open a scene".
- **US3 (Phase 5)**: needs US2 (tabs hold Scenes and Scene entries).
- **US4 (Phase 6)**: needs US3 (or T022 on a no-go: then only T043).
- **Polish (Phase 7)**: last.

### Within stories

- Tests (T010–T011, T023–T024, T034) before their implementation.
- US1: T012 → T013 → T014 → T015 → T016 → T017 → T018/T019 → T020 → T022 (T021 only on a
  failure).
- US2: T025 → T026 → T027 → T028 → T029 → T030 → T031 → T033 (T032 only on a failure).
- US3: T035 → T036 → T037 → T038 → T039.
- US4: T040 → T041 → T042 → T043.

### Parallel opportunities

- Phase 1: T002 and T003.
- Phase 2: T007, T008 and T009, once T004–T006 have their shapes.
- US1: T010 and T011 together, then T012–T014 are separate files but sequential by dependency;
  T018 (controls) can start beside T015–T017 once T011 exists.
- US2: T023, T024 and T025 together; T027 beside T026.
- US3: T034 beside T036.
- Polish: T045 beside T044.

## Parallel Example: User Story 1

```text
Task: "T010 [P] [US1] Frame-slot ring tests in native-ui/tests/video_slots.rs"
Task: "T011 [P] [US1] Control logic tests in native-ui/tests/controls.rs"
# then, while the video path (T012–T017) is built:
Task: "T018 [US1] Controls view in native-ui/src/player/controls.rs"
```

## Parallel Example: User Story 2

```text
Task: "T023 [P] [US2] Keyboard tests in native-ui/tests/keyboard.rs"
Task: "T024 [P] [US2] Page state tests in native-ui/tests/page.rs"
Task: "T025 [P] [US2] Port keyboard.ts to native-ui/src/scenes/keyboard.rs"
```

## Implementation Strategy

### Gate first (US1 only)

1. Phases 1 and 2.
2. Phase 3 through T022, the playback gate, in roughly the first week of the time box.
3. **Stop and decide.** A no-go goes straight to T043 with the measurements in hand.

### Then incremental

1. US2: the grid, measured. This is where the web build struggled.
2. US3: tabs with video.
3. US4: both builds through the harness, the stability run, the decision.
4. Polish and the gate.

### Notes

- Keep the code spike-quality but gate-clean (fmt, clippy, tests). It's evidence, and on a GO it
  becomes the port's starting point.
- Commit after each phase checkpoint, once the user has looked at it.
- Measurements go in `quickstart.md` "Results" as they're taken, so the decision record is
  assembled, not reconstructed.
