# Feature Specification: Complete Phase 0 (Cache, Performance Harness, CI)

**Feature Branch**: `003-complete-foundation`

**Created**: 2026-09-27

**Status**: Draft

**Input**: User description: "complete phase 0"
(Roadmap Phase 0: the three unchecked items. They are the cache layer, the performance harness,
and CI.)

## Overview

Phase 0 builds the foundation that the browsing features in Phase 1 will depend on. Six of its
nine items are done: the scaffold, the Stash adapter, connection profiles, the version check,
TLS, and the mpv spike. The exit criterion ("add a server profile, connect, see server info, and
play one scene in mpv") is already met. This feature closes the three items that are still open:

1. **Cache layer.** The viewer keeps a local, discardable copy of what it has read from Stash.
   Screens it has shown before appear immediately, even right after a restart, and are then
   refreshed from the server in the background. Writes the viewer makes later (Phase 2) must
   update or drop exactly the cached data they affect. The cache manages itself. Clearing it is a
   troubleshooting option in Settings, not something users need to do.
2. **Performance harness.** A repeatable, unattended way to measure the constitution's
   Principle VI budgets. These are input acknowledgement, view navigation first paint, scroll
   frame rate, and cold start, plus the playback timings from the mpv spike. It compares each
   result with its budget and with the previous run.
3. **CI.** An automated check already runs on every push to GitHub and passed on the first push
   (about 6 minutes). This feature confirms that it covers every hygiene gate the constitution
   requires, fills the gaps, and makes its status visible. After that, the roadmap item can be
   ticked.

**Web UI capabilities covered** (Principle VIII): none directly. This is infrastructure. The
manual "clear cache" control is a viewer-only capability that Principle IV requires.

## Clarifications

### Session 2026-09-27

- Q: How visible should the cache be to the user? → A: Barely visible. The cache works without
  the user noticing or managing it. Its only control (clearing it) lives in the viewer's
  Settings for troubleshooting. A user shouldn't need to touch it if the app is working well.
- Q: When the server can't be reached, should cached screens still show, and how is the user
  told? → A: Cached screens stay usable with no cache wording. "Server unreachable" is shown by
  the connection indicator and the notification centre (constitution Principle IX). Actions
  that need the server, such as playback, explain why they're unavailable.
- Q: How big may the cache get? → A: A share of free disk space: 5% of the free space on the disk
  that holds the cache, never below 64 MB and never above 10 GB. It's re-checked as the disk
  fills, so the cache shrinks when space runs low.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Screens appear instantly from the local cache (Priority: P1)

A user opens the viewer, and it reconnects to their Stash server. The screens they used last time
(the server summary, the scene picker lists) appear right away from the viewer's local copy,
instead of waiting on the network. In the background, the viewer asks the server for fresh data
and quietly updates the screen if anything changed. The user never has to think about the cache:
it keeps itself current and within its size limit. If something ever looks wrong, Settings has a
"Clear cache" option for troubleshooting. Clearing it changes nothing in their library.

**Why this priority**: Phase 1 browsing (grids over tens of thousands of scenes) depends on
cached data to meet the cold-start and navigation budgets. The constitution (Principles I and IV)
requires the cache to be discardable, invalidated by writes, and clearable by hand. Building it
before the first grid prevents each screen from inventing its own caching.

**Independent Test**: Connect to a server and open the scene picker. Quit, disconnect the network
to the server, and relaunch. The picker lists appear from the cache, and the connection indicator
says the server is unreachable. Reconnect: they refresh. Clear the cache: the next visit loads from the server again, and
the library is unchanged.

**Acceptance Scenarios**:

1. **Given** the user has viewed a screen before, **When** they open it again, even after
   restarting the viewer, **Then** its content appears from the cache without waiting for the
   server, and a background refresh follows.
2. **Given** a screen is shown from the cache, **When** the background refresh returns data that
   differs, **Then** the screen updates in place without losing the user's scroll position or
   selection, and without a full-screen loading state.
3. **Given** the server can't be reached, **When** the user opens a screen they've viewed before,
   **Then** the cached content is shown with no cache wording. The connection indicator and the
   notification centre say the server is unreachable, and actions that need the server (such as
   playback) explain why they're unavailable.
4. **Given** cached data exists, **When** the user opens Settings and chooses "Clear cache",
   **Then** the cached data is deleted, Settings confirms how much space was freed, and the next
   visit to each screen loads from the server.
5. **Given** the viewer makes a write to Stash (from Phase 2 on), **When** the write succeeds,
   **Then** every cached entry the write affects is updated or dropped, and no unrelated cached
   entry is touched.
6. **Given** two server profiles, **When** the user browses one, **Then** it never shows data
   cached from the other.
7. **Given** the app is working normally, **When** the user browses and plays scenes, **Then**
   nothing on the main screens mentions the cache or asks the user to manage it.

---

### User Story 2 - Measure the performance budgets with one command (Priority: P2)

A developer (the project owner) wants to know whether the viewer meets Principle VI's budgets,
and whether a change made anything slower. They run the performance harness against their Stash
server. It drives the real app window with no manual input, and a few minutes later produces a
report. The report lists each budget with the measured value (median and 95th percentile), shows
pass or fail against the budget, and flags anything that got notably slower than the previous run.

**Why this priority**: The constitution's performance gate requires changes to list views, GraphQL
operations, and the index to be checked against the budgets before merge. Without a harness,
that check is guesswork. It's needed before Phase 1's grids, but the app works without it, so it
follows the cache.

**Independent Test**: Run the harness against the user's library. It completes unattended and
writes a report covering cold start, navigation first paint, input acknowledgement, scroll frame
rate, and playback open/seek time, each marked pass or fail. Run it again: the second report
compares itself with the first.

**Acceptance Scenarios**:

1. **Given** a connected server profile, **When** the developer runs the harness, **Then** it
   completes without manual interaction and produces a report with a value and a pass/fail result
   for each measured budget.
2. **Given** a previous report exists, **When** the harness runs again, **Then** the new report
   shows, for each measurement, the change from the previous run, and flags any measurement that
   got more than 20% slower.
3. **Given** the viewer has no screen with a long enough list yet (before Phase 1), **When** the
   harness measures scrolling, **Then** it scrolls a harness-provided list of at least 10,000 items
   so the frame-rate budget can still be checked.
4. **Given** the harness runs, **When** it measures anything, **Then** it doesn't write to Stash
   and doesn't make network requests to anything other than the configured Stash server.

---

### User Story 3 - Every push is checked automatically (Priority: P3)

The developer pushes a change to GitHub. Within minutes, an automated check runs every hygiene
gate the constitution requires: formatting, lint with no warnings, type checks, all unit tests,
and the check that the generated UI bindings match the Rust code. If anything fails, the commit
and pull request show which check failed. The README shows the current status of the main branch.

**Why this priority**: The check already exists and passes (about 6 minutes on the first push).
What's left is confirming full coverage, making the status visible, and ticking the roadmap item.
That's the smallest of the three.

**Independent Test**: Push a commit that breaks formatting on a branch: the check fails and names
the formatting step. Push the fix: it passes. The README's status badge reflects the main branch.

**Acceptance Scenarios**:

1. **Given** a push or pull request, **When** the check runs, **Then** it runs every hygiene gate
   listed in the constitution's Development Workflow section, plus the bindings drift check and
   the new cache tests.
2. **Given** a check fails, **When** the developer looks at the commit or pull request, **Then** it
   names the failing step.
3. **Given** the check runs, **When** it needs test data, **Then** it uses only recorded fixtures
   and generated media. It needs no private Stash server, no API keys, and no secrets.

---

### Edge Cases

- **Cache files damaged or from an older viewer version**: the viewer discards them and starts
  with an empty cache, without an error dialog. At most, a note goes in the log.
- **Cache grows large**: the cache stays under a size limit and drops the least recently used
  entries first.
- **A profile's server address is changed to a different server**, or the server is replaced (a
  different Stash instance at the same address): the viewer detects that it's a different server
  and doesn't show the old server's cached data.
