# Quickstart & Validation: mpv Playback Spike

**Feature**: [spec.md](spec.md) | **Contracts**: [contracts/](contracts/) |
**Model**: [data-model.md](data-model.md) | **Research**: [research.md](research.md)

## Prerequisites

| Need | Check |
|---|---|
| Everything from feature 001 | see `specs/001-connect-to-stash/quickstart.md` |
| libmpv with the render API | `pkg-config --modversion mpv` (2.x) and `/usr/include/mpv/render_gl.h` present |
| EGL | `pkg-config --modversion egl` |
| VA-API (for hardware decoding) | `vainfo` lists H.264/HEVC decode profiles |
| A connected profile | Your library (`localhost:9999`) for playback; the test instance for authenticated access once it has media |

## Automated checks

```bash
cargo test -p stash-core      # stream-URL builder rejects transcode URLs; scene queries parse fixtures
cargo test -p player          # session state machine with mpv vo=null/ao=null on a generated test clip
npm --prefix ui test          # player controls, keyboard map, auto-hide, ended state
```

## Manual validation (Wayland/KDE)

Run `cargo tauri dev`, connect to `localhost:9999`, and open the player screen.

### V1: In-window playback (US1, SC-001, FR-002)
1. Pick the first recent scene. **Expect**: video and audio start within 1.5 s, drawn inside
   the viewer window (move and resize the window: the video moves with it; no separate window
   appears).
2. Enter a scene ID from the test set. **Expect**: same.

### V2: No transcoding (SC-005, FR-003)
While a scene plays: `pgrep -a ffmpeg` on the Stash machine shows no transcode for it, and the
Stash log has no transcode lines. The debug `player_stats` shows `hwdec: vaapi` for H.264/HEVC.

### V3: Controls over video (US2, SC-004, FR-008–FR-011)
Use every control with the mouse, then with the keyboard map
([contracts](contracts/player-commands.md#keyboard-map-handled-in-the-ui-fr-009)).
**Expect**: each responds visibly within 50 ms; the controls draw cleanly over moving video (no
flicker, black boxes, or stale frames); they hide after 3 s without mouse movement and come
back on movement; resize and fullscreen keep the aspect ratio.

### V4: Seeking (SC-002)
10 random seeks each in a 1080p scene and the 4K scene. **Expect**: the new frame within 1 s,
at least 9 of 10 times.

### V5: Dropped frames (SC-003)
Play a 1080p scene for 5 minutes, toggling the controls every 20 s. **Expect**: at most 5
dropped frames in total (`player_stats`). Note main-thread CPU (`top -H -p <pid>`).

### V6: Test set (SC-006)
Open each of the 12 test-set scenes (research R7), including the broken one. **Expect**: each
plays, or shows a plain-language error; none leaves a blank or frozen screen.

### V7: Tracks (FR-014)
Open the multi-track matroska scene. **Expect**: `player_snapshot().tracks` lists its audio and
subtitle tracks.

### V8: Clicks, resize, fullscreen don't crash (research R2)
Click around the webview (controls, empty areas), drag-resize the window, toggle fullscreen,
open and close the player repeatedly. **Expect**: no crash (this guards the Tauri
grandparent-downcast issue).

### V9: Leaving playback (FR-007)
Close the player mid-playback; separately, switch servers and disconnect during playback.
**Expect**: audio stops immediately each time, and the video surface disappears.

### V10: Ended state (FR-015)
Seek to 5 s before the end. **Expect**: the ended state with Replay; Replay restarts from 0.

## Decision record

Record the results of V1–V10 and the measurements in `decision.md` in this folder (FR-012),
using the template in the plan.
