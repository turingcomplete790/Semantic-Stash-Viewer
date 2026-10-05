# Implementation Plan: Native UI Spike (iced)

**Branch**: `006-native-ui-spike` | **Date**: 2026-10-03 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/006-native-ui-spike/spec.md`

## Summary

A time-boxed spike that builds a second, native build of the viewer with **iced 0.14** to decide
whether the interface leaves Tauri and the Linux webview. It reuses `stash-core` and the `player`
crate directly and leaves the web build untouched.

Work runs in gated order:

1. **Playback (the gate)**: mpv renders on its own thread into a surfaceless EGL context, into
   DMA-BUF-backed framebuffers that wgpu imports as Vulkan textures. An iced `shader` widget draws
   them, with ordinary iced controls stacked on top (research R1). If this misses the playback
   criteria after two fallbacks, the spike stops with a no-go.
2. **The paged Scenes grid**, with thumbnails straight from the core's thumbnail service, without
   the URI scheme (R3, R4).
3. **Tabs**, with a scene playing (R6).
4. **Measurement and decision**: both builds through the same harness, with a new `--app native`
   option plus memory and CPU rows (R8), ending in a go/no-go decision record.

## Technical Context

**Language/Version**: Rust; the new crate needs 1.88 (iced 0.14), the existing crates stay at 1.80;
the machine has 1.94.

**Primary Dependencies** (new, native crate only; justified in Complexity Tracking):
- `iced` 0.14 (`wgpu`, `image`, `tokio`, `advanced`) and `iced_test` 0.14 (dev).
- `wgpu-hal` 27 and `ash` 0.38, matching iced's wgpu, for the Vulkan DMA-BUF import.
- `khronos-egl` (dynamic loading) for the surfaceless side context, the `gl` loader (already used
  by the web build) for the few GL calls that set up mpv's framebuffers, and `libloading` (already in
  the tree) for mpv's Wayland connection (research R1 amendment).

Reused unchanged: `stash-core`, `player` (`libmpv2` 6), `tokio`, `uuid`, `serde`.

**Storage**:
- The web build's `profiles.json`, read only.
- The per-profile SQLite view cache, shared, used by one build at a time.
- New: `shell/tabs-native.json` and `shell/notifications-native.json` (R5).

**Testing**:
- `cargo test` for the logic.
- `iced_test` headless flows.
- A DMA-BUF test that runs only where a render node exists.
- The harness, with `--app native`.
- The existing gate stays green.

**Target Platform**: Linux, KDE on Wayland, with the Radeon RX 9060 XT (RADV, Mesa 26.2.2) and
mpv 0.41. Windows, macOS and X11 are recorded as unknowns.

**Project Type**: a desktop app; a second binary in the existing Cargo workspace.

**Performance Goals**: the spec's SC-001 to SC-010. They are the constitution VI budgets and the
002/005 playback and grid budgets. New: the 1000-card scroll stays under 1% missed frames while
thumbnails are still arriving.

**Constraints**:
- Read-only against Stash.
- No behaviour change to the web build.
- No new network destinations.
- The API key stays in request headers, as the core already does.
- Time-boxed to about two weeks, with the playback gate in the first half.

**Scale/Scope**:
- The Production library (36,350 scenes).
- Pages of 20 to 1000.
- Up to 20 tabs.
- One player.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Check | Status |
|---|---|---|
| I. Stash is the system of record | Read-only; the cache stays the discardable 003 cache; nothing is written to Stash | ✅ |
| II. Semantic-tagging parity | Not touched | ✅ n/a |
| III. Brain/UI separation | Domain logic stays in `stash-core` and `player`, unchanged. The native UI reaches Stash only through the core (the spike's glue copy, R5), never through its own GraphQL. Deviation: the principle names "Tauri commands/events" as the only path; the spike reaches the core by direct calls instead (Complexity Tracking) | ⚠️ justified |
| IV. Lean GraphQL; paged lists | Same core queries: one card-sized query per page, neighbour prefetch, sizes 20–1000, default 50 | ✅ |
| V. mpv is the media player | mpv still plays all video, from the direct stream, with hwdec; thumbnails are images; nothing else plays media | ✅ |
| VI. Responsive UI | The spike measures every budget, for both builds, with the same harness rules | ✅ |
| VII. User-owned connection | Same profiles, same key handling (headers only); no new destinations | ✅ |
| VIII. Web UI parity | Scenes grid and list, sorts, and pagination as 005; the rest is out of scope for a spike | ✅ |
| IX. One app shell; tabs never reset | Tab strip, per-entry view state, and restore on relaunch (R6) | ✅ |
| X. Galleries beyond Stash | Not part of the spike; the mpv-in-iced path is what galleries would use | ✅ n/a |
| Technology & Platform Constraints | "Application shell: Tauri" and "Frontend: web UI in the Tauri webview" are deviated from by a separate, unshipped spike build (Complexity Tracking). They are amended only on a GO, through `/speckit-constitution` | ⚠️ justified |
| Dependencies must be justified | Each new crate is justified below | ✅ |

The gate passes, with justified deviations. *Re-checked after Phase 1 design: unchanged.* The
design keeps the spike in its own crate, touches `src-tauri` only additively (the harness's
`--app` option), and changes no core behaviour.

## Project Structure

### Documentation (this feature)

```text
specs/006-native-ui-spike/
├── spec.md
├── plan.md                    # this file
├── research.md                # R1–R9
├── data-model.md
├── quickstart.md              # V1–V6
├── contracts/
│   ├── measurements.md        # MEASURE lines, env, harness --app native, memory/CPU rows
│   └── decision-record.md     # what decision.md must contain
├── checklists/
│   └── requirements.md
├── decision.md                # written at the end of the spike (or at a playback no-go)
└── tasks.md                   # /speckit-tasks
```

### Source Code (repository root)

```text
native-ui/                                  # NEW crate: semantic-stash-viewer-native (rust-version 1.88)
├── Cargo.toml
├── src/
│   ├── main.rs                             # entry: tokio runtime, paths, env flags, iced::application
│   ├── app.rs                              # State, Message, update, view, subscriptions
│   ├── services.rs                         # glue copy (R5): profiles, connection, CacheRegistry, read_cached, thumbs, player
│   ├── shell/
│   │   ├── tabs.rs                         # Tab, history, R13 return rules, persistence to tabs-native.json
│   │   └── tab_strip.rs                    # the tab strip view
│   ├── scenes/
│   │   ├── page.rs                         # page loading state machine (005 createScenePage)
│   │   ├── view.rs                         # toolbar, sort, mode, page controls, grid/list
│   │   ├── grid.rs                         # rows of cards; windowing if needed (R4)
│   │   ├── keyboard.rs                     # port of ui/src/scenes/keyboard.ts (pure)
│   │   └── thumbs.rs                       # handle LRU, placeholder, next-page warming (R3)
│   ├── player/
│   │   ├── view.rs                         # stack: video widget + controls, auto-hide, fullscreen
│   │   ├── controls.rs                     # seek bar, time, speed, volume, buttons; 002 shortcuts
│   │   └── video/
│   │       ├── mod.rs                      # VideoSurface: slots, resize settling, teardown order
│   │       ├── egl.rs                      # surfaceless EGL; GL loader for mpv
│   │       ├── vk_frames.rs                # linear exportable images on iced's device → DMA-BUFs
│   │       ├── gl_frames.rs                # those DMA-BUFs imported as mpv's framebuffers
│   │       ├── wayland.rs                  # mpv's own wl_display for VA-API
│   │       ├── render_thread.rs            # mpv update callback → render → fence → slot ready
│   │       └── primitive.rs                # iced shader::Primitive drawing the ready slot
│   └── measure/
│       ├── bench.rs                        # UI bench driver (SSV_DEBUG_BENCH), MEASURE lines
│       └── playback.rs                     # SSV_MEASURE playback run
└── tests/
    ├── keyboard.rs, tabs.rs, page.rs       # pure logic
    ├── flows.rs                            # iced_test: page keys, back restores, tab switch keeps state
    └── dmabuf.rs                           # skipped without a render node

