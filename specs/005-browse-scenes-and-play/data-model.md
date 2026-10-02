# Data Model: Browse Scenes and Play — P1 (paged)

**Feature**: [spec.md](spec.md) | **Research**: [research.md](research.md)

All library data is Stash's (Principle I). These are the shapes the viewer passes around and
caches; nothing here is authoritative.

## SceneCard (core → UI) *(kept)*

What one grid card or list row shows (FR-001).

| Field | Type | Notes |
|---|---|---|
| `id` | string | Stash scene ID |
| `title` | string | the scene title, or the primary file's base name when the title is empty |
| `date` | string or null | `YYYY-MM-DD` |
| `durationSeconds` | number or null | primary file |
| `resolution` | string or null | `1920×1080`, primary file |
| `studio` | string or null | studio name |
| `thumb` | string or null | `ssv-thumb://localhost/scene/<id>?v=<version>` (R5) |
| `hasPreview` | bool | `false` until US5 (animated image previews) |

Cards show the title on one line and one line of details (duration · resolution · studio ·
date), both truncated with an ellipsis and never wrapping.

## ScenePage (core → UI)

| Field | Type | Notes |
|---|---|---|
| `count` | number | total matching scenes |
| `page` | number | 1-based; the **page actually returned** (the last page when the request was past the end, R1) |
| `pageSize` | number | the page size used |
| `items` | SceneCard[] | up to `pageSize` (the last page may be shorter) |

Returned inside 003's `Cached<T>`.

## SceneQuery (UI ↔ core) *(kept)*

| Field | Type | Rules |
|---|---|---|
| `search` | string | trimmed; empty means none (US2, later) |
| `sort` | SceneSort | one of the web UI's 27 scene sorts; default `date` |
| `direction` | `asc` \| `desc` | default `desc` |
| `seed` | number (32-bit unsigned) or null | required when `sort` is `random` |

The page and page size travel next to the query, not inside it: the query's hash names the
result set, and the cache key adds the size and the page (R7).

## Page size

One of **20, 40, 50, 60, 120, 250, 500, 1000**; default **50**. Any other value is rejected by
the core (`AppError::Internal`) and replaced by 50 in the UI.

## Scenes view state (per history entry, 004)

| Field | Type | Notes |
|---|---|---|
| `query` | SceneQuery | |
| `page` | number | 1-based, default 1 |
| `pageSize` | number | default 50; kept per tab only (new tabs start at 50) |
| `mode` | `grid` \| `list` | default `grid` |
| `scroll` | number | the grid's scroll offset within the page, in px |

Rules (research R6, R13):
- Changing the query → page 1, scroll 0.
- Changing the page size → the page containing the first visible scene, scroll 0.
- Changing page → scroll 0.
- Returning to Scenes (Back, leaving a scene, or Scenes in the navigation bar) shows the same
  state: the previous entry is reused, or a new entry starts with a copy of the most recent
  Scenes entry's state in that tab.

## Cache entries (003)

| Key | Value | Refresh |
|---|---|---|
| `scenes:q:<hash>:s:<pageSize>:p:<page>` | ScenePage | Auto (5 s floor) |
| `thumb:<kind>:<id>:<version>` | JPEG bytes (480 px wide) | never (the version changes with the image) |
