---

description: "Task list for 005-browse-scenes-and-play (P1: browse the whole library)"
---

# Tasks: Browse Scenes and Play — P1 (browse the whole library), paged

**Input**: Design documents from `specs/005-browse-scenes-and-play/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/scenes-browse.md](contracts/scenes-browse.md),
[quickstart.md](quickstart.md)

**Scope**: **User Story 1 only**, as planned (plan.md, "P1"). US2–US6 get their own plan and
task passes later; nothing here blocks them.

**Revised 2026-10-01 for paged views** (constitution v3.3.0, spec clarifications and plan of
2026-10-01). T001–T009 were built in the first version and stay (marked done); T010 adapts the
core to a page size parameter. The first version's US1 tasks (a continuously scrolling virtual
grid) are replaced by T011–T024 below; its virtual grid, position bar, velocity-aware loading,
and their tests are removed in T022. The previous task list is kept in the session scratchpad.

**Tests**: Included. The constitution's Development Workflow gate requires core tests against
recorded fixtures, and SC-002/SC-007 need measurable checks. Write the tests first and make sure
they fail before implementing.

**Rules that apply to every task**:
- **Read-only** (FR-024): P1 performs no mutations. The production library (36,350 scenes) may be
  used for manual checks and the harness.
- **Paged** (Principle IV): one page at a time, default 50, sizes 20, 40, 50, 60, 120, 250, 500,
  1000; one card-sized `findScenes` query per page shown (SC-007), never a request per card; no
  infinite scrolling.
- **Never resets** (Principle IX, SC-009): page, page size, mode, and scroll position are kept
  per history entry, and returning to Scenes reuses or carries forward its state.
- **No keys in URLs** (Principles V, VII): thumbnails go through `ssv-thumb://`; the core sends
  the API key in a header.
- **The webview plays no media** (Principle V): cards show thumbnails as images only.
- **Cached, per server** (003): pages and thumbnails live in the profile's view cache.
- **Specta**: no `u64`/`i64`/`usize` in DTOs (use `u32` or `f64`); regenerate `ui/src/bindings.ts`
  after any command or DTO change.
- MSRV 1.80: avoid newer std APIs (`is_none_or`, `floor_char_boundary`, …).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on unfinished tasks)
- **[Story]**: US1

---

## Phase 1: Setup (Shared Infrastructure)

- [X] T001 Add the thumbnail dependencies to `crates/stash-core/Cargo.toml` (research R5): `turbojpeg = "1"` (system libturbojpeg; `default-features = false` plus whatever feature links the system library, no bundled CMake build), `fast_image_resize = "5"`, and `image = { version = "0.25", default-features = false, features = ["png", "webp"] }`. Add `libturbojpeg0-dev` to the apt line in `.github/workflows/ci.yml`. Confirm `cargo build -p stash-core` succeeds locally
- [X] T002 [P] Add the GraphQL operation `crates/stash-core/graphql/find_scenes_page.graphql`: `query FindScenesPage($filter: FindFilterType) { findScenes(filter: $filter) { count scenes { id title date files { basename duration width height } studio { name } paths { screenshot } } } }` (card fields only, research R1). Record a fixture `crates/stash-core/tests/fixtures/stash-v0.31.1/find-scenes-page.json` with 3 scenes: one normal, one with an empty title (falls back to `files[0].basename`), and one with no files and no studio. Scrub it (no real titles, paths, or hosts)
- [X] T003 [P] Add `ssv-thumb:` to the CSP's `img-src` in `src-tauri/tauri.conf.json` (`img-src 'self' data: ssv-thumb:`), and generate a 480×270 neutral placeholder JPEG at `crates/stash-core/assets/thumb-placeholder.jpg` (under 5 KB; `ffmpeg -f lavfi -i color=c=0x222222:s=480x270 -frames:v 1 -q:v 8`)

---

## Phase 2: Foundational (Blocking Prerequisites)

- [X] T004 Blob entries in the view cache, test first:
  - tests in `crates/stash-core/tests/cache.rs`: `put_bytes`/`get_bytes` round-trip; blobs count toward `size_bytes()` and are evicted least-recently-used with everything else; `clear()` and a different server identity remove them;
  - implement `ViewCache::put_bytes(key, &[u8])` and `get_bytes(key) -> Option<Vec<u8>>` in `crates/stash-core/src/cache/mod.rs` (same table; a `kind` column or a key prefix distinguishes blobs from JSON; bump `SCHEMA_VERSION` if the table changes, which recreates old files as designed)
