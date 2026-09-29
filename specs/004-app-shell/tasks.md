---

description: "Task list for 004-app-shell"
---

# Tasks: App Shell (Navigation Bar, Tabs, Notifications, Settings)

**Input**: Design documents from `specs/004-app-shell/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/shell-commands.md](contracts/shell-commands.md),
[quickstart.md](quickstart.md)

**Tests**: Included. The constitution's Development Workflow gate requires core tests with
recorded fixtures. Write each story's tests first and make sure they fail before implementing.

**Organization**: Tasks are grouped by user story (US1 navigation bar, US2 tabs, US3
notifications, US4 Settings). Each story is testable on its own once Foundational is done.

**Rules that apply to every task**:
- **No writes to Stash** (FR-032). Watching jobs is read-only. The only mutation ever sent is
  the `metadataScan` used manually on the **test instance** (`localhost:9998`) in quickstart V5.
- **No new network destinations** (Principle VII). The jobs WebSocket uses the active profile's
  server, `ApiKey` header, and strict-TLS setting.
- **Storage is discardable** (Principle I). Files live in
  `~/.local/share/semantic-stash-viewer/shell/`, and a damaged or newer-version file is renamed
  to `.bak` and treated as empty.
- **All file and network I/O stays in the core** (Principle III). The UI uses only typed
  commands and events.
- **Webview grandparent stays the window** (002 research R2). Viewport changes only set
  `GtkGLArea` margins, never the widget tree.
- **Quiet infrastructure** (FR-031). No cache or infrastructure banners on main screens.
- Regenerate `ui/src/bindings.ts` after any command, event, or DTO change (the `export-bindings`
  bin, or `cargo tauri dev`).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on unfinished tasks)
- **[Story]**: The user story the task belongs to (US1–US4)

## Path Conventions

The workspace from features 001–002:
- `crates/stash-core/`: headless core, which gets new `shell/` and `jobs/` modules;
- `crates/player/`: unchanged;
- `src-tauri/`: Tauri shell;
- `ui/src/`: SolidJS, which gets new `shell/`, `views/`, and `settings/` folders.

---

## Phase 1: Setup (Shared Infrastructure)

- [X] T001 Add `tokio-tungstenite` 0.30 (features `connect` + `rustls-tls-native-roots`; `rustls-native-certs` is already in the tree), plus direct `rustls` 0.23 (`aws-lc-rs`, the provider already in use) and `futures-util` for the lenient-TLS verifier and the socket stream, to `crates/stash-core/Cargo.toml` `[dependencies]`, and check `Cargo.lock` reuses the existing `rustls` 0.23 and `futures-util`. Confirm `cargo build -p stash-core` succeeds
- [X] T002 [P] Add the read-only operations from [contracts/shell-commands.md](contracts/shell-commands.md#stash-graphql-core--stash-read-only) as `crates/stash-core/graphql/job_queue.graphql` (`query JobQueue`) and `crates/stash-core/graphql/jobs_subscribe.graphql` (`subscription JobsSubscribe`), selecting only `id status description progress startTime endTime error` (Principle IV)
- [X] T003 [P] Record fixtures in `crates/stash-core/tests/fixtures/stash-v0.31.1/`:
  - `job-queue.json`: a `jobQueue` response with one RUNNING and one READY job, descriptions generic ("Scanning...", "Generating...");
  - `jobs-subscribe-events.json`: the `graphql-transport-ws` message sequence observed in research R5 (`connection_ack`, then `next` messages for ADD READY → UPDATE RUNNING progress 0 → UPDATE progress null → REMOVE FINISHED progress 1), plus a second sequence ending in REMOVE FAILED with an `error` string and one ending in CANCELLED.

  Capture from the test instance (`localhost:9998`) with a scan. Scrub any paths out of `description` and `error`
- [X] T004 [P] Create `ui/src/shell/icons.tsx` with inline SVG components (Home, Scenes, Bell, Settings, Server, Close, Plus, Back, Forward, Play, Pause, More) that use `currentColor` and `1em` sizing. The file header credits Lucide (ISC licence) for any copied glyphs (research R9)

**Checkpoint**: `cargo build --workspace` and `npm --prefix ui run typecheck` succeed.

---

## Phase 2: Foundational (Blocking Prerequisites)

**⚠️ CRITICAL**: No user story work can begin until this phase is complete. It gives the viewer
a shell layout with a content area, typed routes, and views extracted from today's screens,
with the video confined to the content area.

### Tests (write first, must fail)

- [X] T005 [P] Core store tests in `crates/stash-core/tests/shell_store.rs`:
  - save then load round-trips;
  - writes are atomic (a temp file is renamed; no partial file after a simulated crash between write and rename);
  - a file that fails to parse is renamed to `<name>.bak` and loads as empty;
  - a file with `version` greater than supported is renamed to `.bak` and loads as empty;
  - a missing file loads as empty.
- [X] T006 [P] UI route and view tests in `ui/src/__tests__/shell/routes.test.ts` and `ui/src/__tests__/shell/Shell.test.tsx`:
  - `routeTitle` for each variant (`home` → "Home", `scenes` → "Scenes", `scene` → its `title`, `settings` → "Settings");
  - an unknown variant renders a "view isn't available" message with a close action;
  - the shell renders the view for the current route in its content area;
  - while disconnected, the content area shows the server picker, or the connection form when no profiles exist (existing `App` behaviour).
- [X] T007 [P] Viewport test in `ui/src/__tests__/views/SceneView.test.tsx`: the scene view calls `commands.playerSetViewport` with its video element's window-relative rect (CSS px) on mount and on resize, and with `null` when the player snapshot reports fullscreen

### Implementation

- [X] T008 Implement the shared versioned store in `crates/stash-core/src/shell/store.rs`: a generic `JsonStore<T: Serialize + DeserializeOwned + Default>` with a path, a `CURRENT_VERSION`, `load()` (missing → default; parse failure or newer version → rename to `.bak`, log a warning, default), and `save(&T)` (write `.<name>.<uuid>.tmp` in the same directory, fsync, then rename, like `profiles/store.rs`). Add `pub mod shell;` in `crates/stash-core/src/lib.rs`, with `shell/mod.rs` re-exporting it. Makes T005 pass
- [X] T009 [P] Add `crates/stash-core/src/shell/mod.rs` limit constants from [data-model.md](data-model.md): `MAX_HISTORY = 50`, `MAX_TABS = 100`, `MAX_VIEW_STATE_BYTES = 16 * 1024`, `MAX_NOTIFICATIONS = 200`, and add `AppError::TabSetInvalid { reason }` and `AppError::OpenFailed { detail }` (commands already return `AppError`; `ProfileNotFound` is reused), with UI messages in `ui/src/messages/failures.ts`
- [X] T010 Define routes in `ui/src/shell/routes.ts`: `type Route = { kind: "home" } | { kind: "scenes" } | { kind: "scene"; sceneId: string; title: string } | { kind: "settings"; page: "servers" | "keyboard" | "troubleshooting" | "about" }`, plus `routeTitle(route)`, `routeIcon(route)`, `isRoute(value: unknown): value is Route` (for restored data), and `sameView(a, b)`
- [X] T011 [P] Create the section registry `ui/src/shell/sections.ts`:
  - fields: `id`, `label`, `icon`, `shortcut`, `route`, `needsServer`, `order`;
  - initial entries: `home` (`g h`, needsServer true) and `scenes` (`g s`, needsServer true);
  - document the Stash order and keys for later sections in a comment (`images g i`, `groups g v`, `markers g k`, `galleries g l`, `performers g p`, `studios g u`, `tags g t`; research R7).
- [X] T012 [P] Create the keymap registry and dispatcher in `ui/src/shell/keymap.ts`:
  - `register({ id, keys, scope: "shell" | "scene", description, run })` and `bindings()` for listing;
  - one `keydown` listener that supports modifier chords (`Ctrl+T`) and two-key sequences (`g s`, 1 s timeout);
  - sequences and unmodified single keys are skipped while a text input (reuse `TEXT_INPUT_TYPES` from `ui/src/player/keyboard.ts`), `textarea`, `select`, or contenteditable has focus, but modifier chords still work;
  - `scene` scope handlers run only when `isSceneActive()` is true.

  Register the existing player map (`handlePlayerKey`) as `scene` scope instead of its own window listener
- [X] T013 Extract the views into `ui/src/views/`:
  - `HomeView.tsx`: the content of `SessionView`/`ServerSummary`, i.e. server info, retry, and update-key actions;
  - `ScenesView.tsx`: the picker part of `ui/src/player/PlayerScreen.tsx`, with Recently added and Test scenes plus Shuffle. Choosing a scene calls `navigate({ kind: "scene", sceneId, title })` instead of opening the player in place;
  - `SceneView.tsx`: the player part, with the transparent video area and `Controls`. It calls `playerOpen(sceneId)` when shown for a scene that isn't loaded.

  Keep the existing behaviour and move the tests in `ui/src/__tests__/player/PlayerScreen.test.tsx` to `ui/src/__tests__/views/` (depends on T010)
- [X] T014 Create the shell layout `ui/src/shell/Shell.tsx` and `ui/src/shell/shell.css`, and make `ui/src/App.tsx` host it:
  - a 48 px top bar slot (the navigation bar, US1), a 36 px tab strip slot (US2), the content area, a bottom now-playing slot (US2), and a toast region (US3);
  - opaque background everywhere except the scene view's video area;
  - in this phase, a single current route with `navigate(route)` replacing it (tabs come in US2).

  Keep the disconnected routing from today's `App.tsx` (connection form / server picker), the `KeyPrompt` dialog, and the `ProfileManager` dialog until US4 moves it. Makes T006 pass (depends on T010, T013)
- [X] T015 [P] Add `player_set_viewport(viewport: Option<Viewport>)` in `src-tauri/src/player_commands.rs` and `set_viewport(Option<Rect>)` in `src-tauri/src/video_surface/mod.rs`:
  - it runs on the GTK main context through the existing thread-local `VIDEO_AREA`;
  - it sets the `GtkGLArea`'s `margin_top`, `margin_start`, `margin_end`, and `margin_bottom` from the rect and the window size (CSS px × scale factor where GTK needs device px);
  - `None` clears the margins.

  Register it in `specta_builder` in `src-tauri/src/lib.rs`. Never change the widget tree (research R6)
- [X] T016 Make `SceneView` report its video area with a `ResizeObserver` (and on window resize) to `commands.playerSetViewport`, and pass `null` while `snapshot.fullscreen` is true. Makes T007 pass (depends on T013, T015)
- [X] T017 Regenerate `ui/src/bindings.ts` and run the full gate (fmt, clippy `-D warnings`, `cargo test --workspace`, bindings drift, UI lint/typecheck/test)

**Checkpoint**: the app runs with a layout shell. Home, Scenes, and Scene render as views inside
the content area, video plays inside the content area only, and every existing capability still
works.

---

## Phase 3: User Story 1 - Find my way around with a navigation bar (Priority: P1) 🎯 MVP

**Goal**: a Stash-like navigation bar with Home and Scenes, the bell, Settings, and the current
server, all mouse- and keyboard-reachable.

**Independent Test**: quickstart V1. Every entry is reachable by click and by `g h` / `g s` /
`g z` / `g n`, the current section is highlighted, only Settings and the bell show while
disconnected, overflow works, and the server menu works as today.

### Tests for User Story 1 (write first, must fail)

- [X] T018 [P] [US1] Navigation bar tests in `ui/src/__tests__/shell/NavBar.test.tsx`:
  - shows the viewer name, Home, and Scenes in registry order, then the bell, Settings, and the server entry;
  - no entries for sections not in the registry;
  - highlights the current section;
  - clicking the viewer name goes Home;
  - while disconnected, shows only Settings and the bell;
  - `g h`, `g s`, `g z`, and `g n` navigate or open, but not while an input has focus;
  - with a narrow container (mocked `ResizeObserver` widths), sections that don't fit appear under "More";
  - the server entry's menu offers switch server, update API key, and manage servers.

### Implementation for User Story 1

- [X] T019 [US1] Implement `ui/src/shell/NavBar.tsx`:
  - the viewer name (button → Home), section entries from `sections.ts` filtered by `needsServer` and the connection state, with `aria-current="page"` on the current one;
  - the right-hand group: a bell button (opens the notification centre panel; an empty "No notifications" panel until US3), Settings (navigates to `{ kind: "settings", page: "servers" }`), and the server entry;
  - `nav` landmark semantics and visible focus (FR-001–FR-004, FR-006)
- [X] T020 [US1] Move the `ConnectionIndicator` menu into the navigation bar's server entry in `ui/src/components/ConnectionIndicator.tsx` (or a new `ui/src/shell/ServerMenu.tsx` that reuses it). It shows the status and security state, lists saved servers to switch to, updates the API key (opens `KeyPrompt`), and opens server management (FR-005)
- [X] T021 [US1] Add the overflow behaviour to `NavBar.tsx`: measure with a `ResizeObserver`, and move section entries that don't fit into a "More" menu button, keeping their order (FR-007)
- [X] T022 [US1] Register the section shortcuts (`g h`, `g s`) and `g z` (Settings) and `g n` (notification centre) in `ui/src/shell/keymap.ts` from the registry, then mount `NavBar` in `Shell.tsx`'s top bar slot. Makes T018 pass
- [X] T023 [US1] Run the UI tests and the full gate. Then manual check quickstart V1 with the user

**Checkpoint**: US1 is usable on its own: a Stash-like navigation bar over the single-view shell.

---

## Phase 4: User Story 2 - Keep several views open in tabs (Priority: P2)

**Goal**:
- tabs with per-tab history and kept view state, restored per server;
- browser-style shortcuts and a `?` help overlay;
- a "now playing" bar, so playback continues while browsing.

**Independent Test**: quickstart V2, V3, and V7.

### Tests for User Story 2 (write first, must fail)

- [X] T024 [P] [US2] Core tab store tests in `crates/stash-core/tests/shell_tabs.rs`:
  - save/load per profile round-trips;
  - rejects with `TabSetInvalid` when there are 0 tabs, `selectedTabId` is missing, there are more than 100 tabs, a tab has more than 50 history entries, `index` is out of range, or a `viewState` exceeds 16 KB;
  - `delete_profile_tabs` removes only that profile;
  - entries for profile IDs not in the profile store are dropped on load;
  - a damaged `tabs.json` → `.bak` and `None`.
- [X] T025 [P] [US2] UI tab store tests in `ui/src/__tests__/shell/tabs.test.ts`:
  - navigating the current tab pushes history (capped at 50, dropping the oldest);
  - back/forward move the index;
  - open-in-new-tab inserts after the current tab and selects it;
  - closing the selected tab selects the neighbour;
  - closing the last tab leaves one Home tab;
  - reopen-closed restores up to 10 (session only);
  - `select(n)` / last tab work;
  - saves are debounced 500 ms for view state and immediate for structure (mocked `commands.shellSaveTabs`);
  - loading `null` starts with one Home tab;
  - restored routes that fail `isRoute` become "unavailable" tabs.
- [X] T026 [P] [US2] UI tab strip and keyboard tests in `ui/src/__tests__/shell/TabStrip.test.tsx`:
  - renders titles and marks the selected tab;
  - click selects, middle-click closes, and the close button closes;
  - Ctrl+T, Ctrl+W, Ctrl+Tab / Ctrl+Shift+Tab, Ctrl+PgDn / PgUp, Ctrl+1–8, Ctrl+9, Ctrl+Shift+T, Alt+Left / Right, and mouse buttons 3 / 4 act as in [contracts/shell-commands.md](contracts/shell-commands.md#keyboard-map-ui-research-r7);
  - Ctrl+click and middle-click on a navigation bar section open a new tab;
  - `?` opens the keyboard overlay listing every registered binding.
- [X] T027 [P] [US2] View state and keep-alive tests in `ui/src/__tests__/shell/viewState.test.tsx`:
  - a view using `useViewState` gets its saved state back after its tab is hidden and shown, and after it's unmounted (more than 8 tabs) and remounted;
  - `ScenesView` restores its scroll offset;
  - at most 8 tabs are mounted, least recently used unmounted first.
- [X] T028 [P] [US2] Now-playing tests in `ui/src/__tests__/shell/NowPlayingBar.test.tsx`:
  - appears when a scene is loaded and its tab isn't selected, showing the title, play/pause (calls `playerTogglePause`), and "back to scene" (selects its tab);
  - hidden when the scene's tab is selected, in fullscreen, or when idle;
  - leaving the scene tab calls `playerSetVideoVisible(false)` and returning calls `true`;
  - closing the scene's tab calls `playerClose`;
  - a restored scene tab shows its title and a Play button and doesn't call `playerOpen` until shown. It doesn't call it at all while another scene is loaded (research R10).

### Implementation for User Story 2

- [X] T029 [US2] Implement the core tab types and store in `crates/stash-core/src/shell/tabs.rs`:
  - `HistoryEntry { route: serde_json::Value, view_state: Option<serde_json::Value> }`, `Tab { id: String, history: Vec<HistoryEntry>, index: u32 }`, and `TabSet { tabs: Vec<Tab>, selected_tab_id: String }` (serde camelCase, specta; the route and view state are opaque JSON);
  - validation against the limits verbatim: "1–100 tabs", "1–50 entries", "`0 ≤ index < history.length`", "view state ≤ 16 KB serialised", "`selectedTabId` must match a tab";
  - a `TabsStore` on `JsonStore<TabsFile { version, profiles: HashMap<Uuid, TabSet> }>` with `load(profile)`, `save(profile, set)` (validated, then written on a background task), `delete(profile)`, and `prune(known_profiles)`.

  Makes T024 pass
- [X] T030 [US2] Add `shell_load_tabs(profile_id)` and `shell_save_tabs(profile_id, tabs)` in a new `src-tauri/src/shell_commands.rs`:
  - create the `TabsStore` at `local_data_dir/semantic-stash-viewer/shell/tabs.json` in `lib.rs` setup and add it to `AppState`;
  - make `delete_profile` in `src-tauri/src/commands.rs` also delete the profile's tab set;
  - register the commands in `specta_builder` and regenerate bindings.
- [X] T031 [US2] Implement the UI tab store `ui/src/shell/tabs.ts`:
  - operations: `navigate(route, { newTab })`, `back()`, `forward()`, `select(id | index)`, `close(id)`, `reopenClosed()`, `move(from, to)`;
  - `currentRoute()`, and `tabs()`/`selectedId()` signals;
  - per-profile load on connect / profile switch (save the old set first) through `shellLoadTabs`/`shellSaveTabs`, with debouncing as in T025;
  - FR-013 last-tab rule and the FR-009 new-tab rule.

  Replace the single-route state from T014. Makes T025 pass
- [X] T032 [US2] Implement `ui/src/shell/viewState.ts`:
  - `useViewState<T>(key, initial)` returns `[state, setState]`, backed by the current history entry's `viewState`, and a `bindScroll(el)` helper that saves `scrollTop`/`scrollLeft` on hide and debounced scroll, and restores them on mount;
  - adopt it in `ScenesView` (scroll offset) and `SettingsView` (current page).

  *Implemented as:* each tab's pane is its own scroll container and saves and restores `scroll` per history entry for every view (`TabPanes.tsx`). `useViewState` is available for other state. The Settings page lives in its route (`settings {page}`), so it needs no view state.
- [X] T033 [US2] Render tabs with keep-alive in `Shell.tsx`: keep up to 8 most recently used tabs mounted (hidden ones get `hidden` plus `inert`), unmount older ones, and render only the selected tab visibly. Makes T027 pass (depends on T031, T032)
- [X] T034 [US2] Implement `ui/src/shell/TabStrip.tsx`: the tab list with titles and icons (from `routeTitle`/`routeIcon`), a close button, middle-click to close, and a new-tab button; minimum tab width, then horizontal scroll; `role="tablist"`/`tab` semantics. Add back/forward buttons at its left
- [X] T035 [US2] Register the tab and history shortcuts in `keymap.ts`, and handle mouse buttons 3/4 on the window. Make NavBar entries honour Ctrl+click and middle-click as new tab (FR-009, FR-011, FR-012)
- [X] T036 [US2] Implement `ui/src/shell/KeyboardHelp.tsx`: a `?` overlay listing `bindings()` grouped by scope, closed with Escape. Makes T026 pass
- [X] T037 [US2] Add `player_set_video_visible(visible)` in `src-tauri/src/player_commands.rs` and `set_visible(bool)` in `src-tauri/src/video_surface/mod.rs`: show or hide the GL area on the GTK main context, while playback continues (research R6). Register it and regenerate bindings
- [X] T038 [US2] Implement `ui/src/shell/NowPlayingBar.tsx` and wire it into `Shell.tsx`:
  - a 56 px bottom bar that shrinks the content area, showing the title, play/pause, and back-to-scene;
  - visibility derived from the player snapshot and the tab set (data model "Now playing");
  - call `playerSetVideoVisible` on scene-tab selection changes;
  - closing the playing scene's tab calls `playerClose`;
  - hide it in fullscreen;
  - restored scene tabs show the title and Play, per research R10.

  Makes T028 pass (depends on T031, T037)
- [X] T068 [US2] Apply the rendering rules from research R11 to the player (out of ID order; added by the `/speckit-analyze` review):
  - in `ui/src/player/player.css`, add `will-change: opacity` to `.player-controls` and `.player-topbar`;
  - replace `.player-stage:has(.player-controls-root.idle) *` with `cursor: none` on `.player-stage:has(.player-controls-root.idle)` only (children inherit it);
  - in `ui/src/player/Controls.tsx`, position the seek-bar hover time with `transform: translateX(calc(<fraction> * <track width>px - 50%))` (or a CSS variable driving a `transform`) instead of the inline `left: %`.

  Keep the existing Controls tests passing, and add one asserting the hover bubble uses `transform` and not `left`
- [X] T039 [US2] Handle server switching in the tab store and shell: save the old profile's set, stop playback (existing core behaviour), then load the new profile's set or a Home tab (spec edge case, quickstart V7)
- [X] T040 [US2] Run the full gate. Then manual check quickstart V2, V3, and V7 with the user

**Checkpoint**: tabs work end-to-end, including playback across tabs and restore on relaunch.

---

## Phase 5: User Story 3 - See what needs my attention in one place (Priority: P3)

**Goal**: a notification centre in the core with connection alerts, playback failures, and
read-only Stash jobs, plus the bell badge and rate-limited toasts in the UI.

**Independent Test**: quickstart V4 and V5.

### Tests for User Story 3 (write first, must fail)

- [ ] T041 [P] [US3] Notification centre tests in `crates/stash-core/tests/notifications.rs`:
  - posting creates a notification, newest first;
  - posting with an existing `key` updates it in place (same `id`, new `updatedAt`), and sets it unread again only if severity rises;
  - `mark_read`, `dismiss`, and `dismiss_all` (keeps active jobs) work;
  - caps at 200 by dropping the oldest non-active entries, never an active job;
  - persists and reloads;
  - on load, active job entries become `unknown`;
  - every change is broadcast with the full list.
- [ ] T042 [P] [US3] Connection mapping tests in `crates/stash-core/tests/connection_watch.rs`, feeding `ConnectionSnapshot` sequences:
  - Connected → Offline posts one warning "Server unreachable" (`toast: true`, key `connection:<profile>`);
  - repeated Offline attempts post nothing new;
  - Offline → Connected updates the same entry to info "Reconnected" (`toast: true`);
  - AuthFailed → error "API key rejected" with a hint;
  - Failed → error with the failure's plain message;
  - the first connection at launch posts nothing;
  - Connecting/Idle post nothing.
- [ ] T043 [P] [US3] Jobs tests in `crates/stash-core/tests/jobs.rs`:
  - parse `job-queue.json`;
  - map each status to `JobStatus` (READY → queued, RUNNING → running, STOPPING → stopping, FINISHED → finished, FAILED → failed, CANCELLED → cancelled);
  - replay `jobs-subscribe-events.json` through the watcher to get a progress entry and then finished (info), failed (error, toast, Stash's error text), or cancelled;
  - offline marks active jobs `unknown`, and reconnect plus a `jobQueue` reconcile updates or ends them;
  - a local mock WebSocket server (tokio-tungstenite `accept_hdr_async`) asserts the handshake sends subprotocol `graphql-transport-ws` and the `ApiKey` header, then `connection_init`, then `subscribe` with the `JobsSubscribe` document, and answers `ping` with `pong`.
- [ ] T044 [P] [US3] UI notification tests in `ui/src/__tests__/shell/NotificationCentre.test.tsx`:
  - the bell badge counts unread items, and an activity marker shows while any job is queued, running, or stopping;
  - opening the centre lists newest first with severity, title, detail, and relative time, and calls `notificationsMarkRead`;
  - dismiss one and dismiss all call the commands;
  - job entries show a progress bar or indeterminate state and their status;
  - a `notifications-changed` event updates the list.
- [ ] T045 [P] [US3] Toast tests in `ui/src/__tests__/shell/Toasts.test.tsx`:
  - a notification with `toast: true` shows a toast in a `role="status"` `aria-live="polite"` region, and it never takes focus (`document.activeElement` is unchanged);
  - it hides after 5 s;
  - at most one toast per key per 10 s, and at most 3 on screen;
  - toasts are hidden in fullscreen.

### Implementation for User Story 3

- [ ] T046 [US3] Implement `crates/stash-core/src/shell/notifications.rs`:
  - the types from [data-model.md](data-model.md): `Notification { id, key, profile_id, kind, severity, title, detail, created_at, updated_at, read, toast, job: Option<JobProgress> }`, `JobProgress { status, progress, started_at, ended_at }`, and the enums (serde camelCase, specta);
  - a `NotificationCenter` with `post(NewNotification)` (keyed upsert), `mark_read(ids)`, `dismiss(id)`, `dismiss_all()`, `list()`, and `subscribe()` (a `tokio::sync::watch` or `broadcast` of the full list);
  - the cap "at most 200, newest first … Active job entries are never dropped";
  - persisted through `JsonStore<NotificationsFile>` at `shell/notifications.json`, with active jobs set to `unknown` on load.

  Makes T041 pass
- [ ] T047 [US3] Implement `crates/stash-core/src/connection/watch.rs`: a task that subscribes to `ConnectionManager::subscribe()`, tracks the last known state per profile, and posts the notifications in the research R4 table through `NotificationCenter`, using the existing plain-language failure messages where the core has them. Makes T042 pass
- [ ] T048 [US3] Implement `crates/stash-core/src/adapter/jobs.rs` (the typed `JobQueue` query with `graphql_client` and the `JobsSubscribe` document) and `crates/stash-core/src/jobs/mod.rs` (the `Job` domain type, `JobStatus` mapping, and the job → notification mapping, with keys `job:<profile>:<id>`)
- [ ] T049 [US3] Implement `crates/stash-core/src/jobs/ws.rs`:
  - a minimal `graphql-transport-ws` client over `tokio-tungstenite`: connect to `ws(s)://<base>/graphql` with the `Sec-WebSocket-Protocol: graphql-transport-ws` and `ApiKey` headers;
  - rustls: platform roots when strict TLS is on, and a verifier that accepts any certificate when it's off, matching `reqwest`'s `tls_danger_accept_invalid_certs`;
  - send `connection_init` and wait for `connection_ack` (10 s timeout), then `subscribe` with id "1", yielding `next` payloads as a stream;
  - reply `pong` to `ping`, and end the stream on `complete` or `error`.
