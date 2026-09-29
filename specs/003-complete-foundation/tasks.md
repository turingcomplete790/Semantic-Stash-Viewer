---

description: "Task list for 003-complete-foundation"
---

# Tasks: Complete Phase 0 (Cache, Performance Harness, CI)

**Input**: Design documents from `specs/003-complete-foundation/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/cache-and-harness.md](contracts/cache-and-harness.md),
[quickstart.md](quickstart.md)

**Tests**: Included. The constitution's Development Workflow gate requires core tests with
fixtures, and FR-006 / SC-005 require simulated-write tests. Write each story's tests first and
make sure they fail before implementing.

**Organization**: US1 (cache), US2 (performance harness), US3 (CI). US2 uses US1's clear-cache
for its cleared-cache cold start. US3 is independent.

**Rules that apply to every task**:
- **No writes to Stash** (FR-011, FR-017). The cache only avoids repeat reads; the harness only
  reads. Manual checks that change the library use the **test instance** (`localhost:9998`).
- **Discardable cache** (Principle I). It lives in the platform cache dir. Any failure falls back
  to reading from the server. A damaged or newer-schema file is deleted and recreated.
- **Never across servers** (FR-007). Each profile has its own file, checked against the server
  identity on every connect.
- **Quiet** (Principle IX, FR-005). No cache wording, sizes, or controls outside Settings →
  Troubleshooting.
- **Harness code in debug builds only** (FR-018). Reports hold the profile's display name,
  never its address or key.
- Regenerate `ui/src/bindings.ts` after any command, event, or DTO change.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on unfinished tasks)
- **[Story]**: US1–US3

---

## Phase 1: Setup (Shared Infrastructure)

- [X] T001 Add dependencies to `crates/stash-core/Cargo.toml`:
  - `rusqlite` 0.40 (features `bundled`; research R1);
  - `rustix` (the version already in `Cargo.lock`, features `fs`; research R1b, for `statvfs`).

  Confirm `cargo build -p stash-core` succeeds, and that `rustix` resolves to the locked version
- [X] T002 Extend `crates/stash-core/graphql/connect_probe.graphql` `systemStatus` with `databasePath` and `configPath` (research R2; still one request), and add both fields (as `databasePath: "/data/stash-go.sqlite"`, `configPath: "/data/config.yml"`, scrubbed generic paths) to the probe fixtures in `crates/stash-core/tests/fixtures/stash-v0.31.1/` that contain `systemStatus` (`probe-ok.json`, `probe-dev-version.json`, `probe-needs-migration.json`, `probe-old-version.json`). Run `cargo test -p stash-core` to confirm nothing else breaks

**Checkpoint**: builds, and every existing test passes with the new probe fields.

---

## Phase 2: Foundational (Blocking Prerequisites)

**⚠️ CRITICAL**: the server identity is needed before any cache entry is written.

- [X] T003 [P] Identity tests in `crates/stash-core/tests/cache_identity.rs`:
  - `server_identity(url, database_path, config_path)` is 16 lowercase hex digits and stable across calls;
  - it differs when any of the three inputs differs;
  - trailing slashes and URL case in scheme/host don't change it;
  - a probe of `probe-ok.json` through the mock server yields a `ServerInfo` whose `identity` matches `server_identity` of the final URL and the fixture's paths.
- [X] T004 Implement `crates/stash-core/src/cache/identity.rs`:
  - `pub fn server_identity(url: &Url, database_path: Option<&str>, config_path: Option<&str>) -> String`: FNV-1a 64 over the normalised URL (lower-case scheme and host, no trailing slash), then `\0`, the database path, `\0`, the config path; formatted `{:016x}`. No dependency (research R2);
  - add `pub identity: String` to `ServerInfo` in `crates/stash-core/src/connection/mod.rs`;
  - fill it where `StashProber::probe` builds `ServerInfo` from `ProbeData`; `ProbeData` gains `database_path` and `config_path: Option<String>`.

  Create `crates/stash-core/src/cache/mod.rs` (module skeleton) and add `pub mod cache;` in `crates/stash-core/src/lib.rs`. Makes T003 pass

**Checkpoint**: every connection snapshot's `server` carries an `identity`.

---

## Phase 3: User Story 1 - Screens appear instantly from the local cache (Priority: P1) 🎯 MVP

**Goal**: cached server summary and scene lists show at once (even after restart or offline),
refresh quietly in place, stay within a share of free disk space, never cross servers, and can
be cleared from Settings.

**Independent Test**: quickstart V1–V5.

### Tests for User Story 1 (write first, must fail)

- [X] T005 [P] [US1] ViewCache tests in `crates/stash-core/tests/cache.rs` (with a `tempfile` dir):
  - `put` then `get` returns the value, `fetched_at`, and `from_cache: true`, and touches `last_used`;
  - `size_bytes` sums entry sizes;
  - with an injected limit, a `put` over it evicts least-recently-used entries first and keeps the total at or under the limit;
  - `limit_for_free_space(free)` = 5% of `free`, clamped to 64 MB–10 GB (e.g. 100 GB free → 5 GB; 500 MB free → 64 MB; 1 TB free → 10 GB);
  - the limit is re-read after 50 writes (injected free-space function called again);
  - **targeted invalidation (SC-005)**: with keys `scene:1`, `scene:2`, `scenes:recent`, and `server:info`, `invalidate(["scene:1"])` removes exactly that entry, and `invalidate_prefix("scenes:")` removes exactly `scenes:recent`; every other entry is untouched;
  - `replace` overwrites;
  - `check_identity` with a new identity deletes all entries, and with the same identity keeps them;
  - a file of garbage is deleted and recreated empty on open, with no error;
  - a file with `schema_version` greater than 1 is recreated;
  - `clear` returns the bytes freed and leaves an empty cache;
  - `delete_profile_cache(dir)` removes the profile directory.
- [X] T006 [P] [US1] Refresher tests in `crates/stash-core/tests/cache_refresh.rs`, with a fake fetch closure and a `NotificationCenter` on a temp file:
  - **miss** fetches, stores, and returns `from_cache: false`;
  - **hit, under 5 s old**: returns cached data and doesn't fetch;
  - **hit, older than 5 s**: returns cached data and fetches in the background. The change callback fires only when the fetched JSON differs;
  - **offline** (online = false): hit returns cached data with no fetch; miss returns `AppError::Internal { "not connected…" }` as today;
  - **no-auto-refresh keys** (`scenes:test-set`) are never refreshed in the background;
  - 3 consecutive refresh failures post one `background` notification (key `background:<profile>:<key>`, `toast: false`), and failures lasting over 60 s (injected clock) set `toast: true`;
  - a later success updates that notification to "Refreshed" (info).

### Implementation for User Story 1

- [X] T007 [US1] Implement `ViewCache` in `crates/stash-core/src/cache/mod.rs` on `rusqlite`:
  - tables from [data-model.md](data-model.md): `meta(key TEXT PRIMARY KEY, value TEXT)` and `entries(key TEXT PRIMARY KEY, value BLOB, fetched_at INTEGER, last_used INTEGER, size INTEGER)`;
  - `open(path)`, which recreates the file if `PRAGMA quick_check` fails, the file doesn't parse, or `schema_version > 1`;
  - `get<T: DeserializeOwned>(key) -> Option<Cached<T>>`, `put<T: Serialize>(key, &T)`, `replace`, `invalidate(&[&str])`, `invalidate_prefix(&str)`, `size_bytes()`, `clear() -> u64` (followed by `VACUUM`), and `check_identity(&str)`;
  - `Cached<T> { data, from_cache, fetched_at }`, serde camelCase, specta behind the `specta` feature;
  - `limit_for_free_space(free: u64) -> u64` ("5% of the free space … never below 64 MB and never above 10 GB"), with free space from `rustix::fs::statvfs(dir)` (`f_bavail * f_frsize`), and 1 GB when unavailable;
  - the limit is recomputed on open and every 50 `put`s;
  - eviction is `DELETE … ORDER BY last_used LIMIT …` until `SUM(size) <= limit`;
  - `pub fn delete_profile_cache(dir: &Path)`.

  All errors map to `AppError::Storage`, and callers treat cache errors as misses. Makes T005 pass
- [X] T008 [US1] Implement `crates/stash-core/src/cache/refresh.rs`:
  - `Refresher` (per profile: `Arc<Mutex<ViewCache>>`, an online flag, a `NotificationCenter`, a clock, a change callback `Fn(&str)`);
  - `get_or_fetch<T, F, Fut>(key, policy, fetch) -> Result<Cached<T>, AppError>` implementing research R4: a 5 s refresh floor, offline skip, and `RefreshPolicy::{Auto, Manual}`;
  - an in-flight set so one key never refreshes twice at once;
  - failure tracking per research R5 (3 failures → notification; over 60 s → toast; success → "Refreshed").

  Makes T006 pass
- [X] T009 [US1] Hold caches in `src-tauri/src/state.rs`:
  - a `CacheRegistry` that opens `app.path().cache_dir()/semantic-stash-viewer/<profile-id>/cache.sqlite3` lazily per profile (returning a `Refresher`), whose online flag follows the connection manager's snapshots;
  - on each `Connected` snapshot with `server: Some(info)`: call `check_identity(&info.identity)`, then `put("server:info", &info)`;
  - `delete_profile` in `src-tauri/src/commands.rs` also calls `delete_profile_cache` for that profile (FR-009).
- [X] T010 [US1] Update the read commands in `src-tauri/src/player_commands.rs` and add `src-tauri/src/cache_commands.rs`:
  - `list_recent_scenes() -> Cached<Vec<SceneListItem>>` (key `scenes:recent`, Auto);
  - `list_test_scenes(shuffle: bool) -> Cached<Vec<SceneGroup>>` (key `scenes:test-set`, Manual; `shuffle` forces a fetch);
  - `open_scene` uses `get_or_fetch("scene:<id>")` for `PlayableScene` and still checks `is_direct_stream`;
  - new `cached_server_info() -> Option<Cached<ServerInfo>>` (active profile, else last-used);
  - new `cache_size() -> u64` and `clear_cache() -> u64`. The latter emits `view-data-changed` for each cleared key;
  - a `ViewDataChangedEvent { profile_id, key }` (`view-data-changed`) emitted from the refresher's change callback.

  Register everything in `src-tauri/src/lib.rs` and regenerate bindings (contract: [cache-and-harness.md](contracts/cache-and-harness.md))
- [X] T011 [P] [US1] UI tests:
  - `ui/src/__tests__/views/ScenesView.test.tsx`: renders the `data` of `Cached` lists; on a `view-data-changed` event for `scenes:recent` it re-invokes `listRecentScenes` and updates in place, with no "Loading…" shown during the refetch and the pane's scroll offset unchanged; Shuffle calls `listTestScenes(true)`;
  - `ui/src/__tests__/views/HomeView.test.tsx`: while connecting or offline with no live `server`, shows the summary from `cachedServerInfo()`;
  - `ui/src/__tests__/settings/SettingsView.test.tsx`: Troubleshooting shows the formatted cache size, and "Clear cache" calls `clearCache` and shows "Freed 12.4 MB".
- [X] T012 [US1] Add `ui/src/state/viewData.ts`: `onViewDataChanged(key, callback)`, which subscribes to `view-data-changed` for the active profile and cleans up with the owner. Adapt `ui/src/views/ScenesView.tsx` to `Cached<T>` (use `.data`, refetch on change, and use `latest` so a refetch never shows "Loading…"), with Shuffle passing `shuffle: true`
- [X] T013 [US1] Home's cached summary: in `ui/src/views/HomeView.tsx` / `ui/src/components/SessionView.tsx`, when `snapshot.server` is null (connecting or offline), show `ServerSummary` from `commands.cachedServerInfo()` (refetched on `server:info` changes). No cache wording (FR-003)
- [X] T014 [US1] Fill the Clear cache slot in `ui/src/settings/TroubleshootingPage.tsx`: a "Cache" item showing `cacheSize()` formatted (KB/MB/GB), a "Clear cache" button (no confirmation, FR-005), and a "Freed …" status afterwards. Makes T011 pass
- [X] T015 [US1] Run the full gate. Then manual check quickstart V1–V5 with the user (V3 and V5 use the test instance)

**Checkpoint**: screens are instant after restart and offline, the cache is invisible outside
Settings, and it never crosses servers.

---

## Phase 4: User Story 2 - Measure the performance budgets with one command (Priority: P2)

**Goal**: `cargo run --bin perf-harness -- --profile "<name>"` produces a report of every
Principle VI budget with pass/fail/invalid, and the change since the last run.

**Independent Test**: quickstart V6.

### Tests for User Story 2 (write first, must fail)

- [ ] T016 [P] [US2] Report tests in `src-tauri/src/bin/perf_harness/report.rs` (`#[cfg(test)]`):
  - `summarise(samples)` gives the median, p95, and max;
  - a measurement with budget 150 ms and median 120 passes, and one with median 180 fails;
  - one with `invalid_reason` is `invalid` whatever its numbers;
  - comparison with a previous report gives the percent change and sets `regressed` when more than 20% slower (lower is better for `ms` and `count`, higher is better for `fps`);
  - a missing previous measurement leaves the change null;
  - the Markdown render has one row per measurement, and includes the profile display name but never an address.

