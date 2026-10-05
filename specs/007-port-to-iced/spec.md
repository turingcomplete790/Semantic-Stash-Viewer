# Feature Specification: Port the Viewer to iced

**Feature Branch**: `007-port-to-iced`

**Created**: 2026-10-03

**Status**: Draft

**Input**: User description: "port to ice." (Follows feature 006's GO decision and constitution
v4.1.0: the viewer's interface moves from Tauri and the Linux webview to a native Rust UI built
with iced.)

## Overview

Feature 006 proved that the native build plays scenes inside its own window faster than the web
build, and the constitution now makes the native UI the viewer's only interface. The web build
(features 001–005) was a demo used to test ideas; it is frozen. This feature builds a **clean
native UI** that offers every capability the demo had, designed fresh, and then retires the demo.

The specs that built the demo serve as a **capability list**: each capability below must exist
in the native app, but its screens, flows, and details are redesigned freely, and its acceptance
scenarios are written for the new design. Only the behaviours under "Behaviours that stay" are
kept exactly. Preserving anything else from the demo (layouts, flows, saved UI state) never
takes priority over a clean UI.

| From | Capabilities carried over |
|---|---|
| 001 Connect to Stash | US1 first connection, US2 automatic reconnect and health, US3 connection security, US4 several servers |
| 002 mpv playback (with 006) | US1 in-window playback, US2 the viewer's own controls |
| 003 Complete foundation | US1 screens from the local cache (offline included), US2 the performance harness, US3 checks on every push |
| 004 App shell | US1 navigation bar, US2 tabs, US3 notification centre (connection alerts, background failures, Stash jobs with live progress; starting and cancelling jobs come later), US4 Settings |
| 005 Browse scenes | US1 browse the whole library (paged grid and list, sorting, page sizes, keyboard, never resets) |

**Behaviours that stay** (from the constitution and what the user has already tested):

- A tab never resets: leaving a view and coming back shows it exactly as left (page, page size,
  display mode, scroll position, selection), including across back and forward (Principle IX).
- Library lists are paged: default 50, sizes 20–1000, previous/next/first/last and jump to a page,
  the total count (Principle IV).
