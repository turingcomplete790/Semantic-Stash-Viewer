# Quickstart: validating queue autoplay and loop (011)

Prerequisites: the native app built (`cargo build -p semantic-stash-viewer-native`); the Testing
instance on :9998 (saved profile `localhost:9998`). Testing's seeded data (008, 010): performer A
(id 2) is on scenes 181–190; scene 24 is 26 seconds long; scene 1022 can't be decoded.

## Automated

```bash
cargo test --workspace                       # native-ui/tests/queue.rs covers the transitions
cargo run -p perf-harness -- --profile "localhost:9998" --quick   # queue-* rows
```

Expected: `queue-click-play`, `queue-autoplay`, `queue-loop` within 1.5 s; every 010 row still
within its budget ([contract](contracts/queue-autoplay.md)).

## V1: Click to play (US1)

In a Scenes tab filtered to performer A, choose Queue: the first scene starts playing. Click three
other rows: each plays at once, the previous stops, the row is marked playing. Click the playing
row: it carries on. Hold ↓ through the list: nothing plays while it moves; stop on a row: after a
moment it plays.

## V2: Autoplay (US2)

Change the sort: the new first scene plays. Open a second tab, play something there, switch back
to the queue tab: the other scene keeps playing (now-playing bar). Stop playback, switch away and
back: the queue's selection plays. Quit with a queue tab active and restart: the tab shows its
scene, nothing plays until you click.

## V3: Loop (US3)

On performer A's queue (one page), play the last scene and seek near its end: the first scene
plays. On an unfiltered tab at 20 per page, go to the last page, play its last scene to the end:
page 1 loads and its first scene plays. Press `L` (Loop off), repeat: playback stops with Replay.
Scene 1022 in the queue: the error shows and nothing advances.
