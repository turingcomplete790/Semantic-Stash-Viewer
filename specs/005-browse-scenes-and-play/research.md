# Research: Browse Scenes and Play — P1 (browse the whole library)

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md)

This pass covers **User Story 1** only (the plan was requested for P1), **revised on 2026-10-01
for paged views** (constitution v3.3.0, spec clarifications of 2026-10-01). The first version
designed a continuously scrolling, virtualized grid; the demo of that version showed resets on
returning to Scenes and blank thumbnails while flinging, and the constitution now requires paged
library views. Decisions marked *(kept)* are unchanged from the first version.

Measurements were taken against the user's production library (read-only): **36,350 scenes**,
Stash on the same LAN, and in WebKitGTK 2.52 (the app's engine).

## R1. Fetching a page of scene cards

- **Findings (measured, 2026-09-29)**: `findScenes(filter: {page, per_page, sort, direction})`
  with a card-sized selection, including `count`: 46–89 ms for 120 cards at any depth (page 1,
  150, 300; random order included), ≈ 40 KB. Stash paginates with `LIMIT/OFFSET`, and deep pages
  cost the same as the first.
- **Decision**:
  - A page request is `(query, page, page size)`; the page size is one of **20, 40, 50, 60, 120,
    250, 500, 1000** (default 50; spec clarification). One query per page, card fields only
    (Principle IV), with `count` on every page so the total and the page count stay current.
  - A page number past the end (the library shrank, or the page size grew) is answered with the
    **last page that exists**: the core asks again for `ceil(count / size)` when Stash returns an
    empty page with a non-zero count (spec edge case "The page no longer exists").
- **Rationale**: one round trip per page view (SC-007), any page directly (SC-003), and 50 cards
  is ≈ 17 KB, well inside the 150 ms budget.
- **Alternatives rejected**: cursor pagination (Stash has none); fetching ids first (two round
  trips per page).

## R2. Sorting, including a stable random order

- **Findings**: the web UI's scene sorts (`ui/v2.5/src/models/list-filter/scenes.ts` plus
  `MediaSortByOptions`) are: title, path, rating, file modification time, tag count, performer
  count, random, organized, date, production date, file count, file size, duration, frame rate,
  resolution, bit rate, last played, resume time, play duration, play count, interactive,
  interactive speed, perceptual similarity, performer age, studio; and `created_at`/`updated_at`
  from the common options. Random accepts a seed: sort `random_<u64>` (`randomSeedPrefix` in
  `pkg/sqlite/sql.go`).
