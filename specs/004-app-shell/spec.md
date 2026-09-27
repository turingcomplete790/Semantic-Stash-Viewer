# Feature Specification: App Shell (Navigation Bar, Tabs, Notifications, Settings)

**Feature Branch**: `004-app-shell`

**Created**: 2026-09-27

**Status**: Draft

**Input**: User description: "app-shell"
(Constitution Principle IX, and the roadmap's Phase 1 "App shell" section: a Stash-like
navigation bar, tabs inside the main window, a notification centre, and a Settings page. Every
later feature lives inside it.)

## Overview

Today the viewer is a handful of separate screens: the connection form, the server picker, the
server summary, and the player with its scene picker. Profile management and the API-key prompt
open as pop-up dialogs. That was fine for proving the foundation, but it doesn't scale: the
viewer will grow to cover the whole Stash web UI plus semantic tagging.

This feature gives the viewer the frame every later feature plugs into (constitution
Principle IX):

- **A navigation bar** modelled on the Stash web UI. It holds the top-level sections, plus
  Settings, the notification centre, and the current server.
- **Tabs** inside the main window. Users keep several views open, such as a scene list, a scene
  that's playing, and Settings, and switch between them without losing their place.
- **A notification centre** that collects connection alerts, failures from background work, and
  running Stash jobs with live progress (read-only), with a badge and short pop-up toasts.
- **A Settings page** for server management, viewer preferences, and troubleshooting controls
  such as "Clear cache" from feature 003.

The existing screens move into the shell. The server summary becomes the **Home** section. The
scene picker becomes the **Scenes** section, until Phase 1 replaces it with the real scene grid.
The player opens scenes inside the shell. Server management moves into Settings.

**Web UI capabilities covered** (Principle VIII):

- the Stash navigation bar and its section structure (only sections that exist are shown);
- Settings as a destination;
- job progress visibility (read-only; starting and cancelling jobs are Phase 3).

## Clarifications

### Session 2026-09-27

- Q: When the user switches away from the tab of a playing scene, what happens to playback?
  → A: It keeps playing, with a small "now playing" bar in the shell. The bar shows the title,
  play/pause, and a button back to the scene's tab.
- Q: Should the notification centre show Stash server jobs now? → A: Yes, read-only. Running
  jobs (including ones started from the web UI) show with live progress, and each gets a
  notification when it finishes or fails. Starting and cancelling jobs stay in Phase 3.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Find my way around with a navigation bar (Priority: P1)

A user connects to their Stash server. Across the top of the window they see a navigation bar
like the Stash web UI's: the viewer's name (which goes Home), the sections that exist so far
(Home and Scenes), and on the right the notification bell, Settings, and the current server with
its connection status. One click, or one keyboard shortcut, takes them to any section.

**Why this priority**: The navigation bar is the backbone every later feature attaches to.
Without it, Phase 1 would have nowhere consistent to put Images, Galleries, and the other
sections.

**Independent Test**: Launch the viewer connected to a server. Use the navigation bar with the
mouse and then with the keyboard to reach Home, Scenes, Settings, and the notification centre.
Each opens within the navigation budget, and the current section is highlighted.

**Acceptance Scenarios**:

1. **Given** the viewer is connected, **When** the user looks at the top of the window, **Then**
   they see the navigation bar with Home, Scenes, the notification bell, Settings, and the
   current server and its connection status.
2. **Given** the navigation bar, **When** the user clicks a section or uses its keyboard shortcut,
   **Then** that section opens and is highlighted as current.
3. **Given** no server is connected, **When** the viewer shows the server picker or the
   connection form, **Then** the navigation bar shows only what works without a server
   (Settings and notifications), and the library sections are hidden.
4. **Given** a feature that doesn't exist yet (for example Galleries), **When** the user looks at
   the navigation bar, **Then** no placeholder or disabled entry is shown for it.
5. **Given** the user clicks the current server in the navigation bar, **When** the menu opens,
   **Then** they can switch to another saved server, update the API key, or open server
   management in Settings, as the connection indicator allows today.

---

### User Story 2 - Keep several views open in tabs (Priority: P2)

A user is browsing scenes, opens one to play, then wants to check Settings without losing the
scene list. They open each in its own tab below the navigation bar and switch between them. Each
tab comes back exactly as they left it: same scroll position, same filters, same selection. They
use keyboard shortcuts to open, close, and cycle tabs, as in a web browser. When they quit and
relaunch, their tabs for that server come back.

