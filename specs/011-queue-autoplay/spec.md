# Feature Specification: Queue Autoplay and Playlist Controls

**Feature Branch**: `011-queue-autoplay` (builds on 010, merged to `main`)

**Created**: 2026-10-11

**Status**: Draft

**Input**: User description: "queue view should autoplay and autoloop. It should switch to any
video you click on in the queue and play it." Revised the same day: "I meant loop the video …
looping the queue makes sense too. Lets change this to use the typical media controls for a
playlist (shuffle, replay, replay^1 repeat video, autoplay) … use functional programming to
handle a state machine for the various states of the queue."

## Overview

The queue view (010) lists the Scenes tab's results beside a panel with the selected scene. As
built, choosing a scene only shows it; the user then presses Play, and Play next stops after the
last scene. This feature makes the queue a player first: choosing any scene plays it at once, and
the queue gets the controls people know from music and video players: **Autoplay** (keep playing
by itself, as on YouTube), **Shuffle** 🔀, and one **Repeat** button that cycles Off → Repeat all
🔁 → Repeat one 🔂 (as on Spotify, VLC, and Apple Music).

## Clarifications

### Session 2026-10-11

- Q: What should "autoloop" loop? → A: The whole queue: after the last scene of the last page,
  playback starts again from the first scene of page 1. *(Revised below: the request meant
  repeating the current video; repeating the queue stays as Repeat all.)*
