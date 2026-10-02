# Quickstart & Validation: Browse Scenes and Play — P1 (paged)

**Feature**: [spec.md](spec.md) | **Contracts**: [contracts/](contracts/) |
**Model**: [data-model.md](data-model.md) | **Research**: [research.md](research.md)

## Prerequisites

| Need | Check |
|---|---|
| Everything from 001–004 and 003's harness | see their quickstarts |
| A large library, read-only | the Production profile (36,350 scenes on 2026-09-29) |
| The UI dev server and a debug build | `cargo tauri dev` from the repo root |

P1 writes nothing to Stash, so the production library is safe to use.

## Automated checks

```bash
cargo test -p stash-core   # page sizes (20…1000, default 50; others rejected), the page size in
                           # the cache key, a page past the end returns the last page, card
                           # mapping, thumbnails, one request per page shown
npm --prefix ui test       # paged grid and list; page controls (first/prev/next/last, go to page,
                           # page size); neighbours prefetched; keyboard across pages and [ / ];
                           # view state {query, page, pageSize, mode, scroll} saved and restored;
                           # returning from a scene / via the nav bar keeps page, mode, and scroll
```

## Manual validation

### V1: First page (SC-001)
1. Open Scenes. **Expect**: page 1 of 50 cards with thumbnails, the total count, and page
   controls above and below, within 150 ms with a warm cache.
2. Clear the cache (Settings → Troubleshooting) and reopen. **Expect**: page 1 within 1 s.

### V2: Moving between pages (SC-002)
1. Press next a few times (button and `]`), then previous (`[`). **Expect**: each page appears
   at once with its thumbnails, scrolled to the top; no black or grey thumbnails.

### V3: Jumping (SC-003)
1. Go to the last page; type a page number in "Go to page"; type a number past the end.
   **Expect**: each lands within 1 s; past the end shows the last page.

### V4: Page size (FR-002)
1. Scroll to the middle of page 10, then switch to 120 per page. **Expect**: the page that holds
   the first scene you were looking at. Try 1000: scrolling the page stays smooth.
2. Open a new tab with Scenes. **Expect**: 50 per page there.

### V5: Sort (FR-003)
1. Try Duration, Play Count, Title, and Random, in both directions. **Expect**: each goes to page
   1. With Random, go to page 5 and back: the same order until Reshuffle.

### V6: Never resets (SC-009, the demo's bugs)
1. On page 37 in **list** mode, scroll half way down, open a scene, then press Done (or ✕, or
   Escape). **Expect**: page 37, list mode, same scroll position.
2. Same, but go Home with the navigation bar and come back with Scenes. **Expect**: the same.
3. Same with Back, with switching tabs, and after restarting the app. **Expect**: the same.
4. Repeat 1–3 in grid mode.

### V7: Keyboard only (FR-006)
1. Tab into the grid; arrow to the last card and press → once more. **Expect**: the next page,
   focus on its first card. `[` / `]` change page; Home/End go to the first/last card; Enter
   opens; Ctrl+Enter opens a new tab.

### V8: Harness (SC-001–SC-003)
```bash
cargo run --bin perf-harness -- --profile "Production"
```
**Expect**: `nav-first-paint` < 150 ms, `scenes-page-change` < 150 ms, `scenes-page-jump` < 1 s,
`scenes-scroll-1000` < 1% missed frames in grid and list, and every 003 budget still passing.
