# Decision: playback approach for Phase 1

**Chosen approach**: libmpv OpenGL render API into a `GtkGLArea` under a transparent WebKitGTK
webview, inside a `GtkOverlay` in the app window (research R1, primary) |
**Date**: 2026-09-27 |
**Machine**: Manjaro, KDE Plasma on Wayland, AMD RX 9060 XT (Mesa radeonsi, VA-API), mpv 0.41 /
libmpv 2.5, Tauri 2.11.6. Library on local spinning disks, served by Stash v0.31 on `localhost`.

## Why

- It meets every success criterion with wide margins. First frame came in 33–293 ms (target
  1.5 s). The slowest seek was 534 ms (target 1 s). Long runs at 1080p and 4K dropped 0 frames.
- It draws the video in the app window with the viewer's own HTML controls on top, on native
  Wayland, without any frames going through JavaScript.
- The GTK main thread stays light: 6–7% of one core during 1080p and 4K playback, far below
  the ~50% threshold that would trigger fallback 1.
- Hardware decoding works for H.264, HEVC, and VP9 (VA-API). WMV3, MPEG-4 Part 2, and VP8 decode
  in software and still meet the targets.
- It plays Stash's direct stream only. The server did no transcoding during any test.

## Test set (T032)

Picked from `localhost:9999` with read-only queries (research R7). Durations are the whole file.

| # | Scene | Container | Video codec | Resolution | Duration | Why |
|---|---|---|---|---|---|---|
| 1 | 10861 | mp4 | h264 | 1920×1080 | 15 min | Common 1080p |
| 2 | 15402 | mp4 | h264 | 1280×720 | 22 min | Common 720p |
| 3 | 27098 | mp4 | h264 | 4096×2160 | 29 min | 4K H.264 |
| 4 | 2328 | mp4 | hevc | 3840×2160 | 25 min | 4K HEVC |
| 5 | 20405 | wmv | wmv3 | 1920×1080 | 32 min | Web-unfriendly, software decode |
| 6 | 19817 | webm | vp9 | 1920×1080 | 27 s | VP9 |
| 7 | 16406 | webm | vp8 | 852×480 | 20 s | VP8, short clip |
| 8 | 8546 | matroska | h264 | 1920×1080 | 54 min | Several subtitle tracks (FR-014) |
| 9 | 2905 | matroska | hevc | 1920×1080 | 45 min | Matroska HEVC |
| 10 | 21418 | avi | mpeg4 | 640×480 | 13 min | MPEG-4 Part 2 |
| 11 | 27339 | flv | h264 | 640×360 | 27 min | FLV without a keyframe index (research R5c) |
| 12 | 999999999 | — | — | — | — | Doesn't exist (error path) |

## Results per approach tried

Measured in the app with the debug measurement mode (`SSV_MEASURE`, `src-tauri/src/measure.rs`).
It uses the real in-window render path, muted. Open time runs from `player_open` to the first
frame drawn by the GL area. Seek time runs from sending the seek to the next frame drawn. There
were 10 exact seeks per scene, spread over 5–90% of the file, starting 3 s after opening, while
playing.

| Criterion | Target | Primary (GtkGLArea overlay) | Result |
|---|---|---|---|
| SC-001 open → first frame | ≤ 1.5 s, 9 of 10 | 33–293 ms, 11 of 11 playable scenes | Pass |
| SC-002 seek → new frame | ≤ 1 s, 9 of 10, incl. 4K | 110 of 110 under 1 s; max 534 ms (WMV) | Pass |
| SC-003 dropped frames, 1080p | ≤ 1/min over 5 min | 0 in 5 min (scene 10861) | Pass |
| SC-003 extra: 4K HEVC | — | 0 in 2 min (scene 2328) | Pass |
| SC-004 controls respond | ≤ 50 ms | Commands are queued and return at once (worker thread); the seek bar moves at once. Verified by hand during US2, not instrumented | Pass (manual) |
| SC-005 no transcoding | 0 transcode work | No Stash `ffmpeg` process in 196 samples (every 2 s) over the whole test-set run; the player only accepts `/scene/{id}/stream` | Pass |
| SC-006 play or plain error | 100% of ≥ 10 scenes | 11 played; the missing scene shows "That scene doesn't exist" | Pass |
| Main-thread CPU, 1080p | < ~50% of a core | 6–7% (5 min average 7%) | Pass |
| Main-thread CPU, 4K HEVC | < ~50% of a core | 6–7% (2 min average 7%) | Pass |

Per scene (seek median / max over 10 seeks):

