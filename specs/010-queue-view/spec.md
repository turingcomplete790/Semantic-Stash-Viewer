# Feature Specification: Queue View

**Feature Branch**: `010-queue-view`

**Created**: 2026-10-11

**Status**: Draft

**Input**: User description: "queue view." (from the roadmap's Phase 1 Scenes item, suggested
2026-10-10: the list on one side and the selected scene's player and details on the other, so a run
of scenes can be clicked through without leaving the list; it might also serve as an alternative
gallery view)

## Overview

Scenes can be browsed as a grid, a list, or a wall (008). Watching a run of scenes still means
opening each one, playing it, and going back. The **queue view** keeps the list and the scene
together: the current page of scenes as a list down one side, and the selected scene beside it,
with its player and its details. Choosing another scene in the list shows and plays it in place,
and when one finishes the next one plays.

The queue is the Scenes tab's own results: its search, filters, sort, and pages decide what's in
it and in what order. Nothing new is stored in Stash.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Click through scenes beside the list (Priority: P1)

In a Scenes tab, the user switches the display to **Queue**. The page's scenes are listed down the
left; the right panel shows the selected scene: its cover and Play, then its details as on the
scene page. Choosing another scene in the list shows it in the panel at once, without leaving the
list or losing its place.

**Why this priority**: It's the view itself: the list and a scene side by side, which is the whole
point of the request.

**Independent Test**: Switch a Scenes tab to Queue, choose several scenes one after another: each
shows in the panel with its cover, title, and details, the list keeps its scroll position and
highlights the chosen scene, and nothing else in the tab changes.

**Acceptance Scenarios**:

1. **Given** a Scenes tab in Grid, List, or Wall, **When** the user chooses Queue, **Then** the
   page's scenes show as a compact list on the left and the first scene is selected in the panel on
   the right.
2. **Given** the queue view, **When** the user chooses a scene in the list, **Then** the panel shows
   its cover, title, and Play at once (from what the list already holds), and its details (as on
   the scene page) as soon as they load.
3. **Given** the user moves through the list quickly with the keyboard, **When** the selection
   passes over scenes, **Then** each shows its cover and title as it's passed, but details load
   only for the scene the selection rests on, never for every scene passed.
4. **Given** the queue view, **When** the user changes the search, a filter, the sort, or the page,
   **Then** the list shows the new results and the first scene is selected.
5. **Given** the queue view, **When** the tab is restored after switching tabs or restarting,
   **Then** it comes back in Queue with the same results, list position, and selected scene.

---

### User Story 2 - Play a run of scenes in the panel (Priority: P1)

The user presses Play in the panel: the scene plays there, beside the list, with the usual
controls. When it finishes, the next scene in the list plays. The user can skip to the next or
previous scene at any time, or choose any scene in the list to play it now.

**Why this priority**: Watching through a run without going back and forth is what makes it a
queue rather than a list with a preview.

**Independent Test**: Play the first scene in the panel, let it end: the second plays and becomes
the selection. Skip forward and back with the keys: the matching scenes play. Choose a scene
further down: it plays at once.

**Acceptance Scenarios**:

1. **Given** a selected scene, **When** the user presses Play, **Then** it plays inside the panel
   with the player's usual controls, resume point, chapters, and play history (008), and the list
   stays usable beside it.
2. **Given** a scene playing in the panel, **When** it reaches its end and "Play next" is on,
   **Then** the next scene in the list starts from its resume point (or the beginning), becomes the
   selection, and the list scrolls to keep it in view.
3. **Given** the last scene on the page has ended, **When** there's a next page, **Then** the next
   page loads and its first scene plays; on the last page, playback stops and the panel offers to
   replay.
4. **Given** a scene playing, **When** the user presses Next or Previous, **Then** the next or
   previous scene in the list plays (crossing pages as in scenario 3).
5. **Given** a scene playing, **When** the user moves the selection to another scene without
   choosing to play it, **Then** the playing scene keeps playing, the panel shows the selected
   scene's details with Play, and the list marks which scene is playing.
6. **Given** "Play next" is off, **When** a scene ends, **Then** it stops on its last frame with
   Replay and Play next, as the player does elsewhere.

---

### User Story 3 - Work the queue from the keyboard (Priority: P2)

Everything in the queue view works without a pointer: move through the list, play the selection,
skip next and previous, toggle "Play next", move focus between the list and the panel, and open the
selected scene's full page.

**Why this priority**: The constitution requires full keyboard navigation (Principle VI), and a
queue is something people drive from the keyboard, but it builds on the two stories above.

**Independent Test**: With the pointer untouched, select scenes, play one, skip forward and back,
toggle "Play next", move into the panel and back, and open a scene's full page.

**Acceptance Scenarios**:

1. **Given** the queue view with the list focused, **When** the user presses ↑ or ↓, **Then** the
   selection moves and the panel follows; Enter plays the selection.
2. **Given** a scene playing, **When** the user presses the next or previous scene key, **Then**
   that scene plays; the player's own keys (pause, seek, volume, chapters) work as elsewhere.
3. **Given** the queue view, **When** the user presses the key to switch focus, **Then** focus moves
   between the list and the panel; the key to open the full page opens the selected scene's page
   in the tab (or a new tab with Ctrl).
4. **Given** the keyboard help, **When** it opens in the queue view, **Then** it lists the queue's
   keys.

---

### Edge Cases

- **An empty result** (no scenes match): the queue view shows the same empty state as the other
  modes, with Clear filters, and an empty panel.
- **A scene that can't play** (the 008 fallback): the panel shows the error and the transcode
  offer; with "Play next" on, the queue doesn't move on by itself, so the user sees why.
