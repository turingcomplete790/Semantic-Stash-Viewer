# Data Model: App Shell

**Feature**: [spec.md](spec.md) | **Research**: [research.md](research.md)

Types are described language-neutrally. TypeScript shapes appear in the
[contract](contracts/shell-commands.md). Rust types live in `stash-core::shell` and are exported
to the UI through tauri-specta.

## Route (UI)

A view a tab can show. Defined in the UI (research R1). The core stores it as part of a tab
without interpreting it, beyond checking that it's valid JSON within the size limits.

| Variant | Fields | Title shown on the tab |
|---|---|---|
| `home` | none | "Home" |
| `scenes` | none | "Scenes" |
| `scene` | `sceneId: string`, `title: string` (last known, for restored tabs) | the scene title |
| `settings` | `page: "servers" \| "keyboard" \| "troubleshooting" \| "about"` | "Settings" |

Later features add variants (`images`, `gallery {id}`, and so on). An unknown variant in a
restored tab (e.g. after a downgrade) opens as a tab that says the view isn't available, with a
way to close it.

## Section (UI, static)

| Field | Notes |
|---|---|
| `id` | `home`, `scenes` (later `images`, `groups`, `markers`, `galleries`, `performers`, `studios`, `tags`) |
| `label` | e.g. "Scenes" |
| `icon` | inline SVG (research R9) |
| `shortcut` | `g h`, `g s`, … (research R7) |
| `route` | the route it opens |
| `needsServer` | `true` for library sections; hidden while disconnected (FR-006) |
| `order` | the Stash web UI order (FR-003) |

## HistoryEntry

| Field | Type | Rules |
|---|---|---|
| `route` | Route | required |
| `viewState` | JSON object or null | opaque to the core. ≤ 16 KB serialised: scroll offsets, filters, selection, typed text (research R2) |

## Tab

| Field | Type | Rules |
|---|---|---|
| `id` | string (UUID) | unique within its tab set |
| `history` | HistoryEntry[] | 1–50 entries. The oldest are dropped past 50 |
| `index` | integer | `0 ≤ index < history.length`: the entry shown |

The title and icon are derived from `history[index].route`.

## TabSet (per server profile)

| Field | Type | Rules |
|---|---|---|
| `tabs` | Tab[] | 1–100 tabs. Never empty: closing the last tab creates a Home tab (FR-013) |
| `selectedTabId` | string | must match a tab |

The recently closed tabs (for Ctrl+Shift+T, up to 10) are **session-only** and not persisted.

**Lifecycle**:
- **Load**: when a profile becomes active, the UI loads its TabSet from the core. If there's none,
  it starts with one Home tab.
- **Save**: the UI saves the TabSet on every structural change (open, close, move, select, or
  navigate) and on a 500 ms debounce for view-state changes. The core writes it atomically.
- **Switch server**: save the current profile's set, then load the other's (spec edge case).
- **Delete profile**: the core deletes that profile's TabSet.

## TabsFile (core, `shell/tabs.json`)

| Field | Type | Rules |
|---|---|---|
| `version` | integer | currently 1. A newer version, or a file that fails to parse, is renamed aside to `tabs.json.bak`, and the store starts empty |
| `profiles` | map: profile ID → TabSet | an entry for an unknown profile is removed on load |

## Notification (core)

| Field | Type | Rules |
|---|---|---|
| `id` | string (UUID) | stable across updates |
| `key` | string or null | set for ongoing conditions, e.g. `connection:<profile>` or `job:<profile>:<jobId>`. Posting with an existing key updates that notification (FR-019) |
| `profileId` | UUID or null | the server it's about, if any |
| `kind` | `connection` \| `playback` \| `job` \| `background` | its source |
| `severity` | `info` \| `warning` \| `error` | |
| `title` | string | plain language, e.g. "Server unreachable" |
| `detail` | string or null | plain language, with any next step |
| `createdAt` / `updatedAt` | ISO 8601 | |
| `read` | bool | set by opening the centre. An update that raises severity sets it back to unread |
| `toast` | bool | the UI should show a toast for this post or update (research R4) |
| `job` | JobProgress or null | only for `kind = job` |

### JobProgress

| Field | Type | Notes |
|---|---|---|
| `status` | `queued` \| `running` \| `stopping` \| `finished` \| `failed` \| `cancelled` \| `unknown` | from Stash's `JobStatus`. `unknown` while disconnected |
| `progress` | number 0–1 or null | null when Stash doesn't report one |
| `startedAt` / `endedAt` | ISO 8601 or null | |

The "active" state (for the bell's activity marker) is any job notification whose status is
`queued`, `running`, or `stopping`.

**State transitions** (connection, key `connection:<profile>`, research R4):

```text
(connected) ──offline──▶ warning "Server unreachable" [toast]
     ▲                         │
     └──────reconnected────────┘ info "Reconnected" [toast]  (same entry)
any ──auth failed──▶ error "API key rejected" [toast]
any ──failed──────▶ error <failure message> [toast]
```

**State transitions** (job, key `job:<profile>:<id>`, research R5):

```text
ADD/UPDATE ─▶ queued / running / stopping (progress updates, no toast)
REMOVE ─▶ finished (info) | failed (error, toast) | cancelled (info)
session offline ─▶ unknown ; reconnect + jobQueue ─▶ current status or "ended while disconnected"
```

## NotificationsFile (core, `shell/notifications.json`)

| Field | Type | Rules |
|---|---|---|
| `version` | integer | currently 1. Handled like TabsFile when damaged or newer |
| `notifications` | Notification[] | at most 200, newest first. Past 200, the oldest dismissible entries are dropped. Active job entries are never dropped |

Dismissed notifications are removed, not flagged. On load, job entries that were active become
`unknown` until the next connection reconciles them.

## Now playing (UI, derived)

| Field | Source |
|---|---|
| `tabId` | the tab whose route is the scene loaded in the player |
| `title` | the player snapshot's title |
| `paused` | the player snapshot |
| `visible` | true when a scene is loaded (playing, paused, or ended) and its tab isn't selected |

Nothing is persisted. It's derived from the player snapshot and the tab set.

## Validation summary

| Rule | Where enforced |
|---|---|
| History ≤ 50, tabs ≤ 100, view state ≤ 16 KB | core, on save. Violations are rejected with a typed error |
| Tab set never empty; selected tab exists | core, on save; UI, on close |
| Notifications ≤ 200; active jobs kept | core |
| Same key ⇒ same notification | core |
| Damaged or newer file ⇒ renamed aside, empty store | core, on load |
