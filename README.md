# Semantic Stash Viewer

A fast, native desktop client for [Stash](https://github.com/stashapp/stash), built in Rust with
[Tauri](https://tauri.app/). Point it at your Stash server, add an API key if your server uses
one, and browse and play your library the way Jellyfin desktop clients work with a Jellyfin
server.

> **Status:** pre-alpha. The first feature works: connecting to Stash servers, saved server
> profiles, automatic reconnection, and a connection-security indicator. Browsing and playback
> come next (see the [roadmap](ROADMAP.md)). The project is specified with
> [Spec Kit](https://github.com/github/spec-kit), and its governing rules live in the
> [constitution](.specify/memory/constitution.md).

## Goals

1. **Semantic tagging, first-class.** Support every feature of the
   [`semantic-tagging`](../Stash-Plugins/semantic-tagging/) Stash plugin, including Pluggables,
   Actions, Roles, Traits, Labels, Mise-en-scène, the Timeline, Archetypes, Looks, and the
   Semantic Search Matrix. The viewer reads and writes the same data as the plugin, so both can
   be used on the same library at the same time.
2. **Near-native speed.** Batched, narrowly scoped GraphQL, a local semantic index, and
   virtualized views. The interface never waits on the network.
3. **No transcoding.** Videos play in [mpv](https://mpv.io/) from Stash's direct stream and are
   decoded on your machine, not transcoded by the server.
4. **Full Stash web UI coverage.** Galleries, scrapers and StashBox, tag editing, markers,
   ratings, library tasks, and the rest, so you never need to go back to the browser.

## How it works

- **Stash stays the source of truth.** All edits are written back to Stash. Anything the viewer
  stores locally is a cache and can be deleted without losing data.
- **Your server only.** The only server the viewer talks to is the Stash server you configure.
  Each server's address and API key are saved in the viewer's local config.
- **Rust core, web frontend.** A Rust core handles all Stash I/O and the semantic engine. The
  Tauri webview only draws the interface.

## Requirements

- Stash **v0.31.1** or newer
- mpv / libmpv
- Linux is the main platform. Windows and macOS are planned where Tauri and mpv allow.

## Development

### Prerequisites

- Rust 1.80 or newer, Node 20 or newer, npm
- Tauri CLI v2: `cargo install tauri-cli --version "^2"`
- Linux: the WebKitGTK 4.1 development package (for example `libwebkit2gtk-4.1-dev` on
  Debian/Ubuntu, `webkit2gtk-4.1` on Arch/Manjaro)

### Run

```bash
npm --prefix ui install
cargo tauri dev
```

Saved server profiles, including their API keys, live in
`~/.config/semantic-stash-viewer/profiles.json`. Logs go to
`~/.local/share/semantic-stash-viewer/logs/`.

### Layout

| Path | What |
|---|---|
| `crates/stash-core/` | Headless Rust core: the only code that talks to Stash, plus connection and profile logic. Never depends on Tauri. |
| `src-tauri/` | Thin Tauri shell: typed commands and events over `stash-core` |
| `ui/` | SolidJS + TypeScript frontend. `ui/src/bindings.ts` is generated from Rust. |
| `specs/` | Spec Kit feature specs, plans, and task lists |

### Checks

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm --prefix ui run lint && npm --prefix ui run typecheck && npm --prefix ui test

# Opt-in test against a real Stash (use a disposable instance)
STASH_TEST_URL=http://localhost:9998 cargo test -p stash-core --test live_stash -- --ignored

# Regenerate ui/src/bindings.ts after changing a command or DTO (CI fails on drift)
cargo run -p semantic-stash-viewer --bin export-bindings
```

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
