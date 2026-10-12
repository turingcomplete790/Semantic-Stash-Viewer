# Data Model: Queue Autoplay and Playlist Controls (011)

Changes to 010's model (`specs/010-queue-view/data-model.md`).

## ScenesState (changed, `native-ui`)

| Field | Type | Saved | Notes |
|---|---|---|---|
| `autoplay` | `bool` | yes | default `true`; serde alias `play_next` (010's field), so a 010 session keeps its setting |
| `repeat` | `Repeat` (`Off` \| `All` \| `One`) | yes | default `Off` |
| `shuffle` | `bool` | yes | default `false` |
| `playlist` | `Playlist` | no | the machine's state for this tab (below); rebuilt on entry, a restored tab starts a new round |
| ~~`play_next`~~ | | | replaced by `autoplay` |
| ~~`loop_queue`~~ | | | 011's first pass, never shipped: removed |

## QueueState (changed, `native-ui`)

| Field | Type | Notes |
|---|---|---|
| `rest_plays` | `bool` | as the first 011 pass: a keyboard rest plays, an arrival rest only loads |
| `pending_play` | `None` \| `Index(u32)` \| `Start` \| `Keyboard` | `Index(i)` replaces 010's `First`/`Last`: play result index `i` when its page arrives; `Start` and `Keyboard` as the first 011 pass |

## Playlist (new, `stash_core::playback::playlist`, pure)

| Field | Type | Notes |
|---|---|---|
| `settings` | `Settings { autoplay: bool, repeat: Repeat, shuffle: bool }` | copied from the tab's saved fields |
| `count` | `u32` | scenes in the result (0: empty) |
| `playing` | `Option<u32>` | result index of the playing (or last played) scene: where Next and Previous move from |
| `live` | `bool` | whether that scene is in the player now; choosing it again only plays when it isn't (after the player closed) |
| `round` | `Option<Round>` | present while Shuffle is on |
| `skips` | `u32` | deleted scenes skipped in a row; reset when a scene plays |

**Round**: `{ seed: u64, first: u32, pos: u32 }`: the order is `first`, then the seeded
permutation of `0..count` without `first`; `pos` is the playing scene's place in it.
`order(round, k)` and its inverse are O(1).

**Event**: `Ended`, `Failed`, `Skipped`, `Next`, `Previous`, `Chosen(u32)`, `Autostart(u32)`,
`ResultsChanged { count, seed }`, `SetAutoplay(bool)`, `CycleRepeat`, `ToggleShuffle { seed }`,
`Started(u32)` (a scene really began playing: resets `skips`), `Idle(Option<u32>)` (nothing plays
in this queue, because the player closed or another tab took it; `Some(i)`: continue from the
selection `i`, as 010's Next and Previous did).

**Command**: `Play(u32)`, `Replay`, `Stop`, `Nothing`.

## Validation rules

- `playing < count` whenever set; `count == 0` → every event returns `Nothing` (or `Stop`).
- A round's order visits each index of `0..count` exactly once.
- `Failed` → `Stop` in every state.
- `Autostart` with `autoplay == false` → `Nothing`.
- `Ended` with `autoplay == false` → `Replay` if `repeat == One`, else `Stop`.
- Repeat one never holds `Next`/`Previous` on the same scene.
- `Previous` never wraps in list order; in shuffle it stops at the round's first.
- `Skipped` stops once `skips` reaches `count`.
- `Started(i)` or `Idle(Some(i))` with Shuffle on: `i` at the round's position keeps the round;
  any other `i` (a scene played from elsewhere) starts a round from `i`, so no scene of the round
  is skipped.

## Transition table (`step`)

| Event | Shuffle off | Shuffle on |
|---|---|---|
| `Ended`, Repeat One | `Replay` | `Replay` |
| `Ended`, autoplay off | `Stop` | `Stop` |
| `Ended`/`Next`/`Skipped`, not at end | `Play(playing+1)` | `Play(order(pos+1))` |
| … at end, Repeat All | `Play(0)` | new round (seed from the last `ToggleShuffle`/`ResultsChanged`, advanced deterministically) → `Play(order(0))` |
| … at end, Repeat Off | `Stop` | `Stop` |
| `Previous` | `Play(playing-1)`, or `Nothing` at 0 | `Play(order(pos-1))`, or `Nothing` at the round's first |
| `Chosen(i)` | `Play(i)` (`Nothing` if `i == playing` and `live`) | new round from `i` → `Play(i)` (`Nothing` if live) |
| `Autostart(i)` | autoplay ? `Play(i)` : `Nothing` | same, round from `i` |
| `Idle(at)` | `live = false`; `playing = at` if given → `Nothing` | same; a round from `at` unless it's the round's scene |
| `ResultsChanged` | `playing = None`, round cleared (a new one if Shuffle) → `Nothing` (the UI follows with `Autostart(0)`) | |
| `ToggleShuffle` | on: round from `playing` → `Nothing`; off: round cleared → `Nothing` (list order continues from `playing`) | |
| `CycleRepeat` | Off → All → One → Off → `Nothing` | |
| `Failed` | `Stop` | `Stop` |
