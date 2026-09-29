# Quickstart & Validation: App Shell

**Feature**: [spec.md](spec.md) | **Contracts**: [contracts/](contracts/) |
**Model**: [data-model.md](data-model.md) | **Research**: [research.md](research.md)

## Prerequisites

| Need | Check |
|---|---|
| Everything from features 001 and 002 | see their quickstarts (libmpv, WebKitGTK, Node, Rust) |
| A saved profile for your library | `localhost:9999`, read-only use |
| The Stash test instance | `localhost:9998`, for job tests (starting a scan there is allowed; it's disposable) |

## Automated checks

```bash
cargo test -p stash-core   # tab/notification stores (limits, atomic save, damaged-file recovery),
                           # keyed notification updates, connection→notification mapping,
                           # job events → notifications (fixtures), jobQueue parsing
npm --prefix ui test       # navigation bar, tab strip + keyboard map, per-tab history and view
                           # state, now-playing bar, notification centre + toast rate limit,
                           # Settings pages, moved screens (existing tests updated)
```

The full gate is the same as the CI check: fmt, clippy with `-D warnings`, all tests, the
bindings drift check, UI lint and typecheck.

## Manual validation (Wayland/KDE)

Run `cargo tauri dev` from the repo root, connected to your library.

### V1: Navigation bar (US1, FR-001–FR-007)
1. **Expect**: a navigation bar showing:
   - the viewer's name;
   - Home and Scenes, and no other sections;
   - the bell, Settings, and the server name with its status.
2. Click each entry, then use `g h`, `g s`, `g z`, and `g n`. **Expect**: each opens in under
   150 ms, and the current section is highlighted.
3. Disconnect. **Expect**: only Settings and the bell remain, and the server picker shows.
4. Narrow the window. **Expect**: sections that don't fit move into "More".
5. Open the server menu. **Expect**: switch server, update API key, and manage servers work as
   before.

### V2: Tabs (US2, FR-008–FR-014, FR-016)
1. Open Home, then Ctrl+click Scenes and Ctrl+click Settings. **Expect**: three tabs.
2. Scroll the Scenes list, switch away and back (click, then Ctrl+Tab). **Expect**: the same
   scroll position, with no reload.
3. In one tab, go Home → Scenes → Settings, then Alt+Left twice, then the mouse forward button.
   **Expect**: the tab walks its own history.
4. Try Ctrl+T, Ctrl+W, Ctrl+1–9, Ctrl+Shift+T, and `?`. **Expect**: each works, and the overlay
   lists them all.
5. Close every tab. **Expect**: a Home tab remains.
6. Quit and relaunch. **Expect**: the same tabs in the same order, with the same one selected.
7. Open 20 tabs and repeat step 2. **Expect**: still under 150 ms.

### V3: Playback across tabs (FR-015, SC-008)
1. Play a scene, then switch to another tab. **Expect**:
   - the audio continues with no gap;
   - a "now playing" bar appears within 150 ms, with the title, play/pause, and a button back to
     the scene;
   - the bar doesn't cover content.
2. Use the bar's play/pause, then its button back to the scene. **Expect**: the video is back
   within 150 ms, drawn inside the content area (not under the navigation bar).
3. Go fullscreen. **Expect**: no navigation bar, tabs, now-playing bar, or toasts. Exit
   fullscreen: the shell returns.
4. Close the playing scene's tab. **Expect**: playback stops and the bar disappears.
5. Relaunch with a scene tab open. **Expect**: it doesn't auto-play, and it shows the title and
   Play.

### V4: Notifications (US3, FR-017–FR-022, SC-005)
1. Stop the Stash server for the active profile (or block its port). **Expect**:
   - one toast, "Server unreachable", which doesn't take focus while you type;
   - a badge on the bell;
   - one entry in the centre.
2. Start the server again. **Expect**: the same entry changes to "Reconnected", with a toast.
3. Toggle the connection 10 times in a minute. **Expect**: one entry, and only a few toasts.
4. Dismiss it. **Expect**: the badge clears. Leave another notification undismissed, relaunch,
   and it's still there.
5. Open a scene whose file is missing. **Expect**: an error notification in plain language.

### V5: Stash jobs (FR-023, SC-009) (test instance)
1. Connect to the test instance. In its web UI (or with a `metadataScan` mutation), start a scan.
   **Expect**: within 2 s, a job entry with progress appears, and the bell shows activity.
2. When it ends. **Expect**: the entry becomes "finished" within 2 s. A failing job (e.g. a scan of
   a removed path) shows an error and a toast.
3. Stop the server while a job runs. **Expect**: the job shows "status unknown". Restart the
   server: it reconciles.

### V6: Settings (US4, FR-024–FR-028)
1. Open Settings. **Expect**: the Servers, Keyboard, Troubleshooting, and About pages.
2. On Servers, add, edit, reorder, and remove a server (with confirmation). **Expect**: the same
   behaviour as the old dialog.
3. On Troubleshooting, choose "Open log folder". **Expect**: the file manager opens
   `~/.local/share/semantic-stash-viewer/logs/`.
4. On About. **Expect**: the viewer version, and the server address and Stash version.

### V7: Switching servers (spec edge case)
With tabs open on server A, switch to server B, then back. **Expect**:
- B shows its own tabs (or Home);
- A's tabs return;
- playback stops on switch, and the now-playing bar goes away.

### V8: Damaged storage (Principle I)
Quit the viewer and write garbage into `~/.local/share/semantic-stash-viewer/shell/tabs.json`
and `notifications.json`. Relaunch. **Expect**:
- no error dialog;
- a single Home tab and an empty centre;
- `.bak` copies of the damaged files.

## Measured (T063, 2026-09-29)

Debug-only bench (`SSV_DEBUG_BENCH=1 cargo tauri dev`) in the real app on the dev machine (KDE
Wayland, RX 9060 XT), connected to the user's library. "Frame" means the next painted frame
after the action (two animation frames). Tab changes during the bench aren't saved.

| Check | Budget | Median | p95 | Max | Result |
|---|---|---|---|---|---|
| Tab switch with 20 tabs open, including tabs beyond the 8 kept mounted (SC-002, SC-007) | < 150 ms | 34 ms | 64 ms | 86 ms | Pass |
| Control press: the bell opens its panel (SC-003) | < 50 ms | 32 ms | 37 ms | 37 ms | Pass |
| Now-playing bar appears after leaving a playing scene (SC-008) | < 150 ms | 61 ms | 76 ms | 76 ms | Pass |
| Back to the playing scene, UI frame (SC-008) | < 150 ms | 59 ms | 70 ms | 70 ms | Pass |

Playback inside the shell (navigation bar and tab strip over the page, viewport margins on the
video), against 002's 0 drops and 6–7% main-thread CPU (details in
[../002-mpv-playback-spike/decision.md](../002-mpv-playback-spike/decision.md)):

| Run | Dropped frames | Main-thread CPU |
|---|---|---|
| 1080p H.264, 5 min | 0 | 9% |
| 4K HEVC, 2 min | 0 | 9% |
