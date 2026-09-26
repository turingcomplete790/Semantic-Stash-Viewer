# Research: mpv Playback Spike

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md) | **Date**: 2026-09-25

Findings marked **(observed)** were checked on the development machine or against the user's
library (`localhost:9999`, read-only). Everything else is a design decision or comes from the
cited prior art.

**Machine (observed):** Manjaro Linux, KDE on **Wayland**; AMD Radeon RX 9060 XT (RDNA4), Mesa
26.2.2 radeonsi with VA-API; mpv 0.41.0 / libmpv 2.5.0 (client API 2.5); GTK 3.24.52;
Tauri 2.11.6 (tauri-runtime-wry 2.11.4, wry 0.55.1, webkit2gtk crate 2.0.2).

---

## R1. How mpv draws on the GPU inside the app (the spike's question)

- **Decision (primary)**: **libmpv render API (OpenGL) into a `GtkGLArea` placed underneath a
  transparent WebKitGTK webview, inside a `GtkOverlay` in the same window.** mpv renders each
  frame into the GL area's framebuffer; the webview, drawn above it with a transparent
  background, holds the SolidJS controls. The GTK compositor combines the two, so the viewer's
  own HTML controls sit on top of GPU-rendered video without any frames passing through
  JavaScript.
- **Rationale**:
  - **(observed)** The installed libmpv offers only two render backends: OpenGL, and a
    software renderer that copies frames through the CPU. There's no Vulkan backend, so GPU
    rendering inside the app means OpenGL.
  - **(observed)** The session is Wayland. Embedding mpv by giving it a native window handle
    (`--wid`) only works on X11, Windows, and macOS, so it can't be the primary approach here.
  - Prior art: several Tauri apps already do exactly this on native Wayland, e.g.
    tauri-video-plugin ("renders through a native GTK GPU widget below the transparent
    WebView"), stremio-accru, and Grid.
- **Fallbacks, in order** (tried only if the primary approach fails a success criterion; each
  measured the same way and written into the decision record):
  1. **Wayland subsurface with its own render thread.** mpv renders via EGL into a
     `wl_subsurface` of the app window from a dedicated thread, so the GTK main thread does no
     per-frame work. Prior art: Aquarium (Tauri + mpv Jellyfin client) reports main-thread use
     dropping "from half a core to idle during playback". More complex (manual EGL/Wayland
     plumbing, placing the subsurface below the webview surface), so it's the first fallback,
     triggered if SC-003 (dropped frames) or main-thread load fails.
  2. **X11 via XWayland (`GDK_BACKEND=x11`) with the same GtkGLArea approach**, or `--wid`
     embedding. Loses native Wayland (scaling, fractional HiDPI), so it's recorded as a
     compromise.
  3. **Separate mpv player window** driven over mpv's IPC. Fails FR-002 (video in the app
     window); only as a last resort, and recorded as such.
- **Alternatives rejected outright**:
  - Software render API → copy frames into a webview canvas: 4K frames through the CPU and IPC
    can't meet SC-003 and defeats "GPU first".
  - A second native window stacked behind a transparent main window: Wayland doesn't let apps
    position windows relative to each other.
  - wgpu/Vulkan rendering: libmpv has no Vulkan render backend (observed).

## R2. Where the overlay goes in the GTK widget tree (a hard constraint)

- **Finding (observed in tauri-runtime-wry 2.11.4 source)**: on Linux, Tauri attaches a
  button-press handler to every window's main webview (`undecorated_resizing.rs`), whether or
  not the window is decorated. On each left click it takes `webview.parent().parent()` and does
  `downcast::<gtk::Window>().unwrap()`. If the webview's grandparent isn't the `GtkWindow`, the
  app **panics on the first click**. (tauri-video-plugin documents the same crash.)
- **Decision**: build the tree as **`GtkWindow → GtkOverlay → { GtkGLArea (main child),
  WebKitWebView (overlay child) }`**. The overlay *replaces* Tauri's default `GtkBox` as the
  window's direct child, so the webview's grandparent is still the window. Never put the overlay
  between the webview and Tauri's box.
