# Research: Native UI Spike (iced)

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md) | **Date**: 2026-10-03

Machine for every measurement: Manjaro, KDE on Wayland, Xeon W-2135, Radeon RX 9060 XT (Mesa
26.2.2, RADV), mpv 0.41.0 (libmpv client API 2.5), Rust 1.94. Reference numbers: the web build's
harness run of 2026-10-03 (005 quickstart V8).

## R1. Embedding mpv's video in iced: mpv in a side EGL context, frames shared as DMA-BUF

The deciding risk (spec FR-015). libmpv's render API draws only through **OpenGL** or a slow
software renderer (`render.h`: `MPV_RENDER_API_TYPE_OPENGL`, `MPV_RENDER_API_TYPE_SW`); iced draws
through **wgpu**, which uses Vulkan on Linux.

- **Decision**: mpv renders on its own **render thread** into a **surfaceless EGL context** on the
  GPU's render node. Its target is a small ring of **4 framebuffers (3 in the first build; see below) whose textures are exported as
  DMA-BUFs** (`EGL_MESA_image_dma_buf_export`, linear modifier). Each DMA-BUF is **imported once into
  wgpu's Vulkan device** as an external-memory image (`VK_EXT_external_memory_dma_buf`,
  `VK_EXT_image_drm_format_modifier`, through `wgpu-hal` and `create_texture_from_hal`). An iced
  `shader` widget draws the newest finished frame as a textured quad. Zero copies end to end.
  - **Amended during implementation (2026-10-03, T013–T015)**:
    - **Vulkan allocates, GL imports.** wgpu 27 enables `VK_EXT_external_memory_dma_buf` (and
      `VK_KHR_external_memory_fd`) on its device but **not** `VK_EXT_image_drm_format_modifier`,
      so a GL-exported buffer can't be imported. Instead each slot is a **linear** RGBA image
      allocated on iced's device with exportable memory; its DMA-BUF (`vkGetMemoryFdKHR`, row pitch
      from `vkGetImageSubresourceLayout`) is imported into the side context with
      `EGL_EXT_image_dma_buf_import` and bound to a texture and framebuffer for mpv
      (`vk_frames.rs`, `gl_frames.rs`). No modifiers, no format negotiation. When iced draws to an
      sRGB surface the image uses the sRGB format of the same bytes.
    - **mpv gets its own Wayland connection for VA-API.** With no display in the side context mpv
      chose `hwdec=vulkan-copy` (GPU decode, copied back). Passing it a `wl_display` from its own
      `wl_display_connect` (libwayland-client, loaded at runtime; `wayland.rs`) restores `vaapi`
      with zero-copy interop, as in the web build.
    - **Orientation**: mpv renders for GL's bottom-left origin, so the shader samples rows upward
      (the first build showed the picture upside down; found by the user).
    - **A fifth slot state, "retiring"**: the frame shown just before the current one may still be
      sampled by a GPU frame in flight, so it's freed only when the next frame goes on screen.
      That needs **four slots**, not three: with three, mpv started each next frame in the ready
      slot before iced drew it, and the UI showed 0.1 of 29.9 fps (found by the user; the
      playback run now measures the displayed rate, and the harness counts unshown frames as
      dropped).
    - Sync is `glFinish` on the render thread; no visible tearing was reported, so the
      sync-file/semaphore path wasn't needed.
  - **Frame flow**: mpv's update callback wakes the render thread → it renders into the next free
    slot → waits for the GPU (`glFinish` first; an exported sync fence imported as a Vulkan
    semaphore only if `glFinish` costs frames) → publishes "slot N ready" → an iced subscription
    asks for a redraw → the widget samples slot N. A slot isn't reused while iced may still be
    sampling it (3 slots: rendering, ready, on screen).
  - **Size**: slots match the video area in physical pixels; a resize reallocates them after the
    size settles for 100 ms, scaling the newest frame meanwhile (no stale or stretched frames:
    edge case "window resize").
  - **Hardware decoding**: `hwdec=vaapi` stays as today. With no Wayland display in the side
    context, mpv's VA-API interop uses the EGL display on the render node
    (`EGL_EXT_image_dma_buf_import`); if mpv doesn't pick the device up, set `vaapi-device` to the
    same render node. Measured, not assumed (SC-001).
  - **Controls on top**: plain iced widgets in a `stack` over the video widget. No GTK overlay, no
    margins trick, no second surface: the overlay problem the web build solved with platform code
    disappears.
  - **Teardown** (SC-010): stop the render thread, free the mpv render context before the mpv
    handle (as `player::render` already orders it), destroy the wgpu textures, then the EGL
    context.
