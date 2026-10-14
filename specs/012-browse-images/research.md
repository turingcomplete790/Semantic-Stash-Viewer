# Research: Browse Images (012)

Decisions for the plan. Each builds on what Scenes established (005, 007, 008) and the player of
007/008.

## R1. One browse layer for Scenes and Images

- **Decision**: move Scenes' entity-agnostic list machinery into a generic layer and instantiate it
  twice. In the core: `browse::ListQuery<S>` (search, criteria, sort, direction, seed; Scenes'
  `SceneQuery` becomes `ListQuery<SceneSort>` with the same serialized fields), paging, and an
  `Entity { Scenes, Images }` that names each entity's criteria, sorts, filter mode (`SCENES`,
  `IMAGES`), and default filter key. The criterion registry (`scenes/filter/kinds.rs`) gains the
  entities each kind applies to plus the image-only kinds (`photographer`, `file_count`); `to_stash.rs` gains `image_filter()` building `ImageFilterType` from the
  same `Criterion`s. In the UI: the grid, pager, filter bar, criterion editor, picker, saved-filter
  menu, card thumbnails, card previews, and tab saving move to `screens/browse/` with the entity as
  a parameter; Scenes keeps its queue view and playback; Images adds its viewer.
- **Rationale**: FR-001–FR-007 are Scenes' behaviour with images' data; Principle VIII's parity and
  the 005/008 budgets were earned there. Galleries, performers, studios, and groups need the same
  layer next (ROADMAP Phase 1). The move happens first, behind the existing tests (564 pass), before
  any image code.
- **Alternatives**: copying `screens/scenes` to `screens/images` (rejected: ~4,000 duplicated lines,
  every fix twice); a trait object per entity (rejected: the differences are data — criteria,
  sorts, card fields — so an enum and generic structs suffice).

## R2. Images' GraphQL: cards and details

- **Decision**: two Cynic operations against the pinned v0.31.1 schema. `ImagesPage`:
  `findImages(filter, image_filter) { count images { id title date paths { thumbnail }
  visual_files { __typename … on ImageFile { width height format } … on VideoFile { width height
  duration } } } }` — enough for a card (title, date, thumbnail, dimensions, whether it may animate
  or is a clip). `ImageDetails`: `findImage(id)` with every field the web UI's image page shows
  (title, code, date, photographer, details, rating100, organized, o_counter, urls, studio,
  performers, tags with the semantic fields 008 reads, galleries, the visual file's path, size,
  format, dimensions). Pages are cached as `images:q:<hash>:s:<size>:p:<page>` like scenes;
  neighbours prefetched as 005 does.
- **Rationale**: one request per page (Principle IV); the card needs only what it draws; details
  only on demand (FR-020).