- Q: Does moving the selection with the keyboard also switch playback? → A: Yes, once it rests:
  rows only passed over are highlighted, and the scene the selection stops on starts playing
  (the same 250 ms rest as 010's details).
- Revision (owner): "loop" meant repeating the current video; the queue uses the typical playlist
  controls instead of a single Loop switch: Shuffle, Repeat (all and one), and Autoplay, with the
  usual symbols; the queue's playback rules are one state machine written as pure functions.
- Q: How should the repeat options be offered? → A: One Repeat button cycling Off → Repeat all 🔁
  → Repeat one 🔂, its icon showing the state, behaving like Spotify, VLC, and Apple Music.
- Q: What does Shuffle shuffle? → A: The whole result in play order: every scene plays once, in
  random order, before any repeats; the list keeps its sort and follows the playing scene across
  pages; turning Shuffle off continues in list order from the current scene.
- Q: What does the Autoplay switch control? → A: Both, as on YouTube: with Autoplay on, the queue
  starts on its own (choosing Queue, new results, switching to an idle queue tab) and moves on
  when a scene ends; with it off, nothing starts or moves on unless the user chooses a scene
  (click, the keyboard's rest, Enter) or presses Next or Previous.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Click a scene, it plays (Priority: P1)

In the queue view, the user clicks any scene in the list. The panel switches to it and it starts
playing straight away, replacing whatever was playing. There is no separate Play step.

**Why this priority**: It's the core of the request ("switch to any video you click on in the
queue and play it"), and it changes the queue's main interaction.

**Independent Test**: With a scene playing in the queue, click three other scenes in turn: each
one starts playing in the panel within the playback budget, the previous one stops, and the
clicked row becomes both the selection and the playing row.

**Acceptance Scenarios**:

1. **Given** the queue view with a scene playing, **When** the user clicks another scene in the
   list, **Then** the clicked scene starts playing in the panel (from its resume point when
   "Resume where you left off" is on) and the previous one stops.
2. **Given** the queue view with nothing playing, **When** the user clicks a scene, **Then** it
   starts playing in the panel (whether Autoplay is on or off).
3. **Given** a scene playing, **When** the user clicks the scene that's already playing, **Then**
   playback carries on uninterrupted (it does not restart).
4. **Given** the user moves through the list with the keyboard, **When** the selection moves,
   **Then** rows passed over are only highlighted, and the scene the selection rests on (250 ms)
   starts playing, replacing the playing scene; holding ↓ through the list starts nothing until
   the selection stops.

---

### User Story 2 - Autoplay: the queue plays by itself (Priority: P1)

With **Autoplay** on (the default), the queue view plays on its own: choosing Queue starts the
selected scene, new results start their first scene, switching to a queue tab while nothing plays
starts its selection, and when a scene ends the next one plays. With Autoplay off, the queue only
plays what the user chooses, and a scene stops at its end.

**Why this priority**: "Autoplay" is half of the request: the queue view is for watching, so it
shouldn't wait for a click.

**Independent Test**: Switch a Scenes tab from Grid to Queue: the selected scene starts playing
within the playback budget with no further input, and the next scene follows when it ends. Turn
Autoplay off and repeat: nothing starts until a click, and the scene stops at its end.

**Acceptance Scenarios**:

1. **Given** Autoplay on and a Scenes tab in Grid, List, or Wall, **When** the user chooses Queue,
   **Then** the selected scene starts playing in the panel.
2. **Given** Autoplay on in Queue, **When** the search, a filter, the sort, or the page changes,
   **Then** the new list's first scene starts playing.
3. **Given** Autoplay on and a scene ending, **When** it reaches its end, **Then** the next scene
   (by Shuffle and Repeat, US3) starts and becomes the selection.
4. **Given** Autoplay on and nothing playing, **When** the user switches to a tab in Queue,
   **Then** its selection starts playing; if something else is playing, it keeps playing.
5. **Given** the app restarts with a tab in Queue, **When** that tab is first shown, **Then** it
   shows the remembered scene and doesn't play until the user switches to it or chooses a scene.
6. **Given** Autoplay off, **When** the user chooses Queue, changes the results, switches tabs, or
   a scene ends, **Then** nothing starts; a scene that ends stops with Replay.

---

### User Story 3 - Shuffle and Repeat (Priority: P1)

Beside Autoplay, the queue has **Shuffle** 🔀 and one **Repeat** button that cycles Off → Repeat
all 🔁 → Repeat one 🔂. Repeat one plays the current scene again each time it ends (the "loop the
video" of the request); Repeat all starts the queue over after its last scene; Shuffle plays the
whole result in random order, every scene once per round.

**Why this priority**: Explicitly requested ("shuffle, replay, replay^1 repeat video"); with
Autoplay it makes the queue a complete player.

**Independent Test**: Turn on Repeat one and let a scene end: it plays again from the start. Cycle
to Repeat all, play the last scene of the last page to its end: page 1's first scene plays. Turn
on Shuffle and play through ten scenes: none repeats, the list keeps its order, and each playing
scene is selected and in view (on its own page).

**Acceptance Scenarios**:

1. **Given** Repeat one, **When** the playing scene ends, **Then** it plays again from the
   beginning (whether Autoplay is on or off); Next and Previous still move to another scene and
   Repeat one stays on.
2. **Given** Repeat all and Autoplay on, **When** the last scene of the queue ends, **Then** the
   queue starts over: page 1's first scene (loaded, or at once when cached), or a new shuffled
   round when Shuffle is on; on a one-page queue, its first scene.
3. **Given** Repeat off and Autoplay on, **When** the last scene of the queue ends, **Then**
   playback stops with Replay, as in 010.
4. **Given** Shuffle on, **When** the queue moves on (a scene ends with Autoplay on, or Next),
   **Then** a scene not yet played in this round plays, from any page; the list keeps its sort and
   shows that scene's page with it selected; Previous goes back through the scenes played in this
   round.
5. **Given** Shuffle on, **When** the user turns it off, **Then** the queue continues in list
   order from the playing scene.
6. **Given** the Repeat button, **When** the user presses it (or its key) repeatedly, **Then** it
   cycles Off → Repeat all → Repeat one → Off, its icon showing the current state.

---

### Edge Cases

- **A scene that can't play**: nothing moves on from a failed scene by itself, and Repeat one
  doesn't retry it; the panel shows the error and the transcode offer (008), and the queue waits
  for the user (as 010 FR-012).
- **Holding ↓ through the list**: rows highlight as they pass and nothing plays until the
  selection stops; the scene that was playing keeps playing meanwhile.
- **Every scene fails**: Repeat all never cycles through failing scenes, since a failure stops the
  queue (above).
- **A scene deleted on the server**: skipped, as in 010; a round of only deleted scenes stops
  instead of repeating.
- **An empty result**: nothing plays; the empty state shows as in 010.
- **New results while shuffling**: a new round starts over the new results.
- **A one-scene queue**: Repeat all and Repeat one both replay it; Shuffle plays it.
- **Switching tabs**: playback continues with the now-playing bar (007); coming back to the queue
  tab doesn't restart or switch playback.
- **Leaving Queue for Grid, List, or Wall**: playback continues with the now-playing bar, as in
  010; switching back to Queue doesn't restart the playing scene if it's still in the list.
- **Play history**: each scene started, by Autoplay, a click, Shuffle, or Repeat, records its play
  history as any playback does (008); a repeated scene starts a new viewing each time.
- **Volume**: playback starts at the player's current volume and mute setting; the queue never
  changes them.

## Requirements *(mandatory)*

### Functional Requirements

**Click to play (US1)**

- **FR-001**: Clicking a scene in the queue's list MUST play it in the panel at once, replacing
  any playing scene, and make it the selection, whatever the Autoplay setting; this replaces
  010's click-to-select (010 FR-004).
- **FR-002**: Clicking the scene that's already playing MUST NOT restart it.
- **FR-003**: Moving the selection with the keyboard MUST switch playback to the scene the
  selection rests on (250 ms, 010's rest), and MUST NOT start playback for rows only passed over;
  Enter plays the selection at once.

**Autoplay (US2)**

- **FR-004**: The queue view MUST offer **Autoplay**, on by default and saved with the tab,
  replacing 010's Play next switch (and its key). With it on: choosing Queue and any change of
  results while in Queue (search, filter, sort, page size, page) start the selected (first)
  scene; switching to a queue tab while nothing plays starts its selection; a scene that ends is
  followed by the next (FR-006–FR-008). With it off: none of these start anything, and a scene
  stops at its end (unless Repeat one, FR-006).
- **FR-005**: A tab in Queue MUST start playing on its own only when it's the tab on screen; a
  restored or background tab waits until it's shown, and returning to a tab whose scene is already
  playing MUST NOT restart it.

**Shuffle and Repeat (US3)**

- **FR-006**: The queue view MUST offer one **Repeat** control cycling Off → Repeat all → Repeat
  one, saved with the tab, Off by default. Repeat one: the playing scene starts again from the
  beginning when it ends (with Autoplay on or off). Repeat all: when the queue's last scene ends
  with Autoplay on (or on Next from it), the queue starts over (page 1's first scene, or a new
  shuffled round). Off: the queue stops at its end with Replay.
- **FR-007**: The queue view MUST offer **Shuffle**, saved with the tab, off by default. With it
  on, moving on (Autoplay or Next) plays a scene of the current results not yet played in this
  round, chosen at random from every page; every scene plays once per round; Previous returns
  through the round's played scenes; the list keeps its sort and follows the playing scene to its
  page, selected. Turning Shuffle off continues in list order; new results start a new round.
- **FR-008**: Next and Previous MUST follow the same rules as a scene ending (Shuffle order, Repeat
  all at the end), except that Repeat one never holds them on the same scene; Previous never
  wraps from the first scene of a list-order queue.
- **FR-009**: The controls MUST use the familiar playlist symbols (shuffle 🔀, repeat 🔁, repeat
  one 🔂 with its "1", and an autoplay switch), each showing its state (on, off, which repeat),
  with a tooltip naming it; each MUST have a key, listed in keyboard help and Settings →
  Keyboard: `A` Autoplay (010's `a`), `R` cycles Repeat, and a key for Shuffle that clashes with
  no other key at the queue's level or the player's.
- **FR-010**: Nothing MUST move on, repeat, or reshuffle after a playback failure (010 FR-012);
  a round of only deleted scenes stops.

**Kept from 010**

- **FR-011**: The playing mark, Next and Previous (`n`, `p`), the list and panel focus, and the
  request budgets of 010 MUST keep working.

### Key Entities

- **Queue settings** (saved with the tab): Autoplay (on/off, default on), Repeat (Off, All, One;
  default Off), Shuffle (on/off, default off).
- **Shuffle round**: the scenes of the current results already played in this round, in the order
  they played (for Previous); a round ends when every scene has played, or the results change.
- **Playing scene / selection** (010): with click-to-play they are the same scene, except for the
  moment the keyboard moves the selection, before it rests.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A clicked scene starts playing in under 1.5 seconds (the playback open budget).
- **SC-002**: With Autoplay on, switching a tab to Queue starts playback in under 1.5 seconds with
  no further input.
- **SC-003**: When a scene ends, the next scene (by Shuffle and Repeat, including a return to
  page 1 or a shuffled pick on another page) or the same scene (Repeat one) starts playing in under
  1.5 seconds.
- **SC-004**: With Autoplay on and Repeat all, a user can leave the queue view playing for an hour
  and it never stops on its own (unless a scene fails); with Shuffle on, no scene repeats until
  every scene of the result has played.
- **SC-005**: Every 010 budget still passes (selection, Play next, requests, list scrolling).

## Assumptions

- **This amends 010 before it ships**: 010's Play button and click-to-select give way to
  click-to-play; 010's Play next switch becomes Autoplay; the panel still shows the scene's
  details below the player.
- **Familiar defaults**: Autoplay on (the queue view is for watching), Repeat Off and Shuffle off
  (as music and video players start).
- **Autoplay follows the app's one player** (007): a queue starting to play stops whatever else
  was playing only when the user chose Queue or a scene; switching tabs never takes playback from
  another tab (FR-005).
- **Shuffle order** lives with the tab while the app runs; a restored tab starts a new round.
- **Design constraint (owner's request)**: the queue's playback rules (what plays next, and when)
  are one state machine expressed as pure functions: given the queue's settings, its position,
  and an event (a scene ended, Next, Previous, a click, new results, a failure), it returns the
  next state and what to do, without side effects, so every combination of Autoplay, Shuffle,
  and Repeat is tested without a player or a server.
- **Out of scope**: a curated queue (adding, removing, reordering scenes), Spotify-style "keep
  playing similar scenes" after the queue ends, and the gallery queue view (galleries don't exist
  yet).
