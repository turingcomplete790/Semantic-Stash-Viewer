# Quickstart: validating Browse Images (012)

## Prerequisites

- The native app built (`cargo build -p semantic-stash-viewer-native`); the Testing instance on
  :9998 (saved profile `localhost:9998`). Production (:9999) is never written to.
- **Testing seeded with images** (it had none on 2026-10-12; [research R8](research.md)):

  ```bash
  tools/seed-testing-images/seed.sh --creds <Testing creds file>   # copies from Production; idempotent
  ```

  Seeded 2026-10-12 (ids only): 1,207 images: 1,205 copied from Production (read-only: queries and
  file reads; most extracted from its zip galleries) into Testing's `ssv-prod/` folder, plus a
  truncated JPEG (id 15) and one image clip (id 1238). Largest images: 1478, 1522, 1838; GIFs:
  1437, 1745, 1849; WebPs: 1362, 1363, 1364. Performer A (id 2) on the first 30 copies, performer B
  (id 3) on copies 21–40, the 008 ordinary tag (id 6) on the first 50. Saved image filter
  "SSV images performer A": id 4. Production holds no image clips, so the clip is generated; one
  of the two generated videos became a clip.

## Automated

```bash
cargo test --workspace                                   # includes the image criteria, viewer, and UI tests
cargo run -p perf-harness -- --profile "localhost:9998" --quick   # images-* and image-* rows
```

Expected: every 012 row within budget ([plan](plan.md), [contract](contracts/images-ui.md)); every
005–011 row still within budget.