- [ ] T050 [US3] Implement `crates/stash-core/src/jobs/watcher.rs`:
  - per connected session: run `jobQueue` once to seed, then open the subscription and apply ADD/UPDATE/REMOVE to notifications;
  - on session offline or socket loss, mark active jobs `unknown` and stop;
  - on reconnect, reconcile with `jobQueue` (jobs no longer queued become "ended while disconnected");
  - reconnect the socket with the session's backoff, and start/stop with the `ConnectionManager` session lifecycle.

  Makes T043 pass
- [ ] T051 [US3] Wire it all up in `src-tauri`:
  - in `src-tauri/src/lib.rs`, create the `NotificationCenter` (path `local_data_dir/.../shell/notifications.json`), start the connection watch and the jobs watcher, and add the centre to `AppState`;
  - add `NotificationsChangedEvent(Vec<Notification>)` (`notifications-changed`) to `src-tauri/src/events.rs`, forwarding every change;
  - add `notifications_list`, `notifications_mark_read`, `notification_dismiss`, and `notifications_dismiss_all` to `src-tauri/src/shell_commands.rs`;
  - in `src-tauri/src/player_commands.rs` `forward_player_state`, post `playback:<sceneId>` error notifications when the player enters `Error`, using the player error's plain message;
  - register everything and regenerate bindings.
