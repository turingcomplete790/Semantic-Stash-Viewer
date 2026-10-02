# Implementation Plan: Browse Scenes and Play — P1 (browse the whole library), paged

**Branch**: `005-browse-scenes-and-play` | **Date**: 2026-10-01 (revised; first version 2026-09-29) |
**Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/005-browse-scenes-and-play/spec.md`, planned for
**User Story 1 only** (requested scope: "P1"), **revised for paged library views** (constitution
v3.3.0; spec clarifications of 2026-10-01). US2–US6 are planned in later passes.

## Summary

Scenes shows the whole library as a **paged** grid or list: 50 scenes per page by default (20 to
1000 selectable, per tab), page controls above and below the grid with first/previous/next/last
and "go to page", every web UI sort including a seeded stable random, keyboard navigation that
crosses page boundaries, and per-tab restore of page, page size, mode, and scroll position that
never resets.

What carries over from the first version (already built and tested): the scene query and sorts,
card mapping, the one-request page fetch, the 480 px thumbnail pipeline (`turbojpeg`, 7.8 ms per
4K screenshot) behind `ssv-thumb://`, blob entries in 003's cache, and the `scenes_page` /
`scene_sorts` commands. What changes: the page size becomes a parameter (no longer a fixed 120),
the virtual grid and velocity-aware loading are replaced by a plain CSS grid per page (measured
at 60 fps even at 1000 cards), the neighbouring pages and their thumbnails are prefetched, and
the shell's navigation stops creating fresh history entries when returning to a view, which
caused the demo's resets.

## Technical Context

**Language/Version**: Rust 1.80 MSRV (edition 2021); TypeScript 5 / SolidJS 1.9

**Primary Dependencies**: unchanged from the first version (Tauri 2.11, tauri-specta rc.25,
reqwest, graphql_client, rusqlite via 003's cache; `turbojpeg`, `fast_image_resize`, trimmed
`image` for thumbnails, research R5). No new dependencies; no UI dependencies.

**Storage**: 003's per-server view cache: pages (JSON, keyed by query hash, page size, and page)
and thumbnails (JPEG blobs).

**Testing**: `cargo test` (stash-core: page-size validation, the page size in the cache key,
past-the-end pages, card mapping, thumbnails, request counting); Vitest + Solid Testing Library
(paged view, page controls, keyboard across pages, view-state restore, the shell's
return-to-entry fix); 003's harness for SC-001–SC-003.

**Target Platform**: Linux desktop (WebKitGTK 2.52), as before.

**Project Type**: desktop app (Rust core crate + Tauri shell + SolidJS UI).

**Performance Goals**: page 1 < 150 ms warm / < 1 s cold (SC-001); next/previous page with
thumbnails < 150 ms up to 250 per page, < 0.5 s at 500 and 1000 (SC-002, research R3); any page
< 1 s (SC-003); 60 fps within a page at every size (< 1% missed frames); one request per page
(SC-007).

**Constraints**: read-only; API key never in URLs; no media in the webview (thumbnails only); at
most three page requests per move (current page plus neighbours) and six thumbnail preparations
at once.

**Scale/Scope**: libraries up to 50,000 scenes (reference: 36,350); pages of 20–1000 cards.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Check | Status |
|---|---|---|
| I. Stash is the system of record | Pages and thumbnails live only in the discardable 003 cache | ✅ |
| II. Semantic-tagging parity | Not affected by P1 | ✅ n/a |
| III. Brain/UI separation | Query, paging, past-the-end handling, caching, and thumbnails in `stash-core`; the UI renders a page and asks for pages | ✅ |
| IV. Lean, batched GraphQL; **paged lists** | Paged grid and list, default 50, sizes 20–1000, page navigation, total count, jump to page; one card-sized query per page; neighbours prefetched; no infinite scroll | ✅ |
| V. mpv is the media player | Thumbnails as images only; nothing plays in the webview | ✅ |
| VI. Responsive UI | Plain grid measured at 60 fps up to 1000 cards (no virtualization needed); page-change budgets per R3; the harness measures them | ✅ |
| VII. User-owned connection | Thumbnails fetched with the key in a header; `ssv-thumb://` URLs carry no credentials | ✅ |
| VIII. Web UI parity | Grid, list, every sort, pagination with Stash's page sizes | ✅ |
| IX. One app shell; tabs never reset | Page, size, mode, and scroll position per history entry; returning to Scenes reuses its entry or carries its state forward (research R13) | ✅ |
| X. Galleries beyond Stash | Not part of P1; the paging and thumbnail pieces are reusable for galleries | ✅ n/a |

No violations. *Re-checked after Phase 1 design: still passes.*

## Project Structure

### Documentation (this feature)

