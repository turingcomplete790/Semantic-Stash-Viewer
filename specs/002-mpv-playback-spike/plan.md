# Implementation Plan: mpv Playback Spike

**Branch**: `002-mpv-playback-spike` | **Date**: 2026-09-25 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/002-mpv-playback-spike/spec.md`

## Summary

Prove that a Stash scene plays inside the viewer's window with mpv rendering on the GPU, the
viewer's own SolidJS controls drawn on top, and Stash serving the original file (no
transcoding). Then write down the approach Phase 1 builds on.

Approach (research R1–R5): libmpv's **OpenGL render API** draws into a **`GtkGLArea`** that
sits underneath the **transparent WebKitGTK webview**, both inside a **`GtkOverlay`** that
replaces Tauri's default box as the window's direct child (a hard constraint from a Tauri
runtime quirk, R2). mpv's redraw callback posts at most one pending redraw to the GTK main
loop; the GL area renders into its bound framebuffer. Hardware decoding uses VA-API through the
Wayland display. The player only ever opens Stash's direct stream `/scene/{id}/stream`, with
the API key in a header.

If the primary approach fails a success criterion, fallbacks are tried in order: a Wayland
subsurface with its own render thread, then XWayland, then a separate mpv window. Each is
measured the same way. All results go into `decision.md`.

## Technical Context

**Language/Version**: Rust 1.94 (workspace minimum 1.80); TypeScript 5.9

**Primary Dependencies** (new):
- `crates/player`: `libmpv2` 6.0 (system libmpv 2.5, render API)
- `src-tauri` (Linux only): `gtk` 0.18 and `glib` 0.18 (the versions Tauri already uses),
  `webkit2gtk` 2.0 (already a transitive dependency), `libloading` (load `eglGetProcAddress`),
  `gl` (only `glGetIntegerv` for the bound framebuffer)
- Existing: tauri 2.11, tauri-specta, reqwest, graphql_client, SolidJS

**Storage**: none (no persistence; FR-013 forbids writes to Stash)

**Testing**: `cargo test` (stream-URL builder, scene-query fixtures, player session state
machine running mpv headless with `vo=null`/`ao=null` on mpv's built-in `av://lavfi:testsrc`
test source, so no media files are needed); Vitest for controls, keyboard map, and auto-hide;
manual validation V1–V10 in [quickstart.md](quickstart.md) with measurements for the decision
record

**Target Platform**: Linux, KDE on Wayland (AMD RX 9060 XT, Mesa radeonsi, VA-API). Windows
and macOS are recorded as untested

**Project Type**: Desktop app (Tauri: Rust core + web UI), extending feature 001

**Performance Goals**: first frame ≤ 1.5 s (SC-001); seek ≤ 1 s including 4K (SC-002); ≤ 1
dropped frame/min at 1080p (SC-003); controls respond ≤ 50 ms (SC-004)

**Constraints**:
- Webview's grandparent must stay the `GtkWindow` (R2), or Tauri panics on click.
- The GTK main thread renders video and runs WebKit. Main-thread load over ~50% of a core
  during playback triggers fallback 1.
- Only the direct stream; no transcoded URLs.
- No writes to Stash.

