# Research: Queue Autoplay and Playlist Controls (011)

Decisions for the plan, revised after the 2026-10-11 clarification (playlist controls replace a
single Loop switch). Each builds on 010's queue view.

## R1. The rules are one pure state machine, in the core

- **Decision**: `stash_core::playback::playlist` holds the queue's playback rules as data and pure
  functions: `step(&Playlist, Event) -> (Playlist, Command)`. `Playlist` holds the settings
  (Autoplay, Repeat, Shuffle), the result's size, the playing position (an index into the whole
  result), the shuffle round, and the deleted-scene skip count. Events: `Ended`, `Failed`,
  `Skipped`, `Next`, `Previous`, `Chosen(i)`, `Autostart(i)`, `ResultsChanged(count)`,
  `SetAutoplay`, `CycleRepeat`, `ToggleShuffle`. Commands: `Play(i)`, `Replay`, `Stop`, `Nothing`.
  No clock, randomness, I/O, or UI inside: a shuffle seed comes in with the event that starts a
  round. Every transition is a `match` on `(event, settings)` that returns a new value.
- **Rationale**: the owner's design constraint (spec Assumptions); Principle III (core logic in a
  UI-free crate, testable headlessly); Autoplay × Repeat × Shuffle × event is a small finite space
  that exhaustive tests can cover, which ad-hoc branches in the UI (010's `advance`, the session's
  skip counter) can't.
- **Alternatives**: keeping the rules in `ScenesState` (rejected: mixed with page loading and
  widgets, untestable without the UI); a trait-object state pattern (rejected: more ceremony than
  an enum and a `match`, and not value-based).

## R2. Positions are indices into the whole result

- **Decision**: the machine speaks in result indices (`0..count`); the UI maps index `i` to page
  `i / page_size + 1`, card `i % page_size`. A `Play(i)` on the current page plays that card; on
  another page it changes page with a pending play of index `i` (generalising 010's pending
  First/Last), and the arriving page plays it. Page size or results changes send
  `ResultsChanged`.
- **Rationale**: Shuffle and Repeat all cross pages freely; one mapping serves Next, Previous,
  shuffle picks, and the return to page 1.
- **Alternatives**: page-relative positions (rejected: every cross-page rule becomes a special
  case, as 010's First/Last were).

## R3. Shuffle: a seeded bijection, each scene once per round

- **Decision**: a round is a permutation of `0..count` given by a seeded bijection (a small
  Feistel network over the next power of two, with cycle-walking to stay below `count`), so the
  k-th scene of a round is computed in O(1) time and memory, for any result size, without listing
  the library. The round's order puts the playing scene first: turning Shuffle on (or choosing a
  scene while shuffling) starts a round at that scene, so it continues and the rest follow at
  random. The round position moves forward on `Ended`/`Next` and back on `Previous`. At the
  round's end, Repeat all starts a new round (a new seed); Repeat off stops. New results start a
  new round.
- **Rationale**: FR-007 / SC-004 ("no repeats until every scene has played") with paging intact
  (Principle IV) and no stored list of played ids.
- **Alternatives**: storing a shuffled id list (rejected: needs every id up front, a whole-result
  fetch); random picks with a played set (rejected: O(played) memory and retries near the end);
  Stash's random sort (rejected in clarification: it reorders the list).

## R4. Repeat: one control, Off → All → One

- **Decision**: `Repeat` is an enum cycled by the button and `R`. **One**: `Ended` → `Replay`,
  which the UI performs with the player's own replay (a seek to 0 and unpause) as a new viewing
  for play history (never the resume point), regardless of Autoplay; reopening the file it holds
  at its end with `keep-open` left mpv unable to report the next end (found by the harness); `Next`/`Previous` move as with Repeat off/all
  and Repeat one stays on. **All**: past the end (list order: the last index; shuffle: the round's
  end) → `Play(0)` or a new round's first. **Off**: past the end → `Stop`.
- **Rationale**: Clarification Q (Spotify/VLC/Apple Music behaviour); FR-006, FR-008.
- **Alternatives**: mpv's own loop-file for Repeat one (rejected: no new viewing per loop, and the
  machine would no longer decide).

## R5. Autoplay: YouTube's meaning, replacing Play next

- **Decision**: `autoplay` (saved, default on; reads 010's `play_next` via a serde alias) gates
  `Ended` (moving on) and `Autostart` (choosing Queue, results arriving after a change, switching
  to an idle queue tab, a pending start on arrival). Off: `Ended` → `Stop` unless Repeat one, and
  `Autostart` → `Nothing`. `Chosen` (click, the keyboard's rest, Enter), `Next`, and `Previous`
  always act. The panel's "Play next" button (010, shown at an end with Play next off) becomes
  "Next".
- **Rationale**: Clarification Q (A, like YouTube); FR-004.
- **Alternatives**: separate switches for starting and moving on (rejected in clarification).

## R6. Choosing, starting, and tab switches (from the first 011 plan, kept)

- **Decision**: a click plays (the playing row only selects); the keyboard's rest plays (rests
  started by a page arriving only load); Enter plays; switching to a queue tab starts its
  selection only while the player is idle (and then only with Autoplay on), with a pending start
  if its page isn't loaded; launch, refreshes, and background tabs never play. Each of these is a
  `Chosen` or `Autostart` event into the machine.
- **Rationale**: FR-001–FR-005; already implemented and tested in the first 011 pass.
- **Alternatives**: as recorded in the first plan.

## R7. Failures and deleted scenes

- **Decision**: `Failed` → `Stop` in every state (no move, no repeat, no reshuffle; Repeat one
  doesn't retry). `Skipped` (a deleted scene) moves on like `Ended` but counts; the machine stops
  when the count reaches the result's size, and resets it on the next scene that plays.
- **Rationale**: FR-010; the skip guard moves from the session into the machine, where it's
  tested with the rest.
- **Alternatives**: none worth recording.

## R8. Controls and symbols

- **Decision**: a playlist strip at the top of the queue's list: Shuffle 🔀 (Lucide `shuffle`),
  Repeat (Lucide `repeat`, or `repeat-1` for One), and Autoplay (a switch with a play glyph),
  each an icon button with a tooltip ("Shuffle: on", "Repeat: all", "Autoplay: off"), the active
  state in the theme's accent colour and the inactive in the muted colour. Keys at the queue level:
  `A` Autoplay (010's `a`), `R` cycles Repeat, `X` Shuffle (its crossing arrows; `S` is Saved
  filters at the Scenes level, which the queue level would shadow). 010's Play next and 011's Loop
  buttons in the top controls bar, and the `L` key, are removed.
- **Rationale**: FR-009; the icons match the app's Lucide set, already credited (ISC) in About and
  `widgets/mod.rs`.
- **Alternatives**: emoji glyphs (rejected: font-dependent and not tinted by the theme); controls
  in the top bar (rejected: far from the list they act on).

## R9. Measuring it

- **Decision**: harness rows `queue-click-play` (click → playing), `queue-autoplay` (choosing Queue
  → playing), `queue-repeat` (Repeat all: the last page's last scene ending → page 1's first
  playing; Repeat one: a scene ending → it playing again), `queue-shuffle` (a scene ending with
  Shuffle on → the pick playing, often on another page), each ≤ 1500 ms; play history off while
  the bench seeks to ends (no writes to Testing). The state machine's tests also assert O(1)
  shuffle steps by computing positions in a 1,000,000-scene result.
- **Rationale**: SC-001–SC-004.
- **Alternatives**: none worth recording.
