# Contract: App Shell Commands & Events (UI ↔ Rust core)

**Feature**: [../spec.md](../spec.md) | **Types**: [../data-model.md](../data-model.md)

Adds to the feature 001 and 002 contracts:
- `specs/001-connect-to-stash/contracts/tauri-commands.md`;
- `specs/002-mpv-playback-spike/contracts/player-commands.md`.

The same rules apply. Typed bindings are generated in `ui/src/bindings.ts` via tauri-specta, the
UI never reads or writes files or talks to Stash directly, and errors are typed.

## Invariants

1. **No writes to Stash.** Watching jobs is read-only (FR-023, FR-032).
2. **No new network destinations.** The jobs subscription uses the active profile's server,
   API key, and strict-TLS setting (Principle VII).
3. **Discardable storage.** Tab and notification files may be lost or damaged at any time. The
   commands then behave as if they were empty (Principle I).
4. **Commands return quickly.** Saves are written off the UI's critical path. `shell_save_tabs`
   returns once the data is validated and queued.

## DTOs

```ts
type Route =
  | { kind: "home" }
  | { kind: "scenes" }
  | { kind: "scene"; sceneId: string; title: string }
  | { kind: "settings"; page: "servers" | "keyboard" | "troubleshooting" | "about" };
// Owned by the UI. The core stores routes as opaque JSON within size limits.

type HistoryEntry = { route: unknown /* Route */; viewState: unknown | null };
type Tab = { id: string; history: HistoryEntry[]; index: number };
type TabSet = { tabs: Tab[]; selectedTabId: string };

type NotificationKind = "connection" | "playback" | "job" | "background";
type Severity = "info" | "warning" | "error";
type JobStatus = "queued" | "running" | "stopping" | "finished" | "failed" | "cancelled" | "unknown";

type JobProgress = {
  status: JobStatus;
  progress: number | null;   // 0–1
  startedAt: string | null;  // ISO 8601
  endedAt: string | null;
};

type Notification = {
  id: string;
  key: string | null;
  profileId: string | null;
  kind: NotificationKind;
  severity: Severity;
  title: string;
  detail: string | null;
  createdAt: string;
  updatedAt: string;
  read: boolean;
  toast: boolean;            // show a toast for this post/update
  job: JobProgress | null;
};

type AppInfo = { version: string };

type Viewport = { x: number; y: number; width: number; height: number }; // CSS px, window-relative

// Errors are the existing AppError, extended with:
//   | { kind: "tabSetInvalid"; reason: string }   // empty, selected tab missing, limits exceeded
//   | { kind: "openFailed"; detail: string }      // log folder couldn't be opened
// and reusing { kind: "profileNotFound"; id: string }.
```

## Commands

| Command | Input | Output | Notes |
|---|---|---|---|
| `shell_load_tabs` | `profileId: string` | `TabSet \| null` | `null` if none is saved or the file was discarded. The UI then starts with one Home tab |
| `shell_save_tabs` | `profileId: string`, `tabs: TabSet` | `void` | Validates (data-model limits), then writes atomically in the background. Fails with `tabSetInvalid` |
| `notifications_list` | none | `Notification[]` | Newest first. Used at startup. Updates arrive by event |
| `notifications_mark_read` | `ids: string[]` | `void` | When the centre is opened |
| `notification_dismiss` | `id: string` | `void` | Removes it |
| `notifications_dismiss_all` | none | `void` | Removes every notification except active jobs |
| `player_set_viewport` | `viewport: Viewport \| null` | `void` | Where mpv draws. `null` means the whole window (fullscreen). Applied on the GTK main thread (research R6) |
| `player_set_video_visible` | `visible: boolean` | `void` | Hides or shows the video surface. Playback and audio continue (FR-015) |
| `open_log_folder` | none | `void` | Opens the viewer's log directory in the system file manager (research R8). Fails with `openFailed` |
| `app_info` | none | `AppInfo` | The viewer's version for Settings → About |

Unchanged and reused: `list_profiles`, `connect`, `disconnect`, `create_profile`,
`update_profile`, `delete_profile` (now also deletes the profile's tab set),
`reorder_profiles`, `get_connection_snapshot`, and every `player_*` command from 002.

## Events

| Event | Payload | When |
|---|---|---|
| `notifications-changed` | `Notification[]` | Every add, update, read, or dismiss. The full list (≤ 200) |

Unchanged: `connection-state`, `profiles-changed`, `player-state`.

## Notification producers (core, research R4–R5)

| Producer | Key | Posts |
|---|---|---|
| Connection watcher | `connection:<profile>` | Server unreachable / Reconnected / API key rejected / connection failed (with the failure's message) |
| Jobs watcher | `job:<profile>:<jobId>` | Job progress. Then finished, failed (toast), or cancelled. Unknown while offline |
| Player forwarder (`src-tauri`) | `playback:<sceneId>` | "Couldn't play <title>" with the player error's plain message |

## Stash GraphQL (core → Stash, read-only)

Added to `crates/stash-core/graphql/`:

```graphql
query JobQueue {
  jobQueue { id status description progress startTime endTime error }
}

subscription JobsSubscribe {
  jobsSubscribe { type job { id status description progress startTime endTime error } }
}
```

**Requests**:
- one `jobQueue` query per successful connection;
- one long-lived WebSocket per session (`/graphql`, subprotocol `graphql-transport-ws`,
  `ApiKey` header).

No polling.

## Keyboard map (UI, research R7)

| Keys | Action | Scope |
|---|---|---|
| `g h` / `g s` / `g z` / `g n` | Home / Scenes / Settings / notification centre | shell |
| `?` | Keyboard help overlay | shell |
| Ctrl+T / Ctrl+W | New tab (Home) / close tab | shell |
| Ctrl+Tab, Ctrl+PgDn / Ctrl+Shift+Tab, Ctrl+PgUp | Next / previous tab | shell |
| Ctrl+1–8 / Ctrl+9 | Tab 1–8 / last tab | shell |
| Ctrl+Shift+T | Reopen last closed tab | shell |
| Alt+Left / Alt+Right, mouse back / forward | Back / forward in the tab's history | shell |
| (existing player map from 002) | Playback | scene tab only |

Shell sequences and single keys are ignored while a text field has focus. Modifier shortcuts
still work.