- **Fallbacks**, tried only if the decision fails a playback criterion, each measured the same way:
  1. **wgpu on its GL backend** (`WGPU_BACKEND=gl`): mpv renders into a texture owned by wgpu's GL
     context, reached through `wgpu-hal`'s GLES backend. Simpler, but ties iced to GL and to wgpu
     internals.
  2. **Read-back**: the side context renders, then copies each frame into an iced texture through a
     pixel buffer. Costs CPU and bandwidth (≈ 500 MB/s at 1080p60, ≈ 2 GB/s at 4K60), so it's
     expected to fail SC-003 at 4K; it gives a floor to compare against.
  3. A separate native child surface is not tried: mpv's `--wid` has no Wayland equivalent.
- **Checked on this machine (2026-10-03)**: Vulkan has `VK_EXT_external_memory_dma_buf` and
  `VK_EXT_image_drm_format_modifier`; EGL has `EGL_MESA_image_dma_buf_export`,
  `EGL_EXT_image_dma_buf_import`, and `EGL_MESA_platform_surfaceless`; render node
  `/dev/dri/renderD128`.
- **Rationale**: the only path that keeps hardware decoding and zero-copy display on Wayland
  without depending on wgpu's GL backend. It's the design `mpv-engine` ships on Linux (side EGL
  context, DMA-BUF export, `wgpu-hal` import), which shows it works with current drivers.