### Implementation for User Story 2

- [ ] T017 [US2] Add `src-tauri/src/harness.rs` (debug builds):
  - record the process start `Instant` in a `OnceLock`, set first thing in `run()`;
  - `SSV_HARNESS_PROFILE=<display name>` connects to that profile instead of the last-used one (in place of `auto_connect`);
  - `SSV_HARNESS_CLEAR_CACHE=1` clears that profile's cache before connecting;
  - `SSV_HARNESS_EXIT=1` exits the app after a `MEASURE {"done":true}` or `{"bench":"done"}` line has been printed;
  - a `debug_mark_interactive()` command prints `MEASURE {"coldStartMs": <since process start>}` the first time it's called.
- [ ] T018 [US2] Extend `ui/src/debug/bench.ts` and `ui/src/App.tsx`:
  - call `debugMarkInteractive()` once when Home first paints with server info (live or cached; after a frame);
  - add section-navigation timing (Home ↔ Scenes via `navigate`, 20 switches, each to the next painted frame);
  - add a scroll test: a debug-only overlay (`ui/src/debug/ScrollBench.tsx`) with a windowed 10,000-row list, scrolled programmatically for 5 s, recording every animation-frame interval and reporting the p95 frame time and the share over 16.7 ms;
  - mark any measurement taken while `document.visibilityState !== "visible"` as `{"invalid": "window hidden"}` (FR-016).
