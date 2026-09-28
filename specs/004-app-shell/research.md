# Research: App Shell

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md) | **Date**: 2026-09-27

Findings marked **(observed)** were checked on the development machine, against the Stash test
instance (`localhost:9998`, disposable), or in Stash's own source (`~/Projects/stash`, v0.31
web UI).

---

## R1. Navigation model: typed routes and per-tab history, no router library

- **Decision**: Views are addressed by a small typed **route** union that lives in the UI:
  `home`, `scenes`, `scene {sceneId}`, and `settings {page}`. Later features add variants. Each
  tab owns its own history (a list of routes and a current index). There's no URL router.
- **Rationale**:
  - Browser-style routers (e.g. `@solidjs/router`) model one global history bound to the
    window URL. Tabs need one history per tab (FR-011), so a router would be fought rather than
    used.
  - A typed union makes "restore a tab" a plain data problem (serialise the route), and keeps
    exhaustiveness checks in TypeScript when variants are added.
  - No new dependency.
- **Alternatives rejected**:
  - `@solidjs/router` with one router instance per tab: heavier, URL-shaped, and awkward to
    persist.
  - Hash-based routing: single history.

## R2. Keeping tab state: mounted background tabs plus explicit view state

- **Decision**:
  - Background tabs stay **mounted but hidden**, most recently used first, up to **8 mounted
    tabs**. Older tabs are unmounted.
  - Every view reports its restorable state through one helper: scroll offsets, filters,
    selection, and typed text, as a small JSON object.
  - The state is stored on the tab's current history entry. It's written when the tab is hidden
    and on a short debounce while visible. It's read back when the view mounts.
- **Rationale**:
  - Keeping a view mounted makes switching back instant (SC-002, < 150 ms) and keeps anything
    the view holds in memory.
  - Explicit state covers the cases mounting can't: unmounted background tabs (FR-016), relaunch
    (FR-014), and back/forward within a tab.
  - Scroll position doesn't reliably survive `display: none` in WebKit. Explicit save/restore
    avoids depending on it.
- **Alternatives rejected**:
  - Unmount every background tab: each switch would rebuild the view and refetch (misses the
    budget once Phase 1 grids exist).
  - Keep everything mounted forever: memory grows without bound with many tabs (SC-007).

## R3. Where tabs and notifications are stored

- **Decision**: `stash-core` gets a `shell` module with two small stores. Both are JSON files in
  the viewer's **local data directory** (`~/.local/share/semantic-stash-viewer/shell/`), next to
  the logs:
  - `tabs.json`: per server profile, the ordered tabs, their histories and view states, and the
    selected tab;
  - `notifications.json`: undismissed notifications, capped at 200.

  Writes use the same atomic temp-file-and-rename approach as `profiles.json`. Each file has a
  version number. A damaged or newer-version file is renamed aside and the store starts empty
  (spec edge cases).
- **Rationale**:
  - Principle I allows local storage for UI preferences and discardable data. Both files are
    discardable: losing them costs open tabs or old alerts, never library data.
  - Keeping the files in the core (Principle III) makes them testable headlessly, and the UI
    never touches the filesystem.
  - The local data dir, not the config dir, marks them as disposable state rather than
    configuration.
  - View state is **opaque JSON to the core**. The core checks only size limits (≤ 16 KB per
    entry, ≤ 50 history entries per tab, ≤ 100 tabs per profile), because the view state's shape
    belongs to each UI view.
- **Alternatives rejected**:
  - `localStorage` in the webview: not reachable by core tests, and silently lost in some WebKit
    storage resets.
  - Putting tabs in `profiles.json`: mixes disposable state into configuration the user may
    back up or edit.

## R4. Notification centre lives in the core