- **From the semantic plugin's image deck** (`semantic-tagging/src/image-deck/graphql.js`, read
  2026-10-12): its deck rows are `id title paths { thumbnail image }`, and 012's cards stay that
  light plus the visual file's dimensions and format (for the animated and clip badges). Its
  image details fetch `custom_fields`, because the semantic panels read them straight off the
  entity. 012 doesn't yet: 008's budget rule lets only the vocabulary select `custom_fields`, and
  012 has no semantic panel on images; when one comes, `ImageDetails` gains `custom_fields` and
  that rule changes with it (Principle II). It pages with a stable sort so positions survive
  reopening; 012's random sort is seeded for the same reason (005 R7). For the Galleries feature
  next, it records three lessons kept here: a gallery's exact image total is its own
  `image_count` (findImages' count is a separate filtered search); a gallery's scenes are paged
  through `findScenes(scene_filter: { galleries })`, not `gallery.scenes` (which fetches every
  scene's full detail at once); and a gallery list selects lightly, with `scenes { id }` for a
  scene count instead of Stash's `SlimGalleryData`.
- **Alternatives**: fetching details with every card (rejected: payload ×10 for fields shown only
  in the panel).

## R3. Images in mpv: an image mode in the player crate

- **Decision**: `OpenRequest` gains `kind: MediaKind { Video, Image { looped: bool } }`. An image
  opens with `image-display-duration=inf` (a still stays until replaced), `mute=yes`, and, for a
  file that animates or is a clip, `loop-file=inf`; videos reset these. New commands, each an mpv
  property so mpv does the work (Principle V): `Zoom(log2 scale)` → `video-zoom`, `Pan(x, y)` →
  `video-pan-x/y` (fractions of the picture), `Rotate(quarter turns)` → `video-rotate`,
  `ActualSize(bool)` → `video-unscaled`, `ResetView` (zoom 0, pan 0, rotate 0, unscaled off). The
  snapshot reports the picture's dimensions (`video-params/w,h`) so the UI can compute "actual
  size" zoom and clamp panning. The direct-stream guard accepts Stash's `/image/<id>/image` (and a
  local neighbour file, R4) for image opens only.
- **Rationale**: Principle V requires mpv to show full-resolution images and do zoom, pan, rotation,
  fit, and animation; the render path (007) already draws mpv's picture in the window.
- **Alternatives**: decoding images in the core and drawing them with iced (rejected:
  Principle V); a separate mpv instance for images (rejected: one player, FR-019).
- **Spike (T003, 2026-10-12)**: `crates/player/examples/image_open.rs`, local files from the seeded
  Testing copies, headless (decode to first frame ready; drawing adds a little), 10 opens each,
  machine busy (load ≈ 8), so these are upper bounds:

  | File | Median | Min–max |
  |---|---|---|
  | JPEG 1.6 MP (Production's median size) | 45 ms | 40–59 |
  | JPEG 12 MP | 38 ms | 34–42 |
  | JPEG 23.7 MP | 222 ms | 212–231 |
  | PNG 80.4 MP (the largest, 14098×5700) | 1,402 ms | 1,296–1,491 |
  | GIF as an image clip, 0.9 MP | 59 ms | 39–73 |
  | WebP 2.3 MP | 87 ms | 82–265 |

  **Verdict**: SC-003 (≤ 150 ms with the neighbour on disk) holds for typical photos, up to about
  12 MP; above that, mpv's decode grows roughly with the pixel count (24 MP ≈ 220 ms, 80 MP ≈
  1.4 s). The viewer keeps showing the current image until the next one's first frame, so a large
  image is a short wait, never a blank. `image-next` is measured on the seeded set (mostly 1–3 MP)
  and reports large files separately; T048 repeats the spike on a quiet machine.
- **Testing's formats**: Stash on Testing reports JPEGs as `mjpeg` (Production mostly as `jpeg`),
  so both are JPEG; and with image clips on, its 20 GIFs are image clips (`VideoFile`), so they
  loop in mpv as clips (research R7) rather than as animated stills.

## R4. Next/previous in 150 ms: neighbours on local disk

- **Decision**: a core `Neighbours` service keeps the full files of the image shown and the ones
  either side (±1; ±2 while a slideshow runs, ahead in its direction) in a bounded directory in the
  cache (`images/<id>.<ext>`, at most 8 files or 512 MB, least recently used out, counted in the
  cache's size limit). Each file is fetched with the key in a header; mpv opens the local path, so
  a step never waits on the network. Crossing a page boundary prefetches the neighbouring page's
  cards first (they're already prefetched by paging, R2) and then its first or last file.
- **Rationale**: SC-003; Principle V allows prefetching neighbours; local files keep the key out of
  mpv's URLs entirely for neighbours.
- **Alternatives**: mpv's `prefetch-playlist` (rejected: experimental, needs a playlist mpv owns,
  and a URL with the header per entry); loading from Stash on each step (rejected: a 10 MB file
  takes longer than 150 ms on a LAN); an in-memory cache (rejected: mpv reads files or URLs; a
  `memory://` URL can't hold binary data).

## R5. The viewer's rules are pure, in the core

- **Decision**: `images::viewer::step(&Viewer, Event) -> (Viewer, Command)` as 011's playlist:
  position (an index into the result), slideshow (running, delay, direction), view (zoom, pan,
  rotation, actual size), details open. Events: `Next`, `Previous`, `Open(i)`, `Zoom(delta, at)`,
  `Pan(dx, dy)`, `Rotate`, `ToggleActual`, `Fit`, `SlideshowToggle`, `SlideshowTick`,
  `LoopDone`, `Shown(i)`, `Failed(i)`, `ResultsChanged(count)`, `Details`. Commands: `Show(i)`
  (the UI maps index → page and file as 011 does), `SetView(..)`, `Prefetch(indices)`, `Close`,
  `Nothing`. Clamping (zoom 1/8×–8×, pan within the picture) lives here.
- **Rationale**: Principle III; the same headless, exhaustive testing that made 011 safe.
- **Alternatives**: rules in the UI state (rejected: untestable without a window).

## R6. Tabs, sections, and saving

- **Decision**: `Section::Images` in the navigation bar (Principle IX, the Stash web UI's order),
  `Screen::Images(Box<ImagesState>)`; the Images tab state is saved additively in session version
  2 (an unknown screen in an older build is skipped, as 007's restore does). The viewer is a mode
  of the Images screen (not saved: a restart shows the grid at the image last viewed, its card
  focused). The slideshow delay is a preference (saved with playback preferences).
- **Rationale**: FR-006, FR-012; no version bump needed.
- **Alternatives**: the viewer as its own screen in the tab's history (rejected: Back from the
  viewer would land on the screen before the grid in some paths, and history fills with images).

## R7. Animated card previews and clips

- **Decision**: hovering a card whose image may animate (format GIF, WebP, or APNG, under 16 MB)
  fetches its original once (cached) and decodes it into frames at the card's width in the core,
  as 008's scene previews do (`previews.rs`); one card at a time; a still WebP or PNG simply shows
  its thumbnail. Clip-backed images show their thumbnail on cards (Stash's image preview for clips
  is video, which the UI may not play, Principle V) and play, muted and looping, in the viewer.
- **Rationale**: FR-016 within Principle V ("moving previews MUST be image formats").
- **Alternatives**: Stash's `paths.preview` for every image (rejected: for clips it's a video
  file).

## R8. Seeding Testing with images

- **Decision** (changed 2026-10-12 at the owner's request, from generating images to copying
  them): `tools/seed-testing-images/seed.sh` samples about 1,200 real images from Production
  (:9999, read-only: GraphQL queries and file reads; most of Production's 42,927 images live in
  zip galleries, so members are extracted from the zips) into Testing's `<library>/ssv-prod/` under
  neutral names: a random spread of JPEGs and PNGs, 20 GIFs, 20 WebPs, and the 15 largest (up to
  80 MP). Production has no image clips and no broken files, so the script adds a truncated JPEG
  (in `ssv-images/`) and a 5 s MP4 in `ssv-clips/`, a library path with videos excluded and Stash's
  "create image clips from videos" on. It scans only those folders, assigns performers A/B and the
  008 ordinary tag, and saves "SSV images performer A". Idempotent; prints ids only. Result
  (quickstart): 1,207 images. With image clips on, Stash also makes the 20 GIFs clips.
- **Rationale**: the spec's dependency (Testing had 0 images); real files give real decode costs
  and real formats (Testing reports JPEGs as `mjpeg`).
- **Alternatives**: generating images (the first version; replaced at the owner's request:
  Production has plenty); writing to Production (refused: read-only); committing files (refused:
  size and privacy).

## R9. Measuring it

- **Decision**: harness rows on Testing: `images-page-change` (≤ 150 ms p95), `images-page-jump`
  (≤ 1 s), `images-scroll-1000-grid` (≤ 3%) and `-wall` (≤ 1%), `image-open` (grid → picture shown,
  ≤ 500 ms median), `image-next` (≤ 150 ms p95, 20 steps including a page boundary),
  `image-zoom` (input → frame, ≤ 1 frame), `slideshow-drift` (≤ 0.3 s), and `gql-images-page`,
  `gql-image-details`, `gql-image-step-prefetched` (0) request rows. "Shown" means mpv reported
  the new file's first frame (the snapshot's file and a frame counter), not the request.
- **Rationale**: SC-001–SC-008; 011 showed conditions must follow what mpv shows, not what was
  asked.
- **Alternatives**: none worth recording.
