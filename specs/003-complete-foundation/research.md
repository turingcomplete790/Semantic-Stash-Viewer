# Research: Complete Phase 0 (Cache, Performance Harness, CI)

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md) | **Date**: 2026-09-29

Findings marked **(observed)** were checked on the development machine, in the Stash schema
(`crates/stash-core/graphql/schema.json`, v0.31), or on GitHub Actions.

---

## R1. Where the cache lives: one SQLite file per profile, in the cache directory

- **Decision**:
  - The cache is a SQLite database per server profile, in the platform cache directory:
    `~/.cache/semantic-stash-viewer/<profile-id>/cache.sqlite3` on Linux.
  - It's accessed through `rusqlite` with its `bundled` feature (SQLite compiled in, so there's no
    system library dependency).
  - One table of entries. Each has a key, a JSON value, `fetched_at`, `last_used`, and `size`.
    There's also a small `meta` table for the schema version and the server identity (R2).
- **Rationale**:
  - The cache directory is where discardable data belongs (Principle I). Clearing it, or the OS
    cleaning it, costs only time.
  - One file per profile makes FR-009 (delete a profile's cache) and FR-007 (never cross servers) a
    matter of which file is open. There are no shared rows to filter.
  - SQLite gives atomic writes, cheap least-recently-used eviction (`ORDER BY last_used`), size
    accounting (`SUM(size)`), and damage detection (`PRAGMA quick_check`) without custom code. Phase
    1's grids and thumbnails will reuse the same file.
  - The size limit (FR-008, R1b) needs per-entry sizes and ordered eviction, which is awkward to
    do well with one JSON file per entry.
- **Alternatives rejected**:
  - JSON files (like 004's shell stores): fine for kilobytes, but rewriting whole files doesn't
    scale to hundreds of MB, and there's no cheap eviction order.
  - `redb` or `sled` (pure Rust): younger, and less widely used than SQLite (constitution:
    "prefer mature, widely used crates").
- **New dependency**: `rusqlite` (with `bundled`). It's the standard Rust SQLite binding, actively
  maintained, and SQLite itself is the most deployed database there is. It adds about 1.5 MB to
  the binary and one C compile on first build.

## R1b. Size limit: a share of free disk space

- **Decision** (user choice, 2026-09-29): the limit is **5% of the free space** on the disk that
  holds the cache, clamped to **64 MB–10 GB** per profile.
  - It's computed when the cache opens and re-computed every 50 writes. If free space shrinks,
    the limit shrinks and the next write evicts down to it.
  - Free space comes from `statvfs` on the cache directory via `rustix::fs::statvfs`. `rustix` is
    already in the dependency tree (safe API, pure Rust, all Unix platforms). Where it's
    unavailable (Windows, until it's supported), the limit falls back to 1 GB.
- **Rationale**:
  - It adapts to the machine: a small laptop disk gets a small cache, and a big disk leaves room
    for Phase 1's covers (about 1–2 GB for a 35,000-scene library).
  - The floor keeps the cache useful on nearly full disks. The cap stops a huge disk from growing
    it without end.
  - Data responses alone stay at a few MB, far below even the floor.
- **Alternatives considered**: a fixed 512 MB (the first draft); split data and image limits;
  a larger fixed default with a Settings option. The user chose the share of free space.

## R2. Telling servers apart

- **Findings (observed)**: Stash's `systemStatus` has `databasePath` and `configPath`. The
  existing connect probe already queries `systemStatus` (for `appSchema` and `status`).
- **Decision**:
  - The **server identity** is a hash of the final base URL, `databasePath`, and `configPath`,
    computed when a session connects. The probe query gains the two fields; it's still one request.
  - The cache's `meta` table stores the identity. When a connection reports a different identity,
    the profile's cache is wiped before anything is shown or written (FR-007: the address now
    points at a different server, or the server was replaced).
  - The identity is a 64-bit FNV-1a hash in hex, a few lines of code with no dependency. The
    cache never stores the server's file paths.
- **Rationale**:
  - Two Stash instances at the same address (say a test instance restored onto a port) have
    different database or config paths.
  - A profile's address edit already creates a different URL.
- **Alternatives rejected**:
  - URL only: misses a replaced server at the same address (spec edge case).
  - Stash's `version.hash`: that identifies the build, not the instance.

## R3. The cache API in the core

- **Decision**: a `stash_core::cache` module with a `ViewCache` per profile:
  - `get<T>(key) -> Option<Cached<T>>`, where `Cached` holds the data and `fetched_at`. A read
    also touches `last_used`.
  - `put<T>(key, &T)`, which evicts least-recently-used entries past the limit;
  - `invalidate(keys)` and `invalidate_prefix(prefix)` (FR-006; Phase 2 writes call these);
  - `replace<T>(key, &T)`, for a write that returns the new object (an alias of `put` that
    documents the intent);
  - `size_bytes()` and `clear() -> bytes_freed` (FR-005);
  - `check_identity(identity)`, which wipes the cache if it doesn't match (R2).

  Opening a damaged or newer-schema file deletes and recreates it (FR-010), logging a warning.
- **Keys** (strings, one per view's data):

  | Key | Data | Source |
  |---|---|---|
  | `server:info` | `ServerInfo` (version, counts) | the connection probe on each connect |
  | `scenes:recent` | recently added list | `RecentScenes` |
  | `scenes:test-set` | the spike's test groups | `TestScenes` |
  | `scene:<id>` | playable scene details | `PlayableScene` |

- **Rationale**:
  - Keys are the unit of targeted invalidation (FR-006). A Phase 2 edit to scene 7 invalidates
    `scene:7` and the list prefixes that show it (`scenes:`) and nothing else, which SC-005 tests.
  - Values are the domain types' JSON, so the cache doesn't know GraphQL shapes.

## R4. "Show cached, then refresh" between the UI and the core

- **Decision**:
  - Read commands return `Cached<T> { data, fromCache, fetchedAt }`. They cover:
    - `list_recent_scenes`;
    - `list_test_scenes`;
    - a new `cached_server_info`;
    - the scene lookup inside `player_open`.
  - **Cache hit**: the command returns at once, and if the entry is older than 5 seconds it starts
    a background refresh. When the refresh finishes and the data **differs**, the core emits
    `view-data-changed { profileId, key }`. The UI re-invokes the command and gets the fresh
    cached copy, which doesn't trigger another refresh because it's under 5 s old.
  - **Cache miss**: fetch as today, store, and return `fromCache: false`.
  - **Offline** (the session isn't connected): return the cached copy if there is one, and skip the
    refresh. A miss offline returns the usual "not connected" error.
  - The **test set** is the exception: it's random by design, so it isn't refreshed automatically,
    and only Shuffle fetches a new one. Otherwise every visit would reshuffle it.
- **Rationale**:
  - One code path per read. The UI's existing `createResource` calls keep working. Solid keeps
    showing the previous value while a refetch runs, so the list updates in place with no loading
    state (FR-002).
  - An event per key, without a payload, keeps the event type simple and specta-friendly.
  - The 5-second floor stops refresh loops, and keeps requests per screen at one per visit
    (FR-011, Principle IV).
- **Alternatives rejected**:
  - Pushing the data in the event: an event type per key, or an untyped payload.
  - Polling from the UI: extra requests.

## R5. Background failures become one notification

- **Decision**:
  - The refresher counts consecutive failures per key. After 3 in a row it posts a `background`
    notification, key `background:<profile>:<key>`: title "Couldn't refresh <what>", with the
    failure's plain message.
  - It sets `toast: true` only once the failures have lasted more than 60 seconds.
  - A later success updates the same entry to "Refreshed" (information), which the user can
    dismiss.
  - Refreshes aren't attempted while the session is offline. The connection notification already
    covers that (004).
- **Rationale**: completes 004 FR-018's producers (FR-011a) without duplicating the "server
  unreachable" alert.

## R6. Cold start and the server summary offline

- **Decision**:
  - On connect, the core writes the probe's `ServerInfo` to `server:info`.
  - Home shows the cached summary while the connection is still being made, or when the server is
    unreachable at launch (instead of an empty "Connecting…"). The connection state still shows
    in the navigation bar.
- **Rationale**: this is the "screen appears instantly after restart" case (SC-001) for the first
  screen users see, and it feeds the cold-start measurement (SC-002).

## R7. Settings → Troubleshooting → Clear cache

- **Decision**:
  - Commands `cache_size() -> u64` and `clear_cache() -> u64` (bytes freed) act on the active
    profile's cache (or, when disconnected, the last-used profile's).
  - The Troubleshooting page shows the size ("12.4 MB") and a "Clear cache" button, with no
    confirmation (FR-005). Afterwards it says how much was freed.
  - Open views keep what they're showing, and reload from the server on their next visit or
    refresh. The core emits `view-data-changed` for every key, so open views refetch from the
    server at once.
- **Rationale**: FR-005, and the "clearing while a screen is open" edge case.

## R8. The performance harness

- **Decision**: a `perf-harness` binary in `src-tauri` (debug builds), run as
  `cargo run --bin perf-harness -- --profile "<name>"`. It:
  1. builds the debug app, starts the UI dev server once, and waits for it;
  2. launches the debug app binary several times with harness environment variables. The app
     exits by itself when its measurements finish (`SSV_HARNESS_EXIT=1`). The runs are:
     - **cold start**, 3 runs with a cleared cache and 3 with a warm one;
     - the **UI bench** (from 004), with 20 tabs;
     - **playback**: the `SSV_MEASURE` open/seek runs on a short scene list, plus a 60-second
       1080p frame-drop run;
  3. collects every `MEASURE` line, and writes a report to
     `~/.local/share/semantic-stash-viewer/perf/<timestamp>.json` and `.md`;
  4. compares it with the previous report: the change for each measurement, with anything more than
     20% slower flagged (FR-015).

  The per-run switches are:
  - `SSV_HARNESS_PROFILE` connects to that profile for the run;
  - `SSV_HARNESS_CLEAR_CACHE` clears the cache first;
  - `SSV_HARNESS_EXIT` makes the app exit once its measurements finish.

  Existing modes are reused: `SSV_MEASURE` (002) and `SSV_DEBUG_BENCH` (004).
- **Measurements and budgets** (Principle VI):

  | Measurement | Budget | Source |
  |---|---|---|
  | Cold start → interactive connected view (warm cache) | < 2 s | process start → Home painted with server info |
  | Cold start (cleared cache) | reported, no budget | same |
  | View navigation first paint (section switch, warm cache) | < 150 ms | UI bench |
  | Input acknowledgement (control press) | < 50 ms | UI bench |
  | Scroll frame rate, 10,000-item list | 60 fps: under 1% missed frames (see below) | UI bench, synthetic list |
  | Playback open → first frame | ≤ 1.5 s (002 SC-001) | `SSV_MEASURE` |
  | Seek → new frame | ≤ 1 s (002 SC-002) | `SSV_MEASURE` |
  | Dropped frames, 1080p | ≤ 1/min (002 SC-003) | `SSV_MEASURE_LONG` |

- **Cold start timing**: `main` records the process start time. When Home first paints with
  server info, the UI calls `debug_report`, and the core computes the elapsed time since process
  start. This doesn't depend on the harness's own timing.
- **Scroll**: the bench mounts a debug-only overlay with a windowed 10,000-row list, used only in
  harness runs (FR-018, no screens in user builds). It scrolls programmatically for 5 seconds.
  Every animation frame's interval is recorded. A frame is **missed** when it takes more than 1.5×
  the display's frame time (25 ms at 60 Hz), and the budget is under 1% missed. The p95 frame time
  and frames per second (`scroll-fps`) are reported too, without a budget.
  - *Amended 2026-09-29 after the first harness run:* the original "p95 ≤ 16.7 ms" failed on
    timer jitter alone (300 frames in 5 s, exactly 60 fps, worst frame 23 ms, none missed),
    because frames on a 60 Hz display land at 16.7 ms ± about 1 ms. The missed-frame rule counts
    real stutters and works at any refresh rate.
  - *Amended 2026-09-30 (005 analysis M1):* the display's frame time is measured from an idle
    animation-frame loop **before** scrolling, not taken from the scrolled frames' own median.
    Judged against their own median, frames that all slow down equally (for example a steady
    44 fps) counted as 0% missed.
- **Playback runs show the scene** in a scene tab (debug measure mode), because the shell hides
  the video surface when no scene tab is on screen; otherwise frames aren't drawn and the run is
  invalid.
- **Invalid, not failed** (FR-016):
  - a UI measurement taken while `document.visibilityState !== "visible"` is marked invalid;
  - playback frame counts are marked invalid if the main-thread CPU sample drops to 0% (no
    drawing, the finding from 002);
  - a run that can't connect within 30 seconds stops, and marks its measurements invalid with the
    reason.
- **Rationale**:
  - It reuses what already works (002's measure mode, 004's bench) behind one command.
  - Launching the built binary directly (not `cargo tauri dev`) makes the runs repeatable and lets
    the app exit on its own.
  - Reports live outside the repo because the numbers belong to one machine and library.
- **Alternatives rejected**:
  - WebDriver automation (`tauri-driver`): not supported for WebKitGTK apps on Wayland today, and
    it adds a dependency.
  - Running in CI: no GPU, noisy timings (spec assumption).

## R9. Repeatability (SC-007)

- **Decision**:
  - Each measurement uses enough samples (at least 10, or 40 for tab switches and 5 seconds of
    frames for scrolling) that medians are stable.
  - The harness runs a warm-up pass first (discarded), so the first app launch's disk cache
    doesn't skew cold start.
  - SC-007 is checked by running the harness twice and comparing the two reports. The harness
    prints the largest median difference.
- **Rationale**: harnesses fail most often through noise, and warm-up plus medians handle it.

## R10. CI: what's there, what's missing

- **Findings (observed)**:
  - `.github/workflows/ci.yml` already runs every FR-019 step as separately named steps: Rust
    format, Clippy (`-D warnings`), Rust tests, the bindings drift check, UI lint (ESLint and
    Prettier), UI typecheck, and UI tests. It installs libmpv, ffmpeg, and WebKitGTK.
  - It passed on GitHub for every pushed commit so far, including `cba2927` with the WebSocket jobs
    test. Runs take about 6 minutes (FR-020, SC-008).
- **Missing**:
  - The README status badge (FR-022).
  - `timeout-minutes`, so a hung test can't hold a runner for 6 hours.
  - `concurrency`, to cancel superseded runs on the same branch.
  - The new cache tests need nothing special: SQLite is bundled, and the mock servers are local.
- **Decision**:
  - Add the badge (`![CI](https://github.com/turingcomplete790/Semantic-Stash-Viewer/actions/workflows/ci.yml/badge.svg?branch=main)`).
  - Add `timeout-minutes: 30` and `concurrency: { group: ci-${{ github.ref }}, cancel-in-progress: true }`.
  - Verify SC-008's "broken commit fails with the step named" once, on a throwaway branch
    (formatting error, push, see the failed "Rust format" step, delete the branch). This needs the
    user's go-ahead, because it pushes.
