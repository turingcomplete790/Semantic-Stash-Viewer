---

description: "Task list for 002-mpv-playback-spike"
---

# Tasks: mpv Playback Spike

**Input**: Design documents from `specs/002-mpv-playback-spike/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/player-commands.md](contracts/player-commands.md),
[quickstart.md](quickstart.md)

**Tests**: Included. The constitution's Development Workflow gate requires core tests; the spike
also needs measurements for its decision record. Write each story's tests first and make sure
they fail before implementing.

**Organization**: Tasks are grouped by user story. This is a spike: US3's decision record
depends on US1 and US2 having been tried for real.

**Rules that apply to every task**:
- **Read-only against Stash** (FR-013): no mutations, no play counts, no resume points.
- **Only the direct stream** (FR-003): the core builds `{base}/scene/{id}/stream`; nothing
  else is ever handed to mpv.
- **Webview grandparent stays the window** (research R2): the tree is
  `GtkWindow → GtkOverlay → { GtkGLArea, WebKitWebView }`, never an overlay between the webview
  and Tauri's box, or Tauri panics on the first click.
- Playback tests use the user's library (`localhost:9999`, read-only). Test runs that need auth
  use the test instance (`localhost:9998`, credentials in its `creds.txt`, never copied into the
  repo).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on unfinished tasks)
- **[Story]**: The user story the task belongs to (US1–US3)

## Path Conventions

Workspace from feature 001, plus a new headless crate: `crates/stash-core/`, `crates/player/`
(new; depends on `libmpv2`, never on Tauri or GTK), `src-tauri/` (Tauri shell; Linux-only
`video_surface/` module), `ui/` (SolidJS).

---

## Phase 1: Setup (Shared Infrastructure)

- [X] T001 Create the `crates/player` library crate: `crates/player/Cargo.toml` (workspace package fields and lints; dependencies `libmpv2 = "6"` with its default `render` feature, `serde`, `thiserror`, `tracing`, `tokio` (workspace, `sync` + `time`), optional `specta` behind a `specta` feature; dev-dependencies `tokio` with `test-util`, `tempfile`), an empty `crates/player/src/lib.rs` with a module doc stating it must not depend on Tauri or GTK, and add `crates/player` to `members` in the root `Cargo.toml`
- [X] T002 Add the player and Linux window-system dependencies to `src-tauri/Cargo.toml`: `player = { path = "../crates/player", features = ["specta"] }`, and under `[target.'cfg(target_os = "linux")'.dependencies]`: `gtk = "0.18"`, `glib = "0.18"`, `webkit2gtk = "2.0"` (the versions Tauri 2.11 already uses; confirm in `Cargo.lock`), `libloading = "0.8"`, `gl = "0.14"`
- [X] T003 [P] Add the read-only GraphQL operations from [contracts/player-commands.md](contracts/player-commands.md#stash-graphql-core--stash-read-only) as `crates/stash-core/graphql/recent_scenes.graphql` and `crates/stash-core/graphql/playable_scene.graphql`, selecting only the listed fields (Principle IV)
- [X] T004 [P] Capture scrubbed fixtures from `localhost:9999` into `crates/stash-core/tests/fixtures/stash-v0.31.1/`: `recent-scenes.json` (a real response shape for 3 scenes, with titles replaced by `Scene A/B/C`, one title set to empty to exercise the basename fallback, and basenames replaced by `scene-a.mp4` etc.), `playable-scene.json` (one scene, same scrubbing), and `scene-not-found.json` (`{"data":{"findScene":null}}`). No real titles, paths, or performer data
- [X] T005 [P] Add `libmpv-dev` (and `ffmpeg` for the tracks test) to the Linux dependencies step in `.github/workflows/ci.yml`

**Checkpoint**: `cargo build --workspace` succeeds with the new crate linked against the system libmpv.

---

## Phase 2: Foundational (Blocking Prerequisites)

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

### Tests first

- [X] T006 [P] Write `crates/stash-core/tests/scenes.rs`: `direct_stream_url(base, id)` gives `http://h:9999/scene/42/stream` and keeps a reverse-proxy sub-path (`https://h/stash` → `https://h/stash/scene/42/stream`); `is_direct_stream` accepts that and **rejects** `…/stream.mp4`, `…/stream.mkv`, `…/stream.webm`, `…/stream.m3u8`, `…/stream.mpd`, and any URL with a `resolution` query parameter; parsing `recent-scenes.json` yields items with the title fallback to basename when empty, `resolution` formatted `1920×1080`, and `durationSeconds` from the primary file; `scene-not-found.json` maps to `SceneNotFound`
- [X] T007 [P] Write `crates/player/tests/session.rs` using mpv headless (`vo=null`, `ao=null`, `--no-config`) on `av://lavfi:testsrc=duration=5:size=320x240:rate=30`: open → `loading` → `playing`; `set_paused(true)` → `paused`; `seek(2.0, exact)` lands within 0.1 s; `set_speed(8.0)` clamps to `4.0` and `set_speed(0.1)` to `0.25`; `set_volume(150)` clamps to `100`; frame step changes position only while paused (ignored while playing); playing to the end → `ended`; `replay` → `playing` at ~0; `close` → `idle`; opening `file:///does/not/exist.mp4` → `error` with `PlayerError::StreamUnreachable` or `UnsupportedFormat` (not a panic). Use the snapshot channel, with timeouts, to observe transitions
- [X] T008 [P] Write `crates/player/tests/tracks.rs`: if `ffmpeg` is on `PATH`, generate a 3 s MKV in a temp dir with one video, two audio (`language=eng`, `language=jpn`), and one subtitle track; open it headless and assert `tracks` lists 1 video, 2 audio (with languages), 1 subtitle (FR-014). If `ffmpeg` is missing, skip with a printed notice

