# Data Model: Queue View (010)

All UI state in `native-ui`; nothing new in `stash-core` or Stash.

## ScenesState (changed)

| Field | Type | Saved | Notes |
|---|---|---|---|
| `mode` | `Mode` (Grid, List, Wall, **Queue**) | yes | Queue is new; a Stash saved filter never sets it |
| `queue_selected` | `Option<String>` (scene id) | yes | serde default `None`; the selected row after a restore, or the first card if it isn't on the page |
| `play_next` | `bool` | yes | serde default `true` |
| `queue` | `QueueState` | no | rebuilt on entry and on restore |

## QueueState (new, transient)

| Field | Type | Notes |
|---|---|---|
| `selected` | `Option<usize>` | index into the page's cards |
| `rest` | `u64` | generation of the pending rest timer (250 ms); a timer for an older generation does nothing |
| `panel` | `Option<Box<SceneState>>` | the 008 scene state for the selected scene, created when the selection rests (or on a click) |
| `focus` | `List` \| `Panel` | which area has the keyboard (F6 switches) |
| `pending_play` | `None` \| `First` \| `Last` | a page change asked by Play next, Next, or Previous: play that card when the page arrives |

## Playback (changed)

| Field / signal | Notes |
|---|---|
| ended | the playing scene's snapshot turned to Ended (not Error): the session decides whether to advance |
| `owner` | as 007: the tab whose scene plays; for the queue, the Scenes tab |

## Validation rules

- `selected` is always within the current page's cards, or `None` on an empty page.
- `panel`'s scene id equals the selected card's id; a selection change drops a panel for another
  scene (its in-flight results are ignored by generation, as 008).
- `pending_play` is cleared when its page arrives (played or not) and by any other page or filter
  change the user makes.
- Play next never advances after an Error snapshot.

## State transitions

```text
Selection (keyboard):  select(i) → panel shows card title/still → rest 250 ms → panel = SceneState(i).enter()
Selection (pointer):   click(i) → panel = SceneState(i).enter() at once
Play:                  Enter / Play → playback.open(card i, owner = this tab) → player in the panel
Ended:                 playing card i ends ∧ play_next → i+1 on page ? play(i+1), select(i+1)
                                                      : next page ? Page(n+1), pending_play = First
                                                      : stop (Replay)
Next / Previous:       as Ended for i+1 / i−1 (Previous across pages: pending_play = Last)
Page arrives:          pending_play = First → play(0) | Last → play(last) | None → select(saved or 0)
Results change:        search, filter, sort, page size, page → select(0), pending_play = None
```