- [ ] T052 [US3] Implement `ui/src/state/notifications.ts`: subscribe to `notifications-changed`, hydrate with `notificationsList`, and expose `unreadCount()`, `active()`, and the list
- [ ] T053 [US3] Implement `ui/src/shell/NotificationCentre.tsx`, a panel opened from the bell and `g n`:
  - newest first, with severity icon, title, detail, relative time, and a job progress bar (determinate or indeterminate) with status;
  - per-item dismiss and "Dismiss all";
  - mark visible items read on open.

  Connect the bell badge (unread count) and the activity marker in `NavBar.tsx`. Makes T044 pass
- [ ] T054 [US3] Implement `ui/src/shell/Toasts.tsx` in `Shell.tsx`'s toast region:
  - bottom-right, above the now-playing bar;
  - `role="status"`, `aria-live="polite"`, never focused;
  - hides after 5 s, at most one per key per 10 s, at most 3 visible, hidden in fullscreen.

  Makes T045 pass
- [ ] T055 [US3] Run the full gate. Then manual check quickstart V4 (library) and V5 (test instance) with the user

**Checkpoint**: alerts and jobs appear in one place. Feature 003's "server unreachable" can now
use the connection notification.

---

## Phase 6: User Story 4 - Manage servers and preferences in Settings (Priority: P4)

