# Implementation Plan: Port the Viewer to iced

**Branch**: `007-port-to-iced` | **Date**: 2026-10-04 (revised after clarification and the library
analysis) | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/007-port-to-iced/spec.md`

## Summary

Build a clean native UI that offers every capability of the Tauri demo, keeps the "behaviours that
stay" exactly, and then retires the demo.

**The UI: a hierarchical state machine in iced's own state** (research R1)
- Nested enums (exclusive states) and structs (regions active at the same time): App →
  Onboarding | Session; Session = connection ∥ shell ∥ playback; Shell → tabs; Tab → a history of
  whole `Screen` states; Screen → Home | Scenes | Scene | Settings.
- `update` performs transitions and bubbles unhandled events upward.
- There are no named routes. Back and forward return whole screen states, so a tab never resets
  by construction.

**Effects as data** (R2)
- `update` never does I/O or decoding. Transitions return effect values that the root turns into
  tasks.
- The core fetches and deserializes off the UI thread; only typed results come back.
- Transitions are tested as pure functions.

**Saving and files** (R3, R4)
- Tabs are saved as a versioned serde snapshot of the state tree, with full restore and no
  migrations.
- All files live under the native app's own `dev.semantic-stash-viewer` directories. Nothing is
  shared with the demo.

**GraphQL** (R5)
- `stash-core` moves from `graphql_client` to **Cynic**, with fragments per view tier, schema from
  Stash v0.31.1, and the same transport and tests. Done first, before the screens depend on it.

**Story order**
1. Onboarding and servers.
2. The shell: tabs, history, overlays, keymap, session save and restore.
3. Scenes.
4. The scene view and playback, with the unsafe-code hardening (R11).
5. Notifications and jobs.
6. Settings, measurement, the capability checklist and sign-off, then the demo's removal.

## Technical Context

**Language/Version**: Rust. The native crate needs 1.88 (iced 0.14) and Cynic 3.14 needs 1.85.
The workspace moves to 1.88 when the demo is removed (until then the core crates keep 1.80 where
possible, and `stash-core` raises to 1.85 with Cynic).

**Primary Dependencies**:
- Kept from 006: `iced` 0.14 (`wgpu`, `image`, `tokio`, `advanced`, plus `svg` for icons),
  `iced_test`, `ash`, `khronos-egl`, `gl`, `libloading`.
- New in `stash-core`: **`cynic` 3.14** (and `cynic-codegen` at build time), replacing
  `graphql_client`. No state machine library (hsmc rejected; statig is the fallback).

**Storage**: the native app's own files under `dev.semantic-stash-viewer/`: `profiles.json`
(core format), `session.json` (snapshot, contract), `notifications.json` (core format), the
per-profile view cache, and logs.

**Testing**:
- Transition tests (pure functions) per state machine level.
- Headless `iced_test` flows.
- Session snapshot round-trips.
- Core tests unchanged (on Cynic).
- The harness and its own tests.
- The capability checklist (`capabilities.md`), checked by hand per story.

**Target Platform**: Linux, KDE on Wayland, with an AMD GPU (RADV). Other platforms are the
documented unknowns from 006.

**Project Type**: a desktop app in a Cargo workspace. After retirement: `stash-core`, `player`,
`native-ui`, `perf-harness`.

**Performance Goals**: spec SC-002 to SC-008: cold start under 2 s; navigation under 150 ms;
input under 50 ms; 20-tab switch under 150 ms; page change under 150 ms; jump under 1 s;
1000-card scroll under 1% missed (cold and cached); playback as 006; no slower than the demo
anywhere.

**Constraints**:
- No writes to Stash are added, and nothing may assume read-only access.
- The demo is frozen; no compatibility with its files or UI.
- The unsafe-code gate applies.
- `update` is pure (R2).

**Scale/Scope**:
- 4 screen kinds, 4 Settings pages, 5 overlays, 3 session regions.
- Libraries up to 50,000 scenes.
- Up to 50 tabs with 50 history entries each.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Check | Status |
|---|---|---|
| I. Stash is the system of record | No local library data; the cache stays discardable; the session file holds only UI state | ✅ |
| II. Semantic-tagging parity | Not touched. Cynic's custom scalar for `custom_fields` and typed mutation inputs prepare for it | ✅ n/a |
| III. Brain/UI separation | Domain logic stays in `stash-core` and `player`. The UI reaches Stash only through effects that call the service layer (typed, async). Cynic stays inside the core adapter | ✅ |
| IV. Lean GraphQL, paged lists | Cynic fragments per view tier (card, player), one query per page, neighbour prefetch; paging rules unchanged | ✅ |
| V. mpv is the media player | 006's in-window zero-copy mpv; the UI plays no media | ✅ |
| VI. Responsive UI | Effects keep `update` free of I/O and decoding (checkable); budgets measured; keyboard-first focus as entry effects | ✅ |
| VII. User-owned connection | Profiles, keys in headers, the lenient-TLS option, and the security indicator. Cynic doesn't change transport | ✅ |
| VIII. Web UI parity | Every carried-over capability tracked in the capability checklist | ✅ |
| IX. One app shell | Navigation, tabs that never reset (by construction), one notification centre, quiet infrastructure | ✅ |
| X. Galleries | Not part of this feature | ✅ n/a |
| Technology: native UI with iced; GraphQL through one adapter, typed operations | iced HSM; Cynic is typed and checked against the schema | ✅ |
| Transition from the Tauri build | Frozen, removed after sign-off (R14) | ✅ |
| Unsafe-code gate | Unsafe only in the video path's foreign-library boundary (`player::render`, `native-ui/src/player/video/*`), serving libmpv, EGL and GL, Vulkan through wgpu-hal, and libwayland, behind owning types; lints and `forbid(unsafe_code)` in `stash-core` (R11) | ✅ after R11 (US4) |
| Dependencies justified | Cynic replaces `graphql_client` (R5; MPL-2.0 noted below); no others | ✅ |

The gate passes. *Re-checked after Phase 1 design: unchanged.*

## Project Structure

### Documentation (this feature)

```text
specs/007-port-to-iced/
├── spec.md
├── plan.md                       # this file
├── research.md                   # R1–R15
├── data-model.md                 # the state machine, level by level
├── quickstart.md                 # V0–V6
├── contracts/
│   ├── session-snapshot.md       # session.json
│   └── measurements.md           # perf-harness crate, bench lines, baseline on Testing
├── checklists/requirements.md
├── capabilities.md               # created in US1; filled per story; signed off before retirement
└── tasks.md                      # /speckit-tasks
```

### Source Code (repository root)

```text
native-ui/src/
├── main.rs, lib.rs
├── app.rs                    # App: Onboarding | Session; iced update/view/subscription glue
├── machine.rs                # Step, UpEvent, enter/exit conventions shared by every level
├── effects.rs                # Effect enum → Task over the service layer (R2)
├── services/                 # profiles, connection, cache + read_cached, thumbs, notifications,
│                             # jobs, player; paths under dev.semantic-stash-viewer (R4)
├── session/
│   ├── mod.rs                # Session: connection ∥ shell ∥ playback
│   ├── connection.rs         # connection region
│   ├── playback.rs           # playback region around the 006 player
│   └── snapshot.rs           # session.json (R3)
├── shell/                    # Shell, Tab (history of Screens), overlays, keymap, now-playing,
│                             # notifications, toasts, tab strip, nav bar views
├── screens/                  # home, scenes/ (state, page loading, grid, card, controls,
│                             # keyboard, thumbs), scene, settings/ (servers, keyboard,
│                             # troubleshooting, about)
├── onboarding.rs             # server form for the first server
├── messages.rs               # plain-language failures (R8)
├── widgets/                  # icons (svg), one-line text, shared controls
├── player/                   # 006 controls, view, video/ (hardened, R11)
└── measure/                  # bench, playback (R13)
native-ui/tests/              # transitions per level, flows (iced_test), snapshot round-trips

crates/stash-core/            # adapter on Cynic (R5): build.rs registers the v0.31.1 schema;
                              # fragments per view tier; #![forbid(unsafe_code)]
crates/player/                # Mpv owner without the 'static transmute (R11)
crates/perf-harness/          # moved from src-tauri/src/bin (R13)
ui/, src-tauri/               # frozen demo; removed in US6 after sign-off (R14)
```

**Structure Decision**: the state tree's levels map to modules (`app`, `session`, `shell`,
`screens`), with shared transition conventions in `machine.rs` and all side effects funnelled
through `effects.rs`. The core keeps the domain and, after R5, owns typed GraphQL through Cynic.
The harness moves out of `src-tauri` so it survives the demo's removal.

## Complexity Tracking

No constitution violations. Dependency note (constitution, Dependencies): **Cynic** is MPL-2.0.
Its file-level copyleft only binds modified copies of Cynic's own files; it's used unmodified from
this MIT project. It replaces `graphql_client` and is widely used (about 5 million downloads,
maintained since 2020).
