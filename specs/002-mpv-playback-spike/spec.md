# Feature Specification: mpv Playback Spike

**Feature Branch**: `002-mpv-playback-spike`

**Created**: 2026-09-25

**Status**: Draft

**Input**: User description: "mpv-playback-spike look into mpv as GPU shader first and foremost."
(Roadmap Phase 0: prove that a Stash scene can play in mpv inside the viewer, from Stash's direct
stream, without server-side transcoding.)

## Overview

This is a **spike**: a time-boxed, working proof that answers one question before Phase 1
browsing is built on top of it:

> Can the viewer play a Stash scene with mpv, drawn by the GPU **inside the viewer's own
> window**, with the viewer's controls on top of the video, from Stash's original file (no
> transcoding)?

The primary approach to prove is having mpv render video frames straight into the viewer's own
GPU drawing surface ("mpv as a GPU renderer inside the app"). Other approaches, such as handing
mpv a native child window or running mpv as a separate player window, are fallbacks. They are
evaluated only if the primary approach fails a success criterion. The spike ends with a working
player screen and a written decision on the approach Phase 1 builds on.

The spike is about **rendering only**: getting mpv's output onto the GPU inside the app.
mpv's user shaders (GPU video filters) aren't part of it. The app will later add playback
features Stash's web player doesn't have, such as choosing embedded subtitle and audio tracks.
Those aren't built here, but the chosen approach must not rule them out.

## Clarifications

### Session 2026-09-25

- Q: Does "mpv as GPU shader" also cover mpv's user shaders (GPU filters such as upscaling)?
  → A: No, rendering only. Embedded subtitles, audio tracks, and similar features Stash doesn't
  offer are later work but in scope for the app, so the chosen approach must keep them possible.
- Q: Which playback controls are in scope? → A: All playback controls are part of this feature.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Play a scene inside the viewer (Priority: P1)

While connected to a Stash server, the user opens a scene (for the spike, by entering its scene
ID or picking it from a short list of recently added scenes) and it starts playing inside the
viewer's window. The video is drawn by the GPU inside the app, not in a separate window, and it
plays the original file directly from Stash, with no server-side transcoding.

**Why this priority**: This is the question the spike exists to answer. Without in-window GPU
playback, Principle V (native playback, no transcoding) and the Jellyfin-style experience can't
be delivered, and Phase 1 would need a different design.

**Independent Test**: Connect to the user's library, open a scene by ID, and confirm it plays
inside the viewer window within the target time, while Stash reports no transcoding activity
for it.

**Acceptance Scenarios**:

1. **Given** a connected server and a scene ID, **When** the user opens that scene, **Then**
   video and audio start playing inside the viewer window, not in a separate window.
2. **Given** a server that requires an API key, **When** the user opens a scene, **Then**
   playback works using the profile's saved key, with no extra prompt.
3. **Given** a scene is playing, **When** the user checks what Stash is doing, **Then** Stash
   is serving the original file and is not transcoding it.
4. **Given** a scene whose file can't be played (missing file, unreachable stream, or a format
   mpv can't decode), **When** the user opens it, **Then** the viewer shows a plain-language
   error instead of a blank or frozen screen, and stays usable.
5. **Given** the recent-scenes list, **When** the user opens the player screen, **Then** it
   shows up to 20 recently added scenes (title and duration) to pick from.

---

### User Story 2 - Control playback with the viewer's own controls (Priority: P2)

While a scene plays, the viewer's own controls sit on top of the video: play/pause, a seek bar
with current time and duration (click, drag, or hover to see the time under the pointer), skip
forward/back, playback speed, frame-by-frame stepping while paused, volume and mute,
fullscreen, and stop/close. Keyboard shortcuts cover all of them. The controls hide after a few
seconds without mouse movement and reappear on movement.

**Why this priority**: Drawing the viewer's interface on top of GPU-rendered video is the
hardest part of in-window playback, and the part most likely to force a different approach.
Proving it here de-risks every later playback feature.

**Independent Test**: Play a scene and use every control with the mouse and the keyboard.
Confirm the controls draw correctly over the moving video and respond within the target time.

**Acceptance Scenarios**:

1. **Given** a playing scene, **When** the user presses space or clicks play/pause, **Then**
   playback pauses and resumes.
2. **Given** a playing scene, **When** the user drags the seek bar or presses the arrow keys,
   **Then** playback jumps to the new position (arrow keys: ±10 s) and the time display
   updates.
3. **Given** a playing scene, **When** the user changes the volume or presses `m`, **Then** the
   volume changes or mutes, and the control shows the new state.