**Goal**: Settings as a tab with the Servers, Keyboard, Troubleshooting, and About pages. It
replaces the server-management dialog.

**Independent Test**: quickstart V6.

### Tests for User Story 4 (write first, must fail)

- [ ] T056 [P] [US4] Settings tests in `ui/src/__tests__/settings/SettingsView.test.tsx`:
  - the page list shows Servers, Keyboard, Troubleshooting, and About, and selecting a page updates the route's `page`;
  - Servers keeps the existing `ProfileManager` behaviour (add, edit, reorder, remove with confirmation; port the assertions from the existing ProfileManager tests);
  - Keyboard lists every `bindings()` entry;
  - Troubleshooting's "Open log folder" calls `commands.openLogFolder` and shows a plain error on `openFailed`;
  - About shows `appInfo().version` and the connected server's address and Stash version.
- [ ] T057 [P] [US4] Core test in `crates/stash-core/tests/shell_open.rs` (or a unit test in `src-tauri/src/shell_commands.rs`): the opener command resolves to `xdg-open <log dir>` on Linux, and the path is the app's fixed log directory, never taken from input (research R8)

### Implementation for User Story 4

- [ ] T058 [US4] Add `open_log_folder` and `app_info` to `src-tauri/src/shell_commands.rs`:
  - `open_log_folder` spawns `xdg-open` (Linux), `open` (macOS), or `explorer` (Windows) with `std::process::Command` on the known log directory, and maps a spawn failure to `AppError::OpenFailed`;
  - `app_info` returns `{ version: app.package_info().version }`.

  Register both and regenerate bindings. Makes T057 pass
