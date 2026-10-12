# Feature Specification: Browse Images

**Feature Branch**: `012-browse-images`

**Created**: 2026-10-12

**Status**: Draft

**Input**: User description: "roadmap images." (ROADMAP.md, Phase 1 "Images": an image grid with
sized thumbnails, filters, and sorting; a full-screen image viewer in mpv with keyboard and mouse
navigation, zoom, pan, rotation, fit/actual size, animated GIF/WebP, a slideshow, and full
resolution loaded on demand with the neighbouring images prefetched, next/previous shown within
150 ms.)

## Overview

The Images section of the navigation bar opens the Stash library's images the way Scenes opens its
scenes (005, 007, 008): a paged grid of sized thumbnails with the web UI's search, every image
filter and sort, and saved filters. Opening an image shows it full screen, in the same embedded
player that plays scenes (constitution Principle V), at full resolution, with the controls a
photo viewer has (next and previous, zoom, pan, rotate, fit or actual size), animated images
moving, and a slideshow. The image's details (title, date, studio, performers, tags, galleries,
file) are a keystroke away.

Galleries (images and scenes together, Principle X) are the next roadmap section and build on this
one: the grid, the viewer, and the details here are what a gallery shows. They are not part of
this feature.

### Web UI capabilities covered (Principle VIII)

| Stash web UI capability | Where |
|---|---|
| Images list: search, every image filter criterion with its modifiers, every sort, page sizes, paging | US1 |
| Display modes for images (grid, wall) and the zoom slider (card size) | US1 |
| Saved filters for images: loading and applying; the images default filter | US1 (saving and deleting: Phase 2) |
| Lightbox: next/previous, zoom, fit, slideshow with a delay, animated images | US2, US3 |
| Image page: title, code, date, photographer, details, studio, performers, tags, galleries, URLs, rating, organized, O-count, file info | US4 (shown, not edited) |
| Image edit, rating and O-count changes, delete, bulk edit, scrape | Not here: Phase 2 (editing) |
| Gallery browsing and gallery viewer | Not here: the Galleries roadmap section |

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Find images: grid, search, filters, sorting (Priority: P1)

A user opens Images and sees the library as a paged grid of thumbnails, each sized to its card. They
search, add any filter the web UI offers (performers, tags, studio, galleries, rating, resolution,
orientation, date, organized, path, and the rest), choose any sort, change the page size and the
card size, switch to the wall, and apply a saved filter. Everything they set is kept with the tab
and comes back after a restart.

**Why this priority**: Nothing else in the feature is reachable without finding images, and it
reuses what Scenes already proved (paging, filters, saved filters).

**Independent Test**: On the Testing server, open Images: the first page shows within the page
budget; filter to one performer and a tag, sort by date descending, set 40 per page and the wall:
the results match the web UI's for the same filter and sort, page by page; restart the app: the tab
comes back as it was.

**Acceptance Scenarios**:

1. **Given** a connected server with images, **When** the user opens Images, **Then** the first page
   of images shows as a grid of thumbnails in the server's default image filter and sort, with the
   result count and the pager.
2. **Given** the Images grid, **When** the user types a search, adds filter criteria, or changes the
   sort or its direction, **Then** the results (and their count) match the web UI's for the same
   search, criteria, and sort, starting from page 1.
3. **Given** the Images grid, **When** the user applies a saved image filter, **Then** its search,
   criteria, sort, page size, and display mode apply, and its name shows until the criteria change.
4. **Given** a result of several pages, **When** the user moves between pages (pager, keyboard, or
   "go to page"), **Then** each page shows within the page budget, with the next page's thumbnails
   already on their way.
5. **Given** an Images tab with a search, criteria, sort, page, page size, card size, display mode,
   and scroll position, **When** the app restarts, **Then** the tab comes back exactly so.

---

### User Story 2 - View an image full screen (Priority: P2)

From the grid the user opens an image. It fills the window at full resolution in the app's own
player. They step to the next and previous image (keys, mouse wheel, on-screen arrows, or a swipe
of the mouse), zoom in on a detail and pan around it, rotate a sideways photo, toggle between
"fit to window" and actual size, and go back to the grid exactly where they left it.

