# Semantic Stash Viewer

[![CI](https://github.com/turingcomplete790/Semantic-Stash-Viewer/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/turingcomplete790/Semantic-Stash-Viewer/actions/workflows/ci.yml)

A fast, native desktop client for [Stash](https://github.com/stashapp/stash), written in Rust and
drawn with [iced](https://iced.rs/). Point it at your Stash server, add an API key if your
server uses one, and browse and play your library the way Jellyfin desktop clients work with a
Jellyfin server.

> **Status:** pre-alpha. Saved servers with automatic reconnection and a connection-security
> indicator; a Stash-style navigation bar with tabs that never reset and come back exactly after
> a restart; the whole scene library as a paged grid or list in every web UI sort, with
> thumbnails; a scene view; mpv playback inside the window with the viewer's own controls; a
> notification centre with live Stash jobs; and Settings. Screens open from a local cache and stay
> usable offline, and one command measures every performance budget. Search, filters, and the
> rest of the library come next (see the [roadmap](ROADMAP.md)). The project is specified with
> [Spec Kit](https://github.com/github/spec-kit), and its governing rules live in the
> [constitution](.specify/memory/constitution.md).

## Goals

1. **Semantic tagging, first-class.** Support every feature of the
   [`semantic-tagging`](../Stash-Plugins/semantic-tagging/) Stash plugin, including Pluggables,
   Actions, Roles, Traits, Labels, Mise-en-scène, the Timeline, Archetypes, Looks, and the
   Semantic Search Matrix. The viewer reads and writes the same data as the plugin, so both can
   be used on the same library at the same time.
2. **Native speed.** Batched, narrowly scoped GraphQL, a local cache, and a native UI. The
   interface never waits on the network.
3. **No transcoding.** Videos play in [mpv](https://mpv.io/) from Stash's direct stream and are
   decoded on your machine (hardware-decoded where the GPU can), not transcoded by the server.
4. **Full Stash web UI coverage.** Galleries, scrapers and StashBox, tag editing, markers,
   ratings, library tasks, and the rest, so you never need to go back to the browser.

## How it works

- **Stash stays the source of truth.** All edits are written back to Stash. Anything the viewer
  stores locally is a cache and can be deleted without losing data.
- **Your server only.** The only server the viewer talks to is the Stash server you configure.
  Each server's address and API key are saved in the viewer's local config.
- **Rust all the way down.** A headless core handles all Stash I/O; the UI is one hierarchical
  state machine in iced, whose work leaves as data (effects) and comes back as messages. mpv
  renders on its own thread into frames shared with iced's Vulkan device, so video reaches the
  screen without copies.

## Requirements

- Stash **v0.31.1** or newer
- Linux with a Vulkan driver (Mesa RADV, ANV, or NVIDIA) and libmpv; Wayland or X11
- Windows and macOS are planned where iced and mpv allow

## Development

### Prerequisites

- Rust 1.88 or newer
- libmpv with its development headers: `mpv` on Arch/Manjaro, `libmpv-dev` on Debian/Ubuntu.
  VA-API drivers are optional but give hardware decoding.
- libturbojpeg (`libjpeg-turbo` on Arch/Manjaro, `libturbojpeg0-dev` on Debian/Ubuntu)
- A Vulkan driver and libEGL (part of Mesa on most systems)

### Run

```bash
cargo run -p semantic-stash-viewer-native
```

The first launch asks for your Stash server's address (and API key, if it uses one).

### Files

| What | Where |
|---|---|
| Saved servers (with their API keys) | `~/.config/dev.semantic-stash-viewer/profiles.json` |
| Tabs, notifications, logs | `~/.local/share/dev.semantic-stash-viewer/` |
| Cache (per server; thumbnails too) | `~/.cache/dev.semantic-stash-viewer/` |

The cache is safe to delete at any time; Settings → Troubleshooting → Clear cache does the same
from inside the app. If you used the earlier web demo, its `semantic-stash-viewer/` folders under
`~/.config`, `~/.local/share`, and `~/.cache` are no longer read and can be deleted by hand.

### Keyboard

- Ctrl+T / Ctrl+W new / close tab, Ctrl+Tab, Ctrl+PageUp/PageDown, and Ctrl+1–9 to switch,
  Ctrl+Shift+PageUp/PageDown to move a tab
- Alt+←/→ (or the mouse's back and forward buttons) to go back and forward
- In Scenes: arrows, Home/End, `[` `]` for pages, Enter to open, Ctrl+Enter in a new tab
- In the player: Space, ←/→, ↑/↓, `[` `]` `\`, `.` `,`, F, M, Esc
- F1 or `?` lists every shortcut

### Layout

| Path | What |
|---|---|
| `crates/stash-core/` | Headless core: the only code that talks to Stash (typed GraphQL with Cynic), connections, profiles, the cache, thumbnails, notifications, and Stash jobs. No UI toolkit, no `unsafe`. |
| `crates/player/` | Headless libmpv session: playback state, commands, and the render API wrapper. |
| `native-ui/` | The app: the state machine (`app.rs`, `session/`, `shell/`, `screens/`), effects (`effects.rs`), the service layer over the core (`services/`), the video path (`player/video/`), and the UI bench (`measure/`). |
| `crates/perf-harness/` | Measures every performance budget. |
| `specs/` | Spec Kit feature specs, plans, and task lists |

### Checks

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Opt-in test against a real Stash (use a disposable instance)
STASH_TEST_URL=http://localhost:9998 cargo test -p stash-core --test live_stash -- --ignored
```

Clippy also enforces the constitution's unsafe-code rules: every `unsafe` block documented and
doing one thing, in the two crates that need it (`player`, `native-ui`); none in the core.

### Performance harness

Measures every budget from the constitution against one of your saved servers: cold start,
navigation, control presses, tab switching with a video playing, Scenes paging and 1000-card
scrolling, playback (open, seek, dropped and displayed frames), and memory and CPU:

```bash
cargo run -p perf-harness -- --profile "<server name>"            # --quick for fewer runs
cargo run -p perf-harness -- --profile "<server name>" --scenes 1,2,3 --long 1   # pin scenes
```

It builds the debug app, launches it several times (keep the window visible), and takes about
3 minutes. It only reads from Stash. Reports go to
`~/.local/share/dev.semantic-stash-viewer/perf/<timestamp>.{md,json}`, each with the change since
the previous run; anything more than 20% slower is flagged, and it exits non-zero if a budget
fails.

### Workflow

Features go through Spec Kit:

```text
/speckit-specify → /speckit-plan → /speckit-tasks → /speckit-implement
```

Every plan must pass the constitution's checks. The order features are built in is set out in
the [roadmap](ROADMAP.md).

## Related

- [semantic-tagging](../Stash-Plugins/semantic-tagging/): the Stash plugin whose features and
  data format this viewer follows
- [Stash](https://github.com/stashapp/stash)
- [iced](https://iced.rs/), [mpv](https://mpv.io/)