**Why this priority**: Tabs are how the viewer makes moving between scenes, galleries, and
performers instant and stateful (Principle IX). Phase 1's browsing features depend on them, but
the navigation bar alone already makes the app usable, so tabs come second.

**Independent Test**: Open Home, Scenes, and Settings in three tabs. Scroll the Scenes list and
switch away and back: the scroll position is kept. Use only the keyboard to open a new tab,
cycle tabs, go back within a tab, and close a tab. Quit and relaunch: the tabs are restored.

**Acceptance Scenarios**:

1. **Given** a tab is open, **When** the user clicks a navigation bar section, **Then** the
   current tab navigates to it. **When** they middle-click or Ctrl+click it instead, **Then** it
   opens in a new tab.
2. **Given** several tabs, **When** the user switches to another tab and back, **Then** each tab
   shows its view exactly as left (scroll position, filters, selection, and any typed text),
   without reloading.
3. **Given** a tab has navigated through several views, **When** the user goes back or forward
   (keyboard, mouse back/forward buttons, or on-screen buttons), **Then** the tab moves through
   its own history.
4. **Given** the keyboard only, **When** the user opens a new tab, closes the current tab,
   switches to the next or previous tab, or jumps to tab 1–9, **Then** each works with a
   shortcut, and the shortcuts are listed in a keyboard help overlay.
5. **Given** the user closes the last tab, **When** it closes, **Then** a new tab opens on Home,
   so the window is never empty.
6. **Given** open tabs for a server, **When** the user quits and relaunches, **Then** that server's
   tabs are restored in the same order, with the same one selected.
7. **Given** a scene is playing in one tab, **When** the user switches to another tab, **Then**
   playback continues, and a "now playing" bar appears in the shell with the scene's title,
   play/pause, and a button that returns to the scene's tab. There is only ever one playing
   scene.
8. **Given** the "now playing" bar is showing, **When** the user closes the playing scene's tab,
   **Then** playback stops and the bar disappears.

---

### User Story 3 - See what needs my attention in one place (Priority: P3)

A user's Stash server goes offline while they're browsing. A short toast appears in the corner
saying the server is unreachable, without taking focus or covering what they're doing. The
notification bell shows a badge. When they open the notification centre, the alert is listed
with its time. When the server comes back, the same entry updates to "reconnected" instead of
adding a second alert. They dismiss it. Errors from background work, such as a scene that failed
to open, land here too.

**Why this priority**: The constitution requires one home for alerts and Stash jobs, and
feature 003's offline behaviour depends on it. It's needed before 003's cache is built, but it's
useful only once there's a shell to put it in.

**Independent Test**: With the viewer connected, stop the Stash server. A toast appears, the
bell shows a badge, and the centre lists "server unreachable". Start the server again: the entry
updates to "reconnected". Dismiss it: the badge clears. Open a scene whose file is missing: an
error entry appears.

**Acceptance Scenarios**:

1. **Given** the connection to the server is lost, **When** the viewer notices, **Then** a toast
   says the server is unreachable, the bell shows a badge, and the notification centre lists the
   alert with its time.
2. **Given** an ongoing condition (e.g. server unreachable), **When** it changes (reconnected),
   **Then** its existing entry updates rather than a new one being added for every retry.
3. **Given** a toast is showing, **When** the user is typing, clicking, or using the player,
   **Then** the toast doesn't take focus or block input, and it hides itself after a few seconds.
4. **Given** notifications in the centre, **When** the user dismisses one or all, **Then** they're
   removed and the badge updates. Undismissed notifications stay until dismissed, including
   across restarts.
5. **Given** background work fails (e.g. a scene can't be opened, or a background refresh
   fails repeatedly), **When** it happens, **Then** a notification explains it in plain language
   with any next step.
6. **Given** a Stash job is running on the server (started from the viewer's server or the Stash
   web UI), **When** the user opens the notification centre, **Then** the job is listed with its
   description and live progress, and the bell shows an activity marker.
7. **Given** a Stash job finishes or fails, **When** it ends, **Then** its entry becomes a
   notification saying so. Failures also show a toast. The viewer offers no way to start or
   cancel jobs yet.

---

### User Story 4 - Manage servers and preferences in Settings (Priority: P4)