### Implementation

- [X] T009 Implement `crates/stash-core/src/scenes/mod.rs` (wire it into `crates/stash-core/src/lib.rs`): `SceneListItem { id, title, duration_seconds, resolution: Option<String>, video_codec: Option<String>, container: Option<String> }` and `PlayableScene { id, title, stream_url: Url, duration_seconds, file: SceneFile }` per [data-model.md](data-model.md) (serde camelCase, `specta::Type` behind the feature), `direct_stream_url(base: &Url, id: &str) -> Url` (reuse `adapter::endpoint` so sub-paths survive) and `is_direct_stream(url: &Url) -> bool` ("Always `{profile base}/scene/{id}/stream` … A URL with a suffix (`.mp4`, `.m3u8`, …) or a `resolution` parameter is rejected")
- [X] T010 Implement the queries in `crates/stash-core/src/adapter/scenes.rs`: `recent_scenes(client) -> Result<Vec<SceneListItem>, AppError>` (20 newest by `created_at`) and `playable_scene(client, id) -> Result<PlayableScene, AppError>` (maps `findScene: null` to a new `AppError::SceneNotFound { id }` and no files to `AppError::NoPlayableFile { id }`), typed with `graphql_client`; the stream URL is built with `direct_stream_url` from the client's base URL, never taken from the response. Add the two new `AppError` variants in `crates/stash-core/src/error.rs` and their messages in `ui/src/messages/failures.ts`
- [X] T011 [P] Implement the player types in `crates/player/src/snapshot.rs`, `crates/player/src/tracks.rs`, and `crates/player/src/error.rs`: `PlayerStateKind` (`idle|loading|playing|paused|ended|error`), `PlayerSnapshot`, `Track` (from mpv `track-list`: `id, kind: video|audio|subtitle, title, language, codec, default, external`), `PlayerError` (`NotConnected, SceneNotFound, NoPlayableFile, StreamUnreachable, UnsupportedFormat, PlaybackFailed { detail }`), all camelCase serde with `specta::Type` behind the `specta` feature, matching [contracts/player-commands.md](contracts/player-commands.md#dtos)
- [X] T012 Implement `crates/player/src/session.rs` (depends on T011): `Player::new(config)` creating the `libmpv2::Mpv` with `hwdec=auto-safe`, `keep-open=yes` (so the end shows the ended state), `audio-pitch-correction=yes`, `input-default-bindings=no`, `osc=no`, `osd-level=0`, and `vo=libmpv` (render API) or `vo=null`/`ao=null` in headless test mode; `open(stream: &Url, api_key: Option<&str>, strict_tls: bool)` setting `http-header-fields=ApiKey: <key>` (header, never URL) and `tls-verify=yes|no`, then `loadfile`; `close()` (`stop`, state → `idle`); an event/observer thread observing `pause`, `time-pos`, `duration`, `speed`, `volume`, `mute`, `eof-reached`, `hwdec-current`, `track-list`, and the file-loaded / end-file / error events, turning them into `PlayerSnapshot`s on a `tokio::sync::watch` channel, with `time-pos` updates throttled to ≥ 250 ms apart and all other changes sent immediately; map mpv load/decode failures to `PlayerError` (FR-006). Make T007 and T008 pass
- [X] T013 Implement `crates/player/src/render.rs`: a thin, GL-agnostic wrapper that creates the libmpv OpenGL render context from a caller-supplied `get_proc_address` function and an optional Wayland display pointer (`RenderParam::WaylandDisplay`), exposes `render(fbo, width, height)` (flip Y), `report_swap()`, and `set_update_callback(Fn() + Send)`, and is always dropped before the `Mpv` handle it came from (document this ordering)
- [X] T014 Build the video surface in `src-tauri/src/video_surface/mod.rs` (Linux only, `#[cfg(target_os = "linux")]`; wired from `setup` in `src-tauri/src/lib.rs`): on the GTK main thread, take the main window's `gtk_window()` and the webview (`with_webview` → `webkit2gtk::WebView`), remove Tauri's default box from the window, create a `GtkOverlay` as the window's **direct child** with a `GtkGLArea` as its main child and the webview as an overlay child (research R2: the webview's grandparent must be the window), give the webview a transparent background, and keep the GL area hidden until a player opens. Add a comment block explaining the Tauri grandparent-downcast constraint
- [X] T015 [P] Implement `src-tauri/src/video_surface/egl.rs`: load `eglGetProcAddress` from `libEGL.so.1` with `libloading` (fall back to `glXGetProcAddressARB` from `libGL.so.1` when GDK reports X11), and get the Wayland `wl_display` via `gdk_wayland_display_get_wl_display` from libgdk-3 for hardware-decode interop (research R4)
- [X] T016 Implement `src-tauri/src/video_surface/gl_area.rs` (depends on T013–T015): on `realize`, make the GL area current and create the render context via `player::render`; mpv's update callback sets an atomic flag and posts **at most one** pending `queue_render()` to the GTK main context (drop extra callbacks while one is pending, research R5); on `render`, read `GL_DRAW_FRAMEBUFFER_BINDING`, compute width/height from the allocation × scale factor, call `render`, then `report_swap`; on `unrealize`, drop the render context. Clear to black when no frame is available
- [X] T017 Add `src-tauri/src/player_commands.rs` scaffolding: a `PlayerState` in Tauri state holding the `player::Player`, forwarding its `watch` snapshots as a `player-state` event (`#[tauri_specta(event_name = "player-state")]`), plus `player_snapshot`; register the event and command in `specta_builder` in `src-tauri/src/lib.rs` and regenerate `ui/src/bindings.ts`

