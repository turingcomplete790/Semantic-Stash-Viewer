# Feature Specification: Browse Scenes and Play

**Feature Branch**: `005-browse-scenes-and-play`

**Created**: 2026-09-29

**Status**: Draft

**Input**: User description: "Phase 1"
(The roadmap's Phase 1 is split into five suggested specs. `app-shell` is done (004). This is the
next one, `browse-scenes-and-play`: the roadmap's Phase 1 "Scenes" section.)

## Overview

Today the Scenes section is a stand-in from the playback spike: a "recently added" list, a
box to open a scene by its ID, and a random set of hard-to-play test files. It proves playback,
but nobody can find a scene in a real library with it.

This feature turns Scenes into the real thing, and makes it faster than the Stash web UI:

- **Browse**: the whole scene library as a grid or list, a page at a time (50 by default), with
  instant moves between pages through tens of thousands of scenes, sorted the ways the web UI
  sorts.
- **Find**: search, the web UI's filters, and the saved filters already stored in Stash.
- **Scene page**: everything about one scene (details, performers, studio, tags, groups,
  galleries, markers, files) with Play.
- **Play like the web UI remembers**: resume where you left off, markers as chapters, and play
  history (play count, time watched, resume point) saved back to Stash.
- **Previews**: moving previews on cards and thumbnails on the seek bar.
- **A fallback** for the rare file mpv can't play: an explicit, visible, logged transcoded
  stream.

It is read-only except for play history, which the constitution requires to sync back to Stash
(Principles I and V). All editing arrives in Phase 2.

### Web UI capabilities covered (Principle VIII)

| Stash web UI capability | Covered here |
|---|---|
| Scenes list: grid, list, and wall display modes | Grid and list (US1); wall (US5, with previews) |
| Sorting (every scene sort option, ascending/descending, random) | US1 |
| Pagination and page size | US1: pages with navigation, total count, and page sizes 20–1000 (default 50) (FR-002, FR-004) |
| Search box | US2 |
| Filter criteria for scenes | US2: every criterion and modifier (FR-010) |
| Saved filters: loading and applying; the default filter | US2 (saving and deleting filters: Phase 2) |
| Scene details page: details, performers, studio, tags, groups, galleries, markers, file info | US3 |
| Playback: resume point, play count, play duration, markers on the timeline | US4 |
| "Always start from beginning" / resume behaviour setting | US4 |
| Scene previews on hover, wall previews, scrubber sprite thumbnails | US5 (animated image previews; the webview plays no video, Principle V) |
| Transcoded playback when the browser can't play a file | US6 (explicit fallback only) |
| Tagger view, bulk edit, scene edit, O-counter, rating edits, marker creation | Not here: Phase 2 (editing) and Phase 3 (tagger) |

## Clarifications

### Session 2026-09-29

- Q: Should saving play history to Stash be on by default, off by default, or follow the
  server's own "track activity" setting? → A: On by default, with a viewer-only switch in
  Settings → Playback, independent of the web UI's setting.
- Q: Should this feature cover every scene filter criterion the web UI has, or the most-used
  ones first? → A: All of them now (the full list in the Stash web UI source, about 50,
  including custom fields). The semantic-tagging plugin's existing work on building Stash
  scene filters is prior art for the plan.

### Session 2026-09-30 (constitution v3.2.0)

- mpv is the only media player (Principle V): the webview plays no video, previews included.
  Hover and wall previews therefore use Stash's **animated WebP previews** shown as images, with
  the still thumbnail as the fallback (FR-025, FR-026).
- Galleries will mix images and scenes in one mpv viewer (Principle X, a later spec). Play
  history applies to scenes only, and switching the player from a scene to an image counts as a
  scene change for saving history (FR-020).

### Session 2026-10-01 (constitution v3.3.0: paged library views)

- Scenes is a **paged** grid and list (constitution Principle IV), not continuous scrolling:
  one page at a time, page navigation, the total count, and a page size defaulting to 50. Each
  tab keeps its page, page size, display mode, and scroll position across back/forward, so
  leaving and returning never resets it (Principle IX).
- Q: Which page sizes should the page-size menu offer, alongside the default of 50? → A: 20, 40,
  50, 60, 120, 250, 500, 1000 (Stash's sizes plus 50).
- Q: When the page size changes, should it apply only to that tab or become the default for all
  scene lists? → A: Per tab only; every new tab starts at 50.
- Q: Which keys should move to the previous and next page? → A: `[` / `]` for previous/next
  page; arrowing past the last card of a page goes to the next page, and past the first card to
  the previous page; Home/End go to the first/last card on the page. Plus a way to jump straight
  to a page by number.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Browse the whole scene library (Priority: P1)

A user opens Scenes and sees their library as a grid of cards (thumbnail, title, duration,
resolution, studio, date), or as a compact list, one page at a time (50 by default). They move
between pages instantly, jump to any page, choose how many scenes a page shows, and choose how
it's sorted. Choosing a card opens that scene, and coming back finds the page exactly as it was.

**Why this priority**: browsing is the reason the section exists, and every other story builds
on it. On its own it already replaces the spike's picker.

**Independent Test**: on a library with at least 30,000 scenes, open Scenes, page through it in
grid and list modes, jump to a far page, change the page size and the sort, open a scene and go
back.

**Acceptance Scenarios**:

1. **Given** a connected server, **When** the user opens Scenes, **Then** page 1 (50 scenes)
   appears within the navigation budget, sorted by the library's default (newest first unless a
   default filter says otherwise), with the total count and the page controls.
2. **Given** a page, **When** the user goes to the next or previous page (button or `[` / `]`),
   **Then** that page appears within the navigation budget with its thumbnails, scrolled to its
   top, because adjacent pages were loaded ahead.
3. **Given** a library of 30,000+ scenes, **When** the user jumps to a page by number, or to the
   first or last page, **Then** that page appears directly without loading the ones in between.
4. **Given** a page, **When** the user picks another page size (20, 40, 50, 60, 120, 250, 500, or
   1000), **Then** the grid shows that many per page, staying on the page that contains the
   first scene they were looking at, and the size is kept for that tab only.
5. **Given** the grid, **When** the user switches to list mode, **Then** the same page appears
   as compact rows, and the mode is kept for the tab.
6. **Given** the grid, **When** the user picks a sort (for example "Duration", "Play count",
   "Random") and a direction, **Then** the view reorders and goes to page 1; a random order stays
   the same across pages and changes only when the user asks for a new one.
7. **Given** the grid, **When** the user presses a card (or Enter on a focused card), **Then** the
   scene page opens in the same tab; middle-click or Ctrl+click opens it in a new tab.
8. **Given** the user is on page 37 in list mode, scrolled half way down, **When** they open a
   scene and go back, or switch tabs and return, or restart the app, **Then** they're on page 37,
   in list mode, at the same scroll position, with the same sort and page size.
9. **Given** the keyboard only, **When** the user moves with the arrow keys, **Then** focus moves
   card by card (and the page scrolls to keep it visible); arrowing past the last card of a page
   goes to the next page and past the first card to the previous one; `[` / `]` change page;
   Home/End go to the first/last card on the page.

---

### User Story 2 - Find scenes: search, filters, saved filters (Priority: P2)

A user types a few words to search, narrows the list with the same filters as the Stash web UI
(performers, tags, studio, rating, resolution, duration, date, organized, and so on), or applies
one of the saved filters they already made in the web UI.

**Why this priority**: a large library is only useful if you can find things. Saved filters let
people bring their existing web UI habits with them on day one.

**Independent Test**: search for a known title, add a performer filter and a resolution filter,
check the count and results match the web UI for the same criteria, then apply a saved filter
made in the web UI and check it gives the same scenes.

**Acceptance Scenarios**:

1. **Given** Scenes, **When** the user types in the search box, **Then** results update as they
   type (after a short pause), matching the web UI's search for the same text.
2. **Given** Scenes, **When** the user adds filter criteria, **Then** each active criterion shows
   as a removable chip, the total count updates, and results match the web UI for the same
   criteria.
3. **Given** saved scene filters exist in Stash, **When** the user opens the saved filters menu,
   **Then** they're listed by name, and choosing one applies its search, criteria, sort, and
   display mode.
4. **Given** the server has a default scene filter, **When** the user opens Scenes fresh, **Then**
   that filter is applied.
5. **Given** a performer, tag, or studio shown on a scene page, **When** the user chooses it,
   **Then** Scenes opens filtered to that performer, tag, or studio.
6. **Given** filters that match nothing, **When** results load, **Then** the view says no scenes
   match and offers to clear the filters.
7. **Given** active filters in a tab, **When** the user switches tabs and back, or restarts the
   app, **Then** the tab's search, filters, sort, page, page size, and scroll position are
   restored.

---

### User Story 3 - The scene page (Priority: P3)

A user opens a scene and sees everything about it before (or while) playing: title, date,
details, studio, performers, tags, groups, galleries, markers, rating, play count, organized
flag, and file information (resolution, codec, size, duration, frame rate, bit rate, path).

**Why this priority**: people decide what to watch from the details, and the page is where
playback starts. It depends on US1 to reach it, but not on US2.

**Independent Test**: open scenes with many performers and tags, with markers, with several files,
and with almost no metadata; check every field shown matches the web UI's scene page.

**Acceptance Scenarios**:

1. **Given** a scene, **When** its page opens, **Then** the title, cover image, and Play appear
   within the navigation budget; the remaining details fill in without shifting the layout.
2. **Given** a scene with performers, tags, studio, groups, and galleries, **When** the page
   shows, **Then** each appears as a chip or small card; performers, tags, and studios open
   filtered Scenes (US2), and the rest open once their own features exist.
3. **Given** a scene with markers, **When** the page shows, **Then** the markers are listed with
   their times and titles, and choosing one starts playback at that point.
4. **Given** a scene with more than one file, **When** the page shows, **Then** every file is
   listed with its details, and the primary file is the one that plays.
5. **Given** a scene with no title, **When** its page or card shows, **Then** it's named by its
   file name, as in the web UI.

---

### User Story 4 - Play with resume, chapters, and play history (Priority: P4)

A user plays a scene. If they watched part of it before, it can pick up where they left off.
Markers appear as chapters on the seek bar and can be jumped between. When they stop, their
resume point, play count, and time watched are saved to Stash, so the web UI and other devices
see the same history.

**Why this priority**: resume and play history are what make a library client feel like a
real player (Jellyfin-style), and the constitution requires the sync. Playback itself already
works (002); this story adds what depends on Stash data.

**Independent Test** (test instance): play a scene for a minute and stop; check in the Stash web
UI that the resume point, play duration, and (if the threshold was reached) play count changed.
Reopen it and check it resumes there. Jump between markers with the keyboard.

**Acceptance Scenarios**:

1. **Given** a scene with a saved resume point and "resume where you left off" on, **When** the
   user presses Play, **Then** playback starts at the resume point, with a way to start over.
2. **Given** that setting off, **When** the user presses Play, **Then** playback starts at the
   beginning.
3. **Given** a scene with markers, **When** it plays, **Then** the markers show on the seek bar
   with their titles, and next/previous chapter keys jump between them.
4. **Given** playback, **When** the user stops, closes the tab, switches scene, or quits,
   **Then** the resume point and the time watched are saved to Stash.
5. **Given** the user has watched more than the play-count threshold, **When** they stop,
   **Then** the play count increases by one (once per viewing).
6. **Given** the user watched to (nearly) the end, **When** playback ends, **Then** the resume
   point is cleared, so the next play starts at the beginning.
7. **Given** play history can't be saved (server unreachable), **When** the connection returns
   in the same session, **Then** the history is saved then; if it still can't be saved, the
   notification centre says so.
8. **Given** a fresh install, **When** the user plays scenes, **Then** play history is saved
   (the option is on by default).
9. **Given** the user turned off "Save play history" in Settings → Playback, **When** they play
   scenes, **Then** nothing is written to Stash; the web UI's own setting is unaffected.

---

### User Story 5 - Previews and seek-bar thumbnails (Priority: P5)

Hovering (or focusing) a card plays its short preview clip, the wall mode shows many moving
previews at once, and hovering the seek bar shows a thumbnail of that moment.

**Why this priority**: it makes browsing and seeking much quicker, but everything works without
it.

**Independent Test**: hover cards and check previews play and stop promptly; open wall mode and
scroll; hover the seek bar of a playing scene and check the thumbnail matches the moment.

**Acceptance Scenarios**:

1. **Given** a card whose scene has a generated animated preview, **When** the user hovers or
   focuses it for a moment, **Then** the animated preview replaces the still thumbnail in place,
   and the still returns when the pointer leaves.
2. **Given** wall mode, **When** the user scrolls, **Then** visible scenes show their animated
   previews and scenes scrolled away go back to stills, keeping scrolling smooth.
3. **Given** a playing scene with generated sprites, **When** the user hovers the seek bar,
   **Then** a thumbnail of that moment shows above it with the time.
4. **Given** a scene without generated previews or sprites, **When** the user hovers, **Then**
   the still thumbnail stays and nothing breaks.

---

### User Story 6 - A visible fallback when a file can't play (Priority: P6)

For the rare file mpv can't decode, the viewer says so and offers to play it through Stash's
transcoded stream instead, clearly marked as a fallback.

**Why this priority**: the 002 spike played every format tried, so this is rarely needed, but
the constitution requires the fallback to exist and be explicit.

**Independent Test**: point a scene at a file mpv can't play (test instance); check the error
offers the fallback, that choosing it plays the transcoded stream with a visible "transcoded"
marker, and that the log records it.

**Acceptance Scenarios**:

1. **Given** a scene mpv can't play, **When** playback fails, **Then** the error explains it and
   offers "Play transcoded".
2. **Given** the user chooses "Play transcoded", **When** it plays, **Then** the player shows it's
   a transcoded stream, and the fallback is logged with the scene and the reason.
3. **Given** a normal scene, **When** it plays, **Then** it never uses a transcoded stream.

---

### Edge Cases

- **Empty library, or a filter with no matches**: a clear empty state with a way back (clear
  filters), never a blank grid.
- **Scenes without thumbnails, titles, dates, or studios**: a placeholder image; the file name
  as title; missing fields left out rather than shown blank.
- **Very long titles and many performers or tags**: truncated on cards (full text on the scene
  page and in a tooltip), without breaking the grid.
- **Offline**: pages already loaded stay available, as in 003; going to a page that isn't loaded
  shows that the server can't be reached, with the page controls still working, rather than an
  endless spinner.
- **The library changes while browsing** (scenes added or deleted elsewhere): the list
  refreshes quietly (003's refresh); a scene deleted while its page is open says it no longer
  exists.
- **Jumping far** (the last page of 30,000 scenes, or a page number typed in): the viewer loads
  that page directly rather than every page in between. A page number past the end goes to the
  last page.
- **The page no longer exists** (scenes were deleted elsewhere, or the page size grew): the view
  shows the last page that exists instead of an empty one.
- **Large page sizes** (500 or 1000 per page): scrolling within the page stays smooth.
- **Random sort**: stays stable across pages and back/forward; a new order only on request.
- **Saved filters using criteria the viewer doesn't support yet**: applied as far as possible,
  with a notice naming what was left out, rather than silently showing different results.
- **Resume point near the end** (in the last few percent): treated as finished; playback starts
  from the beginning.
- **Several files per scene**: the primary file plays; the others are listed on the scene page.
- **The same scene playing and its page open in another tab**: one player; the other tab shows
  the now-playing state (004).
- **Play history while another device plays the same scene**: the latest save wins, as in the
  web UI.

## Requirements *(mandatory)*

### Functional Requirements

**Browsing (US1)**

- **FR-001**: Scenes MUST show the library as a grid of cards and as a compact list, one page at a
  time; the mode is kept per tab. Cards show the thumbnail, title, duration, resolution, studio,
  and date.
- **FR-002**: Scenes MUST be paged (constitution Principle IV): one page of results at a time,
  fetched from the server for that page only, with page sizes of 20, 40, 50, 60, 120, 250, 500,
  and 1000 scenes and a default of **50**. The page size is kept per tab; every new tab starts at
  50. The adjacent pages (data and thumbnails) MUST be loaded ahead so moving to them is instant.
  Libraries of at least 50,000 scenes MUST work at every page size, and scrolling within even a
  1000-scene page MUST stay smooth.
- **FR-003**: Users MUST be able to sort by every scene sort option the web UI offers, ascending
  or descending, including a random order that stays stable until the user reshuffles.
- **FR-004**: The view MUST show the total number of matching scenes and where the page is (for
  example "Page 25 of 727 · 1,201–1,250 of 36,350"), with previous, next, first, and last page
  controls and a way to jump straight to a page by number. Page controls appear above the grid
  and again below it.
- **FR-005**: Opening a scene MUST follow the shell's rules: same tab by default, new tab with
  middle-click or Ctrl+click, and back returns to the same page, page size, display mode, and
  scroll position (004; constitution Principle IX).
- **FR-006**: The grid and list MUST be fully keyboard-navigable with visible focus: arrows move
  card by card, and past the last or first card of a page they move to the next or previous
  page; `[` / `]` go to the previous/next page; Home/End go to the first/last card on the page;
  Enter opens the scene (Ctrl+Enter in a new tab).
- **FR-007**: The spike's "recently added" list and ID box are replaced; the hard-to-play test
  set moves out of the user interface (it stays available to the performance harness and debug
  builds).

**Finding (US2)**

- **FR-008**: Scenes MUST have a search box whose results match the web UI's scene search for the
  same text, updating shortly after typing stops.
- **FR-009**: Active filter criteria MUST show as removable chips, with a way to clear them all,
  and the total count MUST reflect them.
- **FR-010**: Users MUST be able to filter by **every** scene filter criterion the web UI offers
  in the minimum supported Stash version (about 50: text fields such as title, code, details,
  director, URL, path, and folder; file facts such as resolution, orientation, duration, frame
  rate, bit rate, codecs, file count, hashes, and duplicates; activity such as rating, organized,
  O-counter, play count, play duration, resume time, and last played; relationships such as
  performers, performer tags, age, and favourites, tags, tag count, studios, studio tags, groups,
  galleries, markers, and StashIDs; dates and timestamps; interactive and captions; "is missing";
  and custom fields), each with the modifiers the web UI offers for it (equals, includes,
  excludes, greater/less than, between, is null, and so on).
- **FR-011**: Users MUST be able to list and apply the saved scene filters stored in Stash,
  including the default filter; applying one sets its search, criteria, sort, and display mode.
  Saving, renaming, and deleting filters are out of scope (Phase 2).
- **FR-012**: A saved filter the viewer can't apply exactly (for example, one saved by a newer
  Stash version with a criterion the viewer doesn't know) MUST say which parts were left out.
- **FR-013**: Performers, tags, and studios shown in the viewer MUST open Scenes filtered to them.
- **FR-014**: Each tab MUST keep its search, filters, sort, page, page size, display mode, and
  scroll position across tab switches, back/forward, and restarts (004's per-tab view state;
  constitution Principle IX).

**Scene page (US3)**

- **FR-015**: The scene page MUST show the title (or file name), cover, date, details, studio,
  performers, tags, groups, galleries, markers, rating, play count, organized flag, and every
  file's details (resolution, codec, size, duration, frame rate, bit rate, path).
- **FR-016**: The page MUST show the title, cover, and Play first, and fill in the rest without
  moving what's already shown.
- **FR-017**: Choosing a marker on the page MUST start playback at that marker.

**Playback (US4)**

- **FR-018**: A Settings → Playback option MUST choose whether scenes start at their saved resume
  point or at the beginning; when resuming, the player MUST offer a way to start over.
- **FR-019**: Markers MUST appear on the seek bar with their titles, with keys to jump to the next
  and previous marker.
- **FR-020**: When playback stops for any reason (stop, close, scene change or a switch to an
  image, quit, disconnect),
  the viewer MUST save the resume point and time watched to Stash; a resume point in the last few
  percent of the scene is cleared instead.
- **FR-021**: The play count MUST increase by one per viewing once the watched share passes the
  play-count threshold (the server's setting where it has one).
- **FR-022**: Play history that can't be saved MUST be retried when the connection returns in the
  same session, and reported in the notification centre if it still fails.
- **FR-023**: A "Save play history" option in Settings → Playback MUST control saving; it is on
  by default, belongs to the viewer (independent of the web UI's "track activity" setting), and
  nothing is written to Stash when it's off.
- **FR-024**: Apart from play history (FR-020–FR-023), this feature MUST NOT write to Stash.

**Previews (US5)**

- **FR-025**: Cards MUST show the scene's **animated image preview** (Stash's generated animated
  WebP) after a short hover or focus, and return to the still when the pointer or focus leaves.
  Previews are images, never video: the webview plays no media (constitution Principle V).
- **FR-026**: A wall mode MUST show scenes as a dense grid of animated image previews that animate
  while visible.
- **FR-027**: The seek bar MUST show a thumbnail of the hovered moment when the scene has
  generated sprites.
- **FR-028**: Scenes without previews or sprites MUST fall back to the still thumbnail quietly.

**Fallback (US6)**

- **FR-029**: When mpv can't play a scene, the error MUST offer "Play transcoded"; the viewer MUST
  NOT use a transcoded stream otherwise.
- **FR-030**: Transcoded playback MUST be visibly marked in the player and logged with the scene
  and the reason.

**Performance and data use**

- **FR-031**: Grid and list requests MUST fetch only the fields cards need, a page at a time;
  there MUST be no request per card (constitution Principle IV).
- **FR-032**: Scene lists, saved filters, and scene pages MUST use 003's cache: shown at once when
  cached, refreshed quietly, and never shared across servers.

### Key Entities

- **Scene card**: what a grid or list row needs: ID, title (or file name), thumbnail, duration,
  resolution, studio name, date, and preview availability.
- **Scene details**: everything on the scene page: the card's fields plus details, performers,
  tags, groups, galleries, markers, rating, play count, resume point, organized flag, and files.
- **Scene query**: what's being shown in a tab: search text, filter criteria, sort field and
  direction (plus the random seed), page, page size, display mode, and scroll position within the
  page.
- **Saved filter**: a named scene query stored in Stash, possibly the default one; read-only here.
- **Marker**: a titled point (and optional end) in a scene, with its primary tag.
- **Play history**: per scene, the resume point, total time watched, and play count, owned by
  Stash; the viewer reports each viewing.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On a library of 30,000+ scenes, opening Scenes shows page 1 (50 cards) in under
  150 ms with a warm cache, and under 1 s from a cleared cache on a LAN server.
- **SC-002**: Moving to the next or previous page shows it, thumbnails included, in under 150 ms
  at page sizes up to 250, and under 0.5 s at 500 and 1000 (the adjacent pages are loaded
  ahead); scrolling within a page stays at 60 fps (under 1% missed frames, as the harness
  measures it) at every page size up to 1000.
- **SC-003**: Jumping to any page by number shows its cards in under 1 s.
- **SC-004**: For the same search, criteria, and sort, the viewer's count and first 100 results
  match the web UI's exactly (checked on 10 varied queries, including 3 saved filters).
- **SC-005**: A scene page shows its title, cover, and Play in under 150 ms from the grid.
- **SC-006**: After watching part of a scene and stopping, the web UI shows the updated resume
  point and time watched within 5 seconds; reopening the scene resumes within 2 seconds of the
  saved point.
- **SC-007**: Showing a page of cards never makes more than one request for the cards' data,
  whatever the page size.
- **SC-008**: A user who knows the web UI can find a named scene by search, and by a
  performer-plus-tag filter, each in under 30 seconds on first use.
- **SC-009**: Leaving Scenes (opening a scene, switching tabs, or restarting) and coming back
  always returns to the same page, page size, display mode, and scroll position, in 10 of 10
  tries in each of grid and list mode.

## Assumptions

- **Server and testing**: Stash v0.31.1 or newer, as in 001. Anything that writes play history is
  tested only against the disposable test instance; the user's production library is used
  read-only (no play history writes during testing).
- **Previews and sprites** are whatever Stash has already generated; generating them is a
  Stash task (Phase 3). Hover and wall previews need Stash's animated WebP previews ("preview
  image" generation); scenes with only MP4 previews show their still thumbnail, since the webview
  plays no video (Principle V).
- **Transcoding** uses Stash's own transcoded stream endpoints; nothing new is needed on the
  server.
- **Play-count threshold** follows the server's setting where it has one, otherwise a sensible
  default (count after 20% watched); resume points in the last 5% are cleared. "Save play
  history" is on by default (Clarifications).
- **Filters** are Stash's scene filter criteria, applied by the server; the viewer doesn't
  re-implement filtering locally. The criteria list and each one's modifiers come from the Stash
  web UI source for the minimum supported version (`ui/v2.5/src/models/list-filter/`, in the
  local Stash checkout at `/home/bbulder/stash-source/stash`), and from the semantic-tagging
  plugin's existing scene-filter building (`Stash-Plugins/semantic-tagging/src/stash/adapter.js`).
- **Other entity pages** (performers, studios, tags, groups, galleries) are later Phase 1 specs;
  until then their chips open filtered Scenes where possible, and otherwise do nothing.
- **Home page rows and global search** are the `home-and-global-search` spec; the Home page is
  unchanged here.
- **Out of scope**: all editing (scene metadata, ratings, O-counter, markers, organized flag,
  bulk edit), saving or deleting saved filters, the tagger, and generating previews or sprites.
