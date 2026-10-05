# Data Model: Port the Viewer to iced

The UI is a hierarchical state machine held in iced's state (research R1). This file lists each
level's states, the events that move between them, and what each state does on entry and exit.
Effects are data that the root turns into tasks (R2). Core types are used as they are:
`ServerProfile`, `ConnectionSnapshot`, `SceneQuery`, `ScenePage`, `SceneCard`, `PlayerSnapshot`,
`Notification`, cache entries.

## App (root)

| State | Meaning | Entry effects |
|---|---|---|
| `Onboarding(ServerForm)` | No saved server | focus the address field |
| `Session(Session)` | One active server | connect; load `session.json` for this profile |

Transitions:
- Onboarding → Session when a server is saved and connects.
- Session → Session when switching servers: save the session, exit, then enter for the new profile.
- Session → Onboarding when the last server is removed.

Global regions beside the state: `services`, the keymap, the toast layer, the window (fullscreen).

## Session

A struct of three regions that are active at the same time:

- **connection**: `Connecting { attempt }` | `Connected` | `Offline { retry_at }` |
  `AuthFailed { failure }` | `Failed { failure }`, driven by the core's connection snapshots.
  Entering `Connected` re-requests any screen whose data isn't ready. Entering
  `AuthFailed` opens the key prompt overlay.
- **shell**: `Shell` (below).
- **playback**: `Idle` | `Opening { scene_id, owner_tab }` | `Playing { owner_tab, controls:
  Shown | Hidden, fullscreen }` | `Ended { owner_tab }` | `Error { owner_tab, message }`, driven
  by the player's snapshots and the user's controls (006 player). Exiting playback (close, owner
  tab closed, server switch) closes the player and leaves fullscreen.

## Shell

| Field | Rules |
|---|---|
| `tabs: Vec<Tab>` | At least one; at most 50 |
| `selected` | Index into `tabs` |
| `overlay` | `None` | `KeyboardHelp` | `Notifications` | `ServerMenu` | `KeyPrompt(form)`: one at a time; Escape exits to `None` |

Events handled here: select, close, move, and open tabs (keyboard and mouse); open overlays;
"open in new tab" bubbling up from screens. Any change to persistent state schedules
`SaveSession` (debounced 500 ms).

## Tab

| Field | Rules |
|---|---|
| `history: Vec<Screen>` | Whole screen states, at most 50; the active screen is `history[cursor]` |
| `cursor` | Back moves it down, forward moves it up; opening a view drops entries after the cursor and pushes a new screen |

The active screen is entered when it becomes active and exited when it stops being active (back,
forward, tab switch, close). Inactive screens keep their state, which is how "a tab never resets"
holds. Bubbled events handled here: "open scene", "open section", "back".

## Screen

| Variant | State | Entry effects | Notes |
|---|---|---|---|
| `Home` | server summary: `Loading` \| `Ready(info)` \| `Unreachable` | load the summary (cache first) | |
| `Scenes` | `query`, `page` (≥ 1), `page_size` (20, 40, 50, 60, 120, 250, 500, 1000; default 50), `mode` (Grid \| List), `scroll`, `focused`; `data: Loading { generation } \| Ready { page, count, cards } \| Unreachable` | load the page (cache first), then neighbours and next-page thumbnails | paging rules are a behaviour that stays; a page past the end shows the last page |
| `Scene` | `scene_id`, `title`; `details: Loading \| Ready(details, cover) \| Failed(message)` | load details and cover | Play emits `OpenScene` to the playback region |
| `Settings` | `page: Servers(ServersState) \| Keyboard \| Troubleshooting(TroubleshootingState) \| About` | per page: list profiles; read the cache size | |

`ServersState`: `list` plus `editor: None | Editing(form) | Testing(form) | Saving(form) |
Error(form, message)`, and `confirm_delete: Option<profile id>`.

`TroubleshootingState`: `cache_size: Option<bytes>`, `clear: Idle | Clearing | Cleared(freed)`.

## Messages and steps

- Messages mirror the tree (`Message → SessionMsg → ShellMsg → TabMsg → ScreenMsg → ScenesMsg …`),
  plus results arriving from effects (`PageLoaded { generation, … }`, `ThumbnailReady`,
  `DetailsLoaded`, …), which carry the address of the state that asked for them.
- Each level's `update` returns `Step { effects: Vec<Effect>, up: Option<UpEvent> }`. Unhandled
  events go up; results for a state that's no longer active (or with an old generation) are
  dropped.

## Effect (data)

The effects:
- `Connect { profile }`
- `LoadSummary`
- `LoadScenesPage { query, page, size, generation }`
- `Prefetch { query, pages, size }`
- `LoadThumbnail { id, version }`
- `LoadSceneDetails { id }`
- `OpenScene { id }`
- `PlayerCommand(…)`
- `SaveProfile`, `DeleteProfile`, `ReorderProfiles`
- `ReadCacheSize`, `ClearCache`
- `SaveSession`
- `SetWindowMode`
- `Focus(widget id)`
- `OpenLogFolder`

`effects.rs` maps each to a task over the service layer. Effects are what transition tests assert
on (R2).

## Keymap

`(chord, level, action, label)`, where level is the shell, a screen kind (Scenes), or playback.
Uncaptured key events go down the active branch; the innermost binding wins; otherwise they bubble
up (research R6).

## Failure message

`{ title, detail, hint? }` for every `ConnectFailure` and `AppError` (R8).

## Saved session

[contracts/session-snapshot.md](contracts/session-snapshot.md): the persistent fields of the tree
(tabs, histories, cursors, screen fields, scroll positions), versioned; transient data rebuilt by
entry effects on restore.

## Capability checklist

`specs/007-port-to-iced/capabilities.md`: one row per carried-over capability and per behaviour
that stays, with its acceptance scenario for the new UI, the test(s) covering it, the result, and
the date. Retirement (US6) requires every row to pass and the user's sign-off.

## VideoSurface

As 006 (four slots), with GPU resources owned per slot generation after the hardening (R11).