**Why this priority**: Viewing images is the point of the section, and the constitution requires
full-resolution viewing in mpv (Principle V); it depends on US1 for something to open.

**Independent Test**: Open the 7th image of a filtered page: it shows at full resolution; press
→ ten times: each image shows within 150 ms; at the page's last image → continues with the next
page's first; zoom to 400%, drag to pan, rotate 90°, press the "actual size" key: each responds at
once; Escape: the grid shows the page of the image last viewed, that image's card focused.

**Acceptance Scenarios**:

1. **Given** the grid, **When** the user opens an image (click, Enter), **Then** it shows full
   screen in the app's player at full resolution, fitted to the window, with its position in the
   result ("7 of 214") and its title.
2. **Given** the viewer, **When** the user moves to the next or previous image, **Then** it shows
   within 150 ms (the neighbours were loaded ahead), and moving past a page's first or last image
   continues on the neighbouring page, in the grid's sort.
3. **Given** the viewer, **When** the user zooms (wheel with a modifier, keys, or a pinch), pans
   (drag, arrows while zoomed), rotates (90° steps), or switches between fit and actual size,
   **Then** the image responds at once and the next image opens fitted again, unrotated.
4. **Given** the viewer, **When** the user closes it (Escape, the close control, Back), **Then** the
   grid shows the page holding the image last viewed, with that image's card focused and in view.
5. **Given** an image whose file can't be shown (unsupported or missing), **When** it's opened,
   **Then** the viewer says so in place of the picture, and next/previous still work.

---

### User Story 3 - Slideshow and moving images (Priority: P3)

The user starts a slideshow from the viewer: images advance on their own at a delay they choose,
pausing when they step or zoom by hand. Animated GIFs and WebPs, and images whose file is a short
video clip, move as they would on the web, looping.

**Why this priority**: The web UI's lightbox has both; they build on the viewer.

**Independent Test**: Start a slideshow at 3 s on a filtered result of 12 images: each advances
after 3 s ± 0.3 s and shows within 150 ms; press → to step: the slideshow pauses; resume it; it
stops after the last image of the result. Open an animated GIF and a clip-backed image: both move
and loop.

**Acceptance Scenarios**:

1. **Given** the viewer, **When** the user starts the slideshow, **Then** images advance in the
   grid's order at the chosen delay (default 5 s, from 1 to 60 s), across pages, and stop after the
   result's last image.
2. **Given** a running slideshow, **When** the user steps, zooms, pans, or pauses it, **Then** it
   pauses until they resume it.
3. **Given** an animated image (GIF, WebP, APNG) or an image whose file is a video clip, **When** it
   shows in the viewer, **Then** it plays and loops, silent, and a slideshow waits for at least one
   full loop before moving on (up to 60 s).
4. **Given** the grid, **When** the user hovers an animated image's card, **Then** the card shows it
   moving (as scene cards preview, 008), and only one card moves at a time.

---

### User Story 4 - An image's details (Priority: P4)

In the viewer (or from a card), the user opens the image's details beside the picture: title,
code, date, photographer, details text, studio, performers, tags (marked by semantic type and
coloured as on the scene page), galleries, URLs, rating, organized, O-count, and the file's
dimensions, size, format, and path. A performer, tag, studio, or gallery chip filters Images to
it, as the scene page's chips do.

**Why this priority**: Stash parity for the image page; useful, but the section works without it.

**Independent Test**: Open an image with performers and tags: the details panel shows every field
the web UI's image page shows, for the same image; click a tag chip: Images shows that tag's
images, in a new tab with Ctrl.

**Acceptance Scenarios**:

1. **Given** the viewer, **When** the user opens the details (a key or a control), **Then** a panel
   beside the picture shows the image's fields as listed above, and the picture shrinks to fit
   beside it; the panel stays open as the user moves between images, following each.
2. **Given** the details panel, **When** the user clicks a performer, tag, or studio chip, **Then**
   Images shows the images matching just that chip (in a new tab with Ctrl), as on the scene page.
