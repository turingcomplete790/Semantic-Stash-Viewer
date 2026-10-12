# Feature Specification: Queue Autoplay and Loop

**Feature Branch**: `010-queue-view` (builds on 010, not yet merged)

**Created**: 2026-10-11

**Status**: Draft

**Input**: User description: "queue view should autoplay and autoloop. It should switch to any
video you click on in the queue and play it."

## Overview

The queue view (010) lists the Scenes tab's results beside a panel with the selected scene. As
built, choosing a scene only shows it; the user then presses Play, and Play next stops after the
last scene. This feature makes the queue a player first: the queue view starts playing as soon as
it opens, choosing any scene in the list switches playback to it at once, and when the queue runs
out it loops, so the view keeps playing until the user stops it.

## Clarifications

### Session 2026-10-11

- Q: What should "autoloop" loop? → A: The whole queue: after the last scene of the last page,
  playback starts again from the first scene of page 1.
- Q: Does moving the selection with the keyboard also switch playback? → A: Yes, once it rests:
  rows only passed over are highlighted, and the scene the selection stops on starts playing
  (the same 250 ms rest as 010's details).

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
   starts playing in the panel.
3. **Given** a scene playing, **When** the user clicks the scene that's already playing, **Then**
   playback carries on uninterrupted (it does not restart).
4. **Given** the user moves through the list with the keyboard, **When** the selection moves,
   **Then** rows passed over are only highlighted, and the scene the selection rests on (250 ms)
   starts playing, replacing the playing scene; holding ↓ through the list starts nothing until
   the selection stops.

---

### User Story 2 - The queue view starts playing on its own (Priority: P1)

The user switches a Scenes tab to Queue (or returns to a tab already in Queue, or the app restores
one). The selected scene starts playing in the panel without the user pressing Play.

**Why this priority**: "Autoplay" is the other half of the request: the queue view is for
watching, so it shouldn't wait for a click.

**Independent Test**: Switch a Scenes tab from Grid to Queue: the first scene (or the remembered
one) starts playing in the panel within the playback budget, with no further input.

**Acceptance Scenarios**:

1. **Given** a Scenes tab in Grid, List, or Wall, **When** the user chooses Queue, **Then** the
   selected scene starts playing in the panel.
2. **Given** a Scenes tab in Queue, **When** the search, a filter, the sort, or the page changes,
   **Then** the new list's first scene starts playing.
3. **Given** the app restarts with a tab in Queue, **When** that tab is first shown, **Then** it
   shows the remembered scene but does not start playing until the user switches to the tab (a
   tab in the background never plays on its own).
4. **Given** something is already playing from another tab, **When** the user switches a tab to
   Queue, **Then** the queue's scene takes over playback (there is one player for the app).

---

### User Story 3 - The queue loops (Priority: P1)

When the queue reaches its end, playback carries on instead of stopping: after the last scene of
the last page, the queue starts again from the first scene of page 1.

**Why this priority**: Explicitly requested; with autoplay it makes the queue view a continuous
player.

**Independent Test**: Play the last scene of the last page and let it end: page 1 loads and its
first scene plays with no input, and the list keeps the playing scene in view and marked.

**Acceptance Scenarios**:

1. **Given** the last scene of the queue is playing and looping is on, **When** it ends, **Then**
   page 1 shows (loaded, or at once when it's cached) and its first scene plays within the
   playback budget; on a one-page queue, the first scene of the same page plays.
2. **Given** a queue with a single scene and looping on, **When** it ends, **Then** it plays
   again from the start.
3. **Given** the user turns looping off, **When** the last scene ends, **Then** playback stops
   with Replay, as in 010.

---

### Edge Cases

- **A scene that can't play**: autoplay, a click, or the loop never skips past a failed scene on
  their own; the panel shows the error and the transcode offer (008), and the queue waits for the
  user (as 010 FR-012).
- **Holding ↓ through the list**: rows highlight as they pass and nothing plays until the
  selection stops; the scene that was playing keeps playing meanwhile.
- **Every scene fails**: looping never cycles endlessly through failing scenes, since a failure
  stops the queue (above).
- **A scene deleted on the server**: skipped, as in 010; a loop of only deleted scenes stops.
- **An empty result**: nothing plays; the empty state shows as in 010.
- **Switching tabs**: playback continues with the now-playing bar (007); coming back to the queue
  tab doesn't restart or switch playback.
- **Leaving Queue for Grid, List, or Wall**: playback continues with the now-playing bar, as in
  010; switching back to Queue doesn't restart the playing scene if it's still in the list.
- **Play history**: each scene played, by autoplay, a click, or the loop, records its play history
  as any playback does (008); a looping scene starts a new viewing each time it starts again.
- **Muted autoplay**: playback starts at the player's current volume and mute setting; autoplay
  doesn't change them.

## Requirements *(mandatory)*

### Functional Requirements

**Click to play (US1)**

- **FR-001**: Clicking a scene in the queue's list MUST play it in the panel at once, replacing
  any playing scene, and make it the selection; this replaces 010's click-to-select (010 FR-004).
- **FR-002**: Clicking the scene that's already playing MUST NOT restart it.
- **FR-003**: Moving the selection with the keyboard MUST switch playback to the scene the
  selection rests on (250 ms, 010's rest), and MUST NOT start playback for rows only passed over;
  Enter plays the selection at once.

**Autoplay (US2)**

- **FR-004**: Choosing Queue, and any change of results while in Queue (search, filter, sort, page
  size, page), MUST start playing the selected scene without further input.
- **FR-005**: A tab in Queue MUST start playing only when it's the tab on screen; a restored or
  background tab waits until it's shown, and returning to a tab whose scene is already playing
  MUST NOT restart it.

**Loop (US3)**

- **FR-006**: The queue view MUST offer looping, on by default and saved with the tab; when on and
  the last scene of the last page ends, page 1 shows and its first scene plays; Previous before
  the first scene of page 1 stays where it is (looping only runs forward).
- **FR-007**: Looping MUST NOT continue after a playback failure; a failed scene stops the queue as
  in 010 FR-012.

**Kept from 010**

- **FR-008**: Play next, Next and Previous, the playing mark, the keyboard keys, and the request
  budgets of 010 MUST keep working; Play next (on) is what moves from one scene to the next, and
  looping decides what happens at the queue's end.

### Key Entities

- **Loop setting**: whether the queue continues past its end; saved with the tab, on by default.
- **Playing scene / selection** (010): with click-to-play they are the same scene, except for the
  moment the keyboard moves the selection, before it rests.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A clicked scene starts playing in under 1.5 seconds (the playback open budget).
- **SC-002**: Switching a tab to Queue starts playback in under 1.5 seconds with no further
  input.
- **SC-003**: With looping on, the queue's last scene ending leads to the next scene playing in
  under 1.5 seconds, including a return to page 1.
- **SC-004**: A user can leave the queue view playing for an hour and it never stops on its own
  (unless a scene fails).
- **SC-005**: Every 010 budget still passes (selection, Play next, requests, list scrolling).

## Assumptions

- **This amends 010 before it ships**: 010's Play button and click-to-select give way to
  click-to-play; the panel still shows the scene's details below the player.
- **Looping is on by default** in the queue view, as Play next is, and is a per-tab switch beside
  Play next so a user can turn it off.
- **Autoplay follows the app's one player** (007): a queue starting to play stops whatever else
  was playing, as starting any scene does.
- **Background tabs never start playback** (FR-005), so restoring a session with several queue
  tabs doesn't fight over the player.
- **Out of scope**: shuffle (Stash's random sort already gives a shuffled queue), a curated queue,
  and loop for the gallery queue view (galleries don't exist yet).
