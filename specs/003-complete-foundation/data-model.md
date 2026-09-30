# Data Model: Complete Phase 0

**Feature**: [spec.md](spec.md) | **Research**: [research.md](research.md)

## Cache file (core, per profile)

`~/.cache/semantic-stash-viewer/<profile-id>/cache.sqlite3` (research R1). Discardable: a damaged
or newer-schema file is deleted and recreated (FR-010).

### Table `meta`

| Column | Type | Notes |
|---|---|---|
| `key` | TEXT PRIMARY KEY | `schema_version`, `server_identity` |
| `value` | TEXT | schema version `1`; identity = 16 hex digits (FNV-1a 64 of final URL, `databasePath`, and `configPath`; research R2) |

### Table `entries`

| Column | Type | Rules |
|---|---|---|
| `key` | TEXT PRIMARY KEY | e.g. `server:info`, `scenes:recent`, `scene:42` (research R3) |
| `value` | BLOB | JSON of the domain type |
| `fetched_at` | INTEGER | Unix milliseconds, when the data came from the server |
| `last_used` | INTEGER | Unix milliseconds, touched on every read (for LRU) |
| `size` | INTEGER | `length(value)` in bytes |

**Rules**:
- **Size limit**: the total `SUM(size)` is at most 5% of the free space on the cache's disk,
  clamped to 64 MB–10 GB (FR-008, research R1b). It's computed on open and every 50 writes. A
  `put` over the limit deletes the least recently used entries until it fits.
- **Identity mismatch** on connect: all entries are deleted, then the new identity is stored
  (FR-007).
- **Profile deleted**: the profile's directory is removed (FR-009).
- **Clear cache**: all entries are deleted and the file is vacuumed. The bytes freed are
  returned (FR-005).

## Cached<T> (core → UI)

| Field | Type | Notes |
|---|---|---|
| `data` | T | the view's data (`ServerInfo`, `SceneListItem[]`, `SceneGroup[]`) |
| `fromCache` | bool | true if served from the cache (a background refresh may follow) |
| `fetchedAt` | ISO 8601 | when the data came from the server |

## Refresh state (core, in memory)

Per profile and key: whether a refresh is in flight, the count of consecutive failures, and when
the failures started. Rules (research R4–R5):
- refresh only when the entry is older than 5 s, and never while the session is offline;
- 3 failures in a row post a `background` notification (key `background:<profile>:<key>`);
- `toast` is set once failures have lasted more than 60 s;
- a later success updates that notification to "Refreshed".

## Performance report (harness, research R8)

Written to `~/.local/share/semantic-stash-viewer/perf/<timestamp>.json` and `.md`.

| Field | Type | Notes |
|---|---|---|
| `createdAt` | ISO 8601 | |
| `appVersion` | string | |
| `machine` | string | OS, CPU, GPU (from `/proc/cpuinfo`, `lspci` when available) |
| `profile` | string | the server profile's **display name** only (no address or key) |
| `measurements` | Measurement[] | |

### Measurement

| Field | Type | Notes |
|---|---|---|
| `name` | string | e.g. `cold-start-warm`, `nav-first-paint`, `scroll-frame-time` |
| `unit` | `ms` \| `fps` \| `count` \| `percent` | |
| `budget` | number or null | from Principle VI (research R8 table); null when there's no budget |
| `median`, `p95`, `max` | number | |
| `samples` | integer | |
| `result` | `pass` \| `fail` \| `invalid` \| `info` | `invalid` includes a `reason` (FR-016) |
| `previousMedian` | number or null | from the last report |
| `change` | number or null | percent change from the previous median |
| `regressed` | bool | true when more than 20% slower (FR-015) |