- **A scene deleted on the server** while in the list: choosing it shows "This scene no longer
  exists"; Play next skips it.
- **The window is narrow**: the list keeps a usable minimum width and the panel takes the rest;
  when even that's too tight, the panel drops the details first and keeps the player.
- **Fullscreen**: the player can go fullscreen from the panel; Play next continues in fullscreen,
  and leaving fullscreen returns to the queue view.
- **Another tab starts playback** while the queue plays: there is one player for the app (007), so
  the queue's playback stops and the list's playing mark clears.
- **Switching tabs while the queue plays**: as elsewhere, playback continues and the now-playing
  bar shows in other tabs; coming back shows it in the panel again.
- **The page changes while a scene plays** (the user pages or filters): the playing scene keeps
  playing; Play next continues from the new list's first scene.

## Requirements *(mandatory)*

### Functional Requirements

**The view (US1)**

- **FR-001**: Scenes MUST offer a **Queue** display mode alongside Grid, List, and Wall; like the
  others, it's saved with the tab.
- **FR-002**: The queue view MUST show the tab's current page of scenes as a compact list (a small
  still, title, and details line) on one side and a scene panel on the other.
- **FR-003**: The scene panel MUST show the selected scene: its cover, title, and Play at once from
  what the list already holds, then its details as on the scene page (performers, tags, studio,
  groups, markers, files, galleries, with chips and chip cards as in 008).
- **FR-004**: Choosing a scene in the list MUST select it without leaving the view, keep the list's
  scroll position, and highlight the selected scene.
- **FR-005**: Details MUST load only for the scene the selection rests on (as chip cards do in
  008), and each scene's details at most once per visit, from the cache when they're there.
- **FR-006**: Changing the search, a filter, the sort, the page size, or the page MUST refresh the
  list and select its first scene; the tab's results are the queue, in their order.
- **FR-007**: Restoring the tab MUST bring back the queue view with its results, list position, and
  selected scene.

**Playing (US2)**

- **FR-008**: Play MUST play the selected scene inside the panel, using the app's one player with
  everything it does elsewhere: resume and "Start from the beginning", chapters, seek-bar
  thumbnails, play history, the transcode fallback, and fullscreen.
- **FR-009**: The queue view MUST offer **Play next**, on by default and saved with the tab: when a
  scene ends, the next scene in the list plays and becomes the selection, and the list scrolls to
  keep it in view.
- **FR-010**: Play next and the Next and Previous controls MUST cross page boundaries: after the
  page's last scene, the next page loads and its first scene plays; before the first, the previous
  page's last. On the last page's last scene, playback stops with Replay.
- **FR-011**: Moving the selection while a scene plays MUST NOT stop it; the list MUST mark the
  playing scene, and the panel shows the selected scene with Play.
- **FR-012**: A scene that fails to play MUST NOT be skipped automatically; the panel shows the
  error as the player does elsewhere.

**Keyboard (US3)**

- **FR-013**: The queue view MUST be fully keyboard-operable: move the selection, play it, next and
  previous scene, toggle Play next, switch focus between list and panel, and open the selected
  scene's full page (Ctrl for a new tab); the keys MUST appear in keyboard help and Settings →
  Keyboard, without clashing with the player's keys.

**Budgets (all stories)**

- **FR-014**: The queue view MUST use the same requests as the rest of Scenes: one request per page
  shown (with the neighbour prefetch), one details request per scene rested on, and nothing per row
  shown or passed over (contracts in 008's query budget).

### Key Entities *(include if feature involves data)*

- **Queue**: the Scenes tab's current results in order (search, filters, sort, page); not a
  separate list, not stored in Stash.
- **Selection**: the scene the panel shows; saved with the tab.
- **Playing scene**: the scene the panel's player is playing, which may differ from the selection.
- **Play next**: whether the queue advances when a scene ends; saved with the tab.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Choosing a scene in the list shows its cover, title, and Play in the panel in under
  150 ms (the navigation budget), and its details in under 1 second on a LAN server, or at once from
  the cache.
- **SC-002**: When a scene ends with Play next on, the next one starts playing in under 1.5 seconds
  (the playback open budget), across a page boundary included.
- **SC-003**: Moving the selection through 50 scenes with the keyboard sends no details request for
  scenes only passed over, and the list scrolls within the constitution's scrolling budget.
- **SC-004**: A user can watch ten scenes in a row from one Scenes tab without leaving the queue
  view or using the pointer.
- **SC-005**: Every budget measured in 007 and 008 still passes, and the harness covers the queue
  view's new rows (selection to panel, Play next to next scene playing).

## Assumptions

- **The queue is the results, not a curated list**: the request was to click through scenes in the
  list, so the queue is the tab's current results in their order. Adding, removing, and reordering
  scenes in a hand-made queue (or saving one) is out of scope, and could be a later feature.
- **Play next defaults to on** in the queue view, since advancing is what makes it a queue; it's a
  per-tab switch so a user can turn it off.
- **Layout**: the list takes about a third of the width and the panel the rest, with minimums so
  neither becomes unusable; dragging the divider is the roadmap's separate "adjustable panel sizes"
  item.
- **Pages**: the queue follows the tab's pages (008's paging, no infinite scroll, constitution
  Principle IV); crossing a page boundary loads that page as paging does now.
- **One player**: the app keeps one player (007, 008); playing in the panel is that player shown in
  the panel instead of the scene page.
- **Out of scope**: a queue-style gallery view (the roadmap lists it under Galleries; galleries
  don't exist yet), shuffle or repeat modes beyond Stash's random sort, and a curated queue.
