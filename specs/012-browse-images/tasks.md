---

description: "Task list for 012 Browse Images"
---

# Tasks: Browse Images

**Input**: Design documents from `specs/012-browse-images/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/images-ui.md](contracts/images-ui.md),
[contracts/player-image-mode.md](contracts/player-image-mode.md), [quickstart.md](quickstart.md);
011 on `main`.

**Tests**: included (the project's practice: core rules tested headlessly, UI transitions in
`native-ui/tests`, budgets as harness rows). Tests come before the code they cover in each phase.

**Organization**: Setup (seeding Testing, the mpv spike) → Foundational (the browse layer refactor
with no behaviour change, the core's image types and queries, the player's image mode) → US1 (find
images, P1) → US2 (the viewer, P2) → US3 (slideshow and moving images, P3) → US4 (details, P4) →
Polish.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1–US4 (spec.md)

---

## Phase 1: Setup

- [X] T001 Write `tools/seed-testing-images/seed.sh` (research R8; changed 2026-10-12 at the
      owner's request from generating images to copying them from Production): refuse any server
      but Testing; read Testing's key from its creds file and never print it; sample about 1,200
      images from Production (read-only: queries and file reads, members extracted from its zip
      galleries) into `<library>/ssv-prod/` under neutral names (a random spread, 20 GIFs, 20
      WebPs, the 15 largest); add the two cases Production lacks (a truncated JPEG, a 5 s MP4 for an
      image clip); scan only those folders; assign performers 2/3 and tag 6; save "SSV images
      performer A"; print ids only
- [X] T002 Enable image clips on Testing for `ssv-clips/` (a library path with videos excluded and
      "create image clips from videos" on, through `configureGeneral`/`configureLibrary`), run
      `tools/seed-testing-images/seed.sh --profile "localhost:9998"`, and record the resulting ids
      and counts (ids only) in `specs/012-browse-images/quickstart.md`; if v0.31.1 can't make clips,
      say so there and mark the clip cases unverified
- [X] T003 [P] Spike `crates/player/examples/image_open.rs`: open each seeded local file (12 MP,
      24 MP, panorama, GIF, WebP) in the player with `image-display-duration=inf` and time open →
      first frame (10 runs each); record medians and the verdict on SC-003 per size in
      `specs/012-browse-images/research.md` R3
- [X] T004 [P] Confirm Images' sorts and criteria against the web UI's `models/list-filter/images.ts`
      for v0.31.1 and record the exact lists (sort names, criterion keys, modifiers) in
      `specs/012-browse-images/data-model.md` (ImageSort, CriterionKind sections)

---

## Phase 2: Foundational (blocks every story)

**Purpose**: the generic browse layer (no behaviour change for Scenes), the core's image model and
queries, and the player's image mode.

### Browse layer in the core (research R1)

- [X] T005 Move `crates/stash-core/src/scenes/paging.rs` to `crates/stash-core/src/browse/paging.rs`
      (new `browse/mod.rs`), re-exported from `scenes` so existing paths keep compiling
- [X] T006 [P] Test in `crates/stash-core/tests/scenes_query.rs`: an 011-era saved `SceneQuery` JSON
      deserializes unchanged and re-serializes identically after the move to `ListQuery<SceneSort>`
- [X] T007 Make the query generic: `browse::query::ListQuery<S>` (search, criteria, sort, direction,
      seed; `cache_hash`, `normalized`) in `crates/stash-core/src/browse/query.rs`, with `pub type
      SceneQuery = ListQuery<SceneSort>` in `crates/stash-core/src/scenes/query.rs`
- [X] T008 Add `browse::entity::Entity { Scenes, Images }` in `crates/stash-core/src/browse/entity.rs`
      (criterion kinds, sorts, saved-filter mode `SCENES`/`IMAGES`, default-filter key, cache prefix
      `scenes:`/`images:`, thumbnail kind `scene`/`image`) and `CriterionKind::entities()` plus the
      image-only kinds `Photographer` and `FileCount` in
      `crates/stash-core/src/scenes/filter/kinds.rs` (the T004 list)
- [X] T009 [P] Tests in `crates/stash-core/tests/image_filters.rs`: each image criterion with each
      modifier builds the `ImageFilterType` JSON the web UI sends (as `scene_filters.rs` does for
      scenes); a scene-only kind is never sent for images
- [X] T010 Implement `image_filter(criteria) -> Option<ImageFilterType>` and `image_filter_json` in
      `crates/stash-core/src/scenes/filter/to_stash.rs`, sharing the per-kind conversions
- [X] T011 Saved filters per entity in `crates/stash-core/src/scenes/filter/saved.rs` and
      `crates/stash-core/src/adapter/filters.rs`: list by mode `IMAGES`, decode the images default
      filter (`defaultFilters.images`), criteria valid for the entity only (others go to "left
      out"); test in `crates/stash-core/tests/scene_filters.rs` with an images saved filter

### Images in the core (research R2)

- [X] T012 Types in `crates/stash-core/src/images/mod.rs`: `ImageSort` (T004 list, default path
      ascending), `ImageCard { id, title, date, thumb, width, height, media: Still | MayAnimate |
      Clip { duration } }`, `ImagePage { count, page, page_size, items }`, `ImageDetails` (every field
      in data-model.md); `pub mod images` in `crates/stash-core/src/lib.rs`
- [X] T013 [P] Tests in `crates/stash-core/tests/images_page.rs` (wiremock): a page request sends
      `findImages` with the filter and sort and maps cards (title falls back to the file name; GIF/
      WebP/APNG → `MayAnimate`; a `VideoFile` → `Clip`); details map every field; one request per
      page, cached as `images:q:<hash>:s:<size>:p:<page>`
- [X] T014 Operations `crates/stash-core/graphql/operations/ImagesPage.graphql` and
      `ImageDetails.graphql` and `crates/stash-core/src/adapter/images.rs` (`find_images_page`,
      `image_details`, `image_bytes` through `resource_bytes(client, "image", id, "image", …)`),
      checked by Cynic against `graphql/stash-v0.31.1.graphql`
- [X] T015 Image thumbnails: `image` kind in `crates/stash-core/src/thumbs.rs` (fetch
      `/image/<id>/thumbnail` with the key in a header) and the card thumbnail key parsing in
      `native-ui/src/screens/scenes/thumbs.rs` (accept `ssv-thumb://localhost/image/<id>`)