```text
specs/005-browse-scenes-and-play/
├── spec.md
├── plan.md              # this file (P1, paged)
├── research.md          # R1–R13 (P1, paged)
├── data-model.md
├── quickstart.md
├── contracts/
│   └── scenes-browse.md
├── checklists/
│   └── requirements.md
└── tasks.md             # /speckit-tasks (to be regenerated)
```

### Source Code (repository root)

```text
crates/stash-core/
├── src/scenes/paging.rs                  # page sizes (20…1000, default 50), page count, last page (R1)
├── src/adapter/scenes.rs                 # find_scenes_page(query, page, size); past-the-end → last page (R1)
├── src/scenes/query.rs, thumbs.rs, cache # kept (R2, R5, R12)
└── tests/                                # scenes_paging.rs, scenes_page_requests.rs, scenes_cards.rs updated

src-tauri/src/
├── scenes_commands.rs                    # scenes_page(query, page, pageSize) with the new cache key (R7)
└── thumb_scheme.rs                       # kept

ui/src/
├── scenes/
│   ├── SceneGrid.tsx                     # NEW: a page of cards as a plain CSS grid or list (R3); roving focus (R8)
│   ├── SceneCard.tsx                     # kept; loading="lazy", no deferred-thumbnail logic (R3)
│   ├── PageControls.tsx                  # NEW: first/prev/next/last, "Page 25 of 727 · 1,201–1,250 of 36,350", go to page, page size (FR-004)
│   ├── pages.ts                          # REWORKED: current page + neighbours, thumbnail warm-up, refresh on view-data-changed (R4, R7)
│   ├── keyboard.ts                       # reworked for page crossing and [ / ] (R8)
│   ├── SortMenu.tsx                      # kept
│   ├── VirtualGrid.tsx, PositionBar.tsx  # REMOVED (replaced by SceneGrid, PageControls)
│   └── scenes.css
├── views/ScenesView.tsx                  # REWORKED: paged view; view state {query, page, pageSize, mode, scroll} (R6)
├── shell/tabs.ts                         # navigation carries a route's state forward (R13)
├── App.tsx                               # onLeave goes back when the previous entry is the fallback (R13)
├── debug/bench.ts                        # scenes-page-change, scenes-page-jump, scenes-scroll-1000 (R10)
└── __tests__/                            # scenes/, views/ScenesView, shell/tabs (return-to-entry)

src-tauri/src/bin/perf_harness.rs         # map the new bench lines (R10)
```

**Structure Decision**: the existing three-part layout. The core changes are small (a page-size
parameter and past-the-end handling); the UI swaps the virtual grid for a paged grid; the shell
gets the navigation fix so every section benefits.

## Phase 0: Research

See [research.md](research.md): R1 a page per request with sizes 20–1000 and past-the-end
handling, R2 sorts *(kept)*, R3 plain CSS grid per page (measured; no virtualization), R4
neighbour prefetch with thumbnail warm-up, R5 thumbnails *(kept)*, R6 per-entry view state, R7
cache keys with the page size, R8 keyboard across pages, R9 the spike picker's retirement
*(kept)*, R10 harness measurements, R11 mpv *(kept)*, R12 SQLite for blobs *(kept)*, R13 the
navigation fix for the demo's resets. No open questions remain.

## Phase 1: Design

- **Data model**: [data-model.md](data-model.md)
- **Contracts**: [contracts/scenes-browse.md](contracts/scenes-browse.md)
- **Validation**: [quickstart.md](quickstart.md): V1–V8

## Notes

- **SC-002 budgets**: 150 ms per page change up to 250 per page (build ≤ 104 ms measured); 0.5 s
  at 500 and 1000, because building 1000 cards takes ≈ 390 ms in WebKitGTK (research R3). The
  spec's SC-002 was updated to match.
- **The first version's code**: the core work (T001–T009) stays; the commands and scheme stay
  with a page-size parameter; the virtual grid, position bar, velocity-aware loading, and their
  tests are replaced. All of it is still uncommitted.

## Risks

| Risk | Mitigation |
|---|---|
| Thumbnail warm-up for neighbours competes with the current page's thumbnails | Warm-up starts only after the current page's cards render; the core prepares at most 6 at a time and serves cached ones instantly |
| 1000-card pages take ≈ 390 ms to build | Budgeted separately (Notes); `loading="lazy"` keeps off-screen thumbnails from loading |
| Restoring the scroll offset before the page has its full height | Restore after the page's cards render (the height follows from the card count and columns) |
| The navigation fix changes shell behaviour other views rely on | Only "leave" and same-route navigation change; tests cover Home, Settings, and scene flows |

## Complexity Tracking

No constitution violations to justify.