A user opens Settings from the navigation bar. Settings opens as a tab with pages on the side:
**Servers** (add, edit, remove, and reorder saved servers, formerly a pop-up dialog), **Keyboard**
(the list of shortcuts), **Troubleshooting** (where feature 003's "Clear cache" goes, plus a way
to open the log folder), and **About** (viewer version, and the connected server's version).

**Why this priority**: Settings gives feature 003's troubleshooting control and future
preferences a home, and replaces the server-management pop-up. The pages it starts with are
small, so it comes last.

**Independent Test**: Open Settings from the navigation bar and by keyboard. On Servers, add,
edit, and remove a server with the same results as today's dialog. The Troubleshooting page
exists and opens the log folder, and About shows both versions.

**Acceptance Scenarios**:

1. **Given** the navigation bar, **When** the user opens Settings, **Then** it opens as a tab
   with the Servers, Keyboard, Troubleshooting, and About pages.
2. **Given** the Servers page, **When** the user adds, edits, reorders, or removes a server,
   **Then** it behaves as the current server-management dialog does, including confirmation
   before removal.
3. **Given** the Troubleshooting page, **When** the user chooses to open the log folder, **Then**
   the folder opens in the system file manager.
4. **Given** later features add preferences or troubleshooting controls, **When** they land,
   **Then** they appear as entries on these pages or as new pages, not as separate screens.

---

### Edge Cases

- **Switching servers with tabs open**: tabs belong to a server. Switching servers saves the
  current server's tabs and restores the other server's tabs, or a Home tab if it has none. The
  playing scene stops (as today), and the "now playing" bar disappears.
- **Jobs while disconnected**: jobs can't be watched while the server is unreachable. Their
  entries show as "status unknown" until the connection returns, then update to the current
  state.
- **A restored scene tab after relaunch**: the scene tab comes back paused at the start. It
  doesn't auto-play.
- **A restored tab points at something that's gone** (e.g. a deleted scene): the tab opens and
  explains that the item no longer exists, with a way to close it. The other tabs are unaffected.
- **Many tabs**: tabs shrink to show titles, then scroll. The strip never wraps onto several rows.
  Background tabs may be unloaded to save memory, but they keep their scroll position, filters,
  and selection, and come back within the navigation budget.
- **Narrow window**: below the window width where all sections fit, sections that don't fit
  move into an overflow menu. Nothing is cut off.
- **Fullscreen playback**: the navigation bar, tabs, the "now playing" bar, and toasts are hidden. Notifications still
  collect in the centre, and the badge shows when fullscreen ends.
- **Notification flood** (e.g. a flapping connection): repeats of the same condition update one
  entry. Toasts are rate-limited so they never stack up on screen.
- **Keyboard conflicts**: shell shortcuts must not clash with the player's keyboard map (space,
  arrows, f, m, [, ], \, comma, period, Escape) while a scene is focused. Escape keeps its player
  meaning (leave fullscreen, then close the player).
- **No server saved yet**: the shell shows the connection form inside it, with only Settings and
  notifications in the navigation bar.
- **Notification storage damaged**: notifications are discarded, and the viewer starts with an
  empty centre. Nothing else is affected.

## Requirements *(mandatory)*

### Functional Requirements

**Navigation bar (US1)**

- **FR-001**: The viewer MUST show a persistent navigation bar across the top of the main window
  whenever it isn't in fullscreen playback.
- **FR-002**: The navigation bar MUST hold, in order:
  - the viewer's name, which goes to Home;
  - the top-level sections that exist (initially Home and Scenes);
  - on the right, the notification bell with its badge, Settings, and the current server with its
    connection status.
- **FR-003**: The navigation bar MUST NOT show entries for sections that don't exist yet. Later
  features add their sections in the Stash web UI's order: Scenes, Images, Groups, Markers,
  Galleries, Performers, Studios, Tags.
- **FR-004**: The current section MUST be highlighted. Every navigation bar entry MUST be
  reachable by keyboard, and each section MUST have a shortcut.
- **FR-005**: The current-server entry MUST offer what the connection indicator offers today:
  connection status and security state, switching to another saved server, updating the API key,
  and opening server management (now in Settings).
- **FR-006**: When no server is connected, the navigation bar MUST show only Settings and the
  notification centre. The main area shows the server picker or the connection form.
- **FR-007**: When the window is too narrow for every section, sections that don't fit MUST move
  into an overflow menu.

**Tabs (US2)**

