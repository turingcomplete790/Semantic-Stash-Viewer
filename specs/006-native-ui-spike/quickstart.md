# Quickstart: Native UI Spike (iced)

How to build, run, and validate the spike. Everything is read-only against the Production profile
(`localhost:9999`); nothing is written to Stash (spec FR-003).

## Prerequisites

- The web build works on this machine (005 is merged); its saved profiles are used as they are.
- Rust ≥ 1.88 (the native crate's minimum; the machine has 1.94), libmpv (mpv 0.41), Mesa with
  RADV, a Wayland session.
- Vulkan device extensions `VK_EXT_external_memory_dma_buf` and `VK_EXT_image_drm_format_modifier`
  (`vulkaninfo | grep -E "external_memory_dma_buf|drm_format_modifier"`), and EGL with
  `EGL_MESA_image_dma_buf_export` (`eglinfo | grep dma_buf_export`).

## Build and run

```bash
cargo run -p semantic-stash-viewer-native          # the native build
cargo test -p semantic-stash-viewer-native         # its logic and headless UI tests
cargo run -p semantic-stash-viewer                 # the web build, unchanged
```

## Validation

### V1: Playback gate (User Story 1; SC-001–SC-004)
Open a scene from the recently added list. Expect video inside the window with hardware decoding
(the player shows `vaapi`), every control working by mouse and keyboard (002 FR-008/FR-009), clean
drawing over moving video, correct aspect through resizes and fullscreen, and a clean close. Then:

```bash
cargo run --bin perf-harness -- --profile "Production" --app native --quick
```

**Expect**: playback open < 1.5 s, seek median < 1 s (4K included), 0 dropped frames per minute at
1080p. **If this fails after the fallbacks (research R1), stop: write the no-go record.**

### V2: Scenes grid (User Story 2; SC-005–SC-007)
Browse at 50 and 1000 per page in grid and list; next/previous, go to page, sort, page size; open a
scene from page 12 in list mode half-scrolled and go back. **Expect**: the 005 behaviour, the same
total count as the web build, and the same page, mode, and scroll on return.

### V3: Tabs (User Story 3; SC-008)
Play a scene, switch to a Scenes tab and change pages, switch back; repeat. Quit and relaunch.
**Expect**: video correct on the first frame after every switch, no tab resets, tabs restored.

### V4: Full harness, both builds (SC-001–SC-011)
```bash
cargo run --bin perf-harness -- --profile "Production" --app native
cargo run --bin perf-harness -- --profile "Production" --app web
```
**Expect**: every budget in [contracts/measurements.md](contracts/measurements.md) passes for the
native build, including both 1000-card scroll runs (cached and cold), plus memory and CPU rows for
both builds.

### V5: Stability (SC-010)
20 cycles of launch, browse, play, quit (the harness's `--app native` cold-start loop plus V1's
playback, or by hand). **Expect**: no crash, hang, or lost tab state.

### V6: Decision (SC-012)
Write `decision.md` per [contracts/decision-record.md](contracts/decision-record.md) from V1–V5.
**Expect**: GO only if SC-001 to SC-010 pass.

## Results

### V1: Playback gate, measured (2026-10-03)

Machine as research R1; native build, debug. Production's library sits on spinning disks that
spin down (first opens there took up to 20 s in **both** builds), so measurements use the Testing
profile (NVMe), loaded with copies of Production files picked by codec (scenes 181–195).

**Codec run** (`SSV_MEASURE` over 15 files; open = to first frame; seeks = 10 per file, exact):

| File | Decoding | Open | Seeks (min–max) | Dropped |
|---|---|---|---|---|
| H.264 1080p MP4 | vaapi | 154 ms | 47–75 ms | 0 |
| H.264 4K MP4 | vaapi | 120 ms | 69–142 ms | 0 |
| HEVC 1080p MKV / MP4 | vaapi | 70 / 103 ms | 67–219 / 77–246 ms | 0 |
| HEVC 4K MP4 (3 files, one high-bitrate) | vaapi | 68–71 ms | 42–563 ms | 0 |
| AV1 1080p / 4K MKV | vaapi | 65 / 44 ms | 67–220 / 111–685 ms | 0 |
| VP9 4K WebM | vaapi | 65 ms | 84–713 ms | 0 |
| H.264 FLV | vaapi | 68 ms | 53–65 ms | 0 |
| MPEG-4 Part 2 AVI (2) | software | 49 / 96 ms | 43–137 ms | 0 |
| WMV3 720p (4) | software | 23–79 ms | 51–196 ms | 0 |
| VP8 1080p WebM | software | 50 ms | 46–131 ms | 0 |

**Quick harness** (`perf-harness --profile "Testing" --app native --quick`): cold start (warm
cache) 513 ms; playback open 82 ms median / 146 ms p95; seek 152 ms median / 513 ms p95 / 583 ms
max; 0 dropped frames over 60 s of 1080p HEVC (vaapi); main-thread CPU 5% while playing. Bench rows
are "not measured" until the US2 bench (T031). Web build, same machine (005, Production, 2026-10-03):
cold start 962–1092 ms, open 109–323 ms median, seek 162–621 ms median, main thread 10–11%.

SC-001 (open < 1.5 s, hwdec for H.264 and HEVC): **pass**. SC-002 (seek median < 1 s, 4K
included): **pass** (worst single seek 713 ms). SC-004: the input-acknowledgement number comes with
the bench (T031); the drawing check is manual (below).

**SC-003 correction.** The first report above said SC-003 passed with "0 dropped". It didn't: the
user found playback was a slideshow. mpv's dropped-frame counter only sees frames mpv fails to
render; mpv rendered every frame (29.9 fps), but the UI showed 0.1 fps. Cause: the slot ring had
three slots, and with one on screen and one retiring, mpv started each next frame in the ready slot
before iced could draw it. Fixed with four slots (a test now fails with three). The playback run
now reports `rendered_fps` and `displayed_fps`, and the harness counts frames rendered but never
shown as dropped, with a `playback-displayed-fps` row. Remeasured after the fix:

| File | Rendered | Displayed | Dropped | Main-thread CPU |
|---|---|---|---|---|
| H.264 1080p 30 fps (scene 185, 20 s) | 29.9 fps | 29.9 fps | 0 | 10% |
| HEVC 1080p 24 fps (scene 191, harness, 60 s) | 24 fps | 24.0 fps | 0 | 5% |
| HEVC 4K 120 fps (scene 187, 30 s) | 119.7 fps | 119.5 fps | 0 | 24% |
| AV1 4K 24 fps (scene 193, 30 s) | 23.9 fps | 23.9 fps | 0 | 5% |

SC-003 (0 dropped/min at 1080p, ≤ 1/min at 4K, counting frames not shown): **pass**, remeasured.

Found and fixed on the way: the picture was upside down (mpv renders for GL's origin; reported by
the user); mpv fell back to `vulkan-copy` until it had a Wayland connection of its own; a launch
could sit on "Connected" forever if the session was up before the app first looked.

**Manual checks** (with the user, 2026-10-03): playback smooth (4K included), picture upright with correct colours, controls and keys working, controls drawn cleanly over the video, clean quit: **pass**. The user: "The latency alone is worth the switch to iced." **Gate: passed.**
