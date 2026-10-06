# Contract: Play history

What the viewer writes to Stash about viewings, when, and how nothing is lost
([research R6](../research.md)). These are the only writes in this feature (FR-018).

## Rules (matching the Stash web UI's player)

- A **viewing** starts when a scene begins playing and ends when it stops for any reason (stop,
  close, scene change, tab close, server switch, quit, disconnect).
- **Checkpoint**: after every 60 seconds of actual playing time (paused time doesn't count), one
  `sceneSaveActivity(id, resume_time, playDuration)`, where `playDuration` is the playing time
  since the previous save (Stash adds it up).
- **End**: one final `sceneSaveActivity` with the remaining playing time.
- **Resume point**: the current position, or **0** when the position is at 98% of the duration or
  more.
- **Play count**: one `sceneAddPlay(id)` per viewing, sent with the first save at which the share
  watched (total playing time ÷ duration × 100) is at least the server's `minimumPlayPercent`
  (0 when the server doesn't set it, as the web UI).
- **Off switch**: with "Save play history" off, none of the above is sent or queued.

## The queue (`<data>/dev.semantic-stash-viewer/play-history.json`)

```text
{ "version": 1,
  "pending": { "<profile uuid>": [ { "scene_id", "resume_time", "play_duration",
                                     "add_play", "recorded_at" }, … ] } }
```

- Every report is appended before it's sent and removed when Stash confirms it (written
  atomically, as the session file).
- Reports for the same scene that are both still pending merge (durations add; the latest resume
  point wins; `add_play` is kept if either had it).
- Sent in order when the connection to that server is up: at once, on reconnect, and when a
  session for that server starts.
- Quit and server switch: the final report is queued, then the queue for that server gets up to
  2 seconds to flush; what's left stays queued for next time.
- After 3 failed attempts in a session, the notification centre says play history couldn't be
  saved yet (one entry per server, updated, not one per report).

## Cache

After a confirmed save, `scene:details:<id>` is patched (resume time, play duration, play count)
so the scene page shows the new values without a request.
