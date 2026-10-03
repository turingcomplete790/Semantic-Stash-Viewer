# Data Model: Native UI Spike (iced)

The spike adds no stored data of its own beyond the native build's tab file. These are the in-app
shapes the plan relies on; core types (`SceneQuery`, `ScenePage`, `SceneCard`, `PlayerSnapshot`,
profiles, cache entries) are reused unchanged from `stash-core` and `player`.

## App state

| Field | Meaning |
|---|---|
| `services` | The glue to the core (research R5): profiles, connection manager, cache registry, thumbnail service, the player |
| `connection` | The latest connection snapshot (as the web build's indicator) |
| `tabs` | Ordered list of `Tab`; exactly one is active |
| `thumbs` | LRU of image handles keyed by `(scene id, screenshot version)`, at most 2,000; plus the placeholder handle |
| `video` | The `VideoSurface` when a scene has been opened, else none |
| `bench` | The measurement driver's state when `SSV_DEBUG_BENCH` or `SSV_MEASURE` is set |

All changes go through the iced update function as messages; core calls run as tasks and come back
as messages carrying typed results (no JSON, no bindings).

## Tab

| Field | Meaning | Rules |
|---|---|---|
| `id` | Stable id | Unique per tab set |
| `history` | Entries, newest last, with a cursor | Back moves the cursor; opening a view from Scenes pushes; returning to an equal earlier entry reuses it (005 R13) |
| `title` | Shown in the tab strip | From the current entry |

### Entry (one of)

- **Scenes**: `query` (search, sort, direction, seed), `page` (≥ 1), `page_size` (20, 40, 50, 60,
  120, 250, 500, 1000; default 50), `mode` (grid or list), `scroll` (offset in logical px),
  `focus` (card index, optional).
- **Scene**: `scene_id`, `title`.

**Persistence**: the tab set (tabs, histories, cursors, view state) is saved per profile to
`<data>/shell/tabs-native.json` 500 ms after a change and on quit; restored on launch. A page past
the end restores as the last page (005 R1).

## Scenes page (per Scenes entry, not stored)

| Field | Meaning |
|---|---|
| `state` | loading, ready, unreachable (as 005) |
| `requested` / `shown` | The page asked for and the page shown (past the end → last) |
| `count` | Total matching scenes |
| `cards` | The page's `SceneCard`s |

Transitions follow 005's `createScenePage`: a new query, page, or size loads; a refresh of the
shown page re-reads it in place; connecting retries a page that isn't ready; while connecting a
failure stays "loading".

## VideoSurface (playback, research R1)

| Field | Meaning | Rules |
|---|---|---|
| `slots` | 4 frame slots, each a DMA-BUF-backed texture shared by the side GL context and wgpu | All the same size and format; reallocated together on a settled resize |
| `slot state` | free, rendering, ready, on-screen, retiring | Only one slot is ready at a time (newer replaces older); an on-screen or retiring slot isn't rendered into (retiring = shown just before, possibly still sampled) |
| `size` | Physical pixels of the video area | Changes apply after 100 ms without further change |
| `render thread` | Owns the side EGL context and the mpv render context | Stopped first on close or quit; the mpv render context is freed before the mpv handle |

## Measurement report

As [contracts/measurements.md](contracts/measurements.md): named results with samples, median,
p95, max, and pass/fail against a budget; plus memory and CPU info rows. One report per app per
run.

## Decision record

As [contracts/decision-record.md](contracts/decision-record.md).