- **A profile is deleted**: its cached data is deleted with it.
- **The library changes outside the viewer** (the Stash web UI, the semantic-tagging plugin,
  a scan): the next background refresh shows the change. The viewer never assumes cached data is
  current.
- **Clearing the cache while a screen is open**: the screen stays usable and reloads from the
  server. Nothing is lost.
- **A write fails** (Phase 2): the cache isn't changed.
- **The harness runs while the server is unreachable or the app can't connect**: it stops with a
  clear message instead of reporting misleading numbers.
- **The harness window is hidden or covered during a run**: frame measurements would be
  meaningless (research from the mpv spike). The harness detects that no frames are being drawn
  and marks those measurements invalid instead of failing them.
- **CI flakiness**: a test that depends on timing must not fail at random on a slower CI machine.
  Timing tests use margins suited to shared runners.

## Requirements *(mandatory)*

### Functional Requirements

**Cache layer (US1)**

- **FR-001**: The viewer MUST keep a local cache of the data it reads from Stash (currently the
  server summary, the scene picker lists, and scene details used for playback), stored per server
  profile. The cache MUST survive restarts.
- **FR-002**: When a screen's data is in the cache, the viewer MUST show it immediately and then
  refresh it from the server in the background. When fresh data differs, it MUST update the
  screen in place. It MUST keep scroll position and selection, and MUST NOT show a full-screen
  loading state.