### The player's image mode (research R3, contracts/player-image-mode.md)

- [X] T016 [P] Tests in `crates/player/tests/image_mode.rs` (headless mpv): an `Image { looped:
      false }` open of a local JPEG reaches `Playing` and stays (no `Ended`); `looped: true` sets
      `loop-file=inf`; a following `Video` open restores `image-display-duration` and `loop-file`;
      `Zoom`, `Pan`, `Rotate`, `ActualSize`, `ResetView` set their properties; `picture` reports the
      dimensions
- [X] T017 `OpenRequest.kind: MediaKind { Video, Image { looped } }` and the per-kind options
      ("`image-display-duration=inf`", "`loop-file=inf`" when looped, "`mute=yes`") in
      `crates/player/src/session.rs`; `picture: Option<(u32, u32)>` and `frames_shown` in
      `crates/player/src/snapshot.rs`
- [X] T018 Commands in `crates/player/src/commands.rs`: `Zoom(z)` → `video-zoom` ("−3..=3", log2),
      `Pan(x, y)` → `video-pan-x/y`, `Rotate(q)` → `video-rotate = 90 × q` ("q in 0..=3"),
      `ActualSize(on)` → `video-unscaled`, `ResetView`; no transcode offer for images

### Browse layer in the UI (research R1)

- [X] T019 Move the entity-agnostic UI from `native-ui/src/screens/scenes/` to
      `native-ui/src/screens/browse/` (the card grid and windowing, pager and top bar, filter bar,
      criterion editor, picker, saved-filter menu, thumbnails, card previews, layout), parameterised
      by `Entity`; Scenes' queue view, playback, and `ScenesState`'s scene-only fields stay in
      `screens/scenes/`; no behaviour change
- [X] T020 Generalise the page effects (changed in implementation: rather than renaming Scenes'
      `LoadScenesPage`/`PrefetchScenes`/`Reply::ScenesPage`, which dozens of tests match, each
      browse `Kind` makes its own load and prefetch effects): `LoadImagesPage`, `PrefetchImages`,
      and `Reply::ImagesPage` in `native-ui/src/effects.rs`, read through the cache as
      `images:q:<hash>:s:<size>:p:<page>`; the reply is routed to the Images screen by T023
