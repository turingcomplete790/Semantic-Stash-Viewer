# Implementation Plan: Complete Phase 0 (Cache, Performance Harness, CI)

**Branch**: `003-complete-foundation` | **Date**: 2026-09-29 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/003-complete-foundation/spec.md`

## Summary

This closes Phase 0's last three items.

**Cache.** A discardable, per-profile SQLite cache in the platform cache directory holds what the
viewer reads from Stash: the server summary, the scene lists, and scene details. Views show the
cached copy at once, refresh it in the background, and update in place when it changes. It stays
under 5% of free disk space (64 MB–10 GB; least recently used goes first). It's wiped if the server behind a profile changes,
repairs itself if damaged, and can be cleared from Settings → Troubleshooting. Writes in Phase 2
use its targeted invalidation.

**Harness.** One command (`cargo run --bin perf-harness`) drives the real app through cold start
(cleared and warm cache), section navigation, a control press, scrolling a 10,000-item list, and
playback open/seek/frame drops. It writes a report with pass/fail/invalid for each measurement,
and the change since the previous run.

**CI.** The existing workflow already covers every gate. It gets a README badge, a timeout, and
cancellation of superseded runs.

From [research.md](research.md):
- **Storage (R1–R3)**: `rusqlite` (bundled). Server identity from the probe's `databasePath`,
  `configPath`, and final URL (R2). Keys per view's data (R3).
- **Refresh (R4–R6)**: `Cached<T>` returns, plus a `view-data-changed` event. A 5-second refresh
  floor, no refresh while offline, and failures posted to the notification centre. Home keeps
  the cached summary while connecting or offline.
- **Clear cache (R7)**: size and "Clear cache" on the Troubleshooting page, in the slot 004
  left for it.
- **Harness (R8–R9)**: reuses 002's `SSV_MEASURE` and 004's `SSV_DEBUG_BENCH`, adds
  `SSV_HARNESS_*` switches, and writes reports outside the repo.
- **CI (R10)**: badge, `timeout-minutes`, and `concurrency`.

## Technical Context

**Language/Version**: Rust 1.94 (workspace minimum 1.80); TypeScript 5.9

**Primary Dependencies**:
- New: `rusqlite` with `bundled`, in `stash-core` (R1).
- `rustix` (feature `fs`, for `statvfs`), already in the tree, becomes a direct dependency of
  `stash-core` for the free-space size limit (R1b).
- Existing: tauri 2.11, tauri-specta, reqwest, graphql_client, SolidJS, and 004's shell stores and
  notification centre.
- No new npm packages.

**Storage**:
- `~/.cache/semantic-stash-viewer/<profile>/cache.sqlite3`: discardable, limited per profile to
  5% of free disk space (64 MB–10 GB, R1b).
- Harness reports in `~/.local/share/semantic-stash-viewer/perf/`.

**Testing**:
- `cargo test`: the ViewCache (LRU, limits, invalidation for SC-005, identity wipe, damaged-file
  recovery, clear), and the refresher with the mock prober and wiremock (5 s floor, offline skip,
  failure notifications).
- Vitest: views over cached data, in-place updates, Home's offline summary, Troubleshooting's
  size and clear.
- Manual checks V1–V7 in [quickstart.md](quickstart.md).

**Target Platform**: Linux (primary). The cache uses Tauri's platform cache dir, so it's portable.

**Project Type**: Desktop app (Tauri: Rust core and web UI), extending features 001–004

**Performance Goals**:
- a revisited screen appears in < 150 ms (SC-001);
- cold start with a warm cache < 2 s (SC-002);
- a full harness run < 10 min (SC-006), with medians repeatable within 10% (SC-007);
- CI < 15 min (SC-008).

**Constraints**:
- no writes to Stash, and no new network destinations (the harness included);
- the cache never crosses servers;
- no cache wording outside Settings;
- harness code in debug builds only (FR-018);
- CI needs no secrets.

**Scale/Scope**:
- 4 cache keys today, and 3 read commands changed to `Cached<T>`;
- 3 new commands and 1 new event;
- 1 new binary (`perf-harness`), 3 harness environment switches, and about 10 measurements.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | How this plan complies | Status |
|---|---|---|
| **I. Stash is the system of record** | The cache is discardable (the cache dir, wiped on damage or identity change, clearable) and never authoritative. Nothing is written to Stash. | ✅ Pass |
| **II. Semantic-tagging parity** | Not applicable. | ✅ N/A |
| **III. Brain/UI separation** | ViewCache, identity, refresher, and harness report logic are in `stash-core` and the harness binary. `src-tauri` only exposes commands and events. The UI does no file or network I/O. | ✅ Pass |
| **IV. Lean, batched GraphQL** | The cache removes repeat reads. Refreshes reuse the same narrow queries, at most one per visit (5 s floor), none while offline. The probe gains two fields in its existing request. Principle IV's "explicit invalidation" and "manual clear" are delivered (FR-005, FR-006). | ✅ Pass |
| **V. Native playback over transcoding** | Unchanged. `player_open` still builds only the direct stream URL, now from cached scene details when present. | ✅ Pass |
| **VI. Responsive UI** | This feature exists to meet and measure Principle VI: instant revisits, cold start < 2 s, and a harness for every budget except semantic queries (Phase 4). | ✅ Pass |
| **VII. User-owned connection** | The harness talks only to the configured server. Reports hold the profile's display name, never its address or key. | ✅ Pass |
| **VIII. Web UI parity** | Infrastructure. Nothing is dropped. | ✅ N/A |
| **IX. One app shell** | Clear cache lives on Settings → Troubleshooting. Refresh failures go to the notification centre. No cache wording on main screens. | ✅ Pass |
| **Tech constraints** | One new crate (`rusqlite`), justified in R1; `rustix` is already in the tree (R1b). | ✅ Pass |
| **Workflow gates** | Core tests with fixtures and simulated writes (SC-005), UI tests, CI unchanged except for hardening, and the performance gate made measurable by the harness. | ✅ Pass |

**Post-design re-check (after Phase 1)**: ✅ unchanged.
- The contract keeps every read path identical on a cache miss.
- The only new events and commands are read-only.
- The data model limits the cache to a share of free disk space and removes it with the profile.

## Project Structure

### Documentation (this feature)

```text
specs/003-complete-foundation/
├── plan.md
├── research.md            # R1–R10
├── data-model.md
├── quickstart.md          # V1–V7
├── contracts/
│   └── cache-and-harness.md
├── checklists/requirements.md
└── tasks.md               # /speckit-tasks
```

### Source Code (repository root)

```text
crates/stash-core/
├── graphql/connect_probe.graphql      # + systemStatus.databasePath, configPath
├── src/
│   ├── cache/                         # new
│   │   ├── mod.rs                     # ViewCache (SQLite), Cached<T>, keys, limits, identity check
│   │   ├── identity.rs                # FNV-1a server identity
│   │   └── refresh.rs                 # background refresh: 5 s floor, offline skip, failure → notification
│   └── connection/                    # probe exposes identity; ServerInfo cached on connect
└── tests/
    ├── cache.rs                       # new
    └── cache_refresh.rs               # new

src-tauri/src/
├── cache_commands.rs                  # new: cached_server_info, cache_size, clear_cache; view-data-changed
├── player_commands.rs                 # list_* return Cached<T>; open_scene uses cached details
├── state.rs                           # per-profile ViewCache handles; delete with profile
├── harness.rs                         # new (debug): SSV_HARNESS_* switches, process-start timing
└── bin/perf_harness.rs                # new: orchestrates runs, collects MEASURE lines, writes reports

ui/src/
├── views/HomeView.tsx, ScenesView.tsx # Cached<T>, refetch on view-data-changed
├── state/viewData.ts                  # new: view-data-changed subscription helper
├── settings/TroubleshootingPage.tsx   # cache size + Clear cache
└── debug/bench.ts                     # + section navigation, 10k-item scroll, cold-start report

.github/workflows/ci.yml               # + timeout-minutes, concurrency
README.md                              # + CI badge
```

**Structure Decision**: this keeps the existing projects.
- The cache is a new `stash-core` module (it's domain infrastructure, Principle III).
- `src-tauri` gains a command module, a debug harness module, and the harness binary.
- The UI changes stay inside existing views plus one small subscription helper.

## Complexity Tracking

No constitution violations to justify.