| Row | Budget |
|---|---|
| `images-page-change` | ≤ 150 ms (p95) |
| `images-page-jump` | ≤ 1 s |
| `images-scroll-1000-grid` / `-wall` | ≤ 3% / ≤ 1% missed frames at 144 Hz |
| `image-open` | ≤ 500 ms (median) |
| `image-next` | ≤ 150 ms (p95), including a page boundary |
| `image-next-large` | info (files over 12 MP: mpv's decode grows with the pixels, research R3) |
| `image-zoom-shown` / `image-pan-drag-shown` | ≤ 1 frame to mpv's picture moving on screen (p95 within 1.5× this display's frame time); `image-zoom` / `image-pan-drag` (the UI's own frame) as info |
| `slideshow-drift` | ≤ 0.3 s (p95, at a 1 s delay over stills) |
| `gql-images-page` / `gql-image-details` / `gql-image-step-prefetched` | 2 / 1 / 0 GraphQL (the page and its neighbour's prefetch, as `gql-page-50`; the files are GETs) |

## V1: Find images (US1)

Open Images: the first page in the server's default image filter. Filter to performer A and the
ordinary tag, sort by date descending, 40 per page, the wall: compare the ids with the web UI's for
the same filter and sort (page 1 and the last page). Apply "SSV images performer A". Restart: the
tab comes back as it was.

## V2: The viewer (US2)

Open the 7th card: the picture fitted, "7 of …". Press → ten times, past the page's end: each
image at once, the next page's first after the last. Open the panorama: fitted at once; `+` to
800% and drag: sharp, smooth. `R` twice, `1`, `0`. Open the corrupt file: the message shows; → still
works. Escape: the grid on the last image's page, its card focused.

## V3: Slideshow and moving images (US3)

`P` with `[`/`]` set to 3 s on performer A's images: advances every 3 s, across a page; → pauses
it; `P` resumes; it stops at the last image. The animated GIF/WebP/APNG move and loop in the
viewer, and their cards move on hover (one at a time). A clip-backed image plays muted and loops;
the slideshow waits for a loop.

## V4: Details (US4)

`I` on an image with performers and tags: every field of the web UI's image page for the same
image; tags coloured as on the scene page. Step with → : the panel follows. Click a tag chip: Images
filtered to it (Ctrl: in a new tab). An image without tags or galleries: those sections absent.

## Requirements and their tests

| Requirement | Automated | By hand |
|---|---|---|
| FR-001 paged grid, sized thumbnails | `native-ui/tests/images.rs` (opening, paging, thumbnails asked for); `images-page-change` | V1 |
| FR-002 search and every image criterion | `stash-core/tests/image_filters.rs`; `images.rs` (requery, scene-only criteria not offered, the folder picker) | V1 |
| FR-003 every image sort, both directions, random | `images.rs` (sort, direction, reshuffle); `stash-core/tests/images_page.rs` | V1 |
| FR-004 page sizes, grid and wall, card size | `images.rs` (card size keeps the first card, square pictures); `images-scroll-1000-*` | V1 |
| FR-005 saved and default image filters | `images.rs` (saved filter applied, default from the UI settings, the menu asks for image filters) | V1 |
| FR-006 the tab saved and restored | `native-ui/tests/snapshot.rs` (an Images tab round-trips; older sessions load) | V1 restart |
| FR-007 the grid's keys | `images.rs` (arrows, End, `]`, `/`, F, S); `keymap.rs` (Images has the Scenes grid's keys) | V1 |
| FR-008 full screen in the embedded player, full resolution | `player/tests/image_mode.rs`; `native-ui/tests/image_viewer.rs` (Enter opens an image of that id); `image-open` | V2 |
| FR-009 next and previous across pages | `stash-core/tests/image_viewer.rs`; `image_viewer.rs` (past the page's last card); `image-next` | V2 |
| FR-010 zoom, pan, rotation, fit, actual size | `stash-core/tests/image_viewer.rs` (clamps, anchor, pan rules); `image_viewer.rs` (keys and wheel through the player); `image-zoom-shown`, `image-pan-drag-shown` | V2 |
| FR-011 position, title, controls that fade | `image_viewer.rs` (position and state); the view by hand | V2 |
| FR-012 close to the grid on the last image's page | `image_viewer.rs` (Escape, Back, Forward reopens) | V2 |
| FR-013 an image that can't be shown | `image_viewer.rs` (failed open, decode failure, arrows still move) | V2 (id 15) |
| FR-014 slideshow at a chosen delay | `stash-core/tests/image_viewer.rs`; `image_viewer.rs` (timer, delay saved, a hidden tab pauses); `slideshow-drift` | V3 |
| FR-015 animated and clip images loop silently | `player/tests/image_mode.rs`; `image_viewer.rs` (looped opens, a slideshow waits for the loop, the 60 s cap) | V3 |
| FR-016 animated card previews | `stash-core/tests/previews.rs` (GIF, WebP, APNG, 16 MB limit); `image_viewer.rs` (hover) | V3 |
| FR-017 the details panel | `image_viewer.rs` (once per image, follows the arrows, fields, overlay under 900 px) | V4 |
| FR-018 chips filter Images | `image_viewer.rs` (here and in a new tab; Back returns to the viewer) | V4 |
| FR-019 one player | `image_viewer.rs` (an image stops a scene; a scene closes viewers; no now-playing bar) | V2 |
| FR-020 GraphQL rules | `stash-core/tests/graphql_operations.rs` (golden files, tiers); `query_budget.rs`; `gql-images-page`, `gql-image-details`, `gql-image-step-prefetched` | — |
| FR-021 keys listed, no clashes | `native-ui/tests/keymap.rs` (Images and Image viewer levels) | F1 |
| SC-001 page change ≤ 150 ms | `images-page-change` | — |
| SC-002 scrolling ≤ 3% / ≤ 1% | `images-scroll-1000-grid`, `-wall` | — |
| SC-003 next ≤ 150 ms | `image-next` (`image-next-large` apart) | V2 |
| SC-004 open ≤ 500 ms | `image-open` | V2 |
| SC-005 results match the web UI | `stash-core/tests/image_filters.rs`, `images_page.rs` (the filter and sort sent) | V1 (ids compared) |
| SC-006 zoom, pan, rotation within a frame | `image-zoom-shown`, `image-pan-drag-shown` | V2 |
| SC-007 slideshow within 0.3 s | `slideshow-drift` (5 steps at 1 s; 100 by hand) | V3 |
| SC-008 request budgets | the `gql-*` rows; `image_viewer.rs` (no details read without the panel) | — |
