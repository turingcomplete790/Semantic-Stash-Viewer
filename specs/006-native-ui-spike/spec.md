# Feature Specification: Native UI Spike (iced)

**Feature Branch**: `006-native-ui-spike`

**Created**: 2026-10-03

**Status**: Draft

**Input**: User description: "for a "native UI spike (iced)" feature. Lets give it a shot. It will
make caching and communicating with the graphql backend easier." (Follows the 2026-10-03
analysis of moving off Tauri and the Linux webview: performance problems and crashes in the
WebKitGTK stack, and a wish to keep all of the viewer in one language.)

## Overview

This is a **spike**: a time-boxed, working proof that answers one question before any more
features are built on the current web-based interface:

> Can a native interface, drawn without a web engine (the **iced** toolkit is the candidate),
> deliver the viewer's two hardest screens — a paged scenes grid with thumbnails, and in-window
> mpv playback with the viewer's controls on top — within the constitution's budgets, on the
> user's Wayland desktop?

The spike builds a second, separate build of the viewer with just enough interface to answer
that: connect with an existing server profile, browse scenes a page at a time, open a scene and
play it with full controls, and switch between two tabs while a scene plays. It reuses the
existing library core (Stash connection, GraphQL adapter, cache, paging, thumbnails) and the mpv
player as they are, calling them directly instead of through a bridge to a web page. The current
web-based build stays as it is and remains the reference: the spike's screens are judged against
its specs (003, 004, 005) and its measured numbers.

The spike ends with a written **go / no-go decision**. "Go" means the viewer's interface moves to
the native toolkit (a constitution amendment and a port feature follow). "No-go" means the web
build continues and the decision record lists what blocked the native approach.

Embedding mpv's hardware-decoded video inside the native window is the deciding risk and is
proved first; if it can't be done within the budgets on Wayland, the spike stops there with a
no-go.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Play a scene inside the native window (Priority: P1)

The user opens a scene in the native build and it plays inside the viewer's own window, drawn by
the GPU with hardware decoding, from Stash's original file. The viewer's own controls sit on top
of the moving video: play/pause, seek bar with time and hover time, skip ±10 s, speed, frame
step, volume, mute, fullscreen, and close, all also on the keyboard, hiding after 3 seconds
without movement.

**Why this priority**: This is the deciding risk. The current build embeds mpv beneath a web page
with platform-specific code; if the native toolkit can't host mpv's hardware-decoded video with
controls on top, on Wayland, within the playback budgets, nothing else in the spike matters.

**Independent Test**: Launch the native build with the user's profile, open a scene from the
recently added list, and use every control with the mouse and keyboard while the playback
measurements run.

**Acceptance Scenarios**:

1. **Given** a connected profile, **When** the user opens a scene, **Then** video and audio play
   inside the viewer window (not a separate window), from the original file, with hardware
   decoding active when the codec allows it.
2. **Given** a playing scene, **When** the user uses each control (mouse and keyboard, as listed
   in feature 002's FR-008/FR-009), **Then** each works and the controls draw cleanly over the
   video, with no flicker, tearing, or black boxes.
3. **Given** a playing scene, **When** the user resizes the window or toggles fullscreen,
   **Then** the video keeps its aspect ratio, fills its area, and shows no stale frames.
4. **Given** a scene that can't be played, **When** the user opens it, **Then** a plain-language
   error appears and the viewer stays usable.
5. **Given** a playing scene, **When** the user closes the player or quits the app, **Then**
   playback stops and audio and video are released at once, and the app exits without crashing.

---

### User Story 2 - Browse scenes a page at a time (Priority: P2)

The user opens Scenes in the native build and sees the library as a paged grid of cards
(thumbnail, title, details), with the page controls of feature 005: page position and total,
first/previous/next/last, go to page, page size (20–1000, default 50), sort, and grid or list
mode. Keyboard navigation moves card to card and page to page. Opening a card plays the scene
(User Story 1); going back returns to the same page, mode, and scroll position.

**Why this priority**: The grid is where the web build struggled (thumbnails loading during
scrolling, layout quirks in hidden views). It's the second half of the question: if playback
works but a 1000-card page can't scroll smoothly, the move isn't worth it.

**Independent Test**: Browse the user's library in both modes at page sizes 50 and 1000, change
pages, jump to a far page, and open a scene and come back, while the grid measurements run.

**Acceptance Scenarios**:

1. **Given** a connected profile, **When** the user opens Scenes, **Then** the first page of 50
   cards appears with thumbnails, the page position, and the total count matching the web build.
2. **Given** a page, **When** the user moves to the next or previous page (button or `]` / `[`),
   **Then** it appears with its thumbnails within the page-change budget.
3. **Given** the page controls, **When** the user enters a page number, **Then** that page's
   cards appear within the jump budget.
4. **Given** page size 1000 in grid or list mode, **When** the user scrolls quickly from top to
   bottom, **Then** scrolling stays smooth, including while thumbnails are still arriving.
5. **Given** the user opened a scene from page 12 in list mode, scrolled half way, **When** they
   go back, **Then** Scenes shows page 12 in list mode at the same scroll position.
6. **Given** a changed sort or page size, **When** the page reloads, **Then** it behaves as
   feature 005 specifies (page 1 for a new sort; the page containing the first visible scene for
   a new size).

---

### User Story 3 - Tabs with a scene playing (Priority: P3)

The native build has a tab strip with at least two tabs: one showing Scenes and one showing a
playing scene. Switching tabs keeps each tab's state; playback continues (or pauses, as feature
004 specifies) while the user browses in the other tab, and switching back shows the video
straight away, correctly sized and positioned.

