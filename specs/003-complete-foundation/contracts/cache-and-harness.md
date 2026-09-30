# Contract: Cache Commands & Events, Harness Switches

**Feature**: [../spec.md](../spec.md) | **Types**: [../data-model.md](../data-model.md)

Adds to the 001, 002, and 004 contracts. The same rules apply: typed bindings, and the UI never
reads files or talks to Stash directly.

## Invariants

1. **Read-only against Stash.** The cache only avoids repeat reads (FR-011). No writes.
2. **Never across servers.** Each profile has its own cache, checked against the server identity
   on every connect (FR-007).
3. **Quiet.** No cache wording, sizes, or controls outside Settings → Troubleshooting (FR-005,
   constitution Principle IX).
4. **Discardable.** Any cache failure falls back to reading from the server (FR-004, FR-010).

## DTOs

```ts
type Cached<T> = { data: T; fromCache: boolean; fetchedAt: string };
```

## Commands (changed and new)

| Command | Input | Output | Notes |
|---|---|---|---|
| `list_recent_scenes` | none | `Cached<SceneListItem[]>` | **Changed** return type. Hit → returns at once, and refreshes in the background if older than 5 s |
| `list_test_scenes` | `shuffle: boolean` | `Cached<SceneGroup[]>` | **Changed.** Cached copy unless `shuffle` is true (no automatic refresh; the set is random) |
| `cached_server_info` | none | `{ server: ServerInfo; fetchedAt: string } \| null` | **New.** (A named shape: the bindings generator mangles `Option<Cached<T>>`.) Last probe result for the active (or last-used) profile, for Home while connecting or offline |
| `player_open` | `sceneId` | `PlayerSnapshot` | **Unchanged signature.** Uses cached scene details when present, and refreshes them in the background |
| `cache_size` | none | `number` (bytes) | **New.** For Settings → Troubleshooting |
| `clear_cache` | none | `number` (bytes freed) | **New.** Emits `view-data-changed` for every key so open views reload |

## Events

| Event | Payload | When |
|---|---|---|
| `view-data-changed` | `{ profileId: string; key: string }` | A background refresh returned different data, or the cache was cleared. The UI re-invokes the matching command |

## Core API (for Phase 2 writes; research R3)

`ViewCache::invalidate(keys)`, `invalidate_prefix(prefix)`, and `replace(key, value)`. A write
that succeeds invalidates or replaces exactly the keys it affects. A failed write changes nothing
(spec edge case).

## Harness switches (debug builds only, research R8)

| Variable | Effect |
|---|---|
| `SSV_HARNESS_PROFILE=<display name>` | Connect to this profile for the run |
| `SSV_HARNESS_CLEAR_CACHE=1` | Clear that profile's cache before connecting (cold start, cleared) |
| `SSV_HARNESS_EXIT=1` | Exit once the run's measurements finish |
| `SSV_DEBUG_BENCH=1` | The UI bench (004), extended with section navigation, scrolling, and cold start |
| `SSV_MEASURE=…`, `SSV_MEASURE_LONG=…` | Playback measurements (002). `auto` picks from the recently added list: the first three scenes, and the first 1080p one for the long run (stable between runs, unlike the random test set) |

The app also exposes `debug_mark_interactive(visible: boolean)` (debug builds; a no-op in release),
which the UI calls at the first frame with server info. It prints `MEASURE {"coldStartMs": …}`.

The harness itself is `cargo run --bin perf-harness -- --profile "<name>" [--quick]`.
