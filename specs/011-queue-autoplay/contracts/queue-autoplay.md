# Contract: Queue autoplay and playlist controls (011)

What changes from 010's [queue-ui contract](../../010-queue-view/contracts/queue-ui.md); the rest
stays as written there.

## Layout

```text
┌─ list ──────────────────────────────┬─ panel ──────────────────────────┐
│ 🔀  🔁/🔂  [▶ Autoplay ⏽]            │ player (or cover + Play)         │
│ ▸ row (selected, ▶ playing)          │──────────────────────────────────│
│   row                                │ details                          │
│   …   pager                          │                                  │
└──────────────────────────────────────┴──────────────────────────────────┘
```

The playlist strip sits at the top of the list: Shuffle, Repeat, Autoplay, as icon buttons with
tooltips; the active state in the accent colour, the inactive muted. 010's "Play next: on/off" and
011's first-pass "Loop" buttons leave the top controls bar.

## Controls

| Control | States (icon) | Default | Key |
|---|---|---|---|
| Shuffle | off / on (`shuffle`) | off | `X` |
| Repeat | Off (`repeat`, muted) → All (`repeat`, accent) → One (`repeat-1`, accent) | Off | `R` (cycles) |
| Autoplay | off / on (a switch with a play glyph) | on | `A` |

All three are saved with the tab and listed under Scenes (queue view) in keyboard help and
Settings → Keyboard. `L` (011's first pass) is removed.

## Pointer and keys (unchanged from the first 011 pass)

- Click a row: plays it (the playing row: selects only). Double-click: the same. Middle-click:
  opens the page in a new tab.
- ↑/↓ (list focused): move the selection; the row it rests on (250 ms) plays. Enter plays at once.
- `n` / `p`: Next / Previous by the rules below.

## Behaviour

| Situation | Autoplay on | Autoplay off |
|---|---|---|
| Choosing Queue; results change; switching to the tab while idle | the selection (first) plays | nothing |
| A scene ends, Repeat One | it plays again from 0 | it plays again from 0 |
| A scene ends, not the last | the next (list or shuffle order) plays | stops (Replay; "Next" offered) |
| The last scene ends, Repeat All | page 1's first, or a new shuffled round | stops |
| The last scene ends, Repeat Off | stops (Replay) | stops |
| Next / Previous | move by the same order; Next at the end follows Repeat All/Off; Previous never wraps | same |
| A failure | nothing moves | nothing moves |

With Shuffle on, every scene of the result plays once per round, in random order, from any page;
the list keeps its sort and shows the playing scene's page with it selected; Previous retraces the
round; turning Shuffle off continues in list order.

## Request budget

As 010: one page request per page shown (a shuffled pick or Repeat all's return to page 1 may show
another page: 1 request, 0 when cached), one `SceneDetails` per scene played (0 from the cache),
nothing per row passed over.

## Harness rows

`queue-click-play`, `queue-autoplay`, `queue-repeat`, `queue-shuffle` (≤ 1500 ms each); every 010
row still within budget.