**Why this priority**: Hiding and showing the video surface when views change is where the web
build had its worst bugs (video off-centre until a resize, images laid out at zero size in hidden
views). The shell shape (Principle IX) has to work with embedded video before the move is
decided.

**Independent Test**: Play a scene, switch to the Scenes tab, change pages, switch back 20 times
while the tab-switch measurement runs; then quit and relaunch to confirm the tabs come back.

**Acceptance Scenarios**:

1. **Given** a playing scene in one tab, **When** the user switches to Scenes and back, **Then**
   the video shows at once at the right size and position, with no resize needed.
2. **Given** two tabs with their own state, **When** the user switches between them, **Then**
   neither resets (page, mode, scroll, playback position).
3. **Given** open tabs, **When** the user quits and relaunches, **Then** the same tabs and their
   state are restored.

---

### User Story 4 - Decide with evidence (Priority: P4)

The spike ends with a short decision record: go or no-go, why, and the measurements behind it,
side by side with the web build's numbers on the same machine and library. It also covers what
the native build made simpler or harder (talking to the core and cache directly, testing,
accessibility, text input), the gaps found in the toolkit, and what a port would need.

**Why this priority**: The decision is the spike's lasting output; it's only meaningful once
playback, the grid, and tabs have been tried for real.

**Independent Test**: Read the decision record and confirm it states go or no-go, gives measured
values for every success criterion for both builds, and lists the gaps and risks.

**Acceptance Scenarios**:

1. **Given** the spike is complete, **When** the user reads the decision record, **Then** it
   states go or no-go and the reason in plain language.
2. **Given** the decision record, **When** the user looks for evidence, **Then** it lists every
   success criterion's measured value for the native build and for the web build.
3. **Given** a "go", **When** the user looks for next steps, **Then** the record lists the
   constitution changes needed and what of the current code carries over unchanged.

---

### Edge Cases

- **Display system**: the user's desktop is a Wayland session (KDE). The native build must work
  there; something that works only on X11 counts as a failure and is recorded.
- **No hardware decoding** for a codec: playback falls back to software decoding and still plays;
  the record notes it.
- **Server offline or unreachable** at start or during browsing: the grid shows its unreachable
  state, cached pages still show, and the build stays usable (as in features 003 and 005).
- **Cold thumbnails**: thumbnails not yet cached come from Stash while the user browses; the grid
  shows a placeholder until each arrives and keeps scrolling smoothly.
- **High-DPI and scaling**: the window at 1× and at a fractional scale (for example 1.25×) draws
  text, thumbnails, and video at the right size.
- **Large and high-bitrate files**: the playback test set includes 4K and high-bitrate scenes.
- **Same profile, two builds**: the native build reads the same saved server profiles as the web
  build; it must not damage them or the shared cache if both builds are used in turn.
- **Quit while thumbnails or pages are loading**: the app exits promptly and cleanly.

## Requirements *(mandatory)*

### Functional Requirements

**Scope and reuse**

- **FR-001**: The spike MUST be a separate build that leaves the current web build working and
  unchanged in behaviour.
- **FR-002**: The spike MUST reuse the existing library core (Stash connection, GraphQL adapter,
  cache, paging, sorting, thumbnails) and the existing mpv player, calling them directly; changes
  to them MUST be additive and keep their existing tests passing.
- **FR-003**: The spike MUST use the existing saved server profiles and the active profile's API
  key, with no new prompts, and MUST NOT write anything to the server: browsing and playback are
  read-only (no resume position, play count, or other sync-back in the spike).

**Playback (User Story 1)**

- **FR-004**: Video MUST be drawn inside the viewer's window by the GPU, from Stash's original
  file stream (no transcoding), with hardware decoding when available and software decoding
  otherwise.
- **FR-005**: The player MUST provide feature 002's controls (FR-008), keyboard shortcuts
  (FR-009), auto-hide after 3 seconds (FR-010), aspect-correct resizing and fullscreen (FR-011),
  drawn by the viewer on top of the video.
- **FR-006**: A scene that can't be played MUST produce a plain-language error, leaving the
  viewer usable; closing the player or quitting MUST release audio and video at once.

**Scenes grid (User Story 2)**

