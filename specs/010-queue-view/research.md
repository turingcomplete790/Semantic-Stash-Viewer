# Research: Queue View (010)

Decisions for the plan. Each builds on what 007 and 008 already have, so the queue view is mostly
composition: Scenes' results and paging, the scene page's panel, and the one player.

## R1. The panel is the scene page, embedded

- **Decision**: the queue keeps a `SceneState` (008) for the selected scene inside the Scenes tab's
  state and draws its existing `details_view` in the panel: performers, tags (typed and coloured),
  studio, groups, markers, files and galleries sections, chip cards, all unchanged. The panel is
  laid out vertically: the player (or the cover with Play, before playing) across the top at 16:9,
  the details scrolling below.
- **Rationale**: FR-003 asks for the scene page's details; reusing its state machine keeps one
  implementation, one request path (`SceneDetails`, files, galleries, chip cards on rest), and the
  same tests.
- **Alternatives**: a slimmer "queue details" summary (rejected: a second details view to keep in
  step with the scene page); opening the real scene page in a split tab (rejected: tabs are one
  screen each, Principle IX).

## R2. The one player, drawn in the panel

- **Decision**: the session's "is the player visible" rule gains a case: the owner tab is a Scenes
  tab in Queue mode whose playing scene is the one now playing. The session view then draws the
  queue with the player in the panel's media slot (as it draws the scene page with the player in
  its media slot today). Fullscreen, the now-playing bar in other tabs, Escape, and closing the
  player behave as in 007/008.
- **Rationale**: one player for the app (007) and every 008 playback feature (resume, chapters,
  seek-bar thumbnails, play history, transcode fallback) come for free.
- **Alternatives**: a second, small player in the panel (rejected: Principle V's one media player,
  and two decoders at once).

## R3. Selection, the rest timer, and the cover

- **Decision**: moving the selection with the keyboard selects at once (the row highlight, and the
  panel's title and the card's own still from the thumbnail cache, so no request) and starts a
  250 ms rest timer; when it comes due on the same selection, the embedded `SceneState` is entered,
  loading details and the cover through the cache. A pointer click is deliberate, so it loads at
  once. Leaving a scene keeps nothing in flight that a later result could misapply (generation
  checks, as 008's chip rest).
- **Rationale**: FR-005 and SC-003 (no requests for scenes only passed over); 250 ms sits between
  the 200 ms preview rest and the 400 ms chip rest, and keyboard repeat (about 30 per second) never
  rests that long on a passed row.
- **Alternatives**: loading on every move (rejected: one request per row passed); a longer rest
  (rejected: sluggish when stepping one at a time).

## R4. Play next, Next, and Previous across pages

- **Decision**: the playback region reports when the playing scene reaches its end (the snapshot
  turning to Ended). If the owner tab is in Queue mode with Play next on, the session plays the
  next card on the page and selects it. After the page's last card it asks the tab for the next
  page with a "play the first card when it arrives" marker; Previous before the first card asks for
  the previous page with "play the last". On the last page's last card, nothing more plays and the
  player shows Replay (as at any end). A playback error never advances (FR-012). The next scene
  starts at its resume point when "Resume where you left off" is on (008).
- **Rationale**: the queue is the tab's results (spec Assumptions), so crossing pages is a page
  change, which 008 already makes instant from the cache or one request.
- **Alternatives**: loading all pages up front (rejected: Principle IV paging, no infinite
  scroll); stopping at the page's end (rejected: SC-004 asks for ten in a row, and pages can be
  smaller).

## R5. Keys and focus

- **Decision**: the queue view has two focus areas, the list (default) and the panel; F6 moves
  between them. With the list focused, ↑ ↓ (and Home, End, `[` `]` for pages) move the selection
  and Enter plays it; with the panel focused (or in fullscreen), the player's own keys apply, as on
  the scene page. In either: `n` and `p` play the next or previous scene, `a` toggles Play next, and
  `o` opens the selected scene's full page (Ctrl+O in a new tab). All are listed in keyboard help
  and Settings → Keyboard (contract).
- **Rationale**: the player binds the arrow keys (seek, volume); without a focus split, moving the
  selection while something plays would seek instead. F6 is the common pane-switch key. `n`, `p`,
  `a`, and `o` are free at every level, and the player doesn't bind them, so they fall through to
  the queue while it plays.
- **Alternatives**: Ctrl+arrows for the list (rejected: Ctrl+↑↓ is a common page-scroll gesture,
  and harder to discover); a modal "queue mode" (rejected: unnecessary state).

## R6. Saved state

- **Decision**: `Mode::Queue` joins the saved display modes; `ScenesState` saves `queue_selected`
  (the selected scene's id) and `play_next` (default on) with serde defaults, so session files from
  008 load unchanged (version 2 stays). A Stash saved filter's display modes don't include Queue, so
  applying one never selects it (Wall, List, and Grid map as in 008).
- **Rationale**: FR-001, FR-007, FR-009; additive fields avoid a session version bump.
- **Alternatives**: a separate queue record (rejected: the queue is the tab's results).

## R7. Layout and narrow windows

- **Decision**: the list takes a third of the window's width, at least 300 px; the panel takes the
  rest, at least 480 px. When the window can't fit both minimums, the list keeps its minimum and
  the panel drops the details (player or cover only); below that, the list alone shows and choosing
  a scene opens its full page. List rows reuse the List mode's row parts at a compact height (a
  small still, title, details line). Dragging the divider is the roadmap's separate "adjustable
  panel sizes" item.
- **Rationale**: the spec's narrow-window edge case; reusing the List row keeps text fitting and
  the windowed rendering that keeps scrolling within budget.
- **Alternatives**: a fixed split (rejected: unusable on small windows); a resizable divider now
  (deferred to its roadmap item).

## R8. Measuring it

- **Decision**: new harness rows: `queue-select` (selection to the panel's cover, title, and Play
  drawn; budget 150 ms), `queue-next` (a scene's end to the next one playing, including a page
  boundary; budget 1500 ms), `gql-queue-sweep` (moving through 20 rows quickly: 0 `SceneDetails`),
  `gql-queue-rest` (resting on one: 1 `SceneDetails`), and `scenes-scroll-1000-queue` (the list,
  at the lists' 1% budget). The bench ends a scene by seeking close to its end on a short Testing
  scene.
- **Rationale**: SC-001–SC-003, SC-005, and 008's request audit.
- **Alternatives**: none worth recording.
