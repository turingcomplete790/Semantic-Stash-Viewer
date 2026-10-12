# Data Model: Queue Autoplay and Loop (011)

Changes to 010's model (`specs/010-queue-view/data-model.md`); all UI state in `native-ui`.

## ScenesState (changed)

| Field | Type | Saved | Notes |
|---|---|---|---|
| `loop_queue` | `bool` | yes | serde default `true`; whether Play next runs past the last page's last scene to page 1 |

## QueueState (changed)

| Field | Type | Notes |
|---|---|---|
| `rest_plays` | `bool` | the pending rest's intent: `true` when a keyboard move started it (it plays when due), `false` when a page arrival started it (it only loads) |
| `pending_play` | `None` \| `First` \| `Last` \| **`Start`** | `Start` (new): play the selection (`queue_selected` or the first card) when the page arrives; set when the user switches to a queue tab whose page isn't loaded yet |

## Session (changed, transient)

| Field | Type | Notes |
|---|---|---|
| `queue_skips` | `u32` | deleted scenes skipped in a row by Play next; reset when a scene starts playing; at the result's size, the queue stops |

## Validation rules

- A row click on the playing scene never reopens it (FR-002).
- A rest whose scene is the playing one plays nothing.
- A page arrival with `pending_play = None` never plays (restores, refreshes).
- Loop never wraps backwards; never runs after an Error snapshot.
- Autoplay on a tab switch only when the player is idle.

## State transitions

```text
Click(i):           select(i, pointer) → playing ≠ card(i) ? play(card i) : —
Key move(i):        select(i, keyboard) → playing ≠ card(i) ? rest(plays) : —
Rest due (plays):   panel.enter → play(card i)
Choose Queue:       select(start, pointer) → play(card start)
Results change:     clear → pending_play = First → page arrives → play(card 0)
Switch to tab:      player idle ∧ Queue ? (ready ? play(selection) : pending_play = Start) : —
Ended ∧ Play next:  next card | next page (First) | loop_queue ? (pages > 1 ? page 1 (First) : play(card 0)) : stop
Deleted scene:      skips < count ? advance : stop
```