- **FR-007**: Scenes MUST show a paged grid and a list of scene cards with feature 005's page
  controls, page sizes (20, 40, 50, 60, 120, 250, 500, 1000; default 50), sort options, and
  past-the-end behaviour.
- **FR-008**: Thumbnails MUST be the core's prepared thumbnails, with a placeholder while one is
  loading or missing.
- **FR-009**: The keyboard MUST move between cards and pages as feature 005 specifies (arrows,
  Enter, `[` / `]`, Home, End, Ctrl+Enter for a new tab).
- **FR-010**: Each Scenes view MUST keep its page, page size, mode, sort, and scroll position
  when the user leaves and returns (feature 005 research R13).

**Tabs (User Story 3)**

- **FR-011**: The build MUST have a tab strip with at least Scenes and scene tabs; switching tabs
  MUST keep each tab's state and show embedded video immediately at the correct size.
- **FR-012**: Open tabs and their state MUST be restored on relaunch.

**Measurement and decision (User Story 4)**

- **FR-013**: The native build MUST report the same measurements as the web build's performance
  harness (cold start, navigation first paint, input acknowledgement, tab switch, page change,
  page jump, scrolling a 1000-card page in both modes, playback open, seek, dropped frames), in
  a form the harness can read, plus memory use and CPU use at idle and while playing.
- **FR-014**: The spike MUST produce a decision record stating go or no-go, the measurements of
  both builds on the same machine and library, the toolkit gaps found (for example text input,
  accessibility, menus, tooltips), and, for a "go", the constitution changes and port plan
  outline.
- **FR-015**: If in-window hardware-decoded playback with overlaid controls can't be achieved on
  Wayland within the playback criteria, the spike MUST stop and record a no-go without building
  the grid and tabs.

### Key Entities

- **Native build**: the second build of the viewer made by this spike; uses the same core,
  player, profiles, and cache as the web build.
- **Measurement report**: the numbers each build produces for the success criteria, on the same
  machine, library, and profile.
- **Decision record**: go or no-go, evidence, gaps, and next steps.

## Success Criteria *(mandatory)*

### Measurable Outcomes

All measured on the user's desktop (Wayland, KDE) against the Production library, read-only,
with the same harness rules as feature 003 (missed frames judged against an idle baseline).

**Go/no-go gate (playback)**

- **SC-001**: A scene opens and plays inside the window in under 1.5 s, with hardware decoding
  active for H.264 and HEVC test files.
- **SC-002**: Seeking shows the new frame in under 1 s (median), including in 4K files.
- **SC-003**: 1080p playback drops 0 frames per minute with the controls showing and hidden; 4K
  playback drops no more than 1 per minute.
- **SC-004**: The controls draw over the video with no visible flicker, tearing, or black boxes
  (manual check), and respond to input in under 50 ms.

**Grid and shell**

- **SC-005**: Moving to the next or previous page shows it with thumbnails in under 150 ms at page
  sizes up to 250, with thumbnails already cached.
- **SC-006**: Jumping to any page shows its cards in under 1 s.
- **SC-007**: Scrolling a 1000-card page at fling speed misses under 1% of frames in grid and
  list mode, **both** with thumbnails cached and while uncached thumbnails are still arriving.
- **SC-008**: Switching tabs (20 tabs, one playing) shows the target tab in under 150 ms, with
  video correctly sized on the first frame.
- **SC-009**: Cold start to an interactive Scenes view takes under 2 s with a warm cache.

**Stability and overhead**

- **SC-010**: 20 consecutive launch, browse, play, and quit cycles finish with no crash, hang, or
  lost tab state.
- **SC-011**: The decision record states, for both builds with the same view open (Scenes, page
  of 50; and a 1080p scene playing), resident memory and CPU use; the native build's figures are
  reported whether better or worse.

**Decision**

- **SC-012**: The spike ends with a decision record that states go or no-go and gives a measured
  value for SC-001 to SC-011 for the native build, and for the web build where it has an
  equivalent measurement. "Go" requires SC-001 to SC-010 to pass.

## Assumptions

- The candidate toolkit is iced, as the user asked; a different native toolkit is considered only
  if the record shows iced itself (not the native approach) is what blocks a "go".
- The spike is time-boxed to about two weeks of work, with the playback gate (User Story 1)
  first; it stops early on a playback no-go (FR-015).
- Linux on Wayland (KDE) is the only platform measured; Windows and macOS are noted as unknowns
  in the record.
- Production-quality polish (theming, every edge state, accessibility) is out of scope; the
  record notes how hard each would be.
- Search, filters, galleries, and other later features are out of scope; the record notes any
  toolkit limits that would affect them (for example text input).
- Measurements use the same machine, library, and profile as the web build's 2026-10-03 harness
  run (feature 005 quickstart V8), so the two builds can be compared directly.
- The web build's uncommitted feature 005 work is committed (after the user's checks) before the
  spike starts, so the reference is a fixed point.
