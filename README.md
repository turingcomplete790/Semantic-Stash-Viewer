# Semantic Stash Viewer

A fast, native desktop client for [Stash](https://github.com/stashapp/stash), built in Rust with
[Tauri](https://tauri.app/). Point it at your Stash server, add an API key if your server uses
one, and browse and play your library the way Jellyfin desktop clients work with a Jellyfin
server.

> **Status:** pre-alpha. Nothing is implemented yet. The project is being specified with
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
- **Your server, your keys.** The only server the viewer talks to is the Stash server you
  configure. API keys are kept in your operating system's credential store.
- **Rust core, web frontend.** A Rust core handles all Stash I/O and the semantic engine. The
  Tauri webview only draws the interface.

## Requirements

- Stash **v0.31.1** or newer
- mpv / libmpv
- Linux is the main platform. Windows and macOS are planned where Tauri and mpv allow.

## Development

This project uses spec-driven development. Features go through:

```text
/speckit-specify → /speckit-plan → /speckit-tasks → /speckit-implement
```

Every plan must pass the constitution's checks. Build instructions will be added here once the
first feature is implemented.

## Related

- [semantic-tagging](../Stash-Plugins/semantic-tagging/): the Stash plugin whose features and
  data format this viewer follows
- [Stash](https://github.com/stashapp/stash)