- **FR-003**: When the server can't be reached, the viewer MUST still show cached screens,
  without any cache wording. It MUST report "server unreachable" through the connection indicator
  and the notification centre (constitution Principle IX). Actions that need the server, such as
  playback, MUST say why they're unavailable.
- **FR-004**: The cache MUST be safely discardable (Principle I). Deleting or corrupting it MUST
  cost only time, never data. It MUST NOT be the only place any user data or setting is kept.
- **FR-005**: The viewer's Settings MUST offer a "Clear cache" action, shown with the cache's
  current size. It deletes the cached data and reports the space freed. It needs no
  confirmation, because nothing is lost. No other screen shows cache controls, sizes, or
  messages. The cache needs no routine user action: it keeps itself current (FR-002), within its
  limit (FR-008), and repairs itself (FR-010).
- **FR-006**: The cache MUST support targeted invalidation. A write the viewer makes MUST update or
  drop exactly the cached entries it affects and no others. The viewer makes no writes in
  Phase 0, so this capability MUST be proven by automated tests with simulated writes.
- **FR-007**: Cached data MUST never be shown for a different server than the one it came from.
  This includes after a profile's address changes or the server at an address is replaced.
- **FR-008**: The cache MUST stay under a size limit of 5% of the free space on the disk that holds
  it, never below 64 MB and never above 10 GB, per profile. The limit MUST be re-checked when the
  cache opens and as it grows, so a filling disk shrinks it. It evicts the least recently used
  entries first.
- **FR-009**: Deleting a server profile MUST delete its cached data.
- **FR-010**: A damaged, unreadable, or incompatible cache MUST be discarded and rebuilt
  automatically, without an error dialog.
- **FR-011**: The cache MUST NOT change what the viewer sends to Stash beyond avoiding repeat
  reads. Background refreshes MUST follow Principle IV: the same narrowly scoped requests, and no
  more requests per screen than without the cache.

- **FR-011a**: Repeated background-refresh failures for a screen MUST post one `background`
  notification in the notification centre (keyed per screen, updated in place, not a toast
  unless it keeps failing for over a minute). This completes feature 004 FR-018's list of
  producers.

**Performance harness (US2)**

- **FR-012**: The developer MUST be able to run the performance harness with a single command
  against a configured server profile. It then runs unattended in the real app window.
- **FR-013**: The harness MUST measure:
  - cold start to an interactive connected view;
  - view navigation first paint between the app's existing screens;
  - input acknowledgement for a control press;
  - scroll frame rate on a list of at least 10,000 items;
  - playback open-to-first-frame and seek-to-frame, from the mpv spike.

  Cache-dependent budgets are measured with a warm cache, as Principle VI specifies. Cold start
  is measured both warm and with a cleared cache.
- **FR-014**: Each measurement MUST be repeated enough times to report a median and a 95th
  percentile. Each MUST be compared with its Principle VI budget and marked pass or fail.
- **FR-015**: The harness MUST save each report in both a human-readable and a machine-readable
  form. When a previous report exists, it MUST show the change for each measurement and flag any
  that got more than 20% slower.
- **FR-016**: The harness MUST mark a measurement invalid, not failed, when its conditions weren't
  met. Examples are a hidden window during frame measurements, or a server unreachable mid-run.
- **FR-017**: The harness MUST NOT write to Stash, and MUST NOT contact anything other than the
  configured Stash server.
- **FR-018**: The harness MUST be available in development builds only. It MUST NOT add
  measurement screens or controls to the viewer users run.

**CI (US3)**

- **FR-019**: An automated check MUST run on every push and pull request to the GitHub
  repository. It MUST run all of the following, and fail if any one fails:
  - Rust formatting;
  - Rust lint with no warnings;
  - all Rust tests (core, player, app);
  - the generated-bindings drift check;
  - frontend lint and formatting;
  - frontend type check;
  - all frontend tests.
