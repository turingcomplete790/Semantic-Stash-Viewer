# Implementation Plan: Queue Autoplay and Loop

**Branch**: `011-queue-autoplay` | **Date**: 2026-10-11 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/011-queue-autoplay/spec.md`

## Summary

Turn 010's queue view into a player first. A click on a row plays it (not only selects it); the
keyboard's selection plays once it rests (the 250 ms rest 010 already has); choosing Queue, a
results change, and switching to a queue tab while nothing plays start playback on their own; and
with **Loop** on (default), Play next runs past the last scene of the last page back to page 1's
first. All of it reuses 010's machinery: `advance`, the pending play on page arrival, the rest
timer, and the one player. No new requests, no core or player changes
([research](research.md), [contracts/queue-autoplay.md](contracts/queue-autoplay.md)).

## Technical Context

**Language/Version**: Rust 1.99 (workspace).

**Primary Dependencies**: iced 0.14, the 010 queue view in `native-ui`, `player` (libmpv),
`stash-core`. No new dependencies.

**Storage**: the session file (version 2): `ScenesState` gains `loop_queue` (default on), additive
with a serde default, so no version bump.

**Testing**: `cargo test` (transition tests in `native-ui/tests/queue.rs`, updated where 010's
click-to-select becomes click-to-play), the query-budget tests, and new perf-harness rows.

**Target Platform**: Linux desktop (as 007–010).

**Project Type**: desktop app (workspace: `stash-core`, `player`, `native-ui`, `perf-harness`).

**Performance Goals**: spec SC-001–SC-003: a click, choosing Queue, and the loop back to page 1
each reach a playing scene in under 1.5 s (the playback open budget); SC-005 keeps every 010
budget (selection 150 ms, `queue-next` 1.5 s, sweep 0 requests, list scrolling ≤ 1%).

**Constraints**: one player for the app; a background or restored tab never starts playback;
nothing passed over by the keyboard plays or loads; a failure never advances or loops; read-only
toward Stash except 008's play history.

**Scale/Scope**: pages up to 1000 scenes; loops over results of any size (by pages); one story's
worth of state per Scenes tab; all in `native-ui`.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | Check | Result |
|---|---|---|
| I. Stash is the system of record | Nothing new stored in Stash; the loop setting is tab view state; play history is 008's, one viewing per scene start | Pass |
| II. Semantic model from the design documents | Not touched | Pass |
| III. Brain/UI separation | UI state over the Scenes results; requests stay in `stash-core` | Pass |
| IV. Lean, batched GraphQL; paging, no infinite scroll | Same requests as 010 (details on rest, one page per page shown); the loop returns to page 1 by paging; no request per row passed | Pass |
| V. mpv is the player | The one mpv player; autoplay is a normal open (resume, transcode fallback) | Pass |
| VI. Responsive UI; keyboard | Click/autoplay/loop to playing < 1.5 s; the keyboard plays on rest; Loop has a key | Pass |
| VII. User-owned connection | Nothing new leaves the machine | Pass |
| VIII. Web UI parity | Beyond the web UI (as 010); nothing dropped | Pass |
| IX. One app shell | Background tabs never take the player; the now-playing bar as before | Pass |
| X. Galleries | Out of scope | Pass |
| XI. Visual identity | The Loop switch beside Play next, in the theme | Pass |
| XII. Semantic engine | Not touched | Pass |

**Operation review (Principle IV)**:

| Operation | When | Requests |
|---|---|---|
| `ScenesPage` | page shown, the loop back to page 1 (0 if cached) | as 008/010 |
| `SceneDetails` | the scene that plays (click, rest, autoplay) | 1 per scene per visit, 0 from the cache |
| Cover | with the details | 1 per scene, 0 from the cache |
| Play history | each scene started | as 008 |

Post-design re-check: the data model and contract keep every budget; no violations.

## Project Structure

### Documentation (this feature)

```text
specs/011-queue-autoplay/
├── plan.md              # This file
├── research.md          # Decisions R1–R7
├── data-model.md        # Loop setting, rest intent, pending start, skip guard
├── quickstart.md        # Validation V1–V3, harness rows
├── contracts/
│   └── queue-autoplay.md  # What changes from 010's queue-ui contract
└── checklists/
    └── requirements.md
```

### Source Code (repository root)

```text
native-ui/src/
├── screens/scenes/
│   ├── queue.rs           # rest intent (plays or only loads), PendingPlay::Start
│   ├── state.rs           # click-to-play, autoplay on Queue and results change, loop in
│   │                      # advance, loop_queue (saved), ToggleLoop
│   └── controls.rs        # the Loop switch beside Play next
├── session/mod.rs         # autoplay on switching to a queue tab while idle; rest → play;
│                          # deleted-scene skip guard; the Loop key
├── shell/mod.rs           # queue_rest_due returns the scene to play
├── shell/keymap.rs        # L: Loop on/off (queue level)
└── measure/bench.rs       # queue-click-play, queue-autoplay, queue-loop rows
crates/perf-harness/src/   # their budgets (1500 ms each)
native-ui/tests/queue.rs   # updated and new transition tests
```

**Structure Decision**: the 010 workspace, unchanged; all changes in `native-ui` (and the
harness's budget table).

## Complexity Tracking

No constitution violations to justify.