3. **Given** an image with no performers, tags, or galleries, **When** its details show, **Then**
   those sections are left out rather than shown empty.

---

### Edge Cases

- **Very large images** (beyond the display's size, e.g. 12000 px panoramas): shown fitted at once
  and sharp when zoomed; zoom goes up to the image's actual size and beyond (to 800%) without
  stalling the app.
- **A huge library** (hundreds of thousands of images): paging keeps every page within budget;
  nothing loads the whole library.
- **Images inside zip galleries**: shown like any other image (Stash serves them).
- **An image deleted on the server** while the user browses: its card's thumbnail or the viewer
  shows it's gone; next/previous skip past it.
- **The result changes under the viewer** (a refresh with different counts): the viewer keeps the
  image it shows; next/previous follow the new result from there.
- **No images, or no matches**: the grid says so, with "Clear filters" when filters are set.
- **The server goes offline while viewing**: the current image stays; moving to one not yet loaded
  shows the connection problem in place of the picture; cached pages and thumbnails stay usable.
- **A scene is playing** when the user opens an image: the viewer uses the one player, so the scene
  stops first (its play history saved, 008), as opening another scene would.
- **Window narrower than the details panel needs**: the panel opens over the picture instead of
  beside it.
- **Images of a disallowed or unknown type**: shown as unsupported (US2 scenario 5), never fetched
  through a transcode.

## Requirements *(mandatory)*

### Functional Requirements

**Finding images (US1)**

- **FR-001**: Images MUST open the library's images as a paged grid of thumbnails, each decoded at
  the size its card is drawn (sharp on high-density displays), with the result count and a pager
  (first, previous, next, last, go to page), as Scenes does.
- **FR-002**: Images MUST offer the web UI's image search and **every** image filter criterion the
  web UI offers, with its modifiers, as removable chips with "Clear filters".
- **FR-003**: Images MUST offer every sort the web UI offers for images, both directions, including
  random with a reshuffle.
- **FR-004**: Images MUST offer the web UI's image page sizes and display modes (grid and wall), and
  a card-size control.
- **FR-005**: Images MUST load and apply the server's saved image filters, and open new tabs in the
  server's default image filter.
- **FR-006**: An Images tab's search, criteria, sort, page, page size, card size, display mode,
  scroll position, and keyboard focus MUST be saved with the tab and restored after a restart.
- **FR-007**: The grid MUST be fully usable from the keyboard (arrows, Home/End, page keys, Enter to
  open, Ctrl+Enter for a new tab, `/` to search), as Scenes is.

**The viewer (US2)**

- **FR-008**: Opening an image MUST show it full screen in the app's embedded player, at full
  resolution, loaded on demand (the grid never downloads full-resolution files).
- **FR-009**: Next and previous MUST follow the grid's result and sort across pages, and each MUST
  show its image within 150 ms; the images on either side MUST be loaded ahead while one is shown.
- **FR-010**: The viewer MUST offer zoom (to 800%), pan, rotation in 90° steps, fit to window, and
  actual size, by keyboard and mouse; a new image opens fitted and unrotated.
- **FR-011**: The viewer MUST show the image's position in the result and its title, and MUST have
  on-screen controls for every action (close, previous, next, zoom, rotate, fit, slideshow,
  details) that fade when the pointer is still.
- **FR-012**: Closing the viewer MUST return to the grid on the page of the last image viewed, its
  card focused and in view.
- **FR-013**: An image that can't be shown MUST show a clear message in place of the picture; the
  viewer MUST stay usable.

**Slideshow and moving images (US3)**

- **FR-014**: The viewer MUST offer a slideshow in the result's order at a chosen delay (1–60 s,
  default 5 s, remembered), pausing on any manual step, zoom, or pan, and stopping after the
  result's last image.
- **FR-015**: Animated images and clip-backed images MUST play silently and loop in the viewer;
  during a slideshow they show for at least one loop (up to 60 s).
- **FR-016**: Hovering an animated image's card MUST show it moving, one card at a time, as scene
  previews do (008).

**Details (US4)**