- **Alternatives rejected**:
  - **Depending on `mpv-engine`** (0.2.1, 2026-09): it needs wgpu 30 while iced 0.14 is on wgpu 27,
    so its textures can't cross into iced; it wraps mpv with `rsmpv`, a second binding beside our
    `libmpv2`; and it's very young (≈ 270 downloads). Used as a **reference design** only.
  - **`mpv-wgpu`**: read-back only (fallback 2's design).
  - **mpv's software renderer**: no hardware decoding of the frames we display; fails 4K.

## R2. iced version and setup

- **Decision**: **iced 0.14.0** (released 2025-12-07, MIT, MSRV 1.88, wgpu 27), features `wgpu`,
  `image`, `tokio`, `advanced` (custom widgets and the `shader` primitive). The new crate declares
  `rust-version = "1.88"`; the existing crates keep 1.80, so the core stays usable by the web build
  unchanged (FR-001/FR-002). `iced_test` 0.14 for headless end-to-end tests.
- **What 0.14 brings that the spike relies on**: reactive rendering (only redraws when something
  changed), concurrent image decoding and uploading, headless testing, input method support, the
  `stack`, `grid`, `sensor` and `table` widgets, multi-window support (not needed), smart
  scrollbars.
- **Rationale**: the newest stable release; COSMIC builds on it, so Wayland (KDE included) is a
  first-class target.
- **Alternatives rejected**: iced `master` (a moving target for a two-week spike; only if a 0.14 bug
  blocks the gate, recorded); other native toolkits (Slint, egui, GTK 4 via gtk-rs) are out of scope
  unless the record shows iced itself blocks a go (spec Assumptions).

## R3. Thumbnails: straight from the core, no URI scheme

- **Decision**: the grid asks the existing `ThumbService` for each card's prepared 480 px JPEG
  (cached in SQLite, or fetched and prepared, at most 6 at a time as today) and wraps the bytes in
  an iced image handle (`image::Handle::from_bytes`). iced 0.14 decodes and uploads off the UI
  thread. The app keeps handles in an LRU keyed by `(scene id, version)` (bounded to 2 pages of the
  largest size, 2,000), so a revisited page doesn't decode again. Missing or loading thumbnails show
  one shared placeholder handle.
  - **Warming** (005 R4): after a page arrives, the next page's thumbnails are requested from the
    core and their handles created, so they're decoded before the user moves.
  - If decoding in iced shows up in frame times, the core decodes to RGBA instead
    (`Handle::from_rgba`; turbojpeg is already a core dependency) and iced only uploads.
- **Rationale**: the `ssv-thumb://` scheme existed only to get bytes into the webview; a native view
  calls the core directly. This is the "easier communication with the backend" the user named.
- **Alternatives rejected**: keeping the scheme (no webview to serve); decoding on the UI thread.

## R4. The grid: plain first, windowed if needed

- **Decision**: a `scrollable` holding rows of fixed-size cards; columns follow the width
  (`responsive`), cards are at least 240 px wide (as in the web build), list rows are 44 px. Card:
  thumbnail, one-line title, one-line details. Plain (every card in the tree) first. If SC-007
  misses at 1000 cards, switch to **windowing**: rows have fixed heights, so only the rows in view
  plus one screen above and below are built, with spacers for the rest (constitution VI allows
  virtualization where a page can't stay in budget).
- One-line text: no wrapping; if iced 0.14 can't ellipsize, the text is clipped and the gap goes in
  the decision record.
- Keyboard: the 005 rules (arrows, Enter, Ctrl+Enter, `[` `]`, Home, End, page edges) are ported
  from `ui/src/scenes/keyboard.ts` into a pure function with the same tests.
- **Rationale**: the web build's measurements say a plain grid of 1000 cards is cheap once
  thumbnails are decoded; iced lays out only on change (reactive rendering), so scrolling should
  mostly re-draw.

## R5. Reusing the core: the app glue is duplicated, not moved

The web build's app glue lives in `src-tauri`: `AppState` (profiles, connection manager, tabs,
notifications, caches), `CacheRegistry` and `read_cached` (`cache_commands.rs`), `active_client`
(`player_commands.rs`).

- **Decision**: the spike carries a **minimal copy** of that glue (`native-ui/src/services.rs`),
  calling `stash-core` and `player` directly. Views run core calls as iced `Task`s and get typed
  results back as messages: no commands, events, bindings, or JSON in between. Nothing in
  `src-tauri` moves (FR-001). The decision record lists the glue that a port would move into a shared
  crate.
- **Files**: same `profiles.json` (read, never rewritten by the spike), same per-profile cache
  directory (SQLite handles a second process; the two builds are used in turn), **separate** tab
  and notification files (`shell/tabs-native.json`, `shell/notifications-native.json`), since the
  native build's view state has a different shape (edge case "same profile, two builds").
- **Read-only** (FR-003): no resume, play-count, or O-counter writes; the jobs watcher isn't
  started.
- **Rationale**: keeps the web build untouched and the spike small; the duplication is evidence for
  the decision record, not code to keep.

## R6. Tabs and view state

- **Decision**: the app state holds tabs; each tab has a history of entries (Scenes or Scene) with
  their view state (Scenes: query, page, page size, mode, scroll offset; Scene: scene id, title).
  Back from a scene returns to the previous Scenes entry with its state (005 R13 semantics). One
  mpv player for the app, as today: the scene tab that owns playback shows the video widget; when
  another tab is active, playback continues and the video widget simply isn't in the tree, so on
  return it draws the newest frame at the size iced lays out (spec User Story 3). Tabs save to
  `tabs-native.json` 500 ms after a change and on quit.
- **Rationale**: iced lays out every frame it draws from the current tree, so the web build's
  "hidden pane at 0×0" and "video off-centre until resize" bugs have no equivalent to work around.

## R7. Testing

- **Decision**:
  - Pure logic (keyboard moves, paging and past-the-end handling, tab history, view-state
    restore, frame-slot ring) as plain functions with `cargo test`.
  - `iced_test` headless scenarios for User Stories 2 and 3 flows (page keys, back restores page
    and mode, tab switch keeps state), using fixture pages, no server.
  - The DMA-BUF path gets one test that runs only when a render node is available (skipped in CI).
  - The existing gate (fmt, clippy `-D warnings`, `cargo test --workspace`, the web build's UI
    tests and bindings check) stays green.
- **Rationale**: the web build's 208 UI tests describe the behaviour; the spike ports the ones its
  screens need, which also shows how testing a native UI compares (decision record).

## R8. Measurements: same lines, same harness

- **Decision**: the native build prints the web build's `MEASURE {…}` lines for the same names
  (contract: [contracts/measurements.md](contracts/measurements.md)), driven by the same
  environment variables. The harness gets an **additive** `--app native` option that builds and
  launches the native binary instead; reports are the same table. Additions:
  - Frame timing from iced's per-frame events (`window::frames()`); missed frames against an idle
    baseline, as 003 R8.
  - The 1000-card scroll runs **twice**: thumbnails cached, then uncached (cache cleared for the
    page first), per SC-007.
  - **Memory and CPU** (SC-011): the harness samples resident memory (`VmRSS`) and CPU time from
    `/proc` for the whole process tree (for the web build that includes the WebKit web and network
    processes), at idle on Scenes (page of 50) and while a 1080p scene plays.
- **Rationale**: SC-012 needs both builds' numbers from one tool on one machine.

## R9. Order of work and the gate

- **Decision**: Phase A, playback only (User Story 1): video export, controls, keyboard, resize,
  fullscreen, teardown; measure SC-001 to SC-004 (and SC-010 for playback). **Stop with a no-go if
  they fail** after the two fallbacks (FR-015). Phase B: the grid (User Story 2). Phase C: tabs
  (User Story 3). Phase D: the full harness for both builds, the stability loop, and the decision
  record (User Story 4).
- **Time box**: about two weeks; Phase A gets the first half. If it's not through the gate by then,
  the record is written with what was measured.