**Checkpoint**: `cargo test --workspace` green (including the new `scenes`, `session`, `tracks` tests); the app starts and clicks anywhere in the webview don't crash (overlay in place, GL area hidden).

---

## Phase 3: User Story 1 - Play a scene inside the viewer (Priority: P1) 🎯 MVP

**Goal**: open a scene by ID or from the 20 most recent and have it play inside the viewer window, GPU-rendered, from the direct stream.

**Independent Test**: quickstart V1, V2, V6 (plays or clean error), V9.

### Tests for User Story 1 ⚠️

- [X] T018 [P] [US1] Write `ui/src/__tests__/player/PlayerScreen.test.tsx` with mocked bindings: the recent list shows up to 20 scenes (title and formatted duration); entering an ID and pressing Enter or "Play" calls `playerOpen(id)`; a snapshot in `error` state shows the plain-language message from `ui/src/messages/player.ts` and a way back to the list; closing calls `playerClose`

### Implementation for User Story 1

- [X] T019 [US1] Add `list_recent_scenes` and `player_open(scene_id)` / `player_close` to `src-tauri/src/player_commands.rs`: `player_open` requires an active connected profile (else `AppError`), calls `stash_core` `playable_scene` for the stream URL, asserts `is_direct_stream`, opens it with the profile's API key and strict-TLS setting (FR-004), and shows the GL area; `player_close` stops playback and hides the GL area. Regenerate `ui/src/bindings.ts`
- [X] T020 [US1] Stop playback on disconnect and server switch (FR-007): call the player's `close` from `commands::disconnect`, `commands::start_session`, and `commands::delete_profile` in `src-tauri/src/commands.rs`
- [X] T021 [P] [US1] Add `ui/src/messages/player.ts`: plain-language title/detail for every `PlayerError` kind (e.g. `streamUnreachable` → "Couldn't open this scene's video", `unsupportedFormat` → "This video format can't be played"), with a test in `ui/src/__tests__/player/messages.test.ts` asserting every kind has a message
- [X] T022 [US1] Add `ui/src/player/state.ts` (store fed by `player-state`, hydrated with `playerSnapshot()`) and `ui/src/player/PlayerScreen.tsx`: a scene picker (recent list + "Scene ID" field) and, once open, a full-size transparent video area. Add a "Player" entry point to `ui/src/components/SessionView.tsx` while connected, and route it in `ui/src/App.tsx`
- [X] T023 [US1] Make the page transparent over the video: while the player is open, add a `player-open` class to `<html>` that sets `html, body, #root, .app, .main` backgrounds to `transparent` (in `ui/src/player/player.css`), and remove it on close so the rest of the app keeps its normal background

