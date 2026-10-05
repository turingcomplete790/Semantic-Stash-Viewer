# Contract: The saved session (`session.json`)

How the native app saves and restores its tabs (spec FR-007, clarified: restore everything;
research R3). The format belongs to the native app; nothing reads or writes the demo's files.

## Location

`<local_data_dir>/dev.semantic-stash-viewer/session.json` (research R4). One file; one entry per
server profile.

## Shape

```text
{
  "version": <u32>,                       // bumped whenever the state tree's saved shape changes
  "sessions": {
    "<profile uuid>": {
      "selected": <tab index>,
      "tabs": [
        { "history": [ <ScreenSnapshot>, … ], "cursor": <index into history> }
      ]
    }
  }
}
```

`ScreenSnapshot` is the serde form of a `Screen` state (data model), externally tagged by the
variant name, with only its **persistent** fields:

| Screen | Persistent fields |
|---|---|
| `Home` | – |
| `Scenes` | `query` (the core's `SceneQuery`), `page` (≥ 1), `page_size` (20, 40, 50, 60, 120, 250, 500, 1000), `mode` (`Grid` or `List`), `scroll` (logical px), `focused` (card index, optional) |
| `Scene` | `scene_id`, `title` (for the tab title before details load) |
| `Settings` | `page` (`Servers`, `Keyboard`, `Troubleshooting`, `About`); a Servers editor that was open is not restored |

**Transient** fields (loaded pages, image handles, details, in-flight generations, playback) are
never written. Restore rebuilds each screen from its snapshot and runs its entry actions, which
request data through the cache first, so tabs show at once and refresh after.

## Rules

- **Version or parse mismatch**: the file is set aside as `session.json.bad` and the app opens a
  fresh Home tab. No migrations.
- **Limits**: at most 50 tabs and 50 history entries per tab (older entries are dropped on save).
  A tab always has at least one entry; a session always has at least one tab.
- **When**: saved 500 ms after a change to any persistent field, and on quit; never from `view`.
  The write is atomic (write to a temporary file, then rename).
- **Server switch**: each profile's tabs are saved separately; switching servers restores that
  server's tabs.