- **FR-008**: The main window MUST show a tab strip below the navigation bar, and every view
  (sections, a playing scene, Settings) MUST open inside a tab.
- **FR-009**: Clicking a section MUST navigate the current tab. Middle-click or Ctrl+click MUST
  open it in a new tab. Items inside views (e.g. a scene in a list) MUST follow the same rule.
- **FR-010**: Each tab MUST keep its own state while in the background, and restore it exactly
  when shown again, without reloading from the server. State means scroll position, filters,
  selection, and typed text.
- **FR-011**: Each tab MUST keep its own back/forward history. It MUST be navigable by keyboard,
  by mouse back/forward buttons, and by on-screen buttons.
- **FR-012**: Keyboard shortcuts MUST exist for:
  - new tab (opens Home);
  - close tab;
  - next and previous tab;
  - jump to tab 1–9;
  - reopen the last closed tab.

  A keyboard help overlay MUST list every shell and player shortcut.
- **FR-013**: Closing the last tab MUST open a new Home tab.
- **FR-014**: Tabs MUST belong to the server they were opened for. On relaunch, and when switching
  back to a server, its tabs MUST be restored in order with the same tab selected.
- **FR-015**: Only one scene MAY play at a time. Opening a scene while another plays MUST replace
  it. When the user switches away from the playing scene's tab, playback MUST continue, and the
  shell MUST show a "now playing" bar with the scene's title, play/pause, and a button that
  returns to the scene's tab. Closing the playing scene's tab MUST stop playback and remove the
  bar. The bar MUST NOT cover content, and MUST stay out of fullscreen playback.
- **FR-016**: Many open tabs MUST NOT slow the viewer down. Background tabs MAY be unloaded to
  save memory, as long as FR-010's state is kept and restoring a tab meets the navigation budget.

**Notification centre (US3)**

- **FR-017**: The viewer MUST keep a notification centre, opened from the bell in the navigation
  bar. It lists notifications newest first, each with a plain-language title, details, a time,
  and a severity (information, warning, error).
- **FR-018**: The viewer MUST notify about:
  - connection changes: server unreachable, reconnected, authentication failed, server version
    refused;
  - failures from background work, such as a scene that failed to open or a background refresh
    that keeps failing;
  - other viewer features' events, as they add them.
- **FR-019**: An ongoing condition MUST be one notification that updates as the condition
  changes (e.g. "Server unreachable" becomes "Reconnected"). It MUST NOT be a new notification
  for every retry.
- **FR-020**: Important notifications (warnings and errors, plus reconnection) MUST also show as
  a toast. Toasts appear in a corner, never take focus or block input, hide themselves after
  about 5 seconds, and are rate-limited so they never stack up.
- **FR-021**: The bell MUST show a badge counting unread notifications, plus an activity marker
  while anything is in progress.
- **FR-022**: Users MUST be able to dismiss one notification or all of them. Undismissed
  notifications MUST persist across restarts, up to the 200 most recent. Notification storage
  is discardable: if it's lost or damaged, the viewer starts with an empty centre.
- **FR-023**: The notification centre MUST show jobs running on the connected Stash server,
  including ones started elsewhere (e.g. the web UI). It MUST show each job's description and
  live progress, read-only, and mark the bell as active while any run. When a job finishes or
  fails, its entry MUST become a notification saying so, with a toast for failures. Starting and
  cancelling jobs are out of scope (roadmap Phase 3). Watching jobs MUST NOT write to Stash, and
  MUST add no per-job polling beyond what live progress needs (Principle IV).

**Settings (US4)**

- **FR-024**: Settings MUST open as a tab from the navigation bar (and by keyboard), with a page
  list: Servers, Keyboard, Troubleshooting, and About.
- **FR-025**: The Servers page MUST replace the current server-management dialog with the same
  capabilities. These are add, edit (address, API key, strict TLS), reorder, and remove with
  confirmation.
- **FR-026**: The Keyboard page MUST list every shell and player shortcut (the same content as
  the help overlay).
- **FR-027**: The Troubleshooting page MUST offer a way to open the viewer's log folder. It is
  where feature 003's "Clear cache" goes.
- **FR-028**: The About page MUST show the viewer's version and the connected server's address
  and Stash version.

**Existing screens and cross-cutting rules**

