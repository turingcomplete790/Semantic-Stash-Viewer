# Contract: Scene Browsing (P1, paged)

**Feature**: [../spec.md](../spec.md) | **Types**: [../data-model.md](../data-model.md)

Adds to the 001–004 contracts. The same rules apply: typed bindings, the UI never talks to Stash
directly, and nothing here writes to Stash.

## Invariants

1. **Paged** (constitution IV): one page of scenes per view, default 50, sizes 20–1000.
2. **One request per page**: one card-sized GraphQL query per page shown; never per card.
3. **No keys in URLs**: thumbnails are fetched by the core with the API key in a header.
4. **Cached, per server**: pages and thumbnails live in 003's cache.
5. **Read-only**.

## Commands

| Command | Input | Output | Notes |
|---|---|---|---|
| `scenes_page` | `query: SceneQuery`, `page: number` (1-based), `pageSize: number` | `Cached<ScenePage>` | Cached copy at once when present; refreshed quietly after 5 s. A page past the end returns the last page (its `page` says which). `pageSize` must be one of 20, 40, 50, 60, 120, 250, 500, 1000. Errors: `NotConnected` when offline with nothing cached |
| `scene_sorts` | none | `{ value: SceneSort; label: string }[]` | *(kept)* |

## URI scheme *(kept)*

`ssv-thumb://localhost/<kind>/<id>?v=<version>` → `image/jpeg` (480 px for scenes); placeholder
when there's nothing to show; `404` for unknown kinds. The CSP's `img-src` includes
`ssv-thumb:`.

## Events

No new events. `view-data-changed` fires with a page's key (`scenes:q:<hash>:s:<size>:p:<n>`)
when a refresh changed it; the view re-reads that page in place.

## Keyboard (listed under "Scene lists"; handled by the grid while it has focus)

| Keys | Action |
|---|---|
| ← → | previous / next card; past the page's first / last card → previous / next page |
| ↑ ↓ | same column, previous / next row (past the edge → previous / next page) |
| `[` / `]` | previous / next page |
| Home / End | first / last card on the page |
| Enter / Ctrl+Enter | open the scene / in a new tab |

## Shell behaviour (research R13)

- **Leaving a view** (`onLeave(fallback)`): if the previous history entry's route equals the
  fallback, go back to it; otherwise navigate to the fallback.
- **Navigating to a route in the same tab**: a new history entry starts with a copy of the view
  state of the most recent entry in that tab with an equal route (empty if there is none).
