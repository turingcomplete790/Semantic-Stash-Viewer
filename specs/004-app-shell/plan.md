# Implementation Plan: App Shell (Navigation Bar, Tabs, Notifications, Settings)

**Branch**: `004-app-shell` | **Date**: 2026-09-27 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/004-app-shell/spec.md`

## Summary

The viewer gets the app shell that constitution Principle IX requires, and every later feature
plugs into it:
- a Stash-like **navigation bar** (Home and Scenes for now, then the bell, Settings, and the
  current server);
- **tabs** with per-tab history and kept view state, restored per server on relaunch;
- a **"now playing" bar**, so playback continues while browsing other tabs;
- a **notification centre** for connection alerts, playback failures, and read-only Stash jobs
  with live progress;
- a **Settings** tab with Servers, Keyboard, Troubleshooting, and About pages.

Approach, from [research.md](research.md):
- **Routes (R1–R2)**: typed routes and per-tab history live in the SolidJS UI, with no router
  library. Background tabs stay mounted (LRU 8), and each view saves explicit view state.
- **Storage (R3)**: tabs and notifications are persisted by new discardable stores in
  `stash-core`, as JSON files in the local data dir.
- **Notifications (R4)**: the notification centre lives in the core. Keyed entries let an ongoing
  condition update in place, and a watcher maps connection-state transitions to notifications.
- **Jobs (R5)**: one `jobQueue` query per connection, then Stash's `jobsSubscribe` over one
  WebSocket (`graphql-transport-ws`). This was verified against the test instance: the REMOVE
  event carries the final status.
- **Video (R6)**: the video area is confined to the scene view by setting `GtkGLArea` margins
  from a UI-reported viewport. Leaving the scene's tab hides the GL area while mpv keeps playing.
- **Keyboard (R7)**: Stash's `g <key>` section shortcuts, browser-style tab shortcuts, and one
  keymap registry driving dispatch, the `?` overlay, and Settings → Keyboard.

## Technical Context

**Language/Version**: Rust 1.94 (workspace minimum 1.80); TypeScript 5.9

**Primary Dependencies**:
- New: `tokio-tungstenite` (with `rustls`), in `stash-core`, for the jobs subscription (R5).
  `rustls`, `tokio`, and `futures-util` are already in the dependency tree.
- Existing: tauri 2.11, tauri-specta, reqwest, graphql_client, SolidJS, libmpv2, gtk 0.18.
- No new UI packages: no router (R1) and no icon package (R9).

**Storage**: two discardable JSON files in `~/.local/share/semantic-stash-viewer/shell/`:
`tabs.json` (per-profile tab sets) and `notifications.json` (≤ 200). Written atomically,
versioned, and renamed aside if damaged (R3).

**Testing**:
- `cargo test`:
  - stores (limits, atomic save, recovery);
  - keyed notification updates;
  - the connection → notification mapping;
  - job events → notifications, from recorded fixtures of the observed subscription messages;
  - `jobQueue` parsing;
  - a local mock WebSocket server for the protocol handshake.
- Vitest:
  - navigation bar, tab strip, per-tab history, view-state save/restore, and the keymap;
  - now-playing bar, notification centre, toast rate limiting, and Settings pages;
  - moved screens.
- Manual checks V1–V8 in [quickstart.md](quickstart.md).

**Target Platform**: Linux, KDE on Wayland (primary). The video viewport and visibility use the
existing GTK video surface. Other platforms stay untested, as recorded in 002.

**Project Type**: Desktop app (Tauri: Rust core and web UI), extending features 001–002

**Performance Goals**:
- section and tab navigation first paint < 150 ms (SC-001, SC-002), including with 20 tabs
  (SC-007);
- controls respond < 50 ms (SC-003);
- the now-playing bar appears, and the video returns, within 150 ms (SC-008);
- a job appears and updates within 2 s (SC-009).

**Constraints**:
- no writes to Stash, and no new network destinations;
- storage is discardable;
- the webview's grandparent must stay the `GtkWindow` (002 R2), so the viewport changes only
  set the GL area's margins, never the widget tree;
- toasts never take focus.

**Scale/Scope**:
- 4 views (Home, Scenes, Scene, Settings), 4 Settings pages, and 2 navigation sections;
- 10 new commands and 1 new event;
- 2 new GraphQL operations (1 query, 1 subscription);
- about 20 keyboard shortcuts.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | How this plan complies | Status |
|---|---|---|
| **I. Stash is the system of record** | Only UI state (tabs) and notifications are stored locally, in discardable files that are renamed aside if damaged. Nothing is written to Stash. | ✅ Pass |
| **II. Semantic-tagging parity** | Not applicable. | ✅ N/A |
| **III. Brain/UI separation** | Notification centre, connection watcher, jobs watcher, WebSocket client, and both stores are in `stash-core` (no Tauri). `src-tauri` only exposes commands and events and adjusts the GTK video surface. The UI owns presentation state (routes, view state), which the core stores as opaque data. The UI does no file or network I/O. | ✅ Pass |
| **IV. Lean, batched GraphQL** | Jobs: one `jobQueue` query per connection and one push subscription, no polling (R5). Each selects only the job fields shown. The shell itself adds no data queries: views keep their existing ones. | ✅ Pass |
| **V. Native playback over transcoding** | Playback is unchanged (direct stream, mpv). It continues while other tabs are shown (FR-015). The resume and play-count sync deferred in 002 stays on the roadmap, unaffected. | ✅ Pass |
| **VI. Responsive UI** | Mounted background tabs and explicit view state give < 150 ms switches. Toasts and saves are asynchronous, notifications arrive by event, and every shell action is keyboard-accessible. Budgets are SC-001–SC-003 and SC-007–SC-008. | ✅ Pass |
| **VII. User-owned connection** | The WebSocket uses the same server, the profile's API key (header), and its strict-TLS setting. No other destinations. | ✅ Pass |
| **VIII. Web UI parity** | Mirrors Stash's navigation bar (order and `g` shortcuts, observed in its source), and adds job progress visibility (read-only). Job start and cancel remain Phase 3, recorded in the spec. | ✅ Pass |
| **IX. One app shell** | This feature implements it: navigation bar, tabs, notification centre, Settings, and quiet infrastructure (no cache or infrastructure banners, FR-031). | ✅ Pass |
| **Tech constraints** | One new crate (`tokio-tungstenite`), justified in R5. No new npm packages. | ✅ Pass |
| **Workflow gates** | Core tests with recorded fixtures (including the observed job messages), UI tests, fmt/clippy/lint, and the bindings drift check. | ✅ Pass |

**Post-design re-check (after Phase 1)**: ✅ unchanged.
- The contract keeps all file and network I/O in the core, and adds only read-only GraphQL (one
  query and one subscription).
- The data model caps every stored collection (history 50, tabs 100, view state 16 KB,
  notifications 200).
- Storage is discardable, with `.bak` recovery.

## Project Structure

### Documentation (this feature)

```text
specs/004-app-shell/
├── plan.md                    # This file
├── research.md                # Phase 0 (R1–R12)
├── data-model.md              # Phase 1
├── quickstart.md              # Phase 1 (V1–V8)
├── contracts/
│   └── shell-commands.md      # Phase 1
├── checklists/requirements.md
└── tasks.md                   # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/stash-core/
├── graphql/
│   ├── job_queue.graphql              # new
│   └── jobs_subscribe.graphql         # new
├── src/
│   ├── shell/                         # new
│   │   ├── mod.rs                     # TabSet/HistoryEntry types, limits
│   │   ├── store.rs                   # versioned atomic JSON files, .bak recovery (shared)
│   │   ├── tabs.rs                    # TabsStore: load/save/delete per profile
│   │   └── notifications.rs           # NotificationCenter: keyed post/update, read, dismiss, cap, events
│   ├── jobs/                          # new
│   │   ├── mod.rs                     # Job/JobStatus domain types, job → notification mapping
│   │   ├── ws.rs                      # graphql-transport-ws client over tokio-tungstenite (ApiKey, TLS)
│   │   └── watcher.rs                 # per-session: jobQueue seed, subscription, offline/reconcile
│   ├── connection/watch.rs            # new: SessionState transitions → notifications
│   └── adapter/jobs.rs                # new: typed JobQueue query + subscription document
└── tests/
    ├── shell_tabs.rs                  # new
    ├── notifications.rs               # new
    ├── jobs.rs                        # new (fixtures + mock WebSocket server)
    └── fixtures/stash-v0.31.1/
        ├── job-queue.json             # new (scrubbed)
        └── jobs-subscribe-events.json # new (observed ADD/UPDATE/REMOVE sequence)