- Everything is reachable from the keyboard (Principle VI), and the player keeps 002's shortcuts
  (space, arrows, `[` `]` `\`, `.` `,`, `f`, `m`, Escape), which the user has used and approved.
- mpv's video plays inside the window with the controls on top, and every frame is shown
  (Principle V, feature 006).
- Connection problems are explained in plain language, with distinct messages per failure, and
  the connection's security state (unencrypted, unverified, verified) is always visible
  (Principle VII, 001).
- One notification centre with a badge, toasts that never take focus, and infrastructure (cache,
  prefetching) that stays out of sight (Principle IX).
- The constitution's performance budgets (Principle VI).

Feature 006's open work (grid, tabs, side-by-side measurement, stability loop) moves here, along
with its decision record's follow-ups: text input, keyboard-first accessibility, and the
video-path hardening that constitution v4.1.0's unsafe-code gate requires.

Out of scope: features the web build doesn't have yet (005 US2–US6: search, filters, saved
filters, the full scene page, resume and play history, previews, the transcode fallback). They
are built natively afterwards, on top of this port.

## Clarifications

### Session 2026-10-03

- Q: How much of writing to Stash should the port itself deliver, as opposed to the features
  built on top of it afterwards? → A: The port keeps today's scope and only drops the "read-only"
  framing. The viewer is not a read-only client (constitution Principles I, II, VIII); writes
  arrive with the features that need them (metadata editing, scrapers, semantic tagging,
  playback sync), each in its own spec.
- Q: When should the web build (the Tauri app and its SolidJS interface) be deleted from the
  project? → A: At the end of this port, after the user signs off the capability checklist (User
  Story 6). Until then it stays buildable for side-by-side measurement.
- Q: How should specs 001–005 be used by this feature? → A: As a capability list plus a short
  list of behaviours that must stay exactly ("Behaviours that stay" in the Overview). Screens,
  flows, and details are redesigned; acceptance scenarios are rewritten for the new UI. The
  Tauri app was a demo, and preserving anything else from it never outranks a clean UI.
- Q: Should the native app keep using the files the demo saved (profiles, cache, tabs), or start
  with fresh files of its own? → A: Start completely fresh: new files for everything, in formats
  designed for the new UI; users re-enter their servers once. The demo's data isn't worth
  compromising the design for.
- Q: When the user quits and relaunches, should open tabs come back? → A: Yes, everything: every
  tab with its full state (scroll position included), its history, and the same tab selected.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Connect and manage servers (Priority: P1)

On first launch the user adds a Stash server (address, optional API key) and connects. On later
launches the viewer reconnects by itself. The connection indicator always shows the state,
including whether the connection is unencrypted, unverified, or verified. Users with several
servers save each one, switch between them, and edit or remove them. All of this works as
features 001 and 004 (US4) specify, in the native build.

**Why this priority**: Nothing else works without a connection, and today's native build can only
borrow the demo's saved profiles: a new user can't get started at all.

**Independent Test**: With no saved profiles, launch the native build, add the Testing server
with its API key, connect, then add Production, switch between them, and relaunch. Then run
001's acceptance scenarios.

**Acceptance Scenarios**:

1. **Given** no saved servers, **When** the user launches the native build, **Then** they're
   guided to add one and reach a connected view (001 US1's capability).
2. **Given** a saved server, **When** the user relaunches, **Then** the viewer reconnects with no
   prompt and shows its state (001 US2).
3. **Given** servers over http, self-signed https, and trusted https, **When** the user connects,
   **Then** the security state is shown and strict validation can be turned on per server (001 US3).
4. **Given** several saved servers, **When** the user switches, edits, or removes one, **Then** it
   works (001 US4, 004 US4), and open views follow the new server.
5. **Given** the demo's saved profiles exist, **When** the native app starts for the first time,
   **Then** it ignores them and starts fresh, leaving the demo's files untouched.

---

### User Story 2 - Work in the app shell: navigation bar and tabs (Priority: P2)

The user moves between sections (Home, Scenes, Settings) with a Stash-like navigation bar. They
keep several views open in tabs, switch with the mouse or keyboard, and find every tab exactly as
they left it, including after a relaunch. Back and forward work within a tab. A small "now
playing" bar keeps a playing scene within reach from other tabs. Keyboard help lists every
shortcut (004 US1–US2's capabilities, redesigned).

**Why this priority**: Every other screen lives in the shell (constitution Principle IX), and tab
state that never resets is a behaviour that stays (Overview).

**Independent Test**: Open 20 tabs across Scenes, a scene, and Settings; switch, go back and
forward, play a scene and browse elsewhere, then relaunch. Run 004's US1 and US2 scenarios.

**Acceptance Scenarios**:

1. **Given** any view, **When** the user picks a section or Settings from the navigation bar or
   its shortcut, **Then** it opens within the navigation budget (004 US1).
2. **Given** several tabs, **When** the user switches, closes, reorders, or reopens tabs by mouse
   or keyboard, **Then** each tab keeps its page, mode, scroll position, and history (004 US2).
3. **Given** a playing scene, **When** the user switches to another tab, **Then** playback
   continues and the now-playing bar offers play/pause and a way back (004 clarification).
4. **Given** open tabs, **When** the user quits and relaunches, **Then** the same tabs come back
   with their full state, scroll positions included, and the same tab selected.

---

### User Story 3 - Browse scenes a page at a time (Priority: P3)

The user browses the whole library in Scenes as a paged grid or list, with the page controls,
sort options, page sizes (20 to 1000, default 50), and keyboard navigation of 005 US1, and
thumbnails from the server. Going into a scene and coming back returns to the same page, mode,
and scroll position.

**Why this priority**: It's the main screen of the viewer and the one most affected by the web
engine's limits.

**Independent Test**: Browse the Testing and Production libraries at 50 and 1000 per page in
both modes, with thumbnails cached and uncached, while the harness measures. Run 005 US1's
scenarios.

**Acceptance Scenarios**:

1. **Given** a connected server, **When** the user opens Scenes, **Then** 005 US1's acceptance
   scenarios pass in the native build.
2. **Given** a 1000-card page whose thumbnails aren't cached yet, **When** the user scrolls
   quickly, **Then** scrolling stays smooth while the thumbnails arrive.
3. **Given** the user went offline, **When** they open a page seen before, **Then** it shows from
   the cache (003 US1).

---

### User Story 4 - Open a scene and play it (Priority: P4)

From the grid, the user opens a scene into a scene view with its title, cover, and Play, and
plays it in the window with the full controls of 002 and 006: smooth, every frame shown, hardware
decoding where available, controls drawn over the video, fullscreen, and a clear error when a
file can't play.

**Why this priority**: Playback is the reason for the switch; it's mostly built (006), but needs
the scene view, the shell's tab behaviour, and the hardening the constitution requires.

**Independent Test**: Open scenes of every codec in the Testing library from the grid, use every
control, switch tabs while playing, play fullscreen, and quit while playing.

**Acceptance Scenarios**:

1. **Given** the grid, **When** the user opens a scene, **Then** its view shows title, cover, and
   Play within the navigation budget, including a restored scene tab after relaunch.
2. **Given** a scene, **When** the user plays it, **Then** 002 US1–US2's scenarios pass, and every
   frame is shown (006).
3. **Given** a playing scene in a background tab, **When** the user switches back, **Then** the
   video shows at once at the right size.

---

### User Story 5 - See what needs attention: notifications and jobs (Priority: P5)

Connection alerts, background failures (such as a scene that can't play), and Stash jobs with live
progress appear in one notification centre with a badge and brief toasts (004 US3's capability)
(jobs show progress; starting and cancelling them comes with a later feature).

**Why this priority**: Important but not blocking: the viewer is usable without it, and the core
already tracks notifications and jobs.

**Independent Test**: Stop and restart the Testing server, start a scan in the Stash web UI on
Testing, and open a scene that can't play. Run 004 US3's scenarios.

**Acceptance Scenarios**:

1. **Given** the connection drops and returns, **When** the user opens the centre, **Then** one
   entry tracks it and updates when it returns.
2. **Given** a Stash job starts on the server, **When** it runs, **Then** it appears with live
   progress within 004's time limit.

---

### User Story 6 - Settings, cache, and retiring the web build (Priority: P6)

Settings offers server management, the keyboard page, troubleshooting (cache size, clear cache,
log folder), and About (004 US4, 003 US1). Once the user has confirmed that the
native build offers every capability in the table above, the web build is removed: its interface, its
Tauri app, and its build steps. The harness and checks on every push run the native build only.

**Why this priority**: The last step, by definition: removal waits until everything else is
confirmed.

**Independent Test**: Use every Settings page; clear the cache and confirm the library is
unchanged; after the user's sign-off, remove the web build and confirm a fresh clone builds,
tests, and runs with one Rust toolchain only.

**Acceptance Scenarios**:

1. **Given** Settings, **When** the user uses each page, **Then** 004 US4's scenarios pass.
2. **Given** the user has signed off the capability checklist, **When** the web build is removed, **Then** the
   project builds, tests, measures, and runs from a fresh clone without Node or a web engine.

---

### Edge Cases

- **The demo still installed**: while it exists, the native app and the demo keep separate files
  (profiles, cache, tabs, notifications) and never read or change each other's.
- **First launch with no profiles** and **a profile whose server is down**: as 001 specifies.
- **Server older than the minimum version**: refused with a clear message (001).
- **Window scale changes** (moving between monitors with different scaling): text, thumbnails,
  and video stay sharp and correctly sized.
- **Keyboard only**: every action in this feature is reachable without a mouse (constitution VI).
- **Text input**: server address and API key fields, the page-number field, and any search box
  accept typing and paste, and IME input where the desktop uses it.
- **Quit during work**: quitting while pages, thumbnails, or a scene are loading or playing exits
  promptly and saves tab state.
- **GPU path unavailable** (a machine without the needed Vulkan or EGL support): the viewer still
  runs, and the player shows a plain explanation instead of a black area.

## Requirements *(mandatory)*

### Functional Requirements

**Capabilities**

- **FR-001**: The native app MUST offer every capability in the Overview's table, with acceptance
  scenarios written for the new design and recorded in the capability checklist.
- **FR-002**: The carried-over stories write nothing to Stash, so this port adds no writes to
  Stash itself. It MUST NOT build anything that assumes read-only access: the viewer is meant to
  write to Stash (constitution Principles I, II, VIII), and later features add writes on top of
  the native build. Local writes (settings, profiles, tabs, cache) work as the earlier specs
  define.
- **FR-003**: The behaviours under "Behaviours that stay" MUST hold exactly. Everything else
  (screens, flows, layout, wording, saved UI state) is designed fresh; no capability may be lost
  (Principle VIII).

**Connection and servers (US1)**

- **FR-004**: Users MUST be able to add, edit, remove, reorder, and switch servers, with the
  address, optional API key, and strict TLS setting, entirely in the native build.
- **FR-005**: The native app MUST keep all of its saved data (profiles, cache, tabs,
  notifications, logs) in files of its own, in formats designed for it. It MUST NOT read or migrate
  the demo's files; users re-enter their servers once.

**Shell (US2)**

- **FR-006**: The shell MUST provide the navigation bar, tabs with per-entry history, the
  now-playing bar, keyboard help, and the server menu of feature 004.
- **FR-007**: Tab state MUST survive switching, back and forward, and relaunch, for every kind of
  tab. On relaunch every tab comes back with its full state (section, scene, page, page size,
  mode, scroll position, history) and the same tab selected.

**Scenes and playback (US3, US4)**

- **FR-008**: Scenes MUST provide 005 US1's paged grid and list, including thumbnails, page
  controls, sorting, page sizes, keyboard navigation, and returning to the same place.
- **FR-009**: The scene view and player MUST provide 002's controls and keyboard shortcuts and
  006's in-window playback, showing every frame mpv renders.

**Notifications (US5)**

- **FR-010**: The notification centre MUST show connection alerts, background failures, and
  Stash jobs with live progress, a badge, and toasts.

**Settings and retirement (US6)**

- **FR-011**: Settings MUST provide server management, the keyboard page, troubleshooting (cache
  size, clear cache, open log folder), and About.
- **FR-012**: After the user's sign-off on the capability checklist, the web build's interface, Tauri app, and
  their build and check steps MUST be removed, leaving one Rust toolchain for building, testing,
  measuring, and running.

**Quality**

- **FR-013**: The performance harness and the checks on every push MUST run the native build,
  including its measurement of frames actually displayed.
- **FR-014**: The native build's video path MUST meet constitution v4.1.0's unsafe-code gate.
  `unsafe` may appear only at foreign-library boundaries, behind safe owning types; every block
  is documented and lint-checked; no lifetime tricks.
- **FR-015**: Every capability and every behaviour that stays MUST be covered by tests of the
  native app: state logic as unit tests, screen flows as headless UI tests.

### Key Entities

- **Server profile**: saved servers (address, API key, TLS setting, order, last used) (001), in the
  native app's own file.
- **Tab set**: per-server tabs, each with its history and full state, saved so every tab comes
  back on relaunch exactly as left; the saved format belongs to the native app.
- **Notification**: connection alerts, failures, and Stash jobs with progress (004).
- **View cache**: per-server cached pages, details, and thumbnails, safe to clear (003).
- **Capability checklist**: each carried-over capability and each behaviour that stays, with the
  native app's acceptance result,
  signed off by the user before retirement.

## Success Criteria *(mandatory)*

### Measurable Outcomes

Measured on the user's desktop (KDE on Wayland) against the Testing library unless noted, with
the harness rules of feature 003.

- **SC-001**: 100% of the carried-over capabilities and behaviours that stay pass their
  acceptance scenarios in the native app, recorded in the capability checklist and signed off by
  the user.
- **SC-002**: Cold start to an interactive view takes under 2 s with a warm cache.
- **SC-003**: Opening any section or tab shows it in under 150 ms; input is acknowledged in under
  50 ms; with 20 tabs open, switching tabs still takes under 150 ms.
- **SC-004**: Moving to the next or previous page of scenes shows it with thumbnails in under
  150 ms at page sizes up to 250 with thumbnails cached; jumping to any page shows its cards in
  under 1 s.
- **SC-005**: Scrolling a 1000-card page at fling speed misses under 1% of frames in grid and list
  mode, both with thumbnails cached and while uncached thumbnails are still arriving.
- **SC-006**: Playback opens in under 1.5 s, seeks in under 1 s (median, 4K included), and drops
  no frames at 1080p (counting frames not shown).
- **SC-007**: 20 consecutive launch, browse, play, and quit cycles finish with no crash, hang, or
  lost tab state.
- **SC-008**: On every carried-over budget, the native build is no slower than the web build's
  last measured numbers on the same machine.
- **SC-009**: After retirement, a fresh clone builds, passes all checks, and runs using only the
  Rust toolchain and system libraries (no Node, no web engine).

## Assumptions

- **UI design direction (the user's)**: the interface is built as a hierarchical state machine
  that makes full use of iced's Elm-style structure (state, messages, update, view), rather than a
  copy of the demo's page-and-route structure. Whether named routes exist at all is decided in the
  plan. This shapes how the behaviours that stay (tabs that never reset, full restore on relaunch)
  are delivered, not what users see.

- Feature 006's `native-ui` crate, its player, and its copied app glue are the starting point; the
  core crates (`stash-core`, `player`) carry over unchanged in behaviour.
- The earlier specs' user-facing requirements are still right; only how they're delivered
  changes. Where a requirement assumed a web engine (for example a URI scheme for thumbnails), the
  plan replaces it with the native equivalent and records the change.
- Measurements use the Testing profile (NVMe); Production (spinning disks) is used for manual
  checks only.
- Only Linux (KDE on Wayland, AMD) is verified; other platforms remain documented unknowns as in
  006's decision record.
- Work is delivered in story order, each story usable on its own, with the user checking each
  before it's committed.
