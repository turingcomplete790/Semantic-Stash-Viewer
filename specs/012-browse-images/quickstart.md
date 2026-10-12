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
| `image-zoom` | ≤ 1 frame |
| `slideshow-drift` | ≤ 0.3 s |
| `gql-images-page` / `gql-image-details` / `gql-image-step-prefetched` | 1 / 1 / 0 |

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
