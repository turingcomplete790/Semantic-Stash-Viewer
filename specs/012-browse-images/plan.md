# Implementation Plan: Browse Images

**Branch**: `012-browse-images` | **Date**: 2026-10-12 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/012-browse-images/spec.md`

## Summary

Add the Images section: a paged grid and wall of sized thumbnails with the web UI's image search,
every image filter and sort, and saved filters (US1); a viewer that shows the opened image in the
embedded mpv player at full resolution, with next/previous across pages in under 150 ms, zoom, pan,
rotation, and fit/actual size, all done by mpv (US2, Principle V); a slideshow, animated and
clip-backed images looping, animated card previews decoded as image frames (US3); and a read-only
details panel (US4).

The approach ([research](research.md)): Scenes' list machinery (query, criteria, saved filters,
paging, grid, filter panels, thumbnails, tab saving) becomes an entity-generic **browse** layer
that Scenes and Images both instantiate, so Images gets every proven behaviour without a copy; the
core gains Images' criteria, sorts, and GraphQL operations, checked against the pinned schema; the
player gains an **image mode** (stills held, animated files looping, zoom/pan/rotate/unscaled as mpv
properties); and the core keeps the images either side of the one shown on local disk, so stepping
opens a local file (R4). Testing (:9998) has no images, so a seeding step comes first (R8).

## Technical Context

**Language/Version**: Rust 1.99 (workspace).

**Primary Dependencies**: iced 0.14 (`native-ui`), libmpv through the `player` crate (render API,
007), Cynic 3.14 against Stash v0.31.1's schema, `image` + `turbojpeg` + `fast_image_resize` for
thumbnails and animated card frames (already in the tree). No new crates.

**Storage**: the session file (version 2) gains an Images tab state (additive, R6); the view cache
(SQLite, 003) holds image pages, details, and thumbnails as for scenes; a bounded directory of
full-resolution files for the viewer's neighbours (R4), inside the cache's size limit.

**Testing**: `cargo test`: core tests for image criteria → `ImageFilterType`, sorts, saved image
filters, page requests against a mock server (as `scenes_*` tests); `native-ui/tests` for the
Images screen and viewer transitions; player tests for the image-mode commands (headless mpv);
perf-harness rows for every budget (R9).

**Target Platform**: Linux desktop (as 007–011).

**Project Type**: desktop app (workspace: `stash-core`, `player`, `native-ui`, `perf-harness`).

**Performance Goals**: SC-001/002: Scenes' page and scroll budgets; SC-003: next/previous ≤ 150 ms
(p95) with neighbours prefetched; SC-004: opening a typical photo ≤ 500 ms; SC-006: zoom, pan,
rotation within a frame; SC-007: slideshow within 0.3 s of its delay.

**Constraints**: read-only toward Stash; full-resolution files fetched with the key in a header,
only for the image shown and its neighbours; the UI never decodes full-resolution images (mpv
does); one player shared with scenes.

**Scale/Scope**: libraries of any size (paged); images up to ~100 MP (a 12000 px panorama); the
neighbour cache holds a handful of files.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | Check | Result |
|---|---|---|
| I. Stash is the system of record | Images, criteria, saved filters read from Stash; nothing kept only locally but cache | Pass |
| II. Semantic model | Tags marked and coloured as on the scene page (008); no new semantic data | Pass |
| III. Brain/UI separation | Criteria, sorts, queries, saved filters, prefetch, and the viewer's next/previous rules in `stash-core`; the UI draws and routes | Pass |
| IV. Lean, batched GraphQL; paging | One `ImagesPage` per page (0 cached), neighbour pages prefetched, `ImageDetails` only with the panel open, full files only for the shown image ±1 (±2 in a slideshow); no infinite scroll | Pass |
| V. mpv is the player | Full-resolution images, zoom, pan, rotation, fit, animated images in the embedded mpv (R3); card previews are decoded image frames, never video; clip-backed images play in mpv only | Pass |
| VI. Responsive UI; keyboard | Budgets above, measured by harness rows; full keyboard operation; I/O off the UI thread | Pass |
| VII. User-owned connection | Key in a header for every GET; files cached locally only, within the cache limit | Pass |
| VIII. Web UI parity | Every image filter, sort, display mode, saved filter; the lightbox's controls; the image page's fields (read-only; edits Phase 2) | Pass |
| IX. One app shell | An Images section in the navigation bar; tabs keep their own grid and viewer; the one player across tabs | Pass |
| X. Galleries | Not here; the grid, viewer, and details are built to serve galleries next | Pass |
| XI. Visual identity | Theme colours, Lucide icons for the viewer's controls | Pass |
| XII. Semantic engine | Not touched | Pass |

**Operation review (Principle IV)**:

| Operation | When | Requests |
|---|---|---|
| `ImagesPage` (`findImages`, cards) | a page shown; its neighbours prefetched | 1 per page, 0 cached |
| `ImageDetails` (`findImage`) | the details panel open on an image | 1 per image, 0 cached |
| Thumbnail GET `/image/<id>/thumbnail` | a card drawn | 1 per card, 0 cached |
| Full file GET `/image/<id>/image` | the image shown and its neighbours | 1 per image viewed or prefetched, 0 cached |
| Saved filters (`findSavedFilters`, mode IMAGES), default filter | Images opened, the menu opened | as 008 for scenes |
| Choice lists (performers, tags, studios, galleries) | a picker opened | as 008 (galleries list new) |

Post-design re-check: the data model, contract, and request table keep every budget; no
violations.

## Project Structure

### Documentation (this feature)

```text
specs/012-browse-images/
├── plan.md              # This file
├── research.md          # Decisions R1–R9
├── data-model.md        # Images query and criteria, cards, details, viewer state, neighbour cache
├── quickstart.md        # Seeding Testing, validation V1–V4, harness rows
├── contracts/
│   ├── images-ui.md     # The grid and viewer: layout, keys, mouse, behaviour
│   └── player-image-mode.md  # The player crate's image mode: open options and commands
└── checklists/
    └── requirements.md