**Checkpoint**: a scene from your library plays inside the window (quickstart V1), Stash does no transcoding (V2), bad scenes show an error (V6), and leaving stops playback (V9).

---

## Phase 4: User Story 2 - Control playback with the viewer's own controls (Priority: P2)

**Goal**: every playback control, drawn by the viewer on top of the video, with the full keyboard map and auto-hide.

**Independent Test**: quickstart V3, V4, V5, V8, V10.

### Tests for User Story 2 ⚠️

- [X] T024 [P] [US2] Write `ui/src/__tests__/player/Controls.test.tsx` with mocked bindings: each button calls its command (play/pause, ±10 s, mute, fullscreen, close); dragging the seek bar sends fast seeks and an exact seek on release; hovering the seek bar shows the time under the pointer; the speed menu offers `0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4`; frame-step buttons appear only when paused; `ended` shows Replay, which calls `playerReplay`; with fake timers, controls and cursor hide after 3 s without mouse movement and reappear on `mousemove`
- [X] T025 [P] [US2] Write `ui/src/__tests__/player/keyboard.test.ts`: the map in [contracts/player-commands.md](contracts/player-commands.md#keyboard-map-handled-in-the-ui-fr-009): space, ←/→ (±10 s), ↑/↓ (volume ±5), `[`/`]` (step through the speed list), `\` (1×), `,`/`.` (frame step only when paused), `f`, `m`, and Escape (leaves fullscreen first, otherwise closes the player); keys are ignored while typing in an input

### Implementation for User Story 2

- [X] T026 [US2] Add the control operations to `crates/player/src/commands.rs`: `toggle_pause`, `set_paused`, `seek(position, exact)` (`absolute+exact` vs `absolute+keyframes`), `seek_relative(seconds)`, `set_speed` (clamp to "0.25–4.0"), `set_volume` (clamp to "0–100"), `set_muted`, `frame_step(forward|back)` (`frame-step`/`frame-back-step`, only when paused), and `replay` (seek to 0 and unpause). Commands must not block the caller: queue them to mpv from a worker so the Tauri command returns immediately (contract invariant 1). Extend T007's tests as needed
- [X] T027 [US2] Expose the controls as Tauri commands in `src-tauri/src/player_commands.rs`: `player_toggle_pause`, `player_set_paused`, `player_seek`, `player_seek_relative`, `player_set_speed`, `player_set_volume`, `player_set_muted`, `player_frame_step`, `player_replay`, `player_set_fullscreen` (the main window's `set_fullscreen`, mirrored into the snapshot), and a debug-only `player_stats`. Regenerate `ui/src/bindings.ts`
- [X] T028 [US2] Build `ui/src/player/Controls.tsx` (+ `ui/src/player/player.css`): overlay at the bottom of the video area with play/pause, seek bar (current time / duration, hover time, drag), ±10 s, speed menu, frame-step buttons (paused only), volume slider, mute, fullscreen, close; ended state with Replay (FR-015); controls and cursor hide after 3 s idle (FR-010). Solid backgrounds on controls (no backdrop blur) to keep compositing cheap
- [X] T029 [US2] Add `ui/src/player/keyboard.ts` implementing the key map and wire it into `ui/src/player/PlayerScreen.tsx` (FR-009)
- [X] T030 [US2] Handle resize and fullscreen in `src-tauri/src/video_surface/gl_area.rs`: queue a render on size-allocate and scale-factor changes so the video keeps its aspect ratio and fills the area without stale frames (FR-011)

**Checkpoint**: every control works by mouse and keyboard over moving video (V3), seeks meet the target (V4), dropped frames are within budget (V5), clicking/resizing/fullscreen never crash (V8), and the end shows Replay (V10).

---

## Phase 5: User Story 3 - Decide the playback approach with evidence (Priority: P3)

**Goal**: a decision record with measurements for each approach tried.

**Independent Test**: `decision.md` names one approach, gives values for SC-001 to SC-006, lists the embedded tracks found (FR-014), and states limitations.

- [X] T031 [US3] Add measurement to `crates/player/src/session.rs` and `crates/player/src/render.rs`: timestamps for open → first rendered frame and seek → next rendered frame, and dropped frames from `frame-drop-count` + `decoder-frame-drop-count`, returned by `player_stats` (`openToFirstFrameMs`, `lastSeekToFrameMs`, `droppedFrames`, `hwdec`)
- [X] T032 [US3] Pick the 12-scene test set from `localhost:9999` (read-only query) following research R7: 2× mp4/h264 (1080p, 720p), 1× mp4/h264 4K, 1× mp4/hevc, 1× wmv3, 1× webm/vp9, 1× webm/vp8, 1× matroska with several audio or subtitle tracks, 1× matroska/hevc, 1× mpeg4, 1× flv/h264, and a non-existent scene ID. Record the IDs, containers, codecs, and resolutions in a "Test set" table in `specs/002-mpv-playback-spike/decision.md`
- [X] T033 [US3] Run quickstart V1–V10 on the primary approach and record the results in `specs/002-mpv-playback-spike/decision.md` using the template in [plan.md](plan.md#decision-record-template-decisionmd): SC-001 to SC-006 values, main-thread CPU during 1080p and 4K playback (`top -H -p <pid>`), `hwdec` per codec, the track list of the multi-track scene (FR-014), and SC-005 evidence (no Stash `ffmpeg` process, no transcode log lines)
- [X] T034 [US3] **Only if T033 shows a failed criterion, or main-thread CPU above ~50% of a core**: implement fallback 1 (a Wayland subsurface rendered from its own thread, research R1) in `src-tauri/src/video_surface/subsurface.rs`, selectable with an environment variable (`SSV_VIDEO_SURFACE=subsurface`), rerun the failing checks, and add a results column to `decision.md`. If nothing failed, write "Fallbacks not needed" with the reason in `decision.md` and mark this task done
- [X] T035 [US3] Finish `specs/002-mpv-playback-spike/decision.md`: the chosen approach and why, known limitations (Windows/macOS untested; X11 untested unless fallback 2 was tried; the Tauri grandparent constraint to recheck on upgrades), and follow-ups for Phase 1 (play-count/resume sync, subtitle and audio track selection UI, seek-bar thumbnails, transcoding fallback decision)

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T036 [P] Update `README.md` Development prerequisites with libmpv (`mpv` package on Arch/Manjaro, `libmpv-dev` on Debian/Ubuntu) and a short "Player (spike)" note linking `specs/002-mpv-playback-spike/decision.md`
- [ ] T037 [P] In `ROADMAP.md` Phase 0, tick "mpv spike" if `decision.md` chose an in-window approach, and add a one-line summary of the decision under it
- [ ] T038 Run the full gate: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, bindings drift check (`cargo run -p semantic-stash-viewer --bin export-bindings` then `git diff --exit-code ui/src/bindings.ts`), `npm --prefix ui run lint`, `npm --prefix ui run typecheck`, `npm --prefix ui test`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: none.
- **Foundational (Phase 2)**: depends on Setup. Blocks all stories. Contains the risky
  embedding work (T014–T016).
- **US1 (Phase 3)**: depends on Foundational. MVP: a scene plays in the window.
- **US2 (Phase 4)**: depends on US1 (a playing session and the player screen).
- **US3 (Phase 5)**: depends on US1 and US2 (the decision measures both). T034 is conditional.
- **Polish (Phase 6)**: after US3 (the README and roadmap reflect the decision).

### Story Dependencies

```text
Setup ─► Foundational ─► US1 (MVP) ─► US2 ─► US3 (decision) ─► Polish
```

Sequential by design: this is a spike, and each step either proves the approach or triggers a
fallback before more is built on it.

### Within Each Phase

Tests first (they must fail) → core (`stash-core`, `player`) → window glue (`src-tauri`) →
commands and bindings → UI → checkpoint validation.

### Parallel Opportunities

- Setup: T003, T004, T005 in parallel after T001–T002.
- Foundational: tests T006, T007, T008 in parallel; T011 and T015 in parallel with T009/T010.
- US1: T018 and T021 in parallel with T019.
- US2: T024 and T025 in parallel.
- Polish: T036 and T037 in parallel.

---

## Parallel Example: Foundational

```bash
# Tests first, different files:
Task: "Stream URL builder and scene parsing tests in crates/stash-core/tests/scenes.rs"
Task: "Headless player session tests in crates/player/tests/session.rs"
Task: "Embedded tracks test in crates/player/tests/tracks.rs"

# Independent modules:
Task: "Player types in crates/player/src/{snapshot,tracks,error}.rs"
Task: "EGL loading and Wayland display in src-tauri/src/video_surface/egl.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1)

1. Setup → Foundational. **Stop and check early**: after T016, confirm video actually appears
   under the webview (a hard-coded test clip is fine) before building UI. If it can't be made
   to work, go straight to T034's fallback path and record why.
2. US1 → a real scene plays from your library.
3. **Validate** V1, V2, V6, V9.

### Then

4. US2 → full controls and keyboard (V3, V4, V5, V8, V10).
5. US3 → measurements and `decision.md` (fallback only if needed).
6. Polish.

Commit after each task or logical group, and only after the user has tried each story
(constitution; user preference).

---

## Notes

- `crates/player` must never gain a Tauri or GTK dependency; window-system code belongs in
  `src-tauri/src/video_surface/`.
- The libmpv render context must be dropped before the `Mpv` handle.
- Never hand mpv any URL other than the direct stream; never write to Stash.
- Regenerate `ui/src/bindings.ts` whenever a command, event, or DTO changes.
