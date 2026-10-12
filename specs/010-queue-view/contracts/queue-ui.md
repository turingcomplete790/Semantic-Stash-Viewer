# Contract: Queue view UI (010)

What the queue view shows, how it responds to keys and the pointer, and what it may ask Stash for.

## Layout

```text
┌─ Scenes tab: controls (search, Filter, Saved filters, sort, Grid | List | Wall | Queue, Play next) ─┐
├───────────────────────┬──────────────────────────────────────────────────────────────────────────┤
│ ▸ row  (selected)     │  player, or cover + Play            (16:9, panel width)                  │
│   row  ♪ (playing)    │──────────────────────────────────────────────────────────────────────────│
│   row                 │  details as on the scene page (scrolls)                                  │
│   …                   │  title · facts · studio · performers · tags · groups · markers · files … │
│ pager                 │                                                                          │
└───────────────────────┴──────────────────────────────────────────────────────────────────────────┘
```

- **List**: a third of the window, at least 300 px; rows show a small still, the title, and the
  details line (date, duration, resolution, studio). The selected row is highlighted; the playing
  row carries a playing mark. Windowed like the other modes.
- **Panel**: the rest of the width, at least 480 px. When both minimums don't fit, the panel shows
  only the player or cover; when the list alone fits, choosing a scene opens its full page.

## Keys

| Key | List focused | Panel focused |
|---|---|---|
| ↑ / ↓, Home / End | move the selection | player keys (volume, seek: as 002/007) |
| `[` / `]` | previous / next page | player speed (as 002) |
| Enter | play the selection | (player) |
| `n` / `p` | play the next / previous scene | same |
| `a` | toggle Play next | same |
| `o` / Ctrl+O | open the selected scene's full page / in a new tab | same |
| F6 | focus the panel | focus the list |
| Space, F, M, Esc, PageUp/PageDown | player keys while something plays | player keys |

The queue's keys are listed under Scenes (queue view) in keyboard help and Settings → Keyboard;
none duplicates a key at the same level (the keymap's uniqueness test).

## Pointer

- Clicking a row selects it and loads its details at once; double-clicking plays it.
- The panel's Play plays the selection; the mode switch's Play next toggles advancing.

## Play next

- On (default): when the playing scene ends, the next card plays and is selected; after the page's
  last card, the next page loads and its first card plays; on the last page's last card, playback
  stops with Replay.
- Off: the scene stops at its end with Replay and a "Play next" button.
- Never after a playback error.

## Request budget (008's contract applies)

| Action | Requests |
|---|---|
| Show a page in Queue | `ScenesPage` ×1 (+1 neighbour prefetch); stills from the thumbnail cache |
| Move the selection over N rows quickly | 0 |
| Rest on a scene (or click it) | `SceneDetails` ×1 and the cover GET ×1; 0 when cached |
| Open files, galleries, a chip card | as 008 (1 each, once) |
| Play next across a page boundary | `ScenesPage` ×1 for the new page (0 if prefetched) |

Harness rows: `queue-select` (≤ 150 ms), `queue-next` (≤ 1500 ms), `gql-queue-sweep` (0),
`gql-queue-rest` (1 `SceneDetails`), `scenes-scroll-1000-queue` (≤ 1% missed frames at 144 Hz).