| Scene | Codec | hwdec | Open | Seek median | Seek max |
|---|---|---|---|---|---|
| 10861 | h264 1080p | vaapi | 293 ms | 87 ms | 106 ms |
| 15402 | h264 720p | vaapi | 147 ms | 202 ms | 235 ms |
| 27098 | h264 4K | vaapi | 276 ms | 182 ms | 315 ms |
| 2328 | hevc 4K | vaapi | 186 ms | 118 ms | 143 ms |
| 20405 | wmv3 1080p | no (software) | 107 ms | 323 ms | 534 ms |
| 19817 | vp9 1080p | vaapi | 83 ms | 138 ms | 204 ms |
| 16406 | vp8 480p | no (software) | 103 ms | 42 ms | 49 ms |
| 8546 | h264 1080p mkv | vaapi | 128 ms | 101 ms | 126 ms |
| 2905 | hevc 1080p mkv | vaapi | 33 ms | 177 ms | 245 ms |
| 21418 | mpeg4 480p avi | no (software) | 167 ms | 116 ms | 193 ms |
| 27339 | h264 360p flv | vaapi | 109 ms | 68 ms | 92 ms |

Notes:

- WMV, AVI, FLV, and clips under 2 minutes use a larger demuxer cache (research R5c). Without
  it, FLV seeks took 0.5–8 s and AVI seeks up to 770 ms on this library's spinning disks.
- A first 5-minute run recorded 1906 dropped frames. At the same time, main-thread CPU fell to
  0% for the last ~80 s (≈ 1918 frames at 23.976 fps), because the window wasn't being drawn
  (hidden or covered). mpv counts undrawn frames as dropped. With the window visible, the rerun
  dropped 0. Playback keeps running while the window is hidden; only the frames go undrawn.

### Quickstart V1–V10

| Check | Result |
|---|---|
| V1 in-window playback | Pass: drawn in the window, follows moves and resizes |
| V2 no transcoding, hwdec | Pass: see SC-005; `vaapi` for H.264, HEVC, VP9 |
| V3 controls over video | Pass (manual, US2): no flicker or stale frames; auto-hide after 3 s; aspect kept in fullscreen |
| V4 seeking | Pass: see SC-002 |
| V5 dropped frames | Pass: see SC-003 (controls auto-hid during the run and weren't toggled every 20 s) |
| V6 test set | Pass: see SC-006 |
| V7 tracks | Pass: see below |
| V8 clicks, resize, fullscreen | Pass (manual, US2): no crash. The Tauri grandparent constraint holds |
| V9 leaving playback | Pass for close and Esc (manual). Disconnect and server switch stop playback in code (`stop_playback`), but weren't exercised mid-playback |
| V10 ended state | Pass: automated test (`end_of_file_is_ended_and_replay_restarts`) and manual check |

## Fallbacks (T034)

**Fallbacks not needed.** No criterion failed, and main-thread CPU stayed at 6–7% of a core
during 1080p and 4K playback, far below the ~50% trigger. Fallback 1 (Wayland subsurface with
its own render thread), fallback 2 (X11/XWayland), and fallback 3 (separate mpv window) weren't
built.

## Embedded tracks (FR-014)

mpv reports embedded tracks for every file, read-only in the spike. Scene 8546 (matroska):

- Video: h264
- Audio: eac3, English
- Subtitles: 34 SubRip tracks, e.g. English, English "SDH", Arabic, Czech, Danish, German, Greek,
  Spanish, Finnish, French, Hebrew, Hindi, Japanese, Korean, Russian, Chinese

Every other file listed one video and one audio track, with titles and languages where the file
has them. Picking audio and subtitle tracks is a Phase 1 feature (below). The snapshot already
carries what the picker needs.

## Known limitations and platform gaps

- **Only Linux / Wayland / KDE was tested.** Windows and macOS are untested. The video surface
  (`src-tauri/src/video_surface`) is GTK-specific and Linux-only. Other platforms need their own
  surface (e.g. a native child view under the webview) with the same `Renderer` API.
- **X11 untested.** The code picks the GLX loader under X11, but no fallback-2 run was made.
- **Tauri's widget tree is replaced** with a `GtkOverlay`, and the webview's grandparent must
  stay the `GtkWindow` (research R2). Recheck on every Tauri upgrade (quickstart V8). Tauri
  menus can't be used.
- **libmpv needs `LC_NUMERIC=C`.** The player resets it before creating mpv (research R4).
- **Early seeks on unindexed FLV** can still take seconds, until the background read-ahead
  passes the target (research R5c). Adding an index to the files fixes it, but the viewer won't
  write to the library.
- **Hidden window**: frames go undrawn and count as dropped while the window is hidden or
  covered. Audio and position keep going.
- **One machine, one GPU.** NVIDIA and Intel hardware decoding (and their EGL interop) are
  untested.

## Follow-ups for Phase 1

- Play count, play duration, and resume position sync back to Stash (constitution Principle V;
  deferred because the spike is read-only).
- Subtitle and audio track selection UI (the track list is already in the snapshot).
- Seek-bar thumbnails from Stash's preview sprites, so dragging shows previews without decoding
  (smoothest path for heavy 4K HEVC; research R5b).
- Transcoding fallback: decide whether to offer Stash's transcoded streams for files mpv can't
  decode. None came up in the test set, so the default stays direct-stream only.
- Consider pausing the render loop while the window is hidden, so the dropped-frame count stays
  meaningful.
- Keep the debug measurement mode (`SSV_MEASURE`) for rechecking performance after upgrades.
