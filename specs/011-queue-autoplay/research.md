# Research: Queue Autoplay and Loop (011)

Decisions for the plan, each built on 010's queue view (`specs/010-queue-view/research.md`).

## R1. A click plays

- **Decision**: a row click selects the scene, loads its panel at once (as 010), and plays it,
  owned by the tab, through the same path as 010's double-click and Enter (`QueuePlay`). A click
  on the scene already playing only selects it (no restart, FR-002). Double-click stays (it's now
  the same as a click). Middle-click still opens the scene's page in a new tab.
- **Rationale**: FR-001/FR-002; the play path already exists and carries resume, history, and the
  transcode fallback.
- **Alternatives**: a setting for click-to-select (rejected: the user asked for click-to-play;
  the keyboard can still browse without playing only while it moves).

## R2. The keyboard plays on rest

- **Decision**: 010's rest timer gains an intent: a rest started by the user's keyboard move
  **plays** when it comes due (and loads the panel), a rest started by a page arriving only
  **loads** (a restore or a refresh never starts playback). The timer is set whenever the
  keyboard selects a scene that isn't the playing one, even if its panel is already loaded.
  Enter still plays at once.
- **Rationale**: Clarification Q2 (A); passed rows load and play nothing, so holding ↓ costs no
  requests and no player opens (FR-003, SC-005's `gql-queue-sweep`).
- **Alternatives**: playing on every move (rejected in clarification: an open and a viewing per
  row passed); a longer rest for playback than for details (rejected: two timers for one
  intent, and 250 ms already exceeds key repeat).

## R3. Autoplay: choosing Queue and changing the results

- **Decision**: choosing Queue plays the selection at once (a deliberate act, like a click; it
  takes over the one player, US2 AS4). A results change (search, filter, sort, page size, page,
  a saved filter) sets 010's pending play to **First**, so the new page's first card plays when
  it arrives, exactly as Play next crossing a page does. A refresh of the same page (cache
  changed) is not a results change: the selection and playback stay.
- **Rationale**: FR-004 with no new mechanism; 010 already clears and re-selects on these
  changes.
- **Alternatives**: playing on every page arrival (rejected: a restore or refresh would start
  playback, FR-005).

## R4. Autoplay when switching to a queue tab

- **Decision**: when the user switches to a tab in Queue (tab click, Ctrl+Tab, Ctrl+1–9, closing
  the tab before it), its selection plays **only if the player is idle**. If something already
  plays (in that tab or another), nothing changes: the now-playing bar stays, and a click takes
  over. If the tab's page isn't loaded yet (a restored tab), a pending **Start** plays the
  selection when it arrives. At launch, the restored active tab doesn't play (the user hasn't
  switched to it).
- **Rationale**: FR-005 and US2 AS3 (a restored tab plays when the user switches to it) without
  tab switches stealing playback from another tab, which would make the now-playing bar useless
  (Principle IX).
- **Alternatives**: always taking over on switch (rejected: switching tabs to look at something
  would stop what plays); never autoplaying on switch (rejected: US2 AS3).

## R5. Loop: forward past the queue's end

- **Decision**: `advance` forward at the last card of the last page, with Loop on: on a multi-page
  result, page 1 with pending **First** (instant when cached, 008's prefetch keeps page 1's
  neighbour); on a one-page result, card 0 at once. Previous never wraps. Loop off: 010's stop
  with Replay. Loop applies wherever `advance` runs forward: Play next on Ended, `n`, and the
  panel's Play next button.
- **Rationale**: Clarification Q1 (A), FR-006; no new path, only a new branch where 010 returned
  Stop.
- **Alternatives**: loading all pages to wrap in memory (rejected: paging, Principle IV).

## R6. Never loop through failures

- **Decision**: a playback error never advances (010 FR-012 unchanged), so a loop can't cycle
  through failing scenes. Deleted scenes are still skipped (010), but the session counts
  consecutive skips and stops once they reach the result's size, so a queue of only deleted
  scenes stops instead of looping forever. The count resets when a scene starts playing.
- **Rationale**: FR-007 and the spec's edge cases.
- **Alternatives**: none worth recording.

## R7. Settings, keys, and measuring

- **Decision**: `loop_queue` is saved with the tab (default on, serde default, session v2). The
  controls show "Loop: on/off" beside "Play next: on/off" in Queue. Keyboard: `L` toggles Loop
  (queue level; free in the player and Scenes levels). New harness rows: `queue-click-play`
  (click → playing, ≤ 1500 ms), `queue-autoplay` (choosing Queue → playing, ≤ 1500 ms), and
  `queue-loop` (last scene's end → page 1's first playing, ≤ 1500 ms). Play history is off
  while the bench seeks to ends (no writes to Testing).
- **Rationale**: SC-001–SC-003, FR-006, Principle VI.
- **Alternatives**: one combined "autoplay" switch (rejected: the spec makes autoplay the queue's
  behaviour, not an option; Loop is the switch).