- **FR-020**: A failing run MUST identify which step failed.
- **FR-021**: The check MUST use only recorded fixtures and generated test media. It MUST NOT
  need a private Stash server, API keys, or other secrets.
- **FR-022**: The README MUST show the current check status of the main branch.
- **FR-023**: The roadmap's Phase 0 items (cache layer, performance harness, CI) MUST be ticked
  and linked to this spec once each is done.

### Key Entities

- **Cached entry**: a copy of data read from one Stash server for one view. It has the server it
  came from, what it holds (e.g. the recent-scenes list or one scene's details), when it was
  fetched, when it was last used, and its size. It's discardable and never authoritative.
- **Server identity**: what the viewer uses to tell whether the server behind a profile is the
  same one that filled the cache (e.g. its address plus an identifier the server reports).
- **Performance report**: one harness run. It holds the date, the app version, the machine, the
  server profile (by display name only), and a list of measurements. Each measurement has a name,
  a budget, a median, a 95th percentile, a pass/fail/invalid result, and the change from the
  previous report.
- **Check run**: one CI run. It has its commit, the steps with their results, and the overall
  result.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A screen viewed before appears in under 150 ms when reopened, including right
  after a restart, in at least 95% of attempts against a LAN server. This compares with the
  current behaviour of waiting for the server on every visit.
- **SC-002**: Cold start to an interactive connected view takes under 2 seconds with a warm
  cache (Principle VI).
- **SC-003**: After data changes on the server, the change is visible in an open screen within
  one background refresh. No case is observed where stale data persists after a successful
  refresh.
- **SC-004**: Clearing a profile's cache always leaves the library unchanged, confirmed by
  comparing the server's library counts before and after. The next visit loads from the server.
- **SC-005**: Automated tests show that a simulated write updates or drops 100% of the affected
  cached entries and 0 unaffected ones.
- **SC-006**: A full harness run completes unattended in under 10 minutes. It reports a value
  and a result for 100% of the measurements in FR-013.
- **SC-007**: Two back-to-back harness runs on an unchanged build agree within 10% (median) for
  each measurement, so real regressions stand out from noise.
- **SC-008**: The CI check finishes in under 15 minutes. It passes on the main branch at the end
  of this feature, and a deliberately broken commit fails with the failing step named.

## Assumptions

- **Cache content**: this feature caches the data responses the viewer reads today (server
  summary, scene lists, scene details). Image and thumbnail caching arrives with Phase 1's grids,
  which will reuse this cache's rules (per server, discardable, size-limited).
- **Notification centre and Settings**: both exist now (built in feature 004). The connection
  watcher already posts "Server unreachable" / "Reconnected" (key `connection:<profile>`), so
  FR-003's notification is done. "Clear cache" goes on Settings → Troubleshooting, where a slot
  is marked (`ui/src/settings/TroubleshootingPage.tsx`).
- **Freshness**: cached data is always shown first and always refreshed in the background
  ("show cached, then refresh"). There is no time-based expiry that hides cached data, because
  showing something instantly and correcting it within one refresh matches Principle VI better
  than waiting.
- **Size limit**: a share of free disk space (5%, 64 MB–10 GB) adapts to the machine. Data
  responses need a few MB (about 50–70 MB even for every scene's details in a 35,000-scene
  library); the headroom is for Phase 1's thumbnails and covers, which reuse this cache. The
  share isn't exposed in the UI yet; it can become a viewer preference in Phase 3.
- **Harness scope**: measurements run on the developer's machine against their Stash server, like
  the mpv spike's measurement mode, which the harness absorbs. Shared CI machines have no GPU and
  noisy timing, so CI doesn't run the harness or gate on performance.
- **Scroll list**: until Phase 1 adds real grids, the harness scrolls a synthetic 10,000-item
  list shown only in harness runs. Phase 1 switches it to the real scene grid.
- **Semantic query budget**: Principle VI's "local semantic queries < 200 ms" budget has nothing
  to measure until Phase 4, so it's out of scope here.
- **CI platform**: GitHub Actions on Linux, where the existing check already runs. Windows and
  macOS builds in CI are out of scope, because Linux is the primary platform (constitution
  Technology & Platform Constraints).
- **Branch protection**: requiring a passing check before merge is a GitHub repository setting
  the owner can turn on. It's recommended, but it isn't part of this feature's deliverables.
- **Writes**: the viewer makes no writes to Stash until Phase 2, so this feature's invalidation
  is exercised by tests only. The spike's read-only rule stays in place.
