# Contract: Player Commands & Events (UI ↔ Rust core)

**Feature**: [../spec.md](../spec.md) | **Types**: [../data-model.md](../data-model.md)

Adds to the feature 001 contract (`specs/001-connect-to-stash/contracts/tauri-commands.md`).
Same rules: typed bindings in `ui/src/bindings.ts` via tauri-specta; the UI never talks to mpv
or Stash directly; errors are typed.

## Invariants

1. **Commands return immediately.** Player commands are sent to mpv asynchronously, so a click
   never waits on mpv (Principle VI, SC-004). Results show up as `player-state` events.
2. **Only the direct stream.** `player_open` takes a scene ID, never a URL. The core builds
   `/scene/{id}/stream` itself (research R6), so the UI can't request a transcode.
3. **No writes to Stash** (FR-013).

## DTOs

```ts
type PlayerStateKind = "idle" | "loading" | "playing" | "paused" | "ended" | "error";

type Track = {
  id: number;
  kind: "video" | "audio" | "subtitle";
  title: string | null;
  language: string | null;
  codec: string | null;
  default: boolean;
  external: boolean;
};

type PlayerError =
  | { kind: "notConnected" }
  | { kind: "sceneNotFound" }
  | { kind: "noPlayableFile" }
  | { kind: "streamUnreachable" }
  | { kind: "unsupportedFormat" }
  | { kind: "playbackFailed"; detail: string };

type PlayerSnapshot = {
  sceneId: string | null;
  title: string | null;
  state: PlayerStateKind;
  positionSeconds: number;
  durationSeconds: number | null;
  paused: boolean;
  speed: number;              // 0.25–4.0
  volume: number;             // 0–100
  muted: boolean;
  fullscreen: boolean;
  hwdec: string | null;       // e.g. "vaapi", "no"
  tracks: Track[];            // FR-014, read-only in the spike
  error: PlayerError | null;
};

type SceneListItem = {
  id: string;
  title: string;
  durationSeconds: number;
  resolution: string | null;
  videoCodec: string | null;
  container: string | null;
};

type PlayerStats = {           // debug builds only
  openToFirstFrameMs: number | null;
  lastSeekToFrameMs: number | null;
  droppedFrames: number;       // vo + decoder
  hwdec: string | null;
};
```

## Commands

| Command | Input | Output | Notes |
|---|---|---|---|
| `list_recent_scenes` | — | `SceneListItem[]` | Up to 20, newest first (FR-001) |
| `player_open` | `sceneId: string` | `PlayerSnapshot` | Looks up the scene, builds the direct stream URL, starts playback. Fails with `AppError` if not connected; playback errors arrive as `state: "error"` |
| `player_close` | — | `void` | Stops mpv, releases audio/video, hides the video surface (FR-007) |
| `player_toggle_pause` | — | `void` | Also `player_set_paused(paused: boolean)` |
| `player_seek` | `positionSeconds: number`, `exact: boolean` | `void` | Absolute seek, always to the exact frame. `exact: false` = drag position (coalesced: only the latest is sent while a seek settles); `exact: true` = release (supersedes queued drag positions) |
| `player_seek_relative` | `seconds: number` | `void` | ±10 for skip and arrow keys |
| `player_set_speed` | `speed: number` | `void` | Clamped to 0.25–4.0; pitch preserved |
| `player_set_volume` | `volume: number` | `void` | 0–100 |
| `player_set_muted` | `muted: boolean` | `void` | |
| `player_frame_step` | `direction: "forward" \| "back"` | `void` | Only while paused |
| `player_replay` | — | `void` | From Ended: seek to 0 and play (FR-015) |
| `player_set_fullscreen` | `fullscreen: boolean` | `void` | Toggles the window's fullscreen state |
| `player_snapshot` | — | `PlayerSnapshot` | Hydration on page load |
| `player_stats` | — | `PlayerStats` | Debug builds only; for the decision record |

The disconnect and server-switch paths from feature 001 also call `player_close` in the core
(FR-007).

## Events

| Event | Payload | When |
|---|---|---|
| `player-state` | `PlayerSnapshot` | Every state change, seek completion, volume/speed/mute change, track list change, and position updates throttled to ~4/s while playing |

## Keyboard map (handled in the UI, FR-009)

| Key | Action |
|---|---|
| Space | Toggle pause |
| ← / → | Seek −10 s / +10 s |
| ↑ / ↓ | Volume +5 / −5 |
| `[` / `]` | Speed down / up (0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4) |
| `\` | Speed 1× |
| `,` / `.` | Previous / next frame (paused only) |
| `f` | Toggle fullscreen |
| `m` | Toggle mute |
| Escape | Leave fullscreen; if not fullscreen, close the player |

## Stash GraphQL (core → Stash, read-only)

Added to `crates/stash-core/graphql/`, each selecting only what the player needs
(Principle IV):

```graphql
query RecentScenes {
  findScenes(filter: { per_page: 20, sort: "created_at", direction: DESC }) {
    scenes { id title files { basename duration width height video_codec format } }
  }
}

query PlayableScene($id: ID!) {
  findScene(id: $id) {
    id title
    files { basename duration width height video_codec audio_codec format frame_rate bit_rate size }
  }
}
```

**Requests**: one per list load, one per `player_open`. The stream itself is fetched by mpv
from `/scene/{id}/stream` with the `ApiKey` header.