- [X] T021 Gate the refactor: `cargo test --workspace` (all 564 existing tests pass unchanged except
      for moved paths), `cargo clippy --workspace --all-targets -- -D warnings`, and the 005–011
      harness rows unchanged within noise (`cargo run -p perf-harness -- --profile "localhost:9998"
      --quick`)

**Checkpoint**: Scenes behaves exactly as before on the browse layer; the core can page images; the
player can show a still.

---

## Phase 3: User Story 1 - Find images (Priority: P1) 🎯 MVP

**Goal**: the Images section with the grid, wall, search, every filter and sort, saved filters,
paging, and tab saving (FR-001–FR-007).

**Independent Test**: quickstart V1.

- [ ] T022 [P] [US1] Tests in `native-ui/tests/images.rs`: opening Images requests page 1 in the
      default image filter; search, a criterion, the sort, and the direction requery from page 1;
      a saved image filter applies its query, page size, and mode; paging requests one page and
      prefetches neighbours; a scene-only criterion isn't offered
- [ ] T023 [US1] `Section::Images` and `Screen::Images(Box<ImagesState>)` in
      `native-ui/src/screens/mod.rs`; `ImagesState` (the browse state with `Entity::Images`, plus
      `viewer: Option<Viewer>` for US2) in `native-ui/src/screens/images/state.rs`; the navigation
      bar entry in the Stash web UI's order in `native-ui/src/shell/mod.rs`; page replies routed in
      `native-ui/src/session/mod.rs`
- [ ] T024 [US1] Image cards in `native-ui/src/screens/images/grid.rs`: the thumbnail at the card's
      size, title and date (none in the wall), badges for animated (loop) and clip images; Grid |
      Wall and the card-size control in the top bar
- [ ] T025 [US1] Images' sort menu (`ImageSort::all()`, random with reshuffle) and criteria in the
      filter menu from `Entity::Images` in `native-ui/src/screens/browse/filters.rs`; a galleries
      choice list for the galleries criterion in `crates/stash-core/src/adapter/choices.rs` and
      `native-ui/src/library.rs`
- [ ] T026 [P] [US1] Snapshot test in `native-ui/tests/snapshot.rs`: an Images tab's query, page,
      page size, card size, mode, scroll, focus, and applied filter round-trip; a session without
      Images tabs loads unchanged; `VERSION` stays 2
- [ ] T027 [US1] Tab saving and restore for Images in `native-ui/src/shell/mod.rs` (capture/restore)
- [ ] T028 [US1] Keys: `Level::Images` in `native-ui/src/shell/keymap.rs` (the Scenes grid's keys:
      arrows, Home/End, `[` `]`, Enter, Ctrl+Enter, `/`, F, S) listed under "Images" in keyboard help

**Checkpoint**: quickstart V1; T022 and T026 pass.

---

## Phase 4: User Story 2 - View an image (Priority: P2)

**Goal**: the viewer in mpv: next/previous across pages in ≤ 150 ms, zoom, pan, rotation, fit and
actual size, close to the card (FR-008–FR-013, FR-019).

**Independent Test**: quickstart V2.

- [ ] T029 [P] [US2] Tests in `crates/stash-core/tests/image_viewer.rs` for every rule in
      data-model.md's Viewer section: "`Next` at `count - 1` → `Nothing`"; "`Previous` at 0 →
      `Nothing` (no wrap)"; a new image resets `view`; "`Zoom` clamps to 1/8×–8× (log2 −3..=3)";
      "panning without zoom does nothing"; each `Show(i)` with `Prefetch` of `i ± 1`; "`Failed(i)`
      keeps `at = i`"; "`ResultsChanged { count }`: `at` clamps to `count - 1`; `count == 0` →
      `Close`"; `Close { at }`
- [ ] T030 [US2] The pure machine `step(&Viewer, Event) -> (Viewer, Command)` in
      `crates/stash-core/src/images/viewer.rs` (research R5)
- [ ] T031 [P] [US2] Tests in `crates/stash-core/tests/image_neighbours.rs` (wiremock, temp dir):
      files fetched with the `ApiKey` header; "at most 8 files or 512 MB, least recently used
      evicted"; a 404 → `Missing`; a ready file is a local path; counted in the cache limit