- **Decision**:
  - A `NotificationCenter` in `stash-core` holds the list and persists it (R3). It emits the full
    list on every change as a `notifications-changed` event, since the list is at most 200 small
    items.
  - Producers:
    - a **connection watcher** that subscribes to the `ConnectionManager` snapshot broadcast;
    - the **jobs watcher** (R5);
    - the **player state forwarder** in `src-tauri`, for playback errors.
  - **Keyed conditions**: a notification may carry a key, e.g. `connection:<profile>` or
    `job:<profile>:<id>`. Posting with an existing key updates that entry in place (FR-019), and
    marks it unread again only if its severity rises.
  - **Toasts**: the core marks a notification as toast-worthy (warnings, errors, and
    reconnection). The UI shows toasts and rate-limits them: at most one per key per 10 s, and
    at most 3 on screen.
- **Connection mapping** (`SessionState` → notification, key `connection:<profile>`):

  | Transition | Notification |
  |---|---|
  | Connected → Offline | warning "Server unreachable" (toast) |
  | Offline → Connected | information "Reconnected" (same entry, toast) |
  | → AuthFailed | error "API key rejected", with a hint to update it (toast) |
  | → Failed (version refused, unreachable at first connect) | error with the failure's plain-language message (toast) |
  | Connecting, Idle | no notification |

  First connection at launch produces nothing on success. Only changes from a known-good state
  notify, so launching doesn't spam "Connected".
- **Rationale**:
  - Most events originate in the core (connection, jobs, player). Collecting them there keeps a
    single source of truth, and means notifications survive restarts without the UI being open.
  - Keys make "one entry per condition" a data rule, not UI logic.
- **Alternatives rejected**: a UI-only notification store (producers are in the core, and it
  would need its own persistence).

## R5. Watching Stash jobs: `jobQueue` plus the `jobsSubscribe` WebSocket subscription

- **Findings (observed)**:
  - Stash's schema has `jobQueue` (all current jobs) and a `jobsSubscribe` subscription. The
    subscription sends `JobStatusUpdate { type: ADD | UPDATE | REMOVE, job }`, where `Job` has
    `id`, `status` (READY, RUNNING, FINISHED, STOPPING, CANCELLED, FAILED), `description`,
    `progress`, `startTime`, `endTime`, and `error`.
  - A WebSocket to `/graphql` with subprotocol **`graphql-transport-ws`** and the **`ApiKey`
    header** on the handshake was accepted by the authenticated test instance (101, then
    `connection_ack`).
  - A scan started on the test instance produced ADD → UPDATE (RUNNING) → UPDATE → REMOVE (status
    FINISHED, progress 1) within about 50 ms of the job running. **REMOVE carries the final
    status**, so finished and failed jobs can be told apart.