- The rebuild happens once at startup (in Tauri's `setup` hook, on the GTK main thread), and
  the overlay and GL area live for the app's lifetime. The GL area is hidden and mpv idle while
  no player is open.
- **Consequences**: Tauri menus (which live in the default box) aren't usable; the app has
  none. Must be retested on every Tauri upgrade (quickstart V8).

## R3. mpv bindings

- **Decision**: the `libmpv2` crate v6.0.0 (default `render` feature), linking the system
  libmpv.
- **Rationale (observed)**: it checks only the libmpv *major* API version (2 = 2), so it works
  with the installed 2.5. It exposes `create_render_context` with OpenGL init params,
  `render(fbo, width, height, flip)`, `set_update_callback`, `report_swap`, and a
  `WaylandDisplay` render parameter (needed for hardware-decode interop).
- **Alternatives**: `libmpv-sys` raw bindings (more unsafe code for no gain); running `mpv` as
  a subprocess (fallback 3 only).

## R4. OpenGL function loading and hardware decoding

- **get_proc_address**: GTK 3 on Wayland creates EGL contexts, so load `eglGetProcAddress` from
  `libEGL.so.1` at runtime (`libloading`) and pass it to mpv. If GDK reports X11 (fallback 2),
  use `glXGetProcAddressARB` instead.
- **Hardware decoding**: `hwdec=auto-safe`. **(observed)** VA-API through radeonsi on the RX
  9060 XT. Pass the Wayland display (`gdk_wayland_display_get_wl_display`, from libgdk-3) as
  `MPV_RENDER_PARAM_WL_DISPLAY` when creating the render context so mpv can use zero-copy
  VA-API → EGL interop. If it's unavailable for a codec, mpv decodes in software (FR-005); the
  `hwdec-current` property says which one is active.

- **Locale (found during implementation)**: `mpv_create` returns NULL unless `LC_NUMERIC` is
  `"C"`. GTK sets the process locale from the environment (`en_US.UTF-8` here), so the app must
  reset `LC_NUMERIC` to `"C"` right before creating mpv. Headless tests don't initialise GTK
  and never hit this. `player::Player::new` does it.

## R5. Render loop and threading

- **Decision**:
  - mpv's update callback (called on an mpv thread) only sets a flag and posts **at most one**
    pending `queue_render()` to the GTK main context. Extra callbacks while one is pending are
    dropped. This avoids flooding GTK (a lesson from tauri-video-plugin, which coalesces render
    notifications into a one-item channel).
  - In the GL area's `render` signal: read the currently bound framebuffer
    (`GL_DRAW_FRAMEBUFFER_BINDING`), call `render(fbo, w, h, flip=true)`, then `report_swap()`.
  - Size comes from the GL area's allocation × scale factor, so HiDPI and fractional scaling
    render at native resolution.
  - The mpv core (commands, properties, events) runs on its own thread. The event loop thread
    turns observed property changes into `PlayerSnapshot` updates.
- **Main-thread budget**: rendering on the GTK main thread shares time with WebKit. The spike
  measures main-thread CPU during 1080p and 4K playback. Over ~50% of a core, or drops beyond
  SC-003, triggers fallback 1.

## R6. Which Stash stream to play (no transcoding)

- **(observed)** `sceneStreams` for a scene on `:9999` lists "Direct stream" at
  `/scene/{id}/stream`, plus transcoded variants: `stream.mkv`, `stream.mp4?resolution=…`,
  `stream.webm?…`, `stream.m3u8?…` (HLS), `stream.mpd?…` (DASH). `paths.stream` on the scene is
  the direct URL.
- **Decision**: the player only ever opens `{base}/scene/{id}/stream` (built by the core from
  the profile's base URL and the scene ID). Any URL with a suffix or a `resolution` parameter is
  rejected by the core, so FR-003 can't be broken by accident.
- **Auth**: mpv option `http-header-fields=ApiKey: <key>` when the profile has a key. The key
  goes in a header, not the URL.
- **TLS**: profile strict off → `tls-verify=no`; strict on → `tls-verify=yes` (feature 001
  semantics).
- **Verifying SC-005**: on the local server, confirm no `ffmpeg` child process of Stash is
  running during playback, and that the Stash log has no transcode entries for the scene.

## R7. Library mix for the test set (observed)

A random sample of 400 of the user's 33,808 scenes (read-only):

| Container, video codec | Count |
|---|---|
| mp4, h264 | 332 |
| wmv, wmv3 | 16 |
| mp4, hevc | 12 |
| webm, vp9 | 11 |
| webm, vp8 | 10 |
| matroska, h264 | 4 |
| matroska, hevc | 3 |
| mp4/avi, mpeg4 | 6 |
| flv, h264 | 2 |

8 of the 400 are 4K. **Decision**: the SC-006 test set is 12 scenes: 2× mp4/h264 (one
1080p, one 720p), 1× mp4/h264 4K, 1× mp4/hevc (4K if available), 1× wmv3, 1× webm/vp9,
1× webm/vp8, 1× matroska/h264 with multiple audio or subtitle tracks (for FR-014), 1×
matroska/hevc, 1× mpeg4 (avi), 1× flv/h264, and 1 deliberately broken case (a non-existent
scene ID). The IDs are picked at implementation time and recorded in the decision record.

## R8. Player API between UI and core

- **Decision**: the player gets its own Tauri commands and a `player-state` event, following
  the feature 001 pattern: typed bindings via tauri-specta, the UI never talks to mpv or Stash
  directly. See [contracts/player-commands.md](contracts/player-commands.md).
- Position updates are throttled to about 4 per second in the event stream (the seek bar
  doesn't need per-frame updates); discrete changes (pause, seek done, volume, speed, ended,
  error) are sent right away.
- Commands use mpv's asynchronous command API so a UI click never waits on mpv (Principle VI,
  SC-004).

## R9. Where the code lives (Principle III)

- **Stash queries** (recent scenes, scene lookup, stream URL): `crates/stash-core` (adapter),
  like the rest of the Stash I/O.
- **mpv session logic** (options, commands, property observation, state): a new headless crate,
  `crates/player`, that depends on `libmpv2` but not on Tauri or GTK. It can be unit-tested
  without a window (mpv's `vo=null` / `ao=null`).
- **GTK/GL embedding** (overlay rebuild, GL area, render loop, proc-address loading): `src-tauri`,
  in a Linux-only `video_surface` module, because it's window-system glue.

## R10. Measurement plan (for the decision record)

| Criterion | How it's measured |
|---|---|
| SC-001 time to first frame | Time from `player_open` to mpv's first `video-reconfig` plus first rendered frame (timestamps in the core); 10 opens over the test set |
| SC-002 seek latency | `seek` command → next rendered frame; 10 random seeks each in a 1080p and a 4K scene |
| SC-003 dropped frames | mpv `frame-drop-count` + `decoder-frame-drop-count` over a 5-minute 1080p run, controls toggled every 20 s |
| SC-004 control response | Browser `performance.now()` from key/click to the control's visible state change (UI-side), plus command round trip in the core |
| SC-005 no transcoding | No Stash `ffmpeg` child process and no transcode log lines during playback |
| SC-006 test set | Each of the 12 scenes opened; result is "plays" or the plain-language error shown |
| Main-thread load | `top -H` / `/proc/<pid>/task/<tid>/stat` for the GTK main thread during 1080p and 4K playback |
| FR-014 tracks | `track-list` property for the multi-track matroska scene, listed in the decision record |

## Sources

- [tauri-video-plugin, Linux architecture](https://github.com/get-air/tauri-video-plugin/blob/main/docs/linux.md)
- [Aquarium: Tauri + mpv Jellyfin client](https://github.com/eggprez/Aquarium)
- [stremio-accru: native Linux player (mpv in the app window)](https://github.com/cernoh/stremio-accru/pull/59)
- [Grid: in-process libmpv on Linux and Windows](https://github.com/lpbborges/grid/pull/13)
- [mpv-examples: GTK render example](https://github.com/mpv-player/mpv-examples/pull/44/files)
- [Tauri discussion: embedding mpv/vlc inside a window](https://github.com/orgs/tauri-apps/discussions/6343)