- [ ] T059 [US4] Implement `ui/src/settings/SettingsView.tsx`: a two-column page list plus the page body, with the page taken from the route. Add pages:
  - `ServersPage.tsx`: the body of `ProfileManager` without the dialog frame;
  - `KeyboardPage.tsx`: from `keymap.bindings()`, the same source as the `?` overlay;
  - `TroubleshootingPage.tsx`: "Open log folder", plus a documented slot for feature 003's "Clear cache";
  - `AboutPage.tsx`: viewer version and server info.
- [ ] T060 [US4] Remove the `ProfileManager` dialog path: "Manage servers" (server menu) now navigates to `{ kind: "settings", page: "servers" }`, and the `managerOpen` state is dropped from `ui/src/App.tsx`/`Shell.tsx`. Makes T056 pass
- [ ] T061 [US4] Run the full gate. Then manual check quickstart V6 with the user

**Checkpoint**: all four stories work, and every capability from before the shell is still
there (SC-006).

---

## Phase 7: Polish & Cross-Cutting Concerns

- [ ] T062 [P] Quickstart V8 (damaged storage), done by hand: garbage `tabs.json` and `notifications.json` give a single Home tab, an empty centre, `.bak` copies, and no dialog
- [ ] T063 [P] Measure the shell's performance and record the numbers in `specs/004-app-shell/quickstart.md` under the relevant checks:
  - **SC-007 / SC-008**: with 20 tabs open, time tab switches and the now-playing bar appearing (a DevTools performance trace, or `performance.now()` logging in a debug build);
  - **SC-003**: time a control press (a navigation entry and the bell) from input to visible change. The target is under 50 ms;
  - **Playback regression against 002**: rerun `SSV_MEASURE_LONG` for 1080p (scene 10861, 5 min) and 4K HEVC (scene 2328, 2 min), with the window visible and the scene shown inside the shell (navigation bar over the video, viewport margins). Compare dropped frames and main-thread CPU with 002's decision record (0 drops, 6–7%), and note any difference in `specs/002-mpv-playback-spike/decision.md` under a "Re-measured in the app shell" line