```

### Source Code (repository root)

```text
crates/stash-core/
├── graphql/operations/ImagesPage.graphql, ImageDetails.graphql   # NEW (checked by Cynic)
└── src/
    ├── browse/            # NEW: entity-generic list layer moved out of scenes/ (R1)
    │   ├── query.rs       #   ListQuery<S> (SceneQuery = ListQuery<SceneSort>)
    │   ├── paging.rs      #   moved from scenes/paging.rs
    │   └── entity.rs      #   Entity { Scenes, Images }: criteria, sorts, filter mode
    ├── scenes/filter/     # kinds.rs gains entities per kind; to_stash.rs gains image_filter()
    ├── images/            # NEW: ImageSort, ImageCard, ImagePage, ImageDetails, neighbours (R4)
    ├── adapter/images.rs  # NEW: find_images_page, image_details, image bytes
    └── thumbs.rs          # `image` thumbnails (the scheme already carries the kind)
crates/player/src/
├── session.rs             # OpenRequest.kind: Video | Image { looped }; image options (R3)
└── commands.rs            # Zoom, Pan, Rotate, Unscaled, ResetView
native-ui/src/
├── screens/browse/        # NEW: the generic grid, pager, filter bar, panels (moved from scenes/)
├── screens/images/        # NEW: ImagesState, viewer (state, view, keys), details panel
├── screens/mod.rs         # Section::Images, Screen::Images
├── session/               # the viewer through the one player; neighbour prefetch effects
├── shell/keymap.rs        # Level::Images, Level::Viewer
└── measure/bench.rs       # images rows (R9)
crates/perf-harness/src/   # their budgets
tools/seed-testing-images/ # NEW: a script that seeds Testing (R8)
```

**Structure Decision**: the 011 workspace. The browse layer moves Scenes' entity-agnostic parts
into `browse/` (core) and `screens/browse/` (UI) behind the existing tests, then Images is a second
instance; scene-only parts (queue, playback, play history) stay in `scenes/`. The viewer is a
mode of the Images tab, so Back and Escape return to the grid (Principle IX).

## Complexity Tracking

| Choice | Why needed | Simpler alternative rejected because |
|---|---|---|
| Moving Scenes' list machinery into a generic browse layer | Images needs all of it (FR-001–FR-007), and galleries, performers, studios, and groups next | A copy of `screens/scenes` for images doubles ~4,000 lines and every later fix |
| A neighbour cache of full files on disk | SC-003 (150 ms) can't be met over the network for multi-megabyte files | Loading from Stash on each step: a 10 MB file takes longer than 150 ms on a LAN |