- [ ] T032 [US2] `Neighbours` in `crates/stash-core/src/images/neighbours.rs` and its wiring in
      `native-ui/src/services/mod.rs` (`open_image`: a ready local file, else Stash's
      `/image/<id>/image` with the key in a header; the direct-source guard accepts these for
      `Image` opens only); `Effect::PrefetchImages` in `native-ui/src/effects.rs`
- [ ] T033 [P] [US2] Tests in `native-ui/tests/image_viewer.rs`: Enter opens the viewer on the
      focused card (an `Image` open of that id); → past the page's last card loads the next page and
      shows its first; Escape closes to the grid on the last image's page with its card focused;
      a failed open shows the message and → still moves; opening an image while a scene plays stops
      the scene (its history saved)
- [ ] T034 [US2] The viewer in `ImagesState` (`native-ui/src/screens/images/state.rs`): events from
      input, the machine's commands applied (`Show(i)` → the card on this page or the page that
      holds it, then an image open through the session; `SetView` → player commands;
      `Prefetch` → `Effect::PrefetchImages`; `Close` → grid focus)
- [ ] T035 [US2] The viewer's view in `native-ui/src/screens/images/viewer.rs` per
      contracts/images-ui.md: the picture (the session's mpv render), the top bar ("7 of 214" ·
      title, close, rotate, fit/actual, slideshow, details), the edge arrows, controls fading after
      2 s still; Lucide icons (chevrons, rotate, maximize, info) in `native-ui/src/widgets/mod.rs`
- [ ] T036 [US2] Input: `Level::Viewer` keys in `native-ui/src/shell/keymap.rs` (contract table:
      → ← Space Backspace Home End + - 0 1 R Shift+R arrows-pan P [ ] I F Escape) and mouse (wheel
      steps, Ctrl+wheel zooms about the pointer, drag pans when zoomed, double-click toggles fit) in
      `native-ui/src/screens/images/viewer.rs`; keys routed before the player's in
      `native-ui/src/session/mod.rs`
- [ ] T037 [US2] The one player in `native-ui/src/session/mod.rs`: the viewer owns the player for
      its tab; opening an image ends a playing scene's viewing (history saved) and closes it; no
      now-playing bar for images; closing the tab or the viewer closes the image

**Checkpoint**: quickstart V2; T029, T031, T033 pass.

---

## Phase 5: User Story 3 - Slideshow and moving images (Priority: P3)

**Goal**: a slideshow at a chosen delay; animated and clip images looping; animated card previews
(FR-014–FR-016).

**Independent Test**: quickstart V3.

- [ ] T038 [P] [US3] Slideshow tests in `crates/stash-core/tests/image_viewer.rs`: "`Tick` advances
      only when not paused"; a manual `Next`, `Previous`, `Zoom`, or `Pan` pauses; it stops after
      the last image (`slideshow = None`); a looping image waits for `LoopDone` "at most 60 s";
      `Prefetch` of `i + 1, i + 2` while running; `SetDelay` keeps "1..=60"
- [ ] T039 [US3] Slideshow in the machine (`crates/stash-core/src/images/viewer.rs`) and its timer
      (`Effect::SlideshowDue`) in `native-ui/src/effects.rs` and `native-ui/src/screens/images/state.rs`;
      `slideshow_delay_s` ("default 5", 1–60) in the playback preferences
      (`native-ui/src/services/settings.rs`)
- [ ] T040 [US3] Looped opens for `MayAnimate` and `Clip` cards and `LoopDone` from the player's
      restart near 0 in `native-ui/src/session/mod.rs`; test in `native-ui/tests/image_viewer.rs`
- [ ] T041 [US3] Animated card previews: decode GIF/WebP/APNG originals under 16 MB into frames at
      the card's width in `crates/stash-core/src/previews.rs` (generalised from scene previews), one
      card at a time on hover in `native-ui/src/screens/browse/preview.rs`; clips show their still;
      test in `crates/stash-core/tests/previews.rs`

**Checkpoint**: quickstart V3; T038 passes.

---

## Phase 6: User Story 4 - Details (Priority: P4)

**Goal**: the read-only details panel; chips filter Images (FR-017, FR-018).

**Independent Test**: quickstart V4.

- [ ] T042 [P] [US4] Tests in `native-ui/tests/image_viewer.rs`: `I` loads `ImageDetails` once per
      image (0 cached) and shows every field; the panel follows → ; empty sections are left out; a
      tag chip filters Images (Ctrl: a new tab); under 900 px the panel overlays the picture
