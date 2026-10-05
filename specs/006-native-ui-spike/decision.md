# Decision: Native UI Spike (iced)

**Feature**: [spec.md](spec.md) | **Date**: 2026-10-03 | **Contract**:
[contracts/decision-record.md](contracts/decision-record.md)

## 1. Decision

**GO.** The viewer's interface moves from Tauri and the Linux webview to a native Rust UI built
with iced. The deciding risk, mpv's hardware-decoded video inside an iced window on Wayland, is
solved with zero copies. Playback beats the web build on every measured number, and the user,
after testing it, judged the latency alone worth the switch. The constitution was amended to
v4.0.0 the same day.

**How this differs from the plan.** The spec's rule was "GO only if SC-001 to SC-010 pass". The
spike stopped after the playback gate (US1). The user made the call on that evidence and on
WebKitGTK's record in features 003–005: layout bugs in hidden views, lazy-image stalls, and
exit-time crashes in the web process. SC-005 to SC-010 (grid, tabs, stability) were not measured
here. They become acceptance criteria of the port, and the web build stays as the reference until
the port meets them (constitution, "Transition from the Tauri build").

## 2. Evidence

The machine for both builds is described in research R1. Native numbers use the **Testing**
profile (NVMe, codec samples from Production). The web build's numbers are from its 2026-10-03
harness runs on **Production**, whose spinning disks add first-open delays. Both builds played
the same files on Production at the start of the spike: native opens were 60–185 ms, and the web
build's harness median was 109 ms.

| Criterion | Budget | Native build | Web build | Pass |
|---|---|---|---|---|
| SC-001 open, hwdec | < 1.5 s; hwdec for H.264 + HEVC | 82 ms median, 146 ms p95; vaapi for H.264, HEVC, AV1, VP9 | 109 ms median, 367 ms p95; vaapi | ✅ |
| SC-002 seek | < 1 s median, 4K included | 152 ms median, worst single 713 ms (4K VP9) | 162 ms median, 1432 ms p95 | ✅ |
| SC-003 dropped | 0/min 1080p, ≤ 1/min 4K, frames shown | 0; every frame shown (29.9/29.9 fps 1080p, 119.5/119.7 fps 4K HEVC, 23.9/23.9 fps 4K AV1) | 0 | ✅ (after the slot fix, §3) |
| SC-004 controls | clean drawing; input < 50 ms | Clean (manual check, user); input latency not benchmarked yet | 32 ms input ack | ✅ manual / number pending |
| SC-005 page change | < 150 ms ≤ 250/page | – (moved to the port) | 81 ms median warm; 4.5 s p95 cold | – |
| SC-006 page jump | < 1 s | – (moved to the port) | 138–215 ms | – |
| SC-007 1000-card scroll, incl. cold | < 1% missed | – (moved to the port) | 0% warm; 1.4–2% grid while thumbnails stream in | – |
| SC-008 tab switch, video sized | < 150 ms | – (moved to the port) | 35–48 ms median, 93–176 ms p95 | – |
| SC-009 cold start | < 2 s warm cache | 513–516 ms | 962–1092 ms | ✅ |
| SC-010 stability | 20 cycles clean | Harness launches and the user's quits were clean; the 20-cycle loop wasn't run | Exit-time WebKit web-process crash (upstream, seen since 003) | partial |
| SC-011 memory / CPU | report both | Main thread 5–10% playing 1080p (24% at 4K/120 fps); memory not measured | Main thread 10–11% playing 1080p | partial |

The detailed codec table is in [quickstart.md](quickstart.md), "Results".

## 3. Playback approach

**The path that shipped** is a variant of research R1's primary design:

- mpv renders on its own thread in a surfaceless EGL context.
- It renders into **four linear RGBA images allocated on iced's Vulkan device**, whose memory is
  exported as DMA-BUFs and imported into mpv's GL context.
- An iced `shader` widget draws the newest frame inside its render pass, with the controls as
  ordinary iced widgets stacked on top.
- mpv gets its own Wayland connection, so VA-API stays zero-copy.

**What changed from the plan, and why:**