4. **Given** a playing scene, **When** the user presses `f` or the fullscreen button, **Then**
   the video fills the screen with the controls still on top; Escape or `f` leaves fullscreen.
5. **Given** no mouse movement for 3 seconds during playback, **When** the time passes,
   **Then** the controls and cursor hide; moving the mouse shows them again.
6. **Given** the controls are visible over moving video, **When** the user watches, **Then**
   the controls draw cleanly on top (no flicker, tearing, or black boxes around them).
7. **Given** a playing scene, **When** the user picks a playback speed (0.25× to 4×) or presses
   `[` / `]`, **Then** playback speed changes with audio pitch preserved, and the current speed
   is shown; `\` returns to 1×.
8. **Given** a paused scene, **When** the user presses `.` or `,`, **Then** the video steps one
   frame forward or back.
9. **Given** the pointer is over the seek bar, **When** the user hovers, **Then** the time at
   that point is shown.
10. **Given** the scene reaches its end, **When** playback finishes, **Then** the player shows
    the ended state with a replay option instead of a black screen.

---

### User Story 3 - Decide the playback approach with evidence (Priority: P3)

The spike ends with a short written decision record: which approach Phase 1 uses, why, and the
measurements behind it. It covers the primary approach (GPU rendering inside the app) and, if
that approach failed any success criterion, the fallback approaches that were tried, each with
the same measurements. It also lists known limitations and platform gaps.

**Why this priority**: The decision is the spike's lasting output, but it's only meaningful
once playback (US1) and overlay controls (US2) have been tried for real.

**Independent Test**: Read the decision record and confirm it names one approach, gives the
measured values for every success criterion, and states known limitations.

**Acceptance Scenarios**:

1. **Given** the spike is complete, **When** the user reads the decision record, **Then** it
   names the chosen approach and the reason in plain language.
2. **Given** the decision record, **When** the user looks for evidence, **Then** it lists the
   measured values for SC-001 to SC-006 for each approach that was tried.
3. **Given** the decision record, **When** the user looks for risks, **Then** it lists known
   limitations (for example platforms or display systems that weren't tested) and what Phase 1
   must do about them.
4. **Given** the decision record, **When** the user checks future playback features, **Then**
   it confirms the chosen approach can read a file's embedded subtitle and audio tracks (shown
   for at least one test file that has several), so choosing them later is possible.

---

### Edge Cases

- **Display system**: the user's desktop is a Wayland session (KDE). The primary approach must
  work there. Approaches that only work on X11 count as failures for this machine and are
  recorded as such.
- **Window resize and fullscreen**: the video keeps its aspect ratio and fills the video area
  without stretching or leaving stale frames while the window is resized.
- **Leaving the player**: closing the player (or switching servers) stops playback and releases
  the video and audio immediately; nothing keeps playing in the background.
- **Connection lost during playback**: if the server goes offline, playback stops with a clear
  message once buffered video runs out; the viewer stays usable, and the connection indicator
  from feature 001 shows Offline.
- **Large and high-bitrate files**: 4K and high-bitrate scenes are included in the test set;
  seeking in them must still meet the target.
- **Formats**: the test set includes the codecs and containers common in the user's library
  (for example H.264, HEVC, AV1; MP4, MKV, WMV).
- **Scene with several files**: the primary file is played; picking other files is out of
  scope for the spike.
- **Hardware decoding unavailable**: if GPU decoding isn't available for a codec, playback
  falls back to software decoding and still plays; the decision record notes it.

## Requirements *(mandatory)*

### Functional Requirements

**Playback**

- **FR-001**: Users MUST be able to open a scene for playback by entering its scene ID, or by
  picking it from a list of up to 20 recently added scenes, while connected to a server.
- **FR-002**: Video MUST be drawn by the GPU inside the viewer's own window, as part of the
  viewer's interface, not in a separate top-level window. (If the primary approach fails, the
  decision record says so and names the fallback used.)
- **FR-003**: Playback MUST use Stash's original file stream and MUST NOT request any
  server-side transcoded stream (constitution Principle V).
- **FR-004**: Playback MUST authenticate with the active profile's saved API key when the
  server requires one, with no additional prompt.
- **FR-005**: Playback MUST use hardware (GPU) decoding when available and fall back to software
  decoding otherwise.
- **FR-006**: When a scene can't be played, the viewer MUST show a plain-language error and
  remain usable.
- **FR-007**: Closing the player, switching servers, or disconnecting MUST stop playback and
  release audio and video immediately.

**Controls**

- **FR-008**: The player MUST provide, drawn by the viewer on top of the video: play/pause; a
  seek bar with current time, duration, and the time under the pointer on hover; skip ±10 s;
  playback speed from 0.25× to 4× with pitch preserved; frame-by-frame stepping forward and back
  while paused; volume; mute; fullscreen; and stop/close.
- **FR-009**: The player MUST support the keyboard shortcuts: space (play/pause), left/right
  arrows (±10 s), up/down arrows (volume), `[` / `]` (slower/faster), `\` (normal speed),
  `.` / `,` (next/previous frame while paused), `f` (fullscreen), `m` (mute), Escape (leave
  fullscreen, then close the player).
- **FR-010**: Controls and the cursor MUST hide after 3 seconds without mouse movement during
  playback, and reappear on mouse movement or a key press.
- **FR-011**: The video MUST keep its aspect ratio and fill the video area during window resizes
  and in fullscreen.

**Spike output**

- **FR-012**: The spike MUST produce a decision record naming the playback approach for Phase 1,
  with the measured values for SC-001 to SC-006 for each approach tried, and the known
  limitations.
- **FR-013**: The spike MUST NOT write anything to Stash (no play counts, resume points, or
  other changes). Playback state syncing is deferred to Phase 1.
- **FR-014**: The chosen approach MUST keep access to the media's embedded subtitle and audio
  tracks. The spike MUST show it can list a file's embedded tracks (no selection interface is
  required yet), so later features Stash's web player doesn't offer stay possible.
- **FR-015**: When playback reaches the end, the player MUST show an ended state with a replay
  option.

### Key Entities

- **Playable Scene**: a Stash scene chosen for playback. Attributes: scene ID, title, duration,
  primary file (codec, resolution, container, size), and the address of its original-file
  stream.
- **Player Session**: one playback of one scene. Attributes: state (loading, playing, paused,
  ended, error), position, duration, speed, volume, muted, fullscreen, whether hardware decoding
  is in use, and the embedded tracks found (audio and subtitle).
- **Decision Record**: the spike's written outcome: approach chosen, alternatives tried,
  measurements per success criterion, limitations, follow-ups for Phase 1.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A scene starts playing (first frame visible) within 1.5 seconds of the user
  opening it, for scenes on a LAN server, in at least 9 of 10 attempts across the test set.
- **SC-002**: Seeking to any point shows the new frame within 1 second, including in 4K scenes,
  in at least 9 of 10 attempts.
- **SC-003**: 1080p playback drops no more than 1 frame per minute on the development machine
  during a 5-minute run, with the controls shown and hidden during it.
- **SC-004**: Controls respond to input (visible change) within 50 milliseconds, matching the
  constitution's input budget, while video plays underneath.
- **SC-005**: During playback, the Stash server does 0 transcoding work for the scene (the
  server's resource use stays at file-serving levels).
- **SC-006**: 100% of the scenes in the test set (at least 10 scenes, covering each common
  codec and container in the user's library, including at least one 4K scene) either play, or
  fail with a plain-language message. None leaves a blank or frozen screen.

## Assumptions

- **Test content**: playback tests use scenes from the user's own library (`localhost:9999`),
  read-only. The disposable test instance currently has no media, so it's used only for
  checking authenticated stream access (API key) once it has a playable file.
- **Scene picking is temporary**: entering a scene ID and a short recent-scenes list stand in for
  Phase 1's real browsing. They may be replaced or removed later.
- **Platform**: Linux on the developer's machine (Wayland, KDE) is the only platform that must
  pass. Windows and macOS are recorded as untested in the decision record.
- **No transcoding fallback in the spike**: the constitution allows an explicit, user-visible
  transcoding fallback, but the spike doesn't build it; unplayable files show an error instead.
- **Out of scope for this spike**: mpv user shaders (GPU filters), subtitle display and
  selection, audio-track selection, markers as chapters, seek-bar thumbnail previews (need
  Stash's sprites), playlists, resume/continue watching, play-count syncing, and
  picture-in-picture. Embedded subtitles and audio tracks are in scope for the app and come
  later; FR-014 keeps them possible.
- **Dependency**: builds on feature 001 (connect-to-stash): the active profile, its API key, its
  strict-TLS setting, and the connection indicator.
- **Constitution**: Principle V (mpv, direct stream, no transcoding) and Principle VI (never
  block the interface; 50 ms input budget) apply directly. The spike's write-free rule (FR-013)
  keeps Principle I intact while playback syncing is deferred.
