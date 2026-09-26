# Data Model: mpv Playback Spike

**Feature**: [spec.md](spec.md) | **Research**: [research.md](research.md)

Nothing here is persisted, and nothing is written to Stash (FR-013). DTOs that reach the UI are
exported to TypeScript with tauri-specta ([contracts/player-commands.md](contracts/player-commands.md)).

---

## SceneListItem (from Stash; `crates/stash-core`)

One row of the "recently added" picker (FR-001).

| Field | Type | Source |
|---|---|---|
| `id` | string | `Scene.id` |
| `title` | string | `Scene.title`, or the primary file's basename if the title is empty |
| `duration_seconds` | number | primary file `duration` |
| `resolution` | string, optional | primary file `width`×`height`, e.g. `1920×1080` |
| `video_codec` | string, optional | primary file `video_codec` |
| `container` | string, optional | primary file `format` |

## PlayableScene (from Stash; `crates/stash-core`)

What the player needs to open one scene.

| Field | Type | Rules |
|---|---|---|
| `id` | string | Required |
| `title` | string | As above |
| `stream_url` | URL | Always `{profile base}/scene/{id}/stream` (the direct, untranscoded stream, research R6). A URL with a suffix (`.mp4`, `.m3u8`, …) or a `resolution` parameter is rejected |
| `duration_seconds` | number | From the primary file |
| `file` | `{ container, video_codec, audio_codec, width, height, frame_rate, bit_rate, size }` | Primary file only; other files are out of scope |

Created only while connected; uses the active profile's base URL, API key, and strict setting.

## PlayerSession (in memory; `crates/player`)

One playback of one scene. At most one exists at a time.

| Field | Type | Notes |
|---|---|---|
| `scene_id` | string | |
| `state` | `PlayerState` | See transitions below |
| `position_seconds` | number | Throttled to ~4 updates/s in events |
| `duration_seconds` | number, optional | From mpv once known |
| `paused` | bool | |
| `speed` | number | 0.25–4.0, default 1.0; pitch preserved |
| `volume` | number | 0–100 |
| `muted` | bool | |
| `fullscreen` | bool | Mirrors the window state |
| `hwdec` | string, optional | mpv `hwdec-current`, e.g. `vaapi` or `no` |
| `tracks` | `Track[]` | Embedded audio/subtitle/video tracks (FR-014); read-only in the spike |
| `error` | `PlayerError`, optional | Set in the `Error` state |

### Track

`{ id, kind: "video" | "audio" | "subtitle", title?, language?, codec?, default: bool,
external: bool }`, from mpv's `track-list`.

### PlayerState and transitions

```text
Idle ──open──► Loading ──first frame──► Playing ⇄ Paused
                 │                         │  │
                 │                         │  └──end of file──► Ended ──replay──► Playing
                 └──failure──► Error ◄─────┘ (stream lost / decode error)
any ──close / disconnect / switch server──► Idle
```

| From | Event | To |
|---|---|---|
| Idle | `open(scene)` | Loading |
| Loading | first frame rendered | Playing |
| Loading | file can't be opened or decoded | Error |
| Playing | `pause` / `toggle_pause` | Paused |
| Paused | `play` / `toggle_pause` | Playing |
| Playing | end of file | Ended (FR-015) |
| Ended | `replay` (seek to 0, play) | Playing |
| Playing / Paused | stream lost or decode failure | Error |
| any | `close`, disconnect, or server switch | Idle (mpv stops, audio/video released; FR-007) |

Seeking, speed, volume, mute, and frame steps don't change the state (frame steps require
Paused).

### PlayerError (maps to plain-language messages, FR-006)

| Variant | Meaning |
|---|---|
| `NotConnected` | No active server |
| `SceneNotFound` | Stash has no scene with that ID |
| `NoPlayableFile` | The scene has no file |
| `StreamUnreachable` | The stream couldn't be opened (network, 401/403, 404) |
| `UnsupportedFormat` | mpv couldn't decode it |
| `PlaybackFailed { detail }` | Anything else mpv reported (short text, no stack traces) |

## Measurement (spike only; written to the decision record)

Timestamps recorded by the core for SC-001/SC-002 (open → first frame, seek → next frame) and
mpv's `frame-drop-count` / `decoder-frame-drop-count` for SC-003. Exposed through a
debug-only `player_stats` command, not shown in the normal UI.