crates/stash-core/, crates/player/          # reused; changes only if additive (none planned)
src-tauri/src/bin/perf_harness.rs           # ADDITIVE: --app web|native; memory/CPU rows for both
Cargo.toml                                  # workspace member "native-ui" added
```

**Structure Decision**: one new workspace crate, `native-ui`, beside `src-tauri`, so both builds
share `stash-core` and `player` and can be measured on the same machine on the same day. Toolkit-
specific video code lives in the native crate, not in `player`, because it depends on iced's wgpu
version. If the spike is a GO, the port decides what moves into shared crates (decision record,
section 6).

## Complexity Tracking

| Deviation | Why needed | Simpler alternative rejected because |
|---|---|---|
| A second app build with a native toolkit (Technology constraints: Tauri plus webview) | The spike's question is whether to leave Tauri and the webview; it can only be answered by building the hard screens natively | Prototyping inside Tauri can't measure a native toolkit; a throwaway outside the repo couldn't reuse the core or the harness |
| Principle III's "Tauri commands/events" path replaced by direct core calls | There's no webview to bridge to; direct calls are the native equivalent and keep the core as the only path to Stash | Building a command layer inside a native app adds the very indirection the spike is testing the removal of |
| Duplicated app glue (`services.rs` mirrors parts of `src-tauri`) | Keeps the web build untouched (FR-001) | Moving the glue into a shared crate now would change the web build before the decision |
| New dependencies: `iced` / `iced_test` (UI, MIT, the subject of the spike); `wgpu-hal` + `ash` (Vulkan DMA-BUF import, already in iced's tree); `khronos-egl` (side GL context, small, MIT/Apache); `gl` (GL calls, already used by the web build); `libloading` (already in the tree) | Required for the native UI and the zero-copy video path (R1) | `mpv-engine` would bundle the video path but needs wgpu 30 against iced's 27 and brings a second mpv binding (R1) |
