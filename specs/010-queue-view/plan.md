# Implementation Plan: Queue View

**Branch**: `010-queue-view` | **Date**: 2026-10-11 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/010-queue-view/spec.md`

## Summary

Add a fourth Scenes display mode, **Queue**: the tab's page of scenes as a compact list beside a
scene panel that is the 008 scene page turned on its side (player or cover on top, details below).
The panel is an embedded `SceneState`, so its details, chips, chip cards, sections, and markers are
the scene page's own; the app's one player is drawn in the panel when the queue's scene is playing.
Selection loads details only once it rests (a timer, as chip cards do), and **Play next** advances
through the list and across pages when a scene ends. No new requests, no new storage beyond the
tab's saved state ([research](research.md), [contracts/queue-ui.md](contracts/queue-ui.md)).

## Technical Context

**Language/Version**: Rust 1.99 (workspace).

**Primary Dependencies**: iced 0.14, the 007/008 native app (`native-ui`), `player` (libmpv),
`stash-core`. No new dependencies.

**Storage**: the session file (version 2): `ScenesState` gains `queue_selected` (a scene id) and
`play_next` (default on), both additive with serde defaults, so no version bump. The view cache's
existing `scene:details:<id>` and `cover:<id>` entries serve the panel.

**Testing**: `cargo test` (state-machine transition tests in `native-ui/tests/queue.rs`, an
`iced_test` flow for clicking rows), the query-budget tests, and new perf-harness rows.

**Target Platform**: Linux desktop (as 007, 008).

**Project Type**: desktop app (workspace: `stash-core`, `player`, `native-ui`, `perf-harness`).

**Performance Goals**: spec SC-001–SC-005: selection to the panel's cover, title, and Play
< 150 ms; details < 1 s on LAN (at once from the cache); next scene playing < 1.5 s after the
previous ends, page boundary included; list scrolling within the constitution's budget (lists
≤ 1% missed frames at 144 Hz, constitution 5.1.0).

**Constraints**: 008's request budgets (one page request plus the neighbour prefetch; one
`SceneDetails` per scene rested on; no request per row); one player for the app; keyboard
operable throughout; read-only toward Stash except 008's play history.

**Scale/Scope**: pages of up to 1000 scenes (008's page sizes); one queue panel per Scenes tab;
three user stories, all in `native-ui`.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | Check | Result |
|---|---|---|
| I. Stash is the system of record | The queue is the tab's results; nothing new is stored in Stash or locally beyond the tab's saved view state; play history writes are 008's | Pass |
| II. Semantic model from the design documents | Not touched; tag chips in the panel are 008's (type marking and colour rules) | Pass |
| III. Brain/UI separation | No new core logic: the queue is UI state over the Scenes results; requests stay in `stash-core` | Pass |
| IV. Lean, batched GraphQL; paging, no infinite scroll | Same requests as Scenes and the scene page (contract); details only on rest; Play next crosses pages by loading the next page, as paging does | Pass |
| V. mpv is the player | The panel shows the app's one mpv player; the transcode fallback and fullscreen are 008's | Pass |
| VI. Responsive UI; keyboard | Selection < 150 ms, list scrolling within budget, full keyboard operation with listed keys (contract) | Pass |
| VII. User-owned connection | Nothing new leaves the machine | Pass |
| VIII. Web UI parity | An addition beyond the web UI (as the wall preview is); no capability dropped | Pass |
| IX. One app shell | The queue lives in a Scenes tab, saved and restored with it; the now-playing bar behaves as elsewhere | Pass |
| X. Galleries | A queue-style gallery view stays on the roadmap for when galleries exist | Pass |
| XI. Visual identity | Uses the app's theme; the playing row and the selection are marked distinctly | Pass |
| XII. Semantic engine | Not touched | Pass |

**Operation review (Principle IV)**:

| Operation | When | Requests |
|---|---|---|
| `ScenesPage` | page shown (and its neighbour prefetched) | as 008 |
| `SceneDetails` | selection rests on a scene (or a click) | 1 per scene per visit, 0 from the cache |
| Cover (screenshot GET) | with the details | 1 per scene, 0 from the cache |
| Files, galleries, chip cards | opened in the panel | as 008 |
| Play history | viewing | as 008 |

Post-design re-check: the data model and contracts keep every budget; no violations.

## Project Structure

### Documentation (this feature)

```text
specs/010-queue-view/
├── plan.md              # This file
├── research.md          # Phase 0 decisions (R1–R8)
├── data-model.md        # Queue state, saved fields, transitions
├── quickstart.md        # Validation scenarios V1–V3, harness rows
├── contracts/
│   └── queue-ui.md      # Layout, keys, focus, Play next, request budget
└── checklists/
    └── requirements.md  # Spec quality checklist
```

### Source Code (repository root)

```text
native-ui/src/
├── screens/
│   ├── mod.rs                 # Mode::Queue
│   ├── scene.rs               # details_view reused in the panel; a vertical panel layout
│   └── scenes/
│       ├── queue.rs           # NEW: QueueState (selection, rest timer, embedded SceneState,
│       │                      #      play next, pending play across pages) and the queue view
│       ├── state.rs           # ScenesState: queue state, saved fields, mode switch
│       ├── grid.rs            # compact queue rows (shared with List's row parts)
│       ├── layout.rs          # list and panel widths, minimums, narrow-window rules
│       └── controls.rs        # "Queue" in the mode switch; the Play next toggle
├── session/
│   ├── mod.rs                 # player visibility in the queue panel; Play next on Ended;
│   │                          # queue keys and focus
│   └── playback.rs            # an "ended" signal for the session
├── shell/keymap.rs            # queue keys (Scenes level, queue mode)
├── effects.rs                 # QueueRestDue
└── measure/bench.rs           # queue rows
crates/perf-harness/src/        # queue-select, queue-next budgets; gql-queue-sweep, gql-queue-rest
native-ui/tests/queue.rs        # NEW: transition tests
```

**Structure Decision**: the 007/008 workspace, unchanged. All of it is `native-ui` state and
views over existing core operations; no `stash-core` or `player` changes are expected.

## Complexity Tracking

No constitution violations to justify.