**Scale/Scope**: 1 player screen, ~14 commands, 1 event; 12-scene test set drawn from a
33,808-scene library (codec mix in research R7)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | How this plan complies | Status |
|---|---|---|
| **I. Stash is the system of record** | Nothing persisted locally; nothing written to Stash (FR-013). | ✅ Pass |
| **II. Semantic-tagging parity** | Not applicable. | ✅ N/A |
| **III. Brain/UI separation** | Stash queries in `stash-core`; mpv session logic in a new headless `crates/player` (no Tauri, no GTK); GTK/GL window glue in `src-tauri`; the UI only uses typed commands/events and never touches mpv or Stash. | ✅ Pass |
| **IV. Lean, batched GraphQL** | Two small typed queries (recent scenes, one scene) selecting only the fields shown. | ✅ Pass |
| **V. Native playback over transcoding** | mpv plays `/scene/{id}/stream` (the original file) with hardware decoding; transcoded URLs are rejected by the core; auth via the profile's API key. The optional transcoding fallback isn't built (the constitution allows but doesn't require it). **Syncing play count/resume back to Stash is deferred**, see Complexity Tracking. | ⚠️ Pass with a recorded deviation |
| **VI. Responsive UI** | Commands are asynchronous; position events throttled to ~4/s; redraws coalesced; main-thread load measured, with a fallback if it's too high. 50 ms input budget is SC-004. | ✅ Pass |
| **VII. User-owned connection** | Uses the active profile's key (header) and strict-TLS setting; no other network destinations. | ✅ Pass |
| **VIII. Web UI parity** | Delivers playback, a prerequisite for the web UI's scene player; the web player's features are covered later in Phase 1. | ✅ Pass |
| **Tech constraints** | mpv/libmpv is the video pipeline; new dependencies justified in research (R3–R5). | ✅ Pass |
| **Workflow gates** | Tests for the stream-URL guard, scene queries, and the player state machine; fmt/clippy/lint; the decision record is a spike deliverable. | ✅ Pass |

**Post-design re-check (after Phase 1)**: ✅ unchanged. The contract keeps the stream-URL
guard in the core (the UI passes only a scene ID), and the only new network requests are the
two read-only queries plus mpv's stream fetch.

## Project Structure

### Documentation (this feature)

```text
specs/002-mpv-playback-spike/
├── plan.md               # This file
├── research.md           # Phase 0: R1–R10 (approach, constraints, measurements)
├── data-model.md         # Phase 1: scenes, player session, states, errors
├── quickstart.md         # Phase 1: validation V1–V10
├── contracts/
│   └── player-commands.md
├── checklists/requirements.md
├── decision.md           # Spike output (FR-012), written at the end of implementation
└── tasks.md              # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/
├── stash-core/
│   ├── graphql/{recent_scenes,playable_scene}.graphql
│   └── src/scenes/          # SceneListItem, PlayableScene, direct-stream URL builder + guard
└── player/                  # NEW: headless, depends on libmpv2; no Tauri/GTK
    ├── Cargo.toml
    ├── src/
    │   ├── lib.rs
    │   ├── session.rs       # PlayerSession: open/close, options (hwdec, http-header-fields, tls-verify)
    │   ├── commands.rs      # async mpv commands (pause, seek, speed, volume, frame-step, replay)
    │   ├── observe.rs       # property observation → PlayerSnapshot (throttled position)
    │   ├── tracks.rs        # track-list → Track (FR-014)
    │   ├── error.rs         # PlayerError mapping
    │   └── render.rs        # thin wrapper over the libmpv render context (GL agnostic)
    └── tests/session.rs     # vo=null/ao=null with av://lavfi:testsrc
src-tauri/src/
├── player_commands.rs       # Tauri commands + player-state event
└── video_surface/           # Linux only
    ├── mod.rs               # rebuild window → GtkOverlay → { GtkGLArea, webview } (R2)
    ├── gl_area.rs           # render signal, coalesced queue_render, FBO lookup, HiDPI size
    └── egl.rs               # eglGetProcAddress via libloading; Wayland display for hwdec
ui/src/
├── player/
│   ├── PlayerScreen.tsx     # scene picker (recent + by ID) and the player
│   ├── Controls.tsx         # overlay controls, seek bar with hover time, speed menu
│   ├── keyboard.ts          # FR-009 key map
│   └── state.ts             # player-state store
└── __tests__/player/*.test.tsx
```

**Structure Decision**: extends the feature 001 workspace. The new `crates/player` keeps mpv
logic headless and testable (Principle III). Window-system glue stays in `src-tauri`, behind a
Linux-only module, so fallbacks (subsurface, XWayland) can replace `video_surface` without
touching the player crate or the UI.

## Decision record template (`decision.md`)

```markdown
# Decision: playback approach for Phase 1
Chosen approach: … | Date: … | Machine: …
## Why
## Results per approach tried
| Criterion | Target | Primary (GtkGLArea overlay) | Fallback … |
| SC-001 … SC-006, main-thread CPU, hwdec per codec | | | |
## Embedded tracks (FR-014)
## Known limitations and platform gaps
## Follow-ups for Phase 1
```

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| Principle V: playback state (play count, resume point) isn't synced back to Stash in this feature | The spike is read-only by design (FR-013), and its playback tests run against the user's real library, where writes would change their data | Writing play counts during a spike would alter the user's library for test runs; syncing is a normal Phase 1 feature ("Play count, play duration, and resume position sync back to Stash") and is tracked on the roadmap |
| Tauri's default window box is replaced by a `GtkOverlay` (menus unusable) | Needed so the webview can sit on top of the GL area while its grandparent stays the window (research R2) | Inserting the overlay between the webview and the box makes Tauri panic on the first click; the app has no Tauri menus |