- [ ] T064 [P] Update `README.md`: the status line mentions the app shell, the Layout table mentions `ui/src/shell/`, and the keyboard basics (`g` shortcuts, tab keys, `?`)
- [ ] T065 [P] Update `ROADMAP.md` Phase 1 "App shell": tick the navigation bar, tabs, notification centre, and Settings page items, and link `specs/004-app-shell/`
- [ ] T066 [P] Update `specs/003-complete-foundation/spec.md`:
  - Assumptions: the notification centre and Settings → Troubleshooting now exist (built in 004), so 003's "server unreachable" and "Clear cache" plug into them;
  - Requirements: add that repeated background-refresh failures post a `background` notification (keyed per screen, updated in place), which completes 004 FR-018's producer list.
- [ ] T067 Run the full gate: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, bindings drift (`cargo run -p semantic-stash-viewer --bin export-bindings` then `git diff --exit-code ui/src/bindings.ts`), `npm --prefix ui run lint`, `npm --prefix ui run typecheck`, and `npm --prefix ui test`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: none.
- **Foundational (Phase 2)**: depends on Setup. Blocks all stories (routes, views, shell layout,
  viewport, keymap, shared store).
- **US1 (Phase 3)**: depends on Foundational. MVP.
- **US2 (Phase 4)**: depends on Foundational. Uses US1's navigation bar for Ctrl+click (T035),
  but otherwise stands alone.