- **FR-029**: The existing screens MUST move into the shell without losing any capability:
  - the server summary becomes Home;
  - the scene picker (recently added and test scenes) becomes Scenes;
  - the player opens scenes in a tab;
  - the connection form and server picker show inside the shell when no server is connected;
  - the API-key prompt stays a prompt.
- **FR-030**: Everything in the shell MUST be usable with the keyboard alone, with visible focus.
- **FR-031**: The shell MUST NOT show cache or other infrastructure messages on the main screens
  (Principle IX "quiet infrastructure"). Such things surface only as notifications when the user
  needs to act, or on the Troubleshooting page.
- **FR-032**: The shell MUST NOT add any network destination beyond the configured Stash server,
  and MUST NOT write to Stash.

### Key Entities

- **Section**: a top-level destination in the navigation bar (Home, Scenes, and later Images and
  so on). It has a name, an icon, a keyboard shortcut, whether it needs a connected server, and
  its position in the Stash web UI order.
- **Now playing**: the one scene currently playing. It has its tab, title, and play/pause state.
  It's shown in the shell's bar when its tab isn't selected.
- **Server job**: a job running on the Stash server. It has a description, progress, a status
  (running, finished, failed, unknown while disconnected), and start and end times. Read-only.
- **Tab**: one open view. It has what it shows (a section, a scene, Settings), a title, its
  history (back/forward), its saved view state (scroll, filters, selection, typed text), and the
  server it belongs to.
- **Tab set**: the ordered tabs for one server, and which one is selected. Restored on relaunch.
- **Notification**: something the user may need to know. It has a severity, a title, details, a
  time, a source (connection, playback, background work, jobs), read or unread, and optionally an
  ongoing condition it tracks (so updates replace it) and progress.
- **Settings page**: a page within Settings (Servers, Keyboard, Troubleshooting, About), which
  later features extend.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: From any view, the user reaches any section, Settings, or the notification centre
  in one click or one keyboard shortcut. The new view paints in under 150 ms (Principle VI).
- **SC-002**: Switching to a background tab shows it exactly as left (same scroll position,
  filters, and selection) in under 150 ms, in 100% of switches in testing.
- **SC-003**: Every shell action (sections, tabs, history, notifications, Settings) works with
  the keyboard alone. Every control responds visibly in under 50 ms.
- **SC-004**: After a relaunch, 100% of the previous session's tabs for the server come back in
  the same order, with the same one selected.
- **SC-005**: A lost connection produces exactly one notification entry, which updates on
  reconnection. Toasts never take focus. In a test that drops and restores the connection 10
  times in a minute, at most one entry and at most a few toasts appear.
- **SC-006**: Every capability of today's screens (connect, switch server, manage servers,
  update the API key, see server info, pick and play a scene) remains available in the shell,
  with the existing automated tests passing or updated to the new layout.
- **SC-007**: With 20 tabs open, navigation and tab switching still meet SC-001 and SC-002.
- **SC-008**: Switching away from a playing scene doesn't interrupt playback: no audible gap, and
  the "now playing" bar appears within 150 ms. Returning to the scene's tab shows the video
  again within 150 ms.
- **SC-009**: A job started from the Stash web UI appears in the notification centre within
  2 seconds, its progress updates at least every 2 seconds while running, and its finish or
  failure notification appears within 2 seconds of it ending.

## Assumptions

- **Sections at launch**: only Home and Scenes exist, because they're the only views the viewer
  has. Phase 1 adds Images, Galleries, and the rest in the Stash web UI's order. No placeholders
  are shown.
- **Tab behaviour follows web browsers**: clicking navigates the current tab, middle- or
  Ctrl+click opens a new tab, and each tab has its own back/forward history. Users already know
  this model.
- **Tabs are per server** and restored on relaunch, like browser sessions. A tab doesn't carry
  over to a different server.
- **Settings pages at launch**: Servers, Keyboard, Troubleshooting, and About. Theme,
  playback, and 10-foot mode preferences arrive with their features (roadmap Phase 3).
- **Notifications are local** to this machine and viewer. They aren't synced to Stash or other
  devices.
- **One main window**: additional windows (e.g. a pop-out player) are out of scope. Principle IX
  allows a later spec to add them.
- **Global search and the configurable home page** are separate roadmap items (Phase 1 App
  shell) and aren't part of this feature. Home keeps showing the server summary until then.
- **Dependency for feature 003**: 003's "server unreachable" notification and its "Clear cache"
  control use this feature's notification centre and Troubleshooting page, so this feature is
  built first.