- [ ] T019 [US2] Update `src-tauri/src/measure.rs`:
  - a long run whose main-thread CPU samples include 0% (frames not drawn) reports `"invalid": "window not drawn"`;
  - runs that can't connect within 30 s report `"invalid": "not connected"`;
  - respect `SSV_HARNESS_EXIT` (research R8, FR-016).
- [ ] T020 [US2] Implement the harness binary `src-tauri/src/bin/perf_harness.rs` (with `report.rs` alongside). With `--profile <name>` and `--quick` (fewer repetitions), it:
  - builds the debug app (`cargo build -p semantic-stash-viewer`);
  - starts `npm --prefix ui run dev` and waits for port 5173;
  - runs a discarded warm-up launch, then cold start ×3 with `SSV_HARNESS_CLEAR_CACHE=1` and ×3 warm;
  - runs one UI bench launch (`SSV_DEBUG_BENCH=1`);
  - runs one playback launch (`SSV_MEASURE` over 3 test-set scenes plus `SSV_MEASURE_LONG` 60 s on a 1080p scene chosen from the library's recent list);
  - uses `SSV_HARNESS_EXIT=1` for every launch, with a 5-minute timeout each;
  - parses the `MEASURE` lines, builds the `Measurement` list with the budgets from research R8, and compares with the newest earlier report in `~/.local/share/semantic-stash-viewer/perf/`;
  - writes `<timestamp>.json` and `.md`, prints the Markdown table, and exits non-zero if any measurement failed.

  Makes T016 pass
- [ ] T021 [US2] Run the full gate. Then run the harness twice with the user watching (quickstart V6): check it finishes in under 10 minutes (SC-006) and that the medians agree within 10% (SC-007). Record the numbers in `specs/003-complete-foundation/quickstart.md`

**Checkpoint**: one command measures every Phase 0 budget and flags regressions.

---

## Phase 5: User Story 3 - Every push is checked automatically (Priority: P3)

**Goal**: CI covers every gate (it already does), can't hang, cancels superseded runs, and shows
its status in the README.

**Independent Test**: quickstart V7.

- [ ] T022 [P] [US3] Harden `.github/workflows/ci.yml`: add `timeout-minutes: 30` to the `check` job, and top-level `concurrency: { group: ci-${{ github.ref }}, cancel-in-progress: true }`. Keep every existing named step (FR-019, FR-020; research R10)
- [ ] T023 [P] [US3] Add the CI badge to the top of `README.md`: `![CI](https://github.com/turingcomplete790/Semantic-Stash-Viewer/actions/workflows/ci.yml/badge.svg?branch=main)`, linked to the workflow page (FR-022)
- [ ] T024 [US3] Verify SC-008 **with the user's go-ahead** (it pushes):
  - push this branch and confirm the run passes in under 15 minutes;
  - push a throwaway branch with one deliberate formatting error, confirm the run fails at "Rust format", then delete the throwaway branch (locally and on the remote).

  Record the run links and durations in `specs/003-complete-foundation/quickstart.md`

**Checkpoint**: CI is visible, bounded, and proven to name failing steps.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T025 [P] Update `ROADMAP.md` Phase 0: tick the cache layer, performance harness, and CI items and link `specs/003-complete-foundation/` (FR-023); tick Phase 0 as complete in the overview if every item is done
- [ ] T026 [P] Update `README.md`: the status line (Phase 0 complete), a short "Performance harness" section (`cargo run --bin perf-harness -- --profile "<name>"`, where reports go), and the cache location (`~/.cache/semantic-stash-viewer/`, safe to delete)
- [ ] T027 Run the full gate: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, bindings drift (`cargo run -p semantic-stash-viewer --bin export-bindings` then `git diff --exit-code ui/src/bindings.ts`), `npm --prefix ui run lint`, `npm --prefix ui run typecheck`, and `npm --prefix ui test`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: none.
- **Foundational (Phase 2)**: depends on Setup (the probe fields). Blocks US1.
- **US1 (Phase 3)**: depends on Foundational. MVP.
- **US2 (Phase 4)**: depends on US1 for `clear_cache` (cleared-cache cold start) and the cached
  Home summary (warm cold start). The report logic (T016) can start any time.
- **US3 (Phase 5)**: independent. It can run any time, and T024 is best after US1/US2 are pushed.
- **Polish (Phase 6)**: after all stories.

### Story Dependencies

```text
Setup ─► Foundational ─► US1 (MVP) ─► US2 ─┐
US3 (any time) ────────────────────────────┼─► Polish
```

### Within Each Story

Tests first (they must fail) → core (`stash-core`) → commands and events (`src-tauri`) →
bindings → UI → checkpoint check.

### Parallel Opportunities

- Foundational: T003 alongside T001–T002.
- US1: T005, T006, and T011 in parallel; T007 and T008 are separate files (T008 depends on T007's types).
- US2: T016 any time; T017 and T018 in parallel.
- US3: T022 and T023 in parallel, and in parallel with US1/US2.
- Polish: T025 and T026 in parallel.

---

## Parallel Example: User Story 1

```bash
# Tests first, different files:
Task: "ViewCache tests in crates/stash-core/tests/cache.rs"
Task: "Refresher tests in crates/stash-core/tests/cache_refresh.rs"
Task: "UI tests for cached views and Clear cache in ui/src/__tests__/"
```

---

## Implementation Strategy

### MVP First (User Story 1)

1. Setup → Foundational (probe identity).
2. US1: the cache. **Validate** quickstart V1–V5 with the user, and commit once confirmed.

### Incremental Delivery

3. US2: the harness (V6), run twice.
4. US3: CI hardening and badge (V7). The throwaway-branch check needs the user's go-ahead.
5. Polish: roadmap (Phase 0 complete), README, full gate.

Commit after each story, only after the user has tried it (constitution; user preference).

---

## Notes

- A cache miss must behave exactly like today's read. The cache only removes repeat requests.
- `scenes:test-set` is never auto-refreshed: it's random by design.
- The harness never writes to Stash, and its reports never include a server address or key.
- Regenerate `ui/src/bindings.ts` whenever a command, event, or DTO changes.