- **US3 (Phase 5)**: depends on Foundational. The bell button comes from US1 (T019). If US1
  isn't done, mount the centre from a temporary button.
- **US4 (Phase 6)**: depends on Foundational, and on US2's keymap bindings for the Keyboard page
  (T012 already provides `bindings()`).
- **Polish (Phase 7)**: after all stories.

### Story Dependencies

```text
Setup ─► Foundational ─┬─► US1 (MVP) ─┐
                       ├─► US2 ────────┤
                       ├─► US3 ────────┼─► Polish
                       └─► US4 ────────┘
```

Recommended order: US1 → US2 → US3 → US4, with a user check after each.

### Within Each Story

Tests first (they must fail) → core (`stash-core`) → commands and events (`src-tauri`) →
bindings → UI → checkpoint check.

### Parallel Opportunities

- Setup: T002, T003, and T004 in parallel after T001.
- Foundational: tests T005, T006, T007 in parallel; T009, T011, T012, and T015 in parallel with
  T008/T010.
- US2: tests T024–T028 in parallel; the core (T029–T030) runs alongside the UI tab store (T031).
- US3: tests T041–T045 in parallel; the core pieces T046, T047, and T048/T049 in parallel
  before T050.
- US4: T056 and T057 in parallel.
- Polish: T062–T066 in parallel.
- T068 (CSS) can run in parallel with any US2 task except T038 (both touch the player's look).

---

## Parallel Example: User Story 3

```bash
# Tests first, different files:
Task: "Notification centre tests in crates/stash-core/tests/notifications.rs"
Task: "Connection mapping tests in crates/stash-core/tests/connection_watch.rs"
Task: "Jobs tests in crates/stash-core/tests/jobs.rs"
Task: "UI notification tests in ui/src/__tests__/shell/NotificationCentre.test.tsx"
Task: "Toast tests in ui/src/__tests__/shell/Toasts.test.tsx"

# Independent core modules:
Task: "NotificationCenter in crates/stash-core/src/shell/notifications.rs"
Task: "Connection watch in crates/stash-core/src/connection/watch.rs"
Task: "graphql-transport-ws client in crates/stash-core/src/jobs/ws.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1)

1. Setup → Foundational. **Stop and check**: the app still does everything it did, with the
   video drawn only inside the content area.
2. US1: the navigation bar.
3. **Validate** quickstart V1 with the user, and commit once confirmed.

### Incremental Delivery

4. US2: tabs and the now-playing bar (V2, V3, V7).
5. US3: notifications and jobs (V4, V5).
6. US4: Settings (V6).
7. Polish (V8, measurements, docs, full gate).

Commit after each story, only after the user has tried it (constitution; user preference).

---

## Notes

- The UI never reads files or opens sockets; the core does (Principle III).
- Keep the keymap registry the single source for dispatch, the `?` overlay, and Settings →
  Keyboard.
- Toasts must never take focus; test it (T045).
- Don't write to Stash. The only mutation anywhere is the manual scan on the test instance in V5.
- Regenerate `ui/src/bindings.ts` whenever a command, event, or DTO changes.