- [ ] T043 [US4] `Effect::LoadImageDetails` (cache key `image:<id>`) in `native-ui/src/effects.rs` and
      the panel in `native-ui/src/screens/images/details.rs`, reusing the scene page's chip and tag
      colouring widgets from `native-ui/src/screens/scene.rs`
- [ ] T044 [US4] Chips → `FilterTo` on Images (a new tab with Ctrl) and the narrow-window overlay in
      `native-ui/src/screens/images/viewer.rs`

**Checkpoint**: quickstart V1–V4; T042 passes.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [ ] T045 [P] Keymap test in `native-ui/tests/keymap.rs`: the Images and viewer keys are listed;
      none clash at `Level::Shell`, `Level::Images`, `Level::Viewer`
- [ ] T046 [P] Bench rows in `native-ui/src/measure/bench.rs` (research R9; "shown" = the new file's
      first frame from the snapshot): `images-page-change`, `images-page-jump`,
      `images-scroll-1000-grid`/`-wall`, `image-open`, `image-next` (20 steps incl. a page boundary),
      `image-zoom`, `slideshow-drift`, and `gql-images-page`, `gql-image-details`,
      `gql-image-step-prefetched`; nothing written to Testing
- [ ] T047 [P] Budgets in `crates/perf-harness/src/main.rs` and `crates/perf-harness/src/report.rs`:
      ≤ 150 ms p95, ≤ 1 s, ≤ 3% / ≤ 1%, ≤ 500 ms median, ≤ 150 ms p95, ≤ 1 frame, ≤ 0.3 s; requests
      1 / 1 / 0
- [ ] T048 Run `cargo run -p perf-harness -- --profile "localhost:9998" --quick` on a quiet machine
      (load under 4, window visible): every 012 row and every 005–011 row within budget
- [ ] T049 [P] Docs: README (Images, the viewer's keys), `ROADMAP.md` (tick the Images items, link
      012), and a requirement-to-test table (FR-001–FR-021, SC-001–SC-008) in
      `specs/012-browse-images/quickstart.md`
- [ ] T050 Gate: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
      `cargo test --workspace`; then quickstart V1–V4 on Testing (the user verifies before any
      commit)

---

## Dependencies & Execution Order

- **Setup (1)**: T001 → T002 (seeding); T003 needs T002's files; T004 independent.
- **Foundational (2)**: core browse T005 → T007 → T008 → T010 → T011 (T006, T009 tests first);
  images core T012 → T014 → T015 (T013 first); player T017 → T018 (T016 first); UI browse T019 →
  T020 → T021. The three tracks (core, player, UI) can proceed side by side; T021 gates the phase.
- **US1 (3)** needs Phase 2. **US2 (4)** needs US1 (cards to open) and the player track.
  **US3 (5)** and **US4 (6)** need US2 (the viewer); either order.
- **Polish (7)** after the stories.

### Parallel opportunities

- Setup: T003 ∥ T004 (after seeding).
- Foundational: the core track ∥ the player track; tests T006, T009, T013, T016 together.
- US1: T022 ∥ T026 (tests).
- US2: T029 ∥ T031 ∥ T033 (tests); T030 (core) ∥ T032 (neighbours) once their tests exist.
- US3 ∥ US4 after US2 (different files, except `image_viewer.rs` tests: in sequence).
- Polish: T045 ∥ T046 ∥ T047 ∥ T049.

### Parallel example: Foundational

```text
Core:    T006, T009 tests → T005 → T007 → T008 → T010 → T011; T013 → T012 → T014 → T015
Player:  T016 → T017 → T018
UI:      T019 → T020 (after T007, T008) → T021 gate
```

---

## Implementation Strategy

### MVP (Phases 1–3)

Seeded Testing, the browse layer, and the Images grid with every filter, sort, and saved filter:
images can be found and browsed like scenes.

### Incremental delivery

1. Setup + Foundational → Scenes unchanged on the browse layer; images pageable; the player shows
   stills.
2. US1 → the Images section (MVP).
3. US2 → the viewer (the section's point).
4. US3 → slideshow and moving images.
5. US4 → details.
6. Polish → bench, budgets, docs, gate; commit after the user's check.
