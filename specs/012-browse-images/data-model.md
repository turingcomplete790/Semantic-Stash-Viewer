# Data Model: Browse Images (012)

New and changed entities. Scenes' model (005, 007, 008, 010, 011) is unchanged in behaviour; parts
of it move into the generic browse layer ([research R1](research.md)).

## Entity (new, `stash_core::browse`)

`Scenes` | `Images`. For each: its criterion kinds, its sorts, its saved-filter mode (`SCENES`,
`IMAGES`), its default-filter key (`defaultFilters.scenes` / `.images`), its cache-key prefix
(`scenes:` / `images:`), and its thumbnail kind (`scene` / `image`).

## ListQuery<S> (moved and made generic, `stash_core::browse::query`)

| Field | Type | Notes |
|---|---|---|
| `search` | `String` | as 005 |
| `criteria` | `Vec<Criterion>` | as 008; each kind valid for the entity |
| `sort` | `S` (`SceneSort` / `ImageSort`) | |
| `direction` | `Asc` \| `Desc` | |
| `seed` | `Option<u32>` | random sort |

`SceneQuery = ListQuery<SceneSort>`; its serialized form is unchanged (a saved 011 session loads).

## CriterionKind (changed)

Each kind gains `entities() -> &[Entity]`. Images use exactly the web UI's v0.31.1 list
(`ui/v2.5/src/models/list-filter/images.ts`, confirmed 2026-10-12), in its order: `title`, `code`,
`details`, `photographer`*, `checksum`, `phash_distance`, `path`, `folder`* (Includes only,
hierarchical, sent as `files_filter: { parent_folder }`), `galleries`, `organized`, `o_counter`,
`resolution`, `orientation`, `is_missing` (the image values), `tags`, `rating100`, `tag_count`,
`performer_tags`, `performers`, `performer_count`, `performer_age`, `performer_favorite`,
`studios`, `url`, `date`, `file_count`, `created_at`, `updated_at`, `custom_fields`; plus the
semantic criteria that apply to images (008). (* new kinds; every other kind exists for scenes.)
`image_filter()` builds `ImageFilterType`; a kind not valid for images is never sent.

## ImageSort (new, `stash_core::images`)

Every sort the web UI's images list offers (v0.31.1 `images.ts` + `filter-options.ts`, confirmed
2026-10-12), as Stash's sort names: `filesize`, `file_count`, `date`, `resolution`, `title`,
`path`, `rating`, `file_mod_time`, `tag_count`, `performer_count`, `random` (with seed),
`o_counter`, `created_at`, `updated_at`. Default: the server's default image filter, else `path`
ascending (the web UI's `defaultSortBy`). Display modes: Grid and Wall only.

## ImageCard (new)

| Field | Type | Notes |
|---|---|---|
| `id` | `String` | |
| `title` | `String` | Stash's title, else the file name (as the web UI) |
| `date` | `Option<String>` | |
| `thumb` | `Option<String>` | `ssv-thumb://localhost/image/<id>?v=<version>` |
| `width`, `height` | `u32` | the visual file's |
| `media` | `Still` \| `MayAnimate` (GIF, WebP, APNG) \| `Clip { duration }` | from the visual file |

## ImagePage (new)

`count: u32`, `page: u32`, `page_size: u32`, `items: Vec<ImageCard>` (as `ScenePage`).

## ImageDetails (new)

Title, code, date, photographer, details, rating (0–100), organized, O-count, URLs, studio
(id, name), performers (id, name, image version), tags (as 008's scene tags, with semantic flags),
galleries (id, title), file (path, size, format, width, height; duration for clips).

## ImagesState (new, `native-ui`; the browse layer's state with `Entity::Images`)

| Field | Saved | Notes |
|---|---|---|
| query, page, page size, mode (Grid \| Wall), card size, scroll, focused, applied saved filter | yes | as Scenes (FR-006) |
| data, panels, search draft, pending request | no | as Scenes |
| `viewer` | no | `Option<Viewer>` below; `None` shows the grid |

## Viewer (new, `stash_core::images::viewer`, pure)

| Field | Type | Notes |
|---|---|---|
| `count` | `u32` | images in the result |
| `at` | `u32` | result index shown |
| `view` | `View { zoom: f32 (log2, −3..=3), pan: (f32, f32), quarter_turns: u8, actual: bool }` | reset on each new image |
| `slideshow` | `Option<Slideshow { delay_s: u32 (1..=60), paused: bool }>` | |
| `details` | `bool` | panel open |
| `failed` | `Option<u32>` | the index that couldn't be shown |

**Events**: `Open(i)`, `Next`, `Previous`, `Shown(i)`, `Failed(i)`, `Zoom { delta, anchor }`,
`Pan { dx, dy }`, `Rotate`, `ToggleActual`, `Fit`, `ToggleSlideshow`, `SetDelay(s)`, `Tick`,
`LoopDone`, `ToggleDetails`, `ResultsChanged { count }`, `Close`.

**Commands**: `Show(i)` (the UI maps the index to a page and a file), `SetView(View)`,
`Prefetch(Vec<u32>)`, `Close { at }` (the grid focuses `at`'s card on its page), `Nothing`.

### Validation and transitions

- `at < count`; `Next` at `count - 1` → `Nothing` (a slideshow stops there: `slideshow = None`);
  `Previous` at 0 → `Nothing` (no wrap).
- Any new image (`Next`, `Previous`, `Open`, a slideshow tick) resets `view` to fit, unrotated.
- `Zoom` clamps to 1/8×–8× (log2 −3..=3); `Pan` clamps so the picture's edge never leaves the
  window's middle; panning without zoom does nothing.
- A manual `Next`, `Previous`, `Zoom`, or `Pan` while a slideshow runs sets `paused = true`.
- `Tick` advances only when not paused; for a looping image it waits for `LoopDone` (at most 60 s).
- Each `Show(i)` comes with `Prefetch` of `i ± 1` (`i + 1, i + 2` in a running slideshow).
- `Failed(i)` keeps `at = i` (the message shows); `Next`/`Previous` still move.
- `ResultsChanged { count }`: `at` clamps to `count - 1`; `count == 0` → `Close`.

## Neighbour files (new, `stash_core::images::neighbours`)

A bounded directory in the cache: `images/<id>.<ext>`, at most 8 files or 512 MB, least recently
used evicted, counted in the view cache's size limit. Entry states: `Fetching`, `Ready(path)`,
`Missing` (404: deleted), `Failed`. A `Show(i)` uses `Ready(path)` at once, else opens Stash's
URL with the key in a header and stores the file when done.

## Preferences (changed)

Playback preferences gain `slideshow_delay_s: u32` (default 5).