- **FR-017**: The viewer MUST offer a details panel showing every field the web UI's image page
  shows (read-only), following the image shown, beside the picture or over it in a narrow window.
- **FR-018**: Performer, tag, and studio chips MUST filter Images to that item (a new tab with
  Ctrl); tags MUST be marked and coloured as on the scene page (008, Principle XI).

**Across the feature**

- **FR-019**: The viewer and the scene player MUST be the same player: opening an image while a
  scene plays stops the scene (saving its play history), and Images MUST respect tabs (each tab its
  own grid; the viewer belongs to the tab that opened it).
- **FR-020**: Every request MUST be within the constitution's GraphQL rules (Principle IV): one page
  request per page shown (0 when cached), the neighbour pages prefetched, details only for the
  image whose details are open, thumbnails and full-resolution files fetched with the profile's
  key in a header, never in the address.
- **FR-021**: The viewer's keys MUST be listed in keyboard help and Settings → Keyboard, and MUST
  not clash with the shell's or the grid's.

### Key Entities

- **Image** (Stash's): title, code, date, photographer, details, rating, organized, O-count,
  studio, performers, tags, galleries, URLs, and its file (dimensions, size, format, path; an image
  file or a short video clip).
- **Images tab state** (saved): search, criteria, sort and direction (and random seed), page, page
  size, card size, display mode, scroll position, focused card, applied saved filter.
- **Viewer state** (per tab, not saved): the image shown (its index in the result), zoom, pan,
  rotation, fit or actual size, slideshow running and its delay (the delay remembered), details
  panel open.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An Images page change shows the new page in under 150 ms (95th percentile, 50 per
  page) and a jump to a far page in under 1 s, as Scenes' budgets.
- **SC-002**: Scrolling a 1000-card Images page misses at most 3% of frames on a 144 Hz display in
  the grid and at most 1% in the wall, as Scenes' budgets.
- **SC-003**: Next and previous in the viewer show the image within 150 ms (95th percentile),
  including across a page boundary when the neighbouring page is cached.
- **SC-004**: Opening an image from the grid shows it, fitted, in under 500 ms for a typical photo
  (up to 24 megapixels) on a local network.
- **SC-005**: For 20 filter-and-sort combinations, the Images results (ids and order, page 1 and the
  last page) match the web UI's exactly.
- **SC-006**: Zoom, pan, and rotation respond within one frame of input; a 12000 px image zooms and
  pans without dropped frames above the scrolling budget.
- **SC-007**: A slideshow advances within 0.3 s of its delay for 100 consecutive images.
- **SC-008**: The request budgets hold: no requests for cards scrolled past beyond their
  thumbnails, one page request per page shown, nothing to the server when stepping to a prefetched
  image.

## Assumptions

- Read-only, as Phase 1: rating, O-count, organized, and every other edit belong to Phase 2;
  the details panel shows them without changing them.
- Images on Stash v0.31.1 have no play count or resume point, so the viewer writes nothing to the
  server (unlike 008's scene play history).
- The grid, paging, filter chips, saved filters, card previews, and tab saving reuse what Scenes
  established (005, 007, 008), with images' own criteria and sorts; a queue-style view and a list
  mode for images are not part of this feature (the gallery's queue-style view is on the roadmap).
- The viewer is the embedded player of 007/008 showing still images (Principle V); thumbnails in
  the grid are decoded sized images, as scene cards are.
- Mouse gestures follow common photo viewers: the wheel steps between images, Ctrl+wheel zooms,
  drag pans when zoomed, double-click toggles fit and actual size.
- Clip-backed images (an image whose file is a video) play muted and loop, never with sound.
- **Dependency**: measurements and the quickstart use the Testing server (:9998), which has no
  images yet (0 images, 0 galleries on 2026-10-12). Testing must first be seeded with a set of
  images covering the edge cases: ordinary photos, a very large panorama, animated GIF/WebP/APNG, a
  clip-backed image, an unsupported file, images with performers and tags, and enough images
  (1000+) for paging and the 1000-card scroll. Production (:9999) stays read-only.