- **Decision**:
  - On each successful connection, the core runs one `jobQueue` query to seed current jobs. It
    then keeps one `jobsSubscribe` subscription open for the session.
  - ADD and UPDATE post or update a job notification (`job:<profile>:<id>`, with progress and
    marked active). REMOVE turns it into "finished" (information), "failed" (error, toast, with
    Stash's error text), or "cancelled" (information).
  - When the session goes offline, running job entries become "status unknown" (spec edge case).
    On reconnect, `jobQueue` reconciles: jobs no longer in the queue become "finished or stopped
    while disconnected".
  - The WebSocket client is **`tokio-tungstenite`** with rustls. It honours the profile's
    strict-TLS setting: when strict checking is off, a certificate verifier that accepts any
    certificate is used, matching `reqwest`'s `tls_danger_accept_invalid_certs`. It speaks the
    small `graphql-transport-ws` protocol directly: `connection_init`, `subscribe`, `next`,
    `error`, `complete`, and `ping`/`pong`. The subscription document is typed with
    `graphql_client` like the other operations.
- **Rationale**:
  - A subscription is pushed only when something happens. Polling `jobQueue` every 2 s would
    send ~1,800 requests an hour, mostly empty (Principle IV).
  - The WebSocket goes to the same server with the same key (Principle VII: no new
    destinations).
  - SC-009 (within 2 s) is met with a wide margin.
- **Alternatives rejected**:
  - Polling: chatty (above).
  - A full GraphQL WebSocket client crate (e.g. `graphql-ws-client`): more code and dependencies
    than the ~150 lines the protocol needs.
- **New dependency**: `tokio-tungstenite` (with its `rustls` feature; the de facto standard async
  WebSocket crate, actively maintained; `rustls`, `tokio`, and `futures-util` are already in the
  dependency tree).

## R6. Video inside a tab: viewport margins, and hiding the video surface

- **Decision**:
  - **Viewport**: the scene view reports its video area's rectangle (CSS pixels, from a resize
    observer) with a new `player_set_viewport` command. `src-tauri` sets the `GtkGLArea`'s
    margins so mpv draws only inside that rectangle, below the navigation bar and tab strip and
    above the "now playing" bar. In fullscreen, the viewport is the whole window.
  - **Leaving the scene's tab**: the UI calls `player_set_video_visible(false)`. The GL area is
    hidden, and mpv keeps decoding and playing audio (FR-015). Coming back shows it again, and
    the next frame draws on the following render (SC-008, < 150 ms).
  - **Transparency**: the shell is opaque everywhere except the scene view's video area. That
    area is transparent, so the video shows through.
- **Found during implementation**:
  - GTK 3's `GtkOverlay` sizes its overlay children (the webview) from the **main child's
    allocation**. With margins on the GL area as the main child, the webview shrank with the
    video, and the loop ended in negative heights and no picture. The GL area now sits inside a
    full-window `GtkBox`, which is the overlay's main child. The webview's grandparent is still
    the window.
  - The shell grid must use named areas. Hiding an empty slot (the tab strip) otherwise moved the
    content area into an auto-sized row, and the absolutely positioned scene view collapsed to
    zero height.
  - Debug builds accept `SSV_DEBUG_OPEN=<scene id>` (open that scene once connected) and
    `SSV_DEBUG_NO_VIEWPORT=1` (ignore viewport reports), to check the video surface without
    clicking.
- **Rationale**:
  - GL area margins are pixel-exact and cost nothing per frame. mpv then letterboxes inside the
    content area instead of behind the navigation bar.
  - Observed in the mpv spike (decision.md): when nothing draws, mpv keeps playing and counts
    the undrawn frames as dropped. Hiding the area is therefore safe for playback. Dropped-frame
    stats are only meaningful while the video is visible, so the harness in feature 003 measures
    only then.
- **Alternatives rejected**:
  - mpv's `video-margin-ratio-*`: relative values that need recomputing on every resize, and
    mpv still draws the whole window.
  - Switching the video track off while hidden: turning it back on forces a seek and a visible
    stall.

## R7. Keyboard shortcuts

- **Findings (observed, Stash `ui/v2.5/src/components/MainNavbar.tsx`)**:
  - Stash binds two-key sequences for sections: `g s` Scenes, `g i` Images, `g v` Groups, `g k`
    Markers, `g l` Galleries, `g p` Performers, `g u` Studios, `g t` Tags, and `g z` Settings.
  - It binds `?` for help.
  - It orders sections Scenes, Images, Groups, Markers, Galleries, Performers, Studios, Tags.