- **Decision**:
  - Offer every sort above, with ascending/descending.
  - Random sort stores a seed in the tab's query and sends `random_<seed>`; the same seed gives
    the same order on every page, back/forward, and restart. "Reshuffle" picks a new seed.
  - The default is **date, descending** (the web UI's default for scenes). A server default
    filter overrides it (later: US2).
- **Rationale**: FR-003 and the "random stays stable" edge case, with no local sorting.

## R3. Rendering a page: a plain CSS grid, no virtualization

- **Findings (measured, 2026-10-01)** in WebKitGTK 2.52, 1600×1000 at 1×, 5 columns, cards with
  unique 480 px thumbnails and two text lines, missed frames against an idle baseline:

  | Page in a plain CSS grid | Build | 64 px/frame: missed, p95 | 240 px/frame: missed, p95 |
  |---|---|---|---|
  | 50 cards | 65 ms | 0%, 17 ms | 0%, 17 ms |
  | 120 cards | 64 ms | 0%, 17 ms | 0%, 17 ms |
  | 250 cards | 104 ms | 0%, 17 ms | 0%, 17 ms |
  | 1000 cards | 390 ms | 0%, 18 ms | 0%, 18 ms |
  | 1000 + `loading="lazy"` | 326 ms | 0%, 18 ms | 0.4%, 18 ms |
  | 1000 + `content-visibility: auto` | 149 ms | 5.9%, 25 ms | 12.6%, 26 ms |

- **Decision**:
  - A page renders as a **plain CSS grid** (`grid-template-columns: repeat(auto-fill,
    minmax(240px, 1fr))`) or list, with no virtualization: every size up to 1000 scrolls at
    60 fps, as Principle VI now allows (virtualization only where a view can't stay in budget).
  - Cards keep `contain: layout paint style`, one-line text (title, details), and
    `decoding="async"` thumbnails with `loading="lazy"` (cheaper builds at large sizes, and
    off-screen thumbnails of a 1000-card page aren't fetched until they're near).
  - `content-visibility` is not used (it makes scrolling worse here).
  - The velocity-aware thumbnail hold-back of the first version is **removed**: it existed for
    flinging through an endless grid, and its grey placeholders are the "black thumbnails" seen
    in the demo.
  - **Page-change budget**: under 150 ms up to 250 per page (build ≤ 104 ms measured); larger
    sizes take longer to build (390 ms at 1000) and are budgeted at under 500 ms. The spec's
    SC-002 is read that way (plan note).
- **Rationale**: simplest code that measures within budget at every size; no row pool, no scroll
  math, nothing to restore but a page number and a scroll offset.
- **Alternatives rejected**: keeping the virtual grid inside a page (unneeded per the table, and
  it carried the demo's bugs); `content-visibility` (worse scrolling).

## R4. Moving between pages: prefetch the neighbours

- **Decision**:
  - When a page is shown, the core is asked for the **previous and next pages** too (data, from
    the cache or Stash), after the current page has arrived.
  - Their thumbnails are warmed by creating off-screen `Image` objects for their cards' `thumb`
    URLs (at most the first 120 per neighbour): the `ssv-thumb://` scheme prepares and caches them
    in the core (≤ 6 at a time), and the webview's memory cache keeps the bytes, so the next page
    shows with its thumbnails immediately (spec scenario 2, SC-002).
  - Changing page scrolls the grid to its top; first/last and "Go to page" fetch the target page
    first, then its neighbours.
  - Requests are bounded: the current page plus two neighbours; a neighbour request still waiting
    when the user moves on is dropped.
- **Rationale**: FR-002/FR-004 and SC-002/SC-003 with at most three page requests per move.

## R5. Thumbnails: full-size screenshots are too big for a grid

- **Findings (measured)**: Stash serves `/scene/{id}/screenshot` at the **source resolution**
  and has no size parameter (`internal/api/routes_scene.go`). Eight random scenes: 19–328 KB,
  average ≈ 100 KB, up to 3840×2160. A page of 50 cards would transfer ≈ 5 MB, and WebKit
  would decode 4K JPEGs (≈ 33 MB each, decoded) to show them ≈ 300 px wide.
  Today's poster (004) is fetched by a command and returned as a base64 data URL: fine for one
  image, far too slow for 60.
- **Findings (measured, 2026-09-29)**: decode → 480 px → encode for one screenshot, median of
  15 runs (scratch benchmark, release build, this machine):

  | Pipeline | 4K (418 KB) | 1080p | 720p |
  |---|---|---|---|
  | `image::thumbnail` (the first choice) | 50.8 ms | 20.4 ms | 17.7 ms |
  | `zune-jpeg` + `fast_image_resize` | 32.4 ms | 10.1 ms | 8.2 ms |
  | `jpeg-decoder` DCT-scaled decode + `fast_image_resize` | 18.0 ms | 7.5 ms | 5.8 ms |
  | **`turbojpeg` (libjpeg-turbo) DCT-scaled decode + `fast_image_resize`** | **9.1 ms** | **3.8 ms** | **3.9 ms** |

  DCT scaling decodes JPEGs directly at 1/2, 1/4, or 1/8 size, so a 4K frame never fully
  decodes. Output is 13–20 KB at quality 80 for every pipeline.
  Stash already serves **640 px image thumbnails** (`/image/{id}/thumbnail`,
  `DefaultGthumbWidth = 640`, generated on demand), so images and galleries (a later spec) need
  no local resizing.
- **Decision**: a **generic custom URI scheme** handled in the core, `ssv-thumb://`:
  - `<img src="ssv-thumb://localhost/<kind>/<id>?v=<version>">`, with `kind` = `scene` now, and
    `image`/`gallery` reserved for the images-and-galleries spec (Principle X's mixed grids use
    the same scheme). The core fetches from Stash's media endpoint with the profile's API key
    (header, never in the URL; Principles V and VII). The fetch lives in the adapter module
    (`adapter/scenes.rs`); these are media endpoints, like the video streams, not GraphQL.
  - **Scenes**: screenshots are resized to **480 px wide** (enough for a ≈ 240 px card on a 2×
    display). JPEG sources (all measured screenshots) take the fast path: libjpeg-turbo decode
    at the largest DCT scale that stays ≥ 480 px, `fast_image_resize` (bilinear) to exactly
    480 px, libjpeg-turbo encode at quality 80. Other formats (uploaded or scraped PNG/WebP
    covers) go through `image`'s decoders, then the same resize and encode.
  - **Images/galleries** (later): Stash's 640 px thumbnail passes straight through into the
    cache, no decoding.
  - Results are stored in **003's view cache** as blobs under `thumb:<kind>:<id>:<version>`, so
    they inherit the size limit, LRU eviction, Clear cache, and the per-server identity wipe.
    `<version>` is the `t` value from the media path, which changes when the image does.
  - At most 6 downloads and resizes run at once; the rest queue. Decoding runs off the async
    threads (blocking pool). Budget: under 15 ms per 4K screenshot.
  - Failures return a placeholder image (the card shows the default look).
  - The CSP gains `img-src ssv-thumb:` (and the platform's `http://ssv-thumb.localhost` form on
    Windows, later).
- **Crates** (constitution "Dependencies": what, size, maintenance):
  - `turbojpeg` 1.x: safe bindings to libjpeg-turbo, the standard SIMD JPEG codec; actively
    maintained. It links the system `libturbojpeg`, which is already on every Linux install
    that runs the app (WebKitGTK and ffmpeg depend on libjpeg-turbo). CI adds
    `libturbojpeg0-dev`. **Platform gap** (documented per the constitution): Windows and macOS
    need the crate's bundled CMake build (with NASM); the pure-Rust `jpeg-decoder` path
    (18 ms at 4K) is the fallback if that proves impractical.
  - `fast_image_resize` 5.x: SIMD resizing (AVX2/SSE4.1/NEON); actively maintained; pure Rust.
  - `image` 0.25 with only the PNG and WebP decoders (`default-features = false`): the
    non-JPEG fallback.
- **Rationale**: 20–30× less image data per screen, 5.6× faster than the first choice at 4K, no
  huge decodes on the UI thread, instant thumbnails on revisits and after restarts, one cache,
  and one scheme ready for mixed image/scene grids.
- **Alternatives rejected**:
  - `image::thumbnail` alone: 50.8 ms per 4K screenshot, over budget.
  - `<img>` straight at Stash's URL with `?apikey=`: puts the key in URLs (and logs), sends full
    size images, and bypasses our cache.
  - Data URLs through commands: base64 inflates by a third and blocks the IPC channel.
  - A separate thumbnail folder: a second cache with its own limit and clearing.

## R6. Where the page, size, mode, and scroll position live

- **Decision**: the tab's history entry (004 view state) holds the **Scenes view state**:
  `{ query, page, pageSize, mode, scroll }`.
  - `scroll` is the inner grid's scroll offset, saved as the user scrolls (coalesced to one write
    per animation frame) and restored after the page's cards render.
  - Changing the query or the page size goes to page 1 (a new page size keeps the first visible
    scene's position: page = `floor(firstIndex / newSize) + 1`, spec scenario 4).
  - The page size is per tab; new tabs start at 50 (spec clarification).
  - The view reads its state once when it mounts and writes changes back; it never takes its
    starting state from anything but its own history entry.
- **Rationale**: FR-005, FR-014, SC-009 ("never resets"); see R13 for why the entry must be the
  right one.

## R7. Caching pages (003)

- **Decision**: each page is a cache entry `scenes:q:<query hash>:s:<page size>:p:<page>`
  (`Auto` refresh: cached copy at once, refreshed quietly after 5 s). If a refresh changes a page,
  `view-data-changed` fires for that key and the view re-reads that page in place (keeping the
  scroll position). Random-sort pages are cached like any other (the seed is in the hash).
- **Rationale**: SC-001's warm-cache budget and 003's quiet refresh; the page size is part of
  the key because a page's contents depend on it.

## R8. Keyboard (FR-006, spec clarification)

- **Decision**: roving focus over the page's cards (one focusable card, `tabindex=0`):
  - ←/→ move by one, ↑/↓ by a row (columns read from the rendered grid); past the last card of
    the page → next page, focus on its first card; past the first card → previous page, focus
    on its last card;
  - `[` / `]` previous/next page; Home/End first/last card on the page; Enter opens, Ctrl+Enter
    opens in a new tab;
  - the keys are listed under "Scene lists" in the keymap (display-only entries; the grid
    handles them while it has focus).
- **Rationale**: Principle VI (fully keyboard-navigable) and the user's choice of keys.

## R9. What happens to the spike's picker (FR-007)

- **Decision**: the Scenes view becomes the grid. The "open by ID" box and the test set leave
  the UI. `list_test_scenes` and `list_recent_scenes` stay as commands for the harness and
  debug builds (the harness's measure mode uses the recent list; the bench uses it to pick a
  scene).
- **Rationale**: FR-007; keeps 003's harness working unchanged.

## R10. Measuring the budgets (harness)

- **Decision**: the 003 harness bench measures, on the real Scenes view:
  - `nav-first-paint` (Home ↔ Scenes, page 1 of 50; SC-001);
  - `scenes-page-change`: 20 next/previous moves at the default size, each until its cards and
    thumbnails are painted (SC-002, budget 150 ms);
  - `scenes-page-jump`: 5 jumps to random pages (SC-003, budget 1 s);
  - `scenes-scroll-1000`: scroll a 1000-card page at 64 px/frame, missed frames against the idle
    baseline (SC-002, budget < 1%), in grid and list mode.
  The first version's continuous-scroll measurements (`scenes-grid-*`, `scenes-jump`) are
  replaced.

## R11. mpv as the media player (constitution v3.2.0) and P1

- **Findings**: mpv 0.41 / FFmpeg 9 here decodes JPEG, PNG, WebP (animated too), GIF, TIFF, BMP,
  JPEG XL, and AVIF. Opening full-resolution images from the production library (29,057 images;
  12 random, 0.3–3 MB, up to 3500×2893) took ≈ 38 ms per image including the fetch, with
  playlist prefetch. WebKitGTK here also decodes JPEG, PNG, GIF, WebP, AVIF, and JPEG XL, which
  covers thumbnails and animated WebP previews.
- **Decision for P1**: nothing in the grid changes: grids and lists stay in the webview with sized
  thumbnails (Principle V requires it, and mpv can't draw grids). The thumbnail scheme is generic
  (R5) so the later mixed image/scene grids (Principle X) reuse it.
- **Noted for later stories**:
  - US4: model the player's current item as a scene or an image; play history applies to scenes
    only, and a switch to an image saves the scene's history like a scene change (FR-020).
  - US5: previews are Stash's animated WebP previews as `<img>` (FR-025/FR-026); plan US5 should
    measure how many scenes have them.
  - Galleries spec: one mpv viewer for images and scenes; "the viewer takes over the player"
    when a scene is playing elsewhere; budget next/previous < 150 ms.

## R12. Blob storage: keep SQLite, not sled (analysis 2026-09-30)

- **Question**: should thumbnail blobs (and, later, the semantic-search index) move from 003's
  SQLite view cache to `sled`?
- **Findings (measured)**: scratch benchmark (`kvbench`), release build, this machine: 5,000
  unique thumbnails of real size (≈ 12–16 KB, 66.5 MB in total), one commit per write as the
  cache does, then random reads of all 5,000 and a reopen. Two runs, averaged:

  | Store | Write (per put) | Read (per get) | Reopen | Disk for 66.5 MB |
  |---|---|---|---|---|
  | **SQLite** (003's cache: WAL, `synchronous=NORMAL`, one table) | ≈ 52 µs | ≈ 11 µs | **0.3 ms** | **70 MB (1.05×)** |
  | sled 0.34.7 | ≈ 48 µs | ≈ 1 µs (from its in-memory cache) | 78–86 ms | 115–118 MB (1.7×) |
  | redb 2 | ≈ 54 µs | ≈ 7 µs | 0.4 ms | 135 MB (2.0×) |

  - Writes are equal. sled's reads are faster only because it keeps data in RAM; SQLite's 11 µs
    × 60 thumbnails is under 1 ms per screen, invisible next to the 150 ms budget.
  - sled's reopen replays its log and grows with the data (≈ 80 ms at 5,000 entries; a fully
    browsed 36,000-scene library is ≈ 36,000 thumbnails). That cost lands on every cold start
    (budget < 2 s; measured 1.1 s today).
  - sled and redb use 1.7–2.0× the logical size on disk, while 003's size limit (5% of free
    disk, R1b) counts logical bytes, so the real footprint would overshoot the user's cap.
  - Maintenance: sled's last stable release, 0.34.7, is from 2021; 1.0 has been in alpha for years
    (1.0.0-alpha.124) with an unstable on-disk format. The constitution's Dependencies rule
    requires justifying maintenance status and prefers mature, widely used crates; 003's R1 had
    already rejected sled for this reason.
- **Decision**: **keep SQLite** (003's `ViewCache`) for thumbnail blobs (T004's `put_bytes` /
  `get_bytes`).
- **Semantic search (later)**: the storage for the semantic index is decided in its own spec.
  The starting assumption is SQLite in a **separate, discardable derived-index file**
  (Principle I), or an in-memory index saved as a snapshot: SQLite already offers indexes for
  subject/predicate/object lookups, recursive queries (archetype inheritance), FTS5 full-text
  search, JSON1, and the `sqlite-vec` extension if embeddings are ever used. With sled, every
  index and the query planner would be hand-built on an ordered key-value store.
- **Alternatives rejected**: sled (above); redb (stable and maintained, but 2× disk use and no
  query capabilities beyond key-value, so no gain over SQLite here).

## R13. Returning to Scenes must reuse its history entry (the demo's reset bugs)

- **Findings (code)**: every way of returning to Scenes except the Back button pushed a **new**
  history entry with empty view state, so the view started over in grid mode at the top:
  - leaving a scene (Done, ✕, Escape, playback ending) calls `onLeave({ kind: "scenes" })` →
    `navigateIn(tab, …)`, which pushes a new entry, although the Scenes view the user came from is
    the previous entry;
  - choosing Scenes in the navigation bar always pushes a new entry.
- **Decision**:
  - **Leaving a view goes back** when the previous history entry is the fallback route (the
    usual case: Scenes → scene → Done returns to that Scenes entry and its state); otherwise it
    navigates as before.
  - **Navigating to a section carries its state forward**: a new entry for a route the tab has
    visited before starts with a copy of the most recent such entry's view state (so Scenes in
    the navigation bar returns to the same page, size, mode, and scroll). A different tab, or a
    first visit, starts fresh.
  - Regression tests cover both paths in grid and list mode (SC-009).
- **Rationale**: constitution IX ("leaving a view and returning never resets it"); the fix lives
  in the shell (`tabs.ts`, `App.tsx`), so every later section gets it for free.
- **Alternatives rejected**: a global per-section memory outside history (would leak state
  between tabs, against the per-tab rule).

