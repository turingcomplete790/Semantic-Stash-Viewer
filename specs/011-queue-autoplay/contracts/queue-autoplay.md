# Contract: Queue autoplay and loop (011)

What changes from 010's [queue-ui contract](../../010-queue-view/contracts/queue-ui.md); all else
stays as written there.

## Pointer

| Action | 010 | 011 |
|---|---|---|
| Click a row | selects; details load at once | selects and **plays** (the playing scene: selects only) |
| Double-click a row | plays | plays (same as a click) |
| Middle-click a row | opens the page in a new tab | unchanged |
| The panel's Play | plays the selection | unchanged (shown when the selection isn't playing) |
| Controls | Play next: on/off | Play next: on/off, **Loop: on/off** |

## Keys (queue level)

| Key | 011 |
|---|---|
| ↑ / ↓, Home / End (list focused) | move the selection; the scene it **rests** on (250 ms) plays |
| Enter (list focused) | plays the selection at once (unchanged) |
| `L` | **turn Loop on or off** (new; listed under Scenes (queue view)) |

All other 010 keys unchanged.

## Autoplay

- Choosing Queue: the selection plays (takes over the one player).
- Search, filter, sort, page size, page, saved filter: the new page's first scene plays when it
  arrives.
- Switching to a queue tab: its selection plays if nothing is playing; otherwise nothing changes.
- A restored tab at launch, a refresh, a background tab: never play.

## Loop

- On (default): after the last scene of the last page, page 1's first scene plays (on a one-page
  result, its first scene). Previous never wraps.
- Off: 010's stop with Replay (and the panel's "Play next" button when Play next is off).
- Never after a playback error; consecutive deleted-scene skips stop at the result's size.

## Request budget

As 010: one page request per page shown (0 when cached), one `SceneDetails` per scene played or
rested on (0 from the cache), nothing per row passed over.

## Harness rows

`queue-click-play` (≤ 1500 ms), `queue-autoplay` (≤ 1500 ms), `queue-loop` (≤ 1500 ms); every 010
row still within budget (`queue-select`, `queue-next`, `gql-queue-sweep` 0, `gql-queue-rest` 1,
`scenes-scroll-1000-queue` ≤ 1%).