- **Decision**:
  - **Sections**: Stash's `g <key>` sequences, plus `g h` Home and `g n` notifications (the
    viewer's additions). Later sections use Stash's keys.
  - **Tabs**:

    | Action | Shortcut |
    |---|---|
    | New tab | Ctrl+T |
    | Close tab | Ctrl+W |
    | Next tab | Ctrl+Tab or Ctrl+PgDn |
    | Previous tab | Ctrl+Shift+Tab or Ctrl+PgUp |
    | Tabs 1–8 | Ctrl+1–8 |
    | Last tab | Ctrl+9 |
    | Reopen closed tab | Ctrl+Shift+T |

  - **History**: Alt+Left and Alt+Right, plus mouse back/forward buttons (mouse buttons 3 and 4).
  - **Help**: `?` opens the keyboard overlay.
  - One **keymap registry** in the UI lists every shortcut, with its scope (shell or scene) and
    description. It drives dispatch, the `?` overlay, and Settings → Keyboard, so they can't
    drift apart.
  - The player's existing keys stay as they are, and apply only while a scene tab is selected.
    Shell shortcuts either use a modifier or are `g` sequences, and the player uses neither, so
    there's no conflict (spec edge case). Sequences and single keys are ignored while a text
    field has focus, as the player's handler does today.
- **Rationale**:
  - Stash users already know the `g` sequences, and browser users know the tab keys.
  - One registry prevents the overlay and handlers from disagreeing.

## R8. Opening the log folder

- **Decision**: a core-side `open_log_folder` command runs the platform's opener: `xdg-open` on
  Linux, `open` on macOS, and `explorer` on Windows. It uses `std::process::Command` on the known
  log directory.
- **Rationale**: a few lines, with no new dependency. The path is fixed by the app, not supplied
  by the UI, so nothing user-controlled reaches the command line.
- **Alternatives rejected**: `tauri-plugin-opener` (a plugin and a capability for one call).

## R9. Icons

- **Decision**: a handful of inline SVG icon components (home, scenes, bell, settings, server,
  close, plus, back, forward, play, pause) in `ui/src/shell/icons.tsx`. Glyphs are taken from
  Lucide (ISC licence, noted in the file header), or drawn by hand.
- **Rationale**: about 10 icons don't justify an icon package. Inline SVG inherits the text colour
  and scales with the font.
- **Alternatives rejected**: Font Awesome (Stash's choice): a package and a webfont for a few
  glyphs.

## R10. Screens that move into the shell

- **Decision**:

  | Today | In the shell |
  |---|---|
  | `SessionView` / `ServerSummary` | **Home** view |
  | `PlayerScreen` picker (recent + test scenes) | **Scenes** view |
  | `PlayerScreen` player (video + `Controls`) | **Scene** view (`scene {sceneId}` route) |
  | `ProfileManager` dialog | **Settings → Servers** page (same component contents, no dialog frame) |
  | `ConnectionIndicator` | **current-server entry** in the navigation bar (same menu) |
  | `ConnectionForm` / `ProfilePicker` | shown in the content area while disconnected, with only Settings and notifications in the navigation bar |
  | `KeyPrompt` | stays a dialog |

  - Opening a scene from Scenes follows FR-009: current tab, or a new tab with a modifier.
  - A restored scene tab shows the scene title and a **Play** button. It opens the scene paused
    at the start the first time it's shown, unless another scene is playing, in which case it
    waits for Play. This way a relaunch never auto-plays, and never interrupts current playback.
- **Rationale**: capabilities stay the same (FR-029, SC-006). The existing component tests move
  with their components.

## R11. Layout details

- **Decision**:
  - The navigation bar is 48 px tall, with the tab strip (36 px) under it. The "now playing" bar
    (56 px) sits at the bottom of the window and shrinks the content area, so it never covers
    content.
  - Toasts appear bottom-right, above the "now playing" bar. They're `role="status"` and
    `aria-live="polite"`, and never focused.
  - Sections that don't fit a narrow window move into a "More" menu, measured with a resize
    observer (FR-007).
  - Tabs shrink to a minimum width, then the strip scrolls horizontally.
- **Rationale**: matches desktop conventions, and keeps FR-015's "doesn't cover content" and
  FR-020's "never takes focus" testable.

## R12. App version for About

- **Decision**: a small `app_info` command returns the viewer's version from Tauri's package
  info. The server's address and version come from the existing connection snapshot.
- **Rationale**: no duplication of the version string in the UI build.