| Change | Reason |
|---|---|
| Vulkan allocates, GL imports (not the reverse) | wgpu 27 doesn't enable `VK_EXT_image_drm_format_modifier` |
| mpv given a `wl_display` of its own | without one, mpv chose `vulkan-copy` (GPU decode, copied back) |
| Shader samples rows bottom-up | mpv renders for GL's origin; the first build was upside down (found by the user) |
| Four slots instead of three | with three, mpv started each frame in the slot waiting to be shown: 0.1 of 29.9 fps displayed, a slideshow (found by the user; mpv's own counters said 0 dropped) |

**Not needed**: the fallbacks (wgpu GL backend, read-back) and a cross-API fence. `glFinish` on the
render thread was enough.

**Hardware decoding per codec:**
- **vaapi**: H.264, HEVC, AV1 and VP9 at up to 4K, and H.264 in FLV.
- **software**: MPEG-4 Part 2, WMV3 and VP8. The GPU has no decoder for them, the same as the web
  build.

**Platform-specific**: the path uses DMA-BUF, EGL and Vulkan external memory, which makes it Linux
only. Windows and macOS need their own sharing paths (D3D11 shared textures, IOSurface), which
`mpv-engine` shows are possible.

## 4. What got simpler, what got harder

**Simpler**
- **Talking to the core and cache.** Views call `stash-core` and its cache directly as tasks and
  get typed results. There's no command layer, no Specta bindings, no JSON, and no
  `ssv-thumb://` scheme.
- **Playback overlays.** Controls over video are a `stack` of widgets. The GTK overlay,
  margin-tracking, and hidden-pane workarounds have no equivalent.
- **Latency and start-up.** Cold start roughly halves (513 ms against about 1000 ms), and playback
  opens and seeks are faster.
- **One language and one toolchain.** `cargo fmt`, `clippy` and `test` cover everything. There's
  no Node, Vite or dev server, and the harness doesn't need one for the native build.
- **Testing.** UI logic (key maps, auto-hide, the seek-bar target, the slot ring) is plain Rust
  with plain unit tests.

**Harder**
- **The video path.** About 1,100 lines of EGL, Vulkan and DMA-BUF interop in `player/video/`, with
  16 `unsafe` blocks. It's contained and tested, but specialised.
- **Embedding mpv.** libmpv embeds only through OpenGL, so a side GL context stays until libmpv
  offers Vulkan (see the follow-up on that question).
- **Build size and time.** The debug binary is 537 MB against the web build's 353 MB, and iced's
  dependencies add a cold build of a few minutes. Release sizes weren't compared.
- **Widgets.** Anything a browser gives for free has to be built or found as an iced widget:
  rich text, context menus, drag and drop.

## 5. Toolkit gaps

The spike's screens didn't need text input, menus, or accessibility, so these are unknowns to
check early in the port rather than measured gaps:

| Area | Matters for | What to check |
|---|---|---|
| Text input and IME | search, filters, metadata editing | iced 0.14's `text_input` / `text_editor` and its IME support on KDE Wayland |
| Accessibility (screen readers) | general use | iced's accessibility status; plan for keyboard-first in any case (constitution VI) |
| Context menus, tooltips, drag and drop | grids, tabs, galleries | available widgets versus building our own |
| One-line text with ellipsis | cards, tab titles | whether 0.14 ellipsizes, or we clip |
| Large image grids | Scenes, galleries | the grid scroll budget with thumbnails arriving (SC-007), the web build's weak spot |

## 6. Glue to move

`native-ui/src/services.rs` copies the web build's app glue: `CacheRegistry` and `read_cached`
from `src-tauri/src/cache_commands.rs`, `active_client` and `open_scene` from
`player_commands.rs`, the per-profile thumbnail service from `thumb_scheme.rs`, and the profile
and connection set-up from `state.rs` and `lib.rs`.

In the port it becomes the app's own service layer, as one module or a small `app-core` crate. The
`src-tauri` originals go when the web build is removed. Also to bring over:
- tab persistence (`stash_core::shell::tabs`, already in the core);
- the notification centre (already in the core);
- the jobs watcher, not started in the spike because the spike was read-only.

## 7. Next steps

- **Constitution**: amended to **v4.0.0**. iced replaces Tauri and the webview; Principle III's
  path is the core's typed async API; Principle V says "the UI toolkit plays no media", with
  playback inside the window; Principle VI requires measuring frames actually displayed; and the
  Tauri build is frozen until the port covers features 001–005.
- **Port feature**, specified next. Screens in order:
  1. connection, profiles and settings (001, 003 cache settings);
  2. the app shell: navigation bar, tabs, notification centre (004);
  3. paged Scenes with thumbnails (005 US1, with SC-005–SC-008 from this spike);
  4. the scene page and player (005, using this spike's player).

  Carried over unchanged: `stash-core`, `player`, the harness (with `--app native`), and this
  spike's `native-ui` crate as the starting point. Rewritten: the SolidJS UI (`ui/`), using its
  tests and specs as the behaviour reference.
- **Web build**: frozen. It stays buildable as the reference for side-by-side harness runs until
  the port reaches parity, then is removed with `ui/` and the Tauri-specific parts of
  `src-tauri`.

## 8. Unknowns

- **Platforms**: only KDE on Wayland with an AMD GPU (RADV) was tested. X11, GNOME, NVIDIA,
  Windows and macOS are untested.
- **Memory use** for both builds wasn't measured (T040 not built).
- **Input latency** in the native build isn't benchmarked yet (comes with the port's bench).
- **The 20-cycle stability loop** wasn't run.
- **Builds**: everything measured was a debug build; release builds will differ.