- [X] T005 [P] The scene query, test first (research R2, R7; data-model "SceneQuery", "SceneSort"):
  - tests in `crates/stash-core/tests/scenes_query.rs`: every sort maps to Stash's sort string; `random` with seed 42 sends `random_42`, and `random` without a seed is rejected; labels match the web UI's (e.g. `file_mod_time` → "File Modification Time"); normalization trims the search text and drops the seed unless the sort is `random`; two equal queries hash the same and a different sort, direction, seed, or search hashes differently;
  - implement `crates/stash-core/src/scenes/query.rs`: `SceneQuery { search: String, sort: SceneSort, direction: SortDirection, seed: Option<u32> }` ("seed: number (32-bit unsigned) or null; required when `sort` is `random`"), the 27-variant `SceneSort` enum with `label()` and `stash_sort(seed)`, `SortDirection { Asc, Desc }`, defaults `date`/`desc`, `normalized()` and `cache_hash() -> String` (FNV-1a 64, `{:016x}`, like 003's identity). `filter` is reserved for US2 and not added here. Derive serde (camelCase) and specta (feature-gated)
- [X] T006 [P] Paging math, test first (research R1, R4):
  - tests in `crates/stash-core/tests/scenes_paging.rs`: `PAGE_SIZE` is 120; item index 0 is page 1, 119 is page 1, 120 is page 2; the pages covering items 100–250 are 1–3; the last page of 36,350 items is 303; an empty library has no pages;
  - implement `crates/stash-core/src/scenes/paging.rs` *(the fixed 120 is replaced by a page-size parameter in T010)*
- [X] T007 Scene cards, test first (data-model "SceneCard", "ScenePage"):
  - tests in `crates/stash-core/tests/scenes_cards.rs` using the T002 fixture: title falls back to the file base name; `resolution` is `1920×1080` from the primary file; `durationSeconds`, `studio`, and `date` are null when missing; `thumb` is `ssv-thumb://localhost/scene/<id>?v=<t>` using the `t` value from `paths.screenshot` (and `v=0` when there's none); `hasPreview` is `false` in P1 (US5 fills it);
  - implement `SceneCard` and `ScenePage { count: u32, page: u32, items: Vec<SceneCard> }` and the mapping in `crates/stash-core/src/scenes/mod.rs`
- [X] T008 Fetch a page, test first:
  - tests in `crates/stash-core/tests/scenes_page_requests.rs` against the mock server: `find_scenes_page(client, &query, page)` sends `per_page: 120`, the right `page`, sort, direction, and `q`; fetching the pages for 5 screens of 5 columns issues exactly `ceil(N·cols/120)` requests and never a per-scene request (SC-007);
  - implement `find_scenes_page` in `crates/stash-core/src/adapter/scenes.rs` (graphql_client derive for `find_scenes_page.graphql`), and `scene_screenshot_bytes(client, id) -> Result<Option<(Vec<u8>, String)>, AppError>` (bytes and content type, key in the header, the existing size cap) next to the existing `scene_screenshot`
- [X] T009 Thumbnails, test first (research R5):
  - tests in `crates/stash-core/tests/thumbs.rs`: a generated 3840×2160 JPEG becomes a 480 px wide JPEG (height kept in proportion, ≤ 30 KB); a PNG cover goes through the `image` path to the same size; a cached `thumb:scene:<id>:<version>` is returned without fetching; a fetch failure returns the placeholder bytes; at most 6 prepare at once (a counting fake fetcher);
  - implement `crates/stash-core/src/thumbs.rs`: `ThumbService::get(kind, id, version)` for `kind` = `scene` (other kinds return `None` for now): cache lookup → fetch via the adapter → JPEG fast path (libjpeg-turbo decode at the largest DCT scale that stays ≥ 480 px wide, `fast_image_resize` bilinear to exactly 480 px, libjpeg-turbo encode at quality 80) or `image` for PNG/WebP → `put_bytes`. A `tokio::sync::Semaphore` of 6; decoding in `spawn_blocking`. Prepare the thumbnail fully before touching the cache, and hold the cache mutex only for the `get_bytes` lookup and the `put_bytes` write (≈ 50 µs), never across the fetch or resize, so page reads aren't blocked (analysis C5). Export from `crates/stash-core/src/lib.rs`
  - add an `#[ignore]` timing test that prints the 4K prepare time in a release build (budget < 15 ms)

**Checkpoint**: the core can list any page of scenes and make any scene's thumbnail.

---

## Phase 2b: Foundational — page size (revision)

- [X] T010 Page size in the core, test first (research R1, R7; data-model "Page size", "ScenePage"):
  - tests: `crates/stash-core/tests/scenes_paging.rs` — `PAGE_SIZES` is `[20, 40, 50, 60, 120, 250, 500, 1000]`, `DEFAULT_PAGE_SIZE` is 50, `validate_page_size(55)` fails and `validate_page_size(250)` passes, `page_count(36_350, 50)` is 727, `page_of(1_250, 50)` is 26 (0-based index → 1-based page), `last_page(0, 50)` is 1; `crates/stash-core/tests/scenes_page_requests.rs` — the request sends `per_page` = the given size; a page past the end (mock returns no scenes with `count` 120 for page 9 at size 50) is re-requested as page 3 and the returned `ScenePage.page` is 3; one request per page shown (SC-007); `crates/stash-core/tests/scenes_cards.rs` — `ScenePage` carries `pageSize`;
  - implement: replace `PAGE_SIZE` in `crates/stash-core/src/scenes/paging.rs` with `PAGE_SIZES`, `DEFAULT_PAGE_SIZE`, `validate_page_size`, and size-aware `page_of`/`page_count`/`last_page` (drop `pages_for_range`); `find_scenes_page(client, &query, page, page_size)` in `crates/stash-core/src/adapter/scenes.rs` validates the size (any other value → `AppError::Internal`), and when Stash returns an empty page with a non-zero count asks once more for `last_page(count, size)`; add `page_size: u32` to `ScenePage` in `crates/stash-core/src/scenes/mod.rs`

**Checkpoint**: the core serves any page at any allowed size, and never an empty page past the end.

---

## Phase 3: User Story 1 - Browse the whole scene library, paged (Priority: P1) 🎯 MVP

**Goal**: Scenes shows the library one page at a time (default 50), with page controls, page
sizes, every web UI sort, keyboard navigation across pages, and per-tab state that never resets.

**Independent Test**: quickstart V1–V8 on the Production profile (read-only).

### Tests for User Story 1 (write first, must fail)

- [X] T011 [P] [US1] Shell navigation tests in `ui/src/__tests__/shell/tabs.test.ts` (research R13): `leave(tabId, fallback)` goes **back** when the previous entry's route equals the fallback (history index decreases, the entry's view state is untouched) and navigates otherwise; `navigateIn(tabId, route)` to a route the tab visited before starts the new entry with a copy of the most recent equal entry's view state, and with `null` view state on a first visit; a different tab never shares state
- [X] T012 [P] [US1] Page grid tests in `ui/src/__tests__/scenes/SceneGrid.test.tsx` (research R3, R8): renders exactly the given page's cards as a CSS grid, or rows in list mode (`.scene-grid.list`); thumbnails have `decoding="async"` and `loading="lazy"` and their `src` set immediately (no hold-back); placeholders while the page is loading, "Can't reach the server" when it's unreachable; the full title as a tooltip; one focusable card (roving focus), arrows move it; → on the last card calls `onEdge("next")`, ← on the first calls `onEdge("previous")`; Home/End go to the first/last card; Enter / Ctrl+Enter call `onOpen(card, false/true)`; middle-click and Ctrl+click open in a new tab
- [X] T013 [P] [US1] Page controls tests in `ui/src/__tests__/scenes/PageControls.test.tsx` (FR-004): text "Page 25 of 727 · 1,201–1,250 of 36,350" for page 25 at 50 per page of 36,350; first/previous disabled on page 1 and next/last disabled on the last page; buttons call `onPage` with the right numbers; "Go to page" with 9999 goes to the last page and with 0 or text does nothing; the page-size menu lists 20, 40, 50, 60, 120, 250, 500, 1000 and calls `onPageSize`
- [X] T014 [P] [US1] Replace `ui/src/__tests__/views/ScenesView.test.tsx` (paged):
  - page 1 at 50 per page is requested and shown, with controls above and below;
  - after page 1 arrives, page 2 is requested ahead (no page 0; on later pages both neighbours), and `Image` objects are created for the next page's thumbnails;
  - next/previous (buttons and `[` / `]` on the grid) change page and scroll to the top;
  - going to page 37, switching to list mode, and scrolling saves `{ query, page: 37, pageSize: 50, mode: "list", scroll }` in the tab's view state, and a view mounted on that entry restores all of it (SC-009);
  - changing the page size to 120 while the first visible scene is #1,850 goes to page 16;
  - a new sort goes to page 1; Random keeps its seed across pages until Reshuffle;
  - a `view-data-changed` event for `scenes:q:<hash>:s:50:p:37` re-reads page 37 and keeps the scroll position;
  - an empty library shows "No scenes in this library yet"; an unreachable page shows "Can't reach the server" with the controls still working;
  - Ctrl+click opens a scene in a new tab
- [X] T015 [P] [US1] Update `ui/src/__tests__/scenes/keyboard.test.tsx` for the paged keys: `nextIndex` returns `{ edge: "next" }` / `{ edge: "previous" }` past the page's last/first card, `{ page: "next" }` / `{ page: "previous" }` for `]` / `[`, Home/End within the page; the "Scene lists" listing includes "Next page" and "Previous page"
- [X] T016 [P] [US1] Regression tests for the demo's bugs in `ui/src/__tests__/shell/Shell.test.tsx` (SC-009): in a rendered App, Scenes on page 3 in list mode → open a scene → its Done button → Scenes is on page 3 in list mode; and Scenes → Home via the navigation bar → Scenes via the navigation bar → still page 3 in list mode

### Implementation for User Story 1

- [X] T017 [US1] The navigation fix (research R13) in `ui/src/shell/tabs.ts`: add `leave(tabId, fallback)` (back when the previous entry's route `routesEqual` the fallback, else `navigateIn`), and make `navigateIn` seed a new entry's view state from the most recent entry in that tab with an equal route; in `ui/src/App.tsx` route `onLeave` through `leave`. Makes T011 and T016 pass
- [X] T018 [US1] Command and key change in `src-tauri/src/scenes_commands.rs` (contract "Commands"): `scenes_page(query, page, page_size)` with cache key `scenes:q:<hash>:s:<page_size>:p:<page>`; regenerate `ui/src/bindings.ts`
- [X] T019 [US1] Rework `ui/src/scenes/pages.ts` (research R4, R7): `createScenePage(query, page, pageSize)` holding the current page's state (`loading` | `ready` | `unreachable`), cards, count, and the page actually returned; after it arrives, request the previous and next pages (the core caches them) and warm the next page's thumbnails with off-screen `Image` objects (at most 120); drop neighbour requests that are still waiting when the page changes; re-read the current page on `view-data-changed` for its key (or `*`); retry an unreachable page when the connection returns to connected (never in a loop)
- [X] T020 [P] [US1] `ui/src/scenes/SceneGrid.tsx` (research R3, R8): the page's cards in a plain CSS grid (`repeat(auto-fill, minmax(240px, 1fr))`) or a list; roving focus with `onEdge` at the page's ends; no virtualization. Update `ui/src/scenes/SceneCard.tsx`: remove `deferThumb` and the hold-back effect, add `loading="lazy"` next to `decoding="async"`; and `ui/src/scenes/scenes.css`: grid/list rules for `.scene-grid`, drop the `.vgrid` rules and `content-visibility`
- [X] T021 [P] [US1] `ui/src/scenes/PageControls.tsx` (FR-004): "Page N of M · first–last of total" (en-US number formatting), first/previous/next/last buttons, "Go to page" (clamped to 1…M), and the page-size menu (20, 40, 50, 60, 120, 250, 500, 1000)
- [X] T022 [US1] Rework `ui/src/views/ScenesView.tsx` (research R6): toolbar (SortMenu, grid/list, PageControls), SceneGrid in its own scroller, PageControls again below the grid; view state `{ query, page, pageSize, mode, scroll }` read once on mount and written on change (scroll coalesced to one write per animation frame, restored after the page renders); page size change → `floor(firstVisibleIndex / newSize) + 1`; query change → page 1; `[` / `]` and `onEdge` change page (focus the first/last card of the new page); empty and unreachable states. Update `ui/src/scenes/keyboard.ts` for `edge` and `page` actions and the "Next page" / "Previous page" listings. Remove `ui/src/scenes/VirtualGrid.tsx`, `ui/src/scenes/PositionBar.tsx`, and `ui/src/__tests__/scenes/VirtualGrid.test.tsx`
- [X] T023 [US1] Harness (research R10): in `ui/src/debug/bench.ts` replace the continuous-scroll measurements (`scenes-grid-*`, `scenes-list-*`, `scenes-jump`) with `scenes-page-change` (20 next/previous moves at 50 per page, each until its cards and thumbnails are painted), `scenes-page-jump` (5 random pages), and `scenes-scroll-1000` (a 1000-card page scrolled 5 s at 64 px per frame, missed frames against the idle baseline, in grid and list); map them in `src-tauri/src/bin/perf_harness.rs` (budgets: page change 150 ms at p95, jump 1000 ms at p95, scroll < 1% missed in each mode)
- [ ] T024 [US1] Run the full gate, then manual checks V1–V8 with the user on the Production profile (read-only), V6 (never resets) in both modes, and one harness run. Record the numbers in `specs/005-browse-scenes-and-play/quickstart.md`

**Checkpoint**: the whole library can be paged, jumped through, resized, sorted, and restored
exactly, within the budgets; the demo's resets and blank thumbnails are gone.

---

## Phase 4: Polish & Cross-Cutting Concerns

- [ ] T025 [P] Update `ROADMAP.md` Phase 1 "Scenes" (paged grid and list views and sorting done for P1; filters, saved filters, and the scene page still open) and `README.md` (the status line; thumbnails are cached with the rest of the cache)
- [ ] T026 Run the full gate: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, the bindings drift check, `npm --prefix ui run lint`, `npm --prefix ui run typecheck`, and `npm --prefix ui test`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup and Foundational (T001–T009)**: done (first version).
- **Phase 2b (T010)**: blocks T018 (the command needs the page-size fetch).
- **US1 (Phase 3)**: tests T011–T016 can be written any time; T017 (shell) is independent of
  the core; T018 needs T010; T019 needs T018; T020–T021 are independent; T022 needs T017–T021;
  T023 needs T022; T024 last.
- **Polish (Phase 4)**: after US1 is checked with the user.

### Within US1

Tests first (they must fail) → shell fix (T017) and command (T018) → page store (T019) → grid
and controls (T020, T021 in parallel) → the view (T022) → harness (T023) → gate and manual
checks (T024).

### Parallel Opportunities

- T011–T016: six test files, all independent.
- T017 alongside T010/T018 (shell vs core).
- T020 and T021 together.
- Polish: T025 alongside the final gate prep.

---

## Parallel Example: User Story 1

```bash
# Tests first, different files:
Task: "Shell navigation tests in ui/src/__tests__/shell/tabs.test.ts"
Task: "Page grid tests in ui/src/__tests__/scenes/SceneGrid.test.tsx"
Task: "Page controls tests in ui/src/__tests__/scenes/PageControls.test.tsx"
Task: "ScenesView tests in ui/src/__tests__/views/ScenesView.test.tsx"

# Then, in parallel:
Task: "SceneGrid.tsx and SceneCard.tsx updates"
Task: "PageControls.tsx"
```

---

## Implementation Strategy

### MVP First (User Story 1)

1. T010: page size in the core.
2. US1: the shell fix, the paged view, controls, keyboard, harness. **Validate** quickstart V1–V8
   with the user (V6 in both modes), and commit once confirmed.

### Incremental Delivery

3. Polish: roadmap, README, full gate.
4. Next passes: `/speckit-plan` for US2 (find), then US3 (scene page), US4 (playback history),
   US5 (animated image previews), US6 (fallback), each with its own tasks.

Commit after the story, only after the user has tried it (constitution; user preference).

---

## Notes

- The page size belongs to the tab (history entry), not to a global setting; new tabs start at 50.
- Returning to Scenes must reuse its history entry or carry its state forward (R13): this is
  what fixes the demo's resets, and it lives in the shell so every section gets it.
- No virtualization and no thumbnail hold-back: pages are small enough (measured, R3).
- `hasPreview` stays `false` until US5; previews will be animated images, never video.
- Regenerate `ui/src/bindings.ts` whenever a command or DTO changes.