src-tauri/src/
├── shell_commands.rs                  # new: tabs, notifications, open_log_folder, app_info
├── events.rs                          # + NotificationsChangedEvent
├── player_commands.rs                 # + player_set_viewport/player_set_video_visible; playback errors → notifications
├── video_surface/mod.rs               # + set_viewport (GL area margins), set_visible
├── commands.rs                        # delete_profile also deletes the tab set
└── lib.rs                             # wire NotificationCenter, watchers, new commands

ui/src/
├── App.tsx                            # becomes the shell host
├── shell/                             # new
│   ├── Shell.tsx                      # layout: NavBar, TabStrip, content, NowPlayingBar, Toasts
│   ├── NavBar.tsx                     # sections, overflow "More", bell, Settings, server menu
│   ├── sections.ts                    # Section registry (Stash order, shortcuts)
│   ├── routes.ts                      # Route union, titles
│   ├── tabs.ts                        # tab store: open/close/select/navigate/history, persistence via core
│   ├── viewState.ts                   # per-view state save/restore helper
│   ├── keymap.ts                      # shortcut registry + dispatcher (shell, scene scopes)
│   ├── KeyboardHelp.tsx               # "?" overlay
│   ├── NowPlayingBar.tsx
│   ├── NotificationCentre.tsx
│   ├── Toasts.tsx                     # rate-limited, aria-live, never focused
│   ├── icons.tsx                      # inline SVG (R9)
│   └── shell.css
├── views/                             # new: HomeView, ScenesView, SceneView (from SessionView/PlayerScreen)
├── settings/                          # new: SettingsView + Servers/Keyboard/Troubleshooting/About pages
├── state/notifications.ts             # new: event subscription + list
├── components/                        # existing; ConnectionIndicator → server menu, ProfileManager → Servers page body
├── player/                            # existing; keyboard.ts now registered through keymap (scene scope)
└── __tests__/shell/, __tests__/settings/  # new; existing tests updated for the moved screens
```

**Structure Decision**: this keeps the three existing crates and projects (`crates/stash-core`,
`crates/player`, `src-tauri`, `ui`). Shell logic that needs I/O (stores, notifications, jobs)
goes into new `stash-core` modules. `src-tauri` gains a command module and small video-surface
functions. The UI gains `shell/`, `views/`, and `settings/` folders, and the old top-level screens
become views inside them.

## Complexity Tracking

No constitution violations to justify.
