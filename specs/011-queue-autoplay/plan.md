# Implementation Plan: Queue Autoplay and Playlist Controls

**Branch**: `011-queue-autoplay` | **Date**: 2026-10-11 (revised after the playlist clarification) |
**Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/011-queue-autoplay/spec.md`

## Summary

Turn 010's queue view into a player with the familiar playlist controls. A click (or the
keyboard's rest, or Enter) plays a scene; **Autoplay** (YouTube-style, replacing 010's Play next)
starts the queue on its own and moves on when a scene ends; one **Repeat** button cycles Off →
All 🔁 → One 🔂; **Shuffle** 🔀 plays the whole result in random order, each scene once per round.
At the centre is a **pure state machine** in `stash-core` (`playback::playlist`): settings,
position, and an event go in; the next state and one command (play scene *n* of the result,
replay, stop, nothing) come out. The UI only translates commands into what 010 already does
(select a card, change page, open the player). No new requests beyond a page per shuffled pick
on another page; no player changes ([research](research.md),
[contracts/queue-autoplay.md](contracts/queue-autoplay.md)).

## Technical Context

**Language/Version**: Rust 1.99 (workspace).

**Primary Dependencies**: iced 0.14, the 010 queue view (`native-ui`), `player` (libmpv),
`stash-core`. No new crates: shuffle order is a seeded bijection written in the core (R3), seeds
come from the existing `uuid` crate's random bytes (as the random sort's seed does today).

**Storage**: the session file (version 2). `ScenesState` gains `autoplay` (default on, reading
010's `play_next` through a serde alias), `repeat` (`Off`/`All`/`One`, default `Off`), and
`shuffle` (default off), all additive; 011's unshipped `loop_queue` is removed. The shuffle round
isn't saved (spec Assumptions).

**Testing**: `cargo test`: exhaustive pure-function tests of `playback::playlist` in `stash-core`
(every Autoplay × Repeat × Shuffle combination and event, no window or server, Principle III);
transition tests in `native-ui/tests/queue.rs` (updated for the new controls); perf-harness rows.

**Target Platform**: Linux desktop (as 007–010).

**Project Type**: desktop app (workspace: `stash-core`, `player`, `native-ui`, `perf-harness`).

**Performance Goals**: SC-001–SC-003: a click, Autoplay starting, the next scene (including page 1
or a shuffled pick on another page), and Repeat one's restart each reach a playing scene in under
1.5 s; the state machine's step is O(1) time and memory per event (a shuffled pick never scans the
library); SC-005 keeps every 010 budget.

**Constraints**: one player; background or restored tabs never start playback on their own; a
failure never moves on, repeats, or reshuffles; read-only toward Stash except 008's play history
(one viewing per scene start, Repeat one included).

**Scale/Scope**: results of any size (shuffle by index over the result's count, pages loaded on
demand); one queue per Scenes tab.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | Check | Result |
|---|---|---|
| I. Stash is the system of record | Settings are tab view state; nothing new in Stash; play history as 008 | Pass |
| II. Semantic model from the design documents | Not touched | Pass |
| III. Brain/UI separation | The playlist rules live in `stash-core` as pure functions with no UI dependency, unit-tested headlessly; the UI only maps commands to effects | Pass |
| IV. Lean, batched GraphQL; paging, no infinite scroll | Shuffle and Repeat all move by index and load the target's page (one `ScenesPage`, 0 if cached); no request per row; no whole-result fetch | Pass |
| V. mpv is the player | The one player; Repeat one reopens the scene from 0 through the normal open (a new viewing) | Pass |
| VI. Responsive UI; keyboard | Every path < 1.5 s; each control has a key (`A`, `R`, `X`) and a tooltip | Pass |
| VII. User-owned connection | Nothing new leaves the machine | Pass |
| VIII. Web UI parity | Beyond the web UI; nothing dropped | Pass |
| IX. One app shell | Tab switches never take playback from another tab | Pass |
| X. Galleries | Out of scope | Pass |
| XI. Visual identity | Lucide shuffle, repeat, repeat-1 icons (the app's set, credited in About), active state in the theme's accent | Pass |
| XII. Semantic engine | Not touched | Pass |

**Operation review (Principle IV)**:

| Operation | When | Requests |
|---|---|---|
| `ScenesPage` | the playing scene's page (shuffle pick, Repeat all to page 1), and its neighbour prefetch | 1 per page shown, 0 when cached |
| `SceneDetails` | the scene that plays | 1 per scene per visit, 0 from the cache |
| Play history | each scene start, including each Repeat one restart | as 008 |

Post-design re-check: the data model and contract keep every budget; no violations.

## Project Structure

### Documentation (this feature)

```text
specs/011-queue-autoplay/
├── plan.md              # This file
├── research.md          # Decisions R1–R9
├── data-model.md        # Settings, the playlist state machine, events and commands
├── quickstart.md        # Validation V1–V4, harness rows
├── contracts/
│   └── queue-autoplay.md  # Controls, keys, behaviour table; changes from 010's contract
└── checklists/
    └── requirements.md
```

### Source Code (repository root)

```text
crates/stash-core/src/playback/
├── mod.rs                 # pub mod playlist
└── playlist.rs            # NEW: Settings, Repeat, Playlist, Event, Command, step(); the
                           #      shuffle order (a seeded bijection over 0..count)
crates/stash-core/tests/playlist.rs   # NEW: exhaustive transition tests
native-ui/src/
├── screens/scenes/
│   ├── state.rs           # saved autoplay/repeat/shuffle; the playlist state; commands →
│   │                      # select/page/play; PendingPlay::Index replaces First/Last
│   ├── queue.rs           # the playlist strip (shuffle, repeat, autoplay) above the list
│   └── controls.rs        # 010's Play next / 011's Loop buttons removed
├── session/mod.rs         # events in (Ended, Failed, Skipped, Next, Previous); commands out
├── shell/keymap.rs        # A Autoplay, R Repeat, X Shuffle (queue level); L removed
├── widgets/mod.rs         # Icon::Shuffle, Repeat, RepeatOne, Autoplay (Lucide)
└── measure/bench.rs       # queue-click-play, queue-autoplay, queue-repeat, queue-shuffle rows
crates/perf-harness/src/   # their budgets
native-ui/tests/queue.rs   # updated transition tests
```

**Structure Decision**: the 010 workspace. The rules move to `stash-core` (Principle III: core
logic, headlessly testable); everything visual stays in `native-ui`. The uncommitted T001–T021
work is the starting point: click and rest play, the tab-switch autoplay, and the arrival intents
stay; the Loop switch and `L` become Repeat; `advance` and the session's skip counter are
replaced by the state machine.

## Complexity Tracking

No constitution violations to justify.
