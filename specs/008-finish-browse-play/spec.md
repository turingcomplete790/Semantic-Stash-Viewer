# Feature Specification: Finish Browse and Play

**Feature Branch**: `008-finish-browse-play`

**Created**: 2026-10-05

**Status**: Draft

**Input**: User description: "Finish Browse and Play."

## Overview

The native app (007) can browse the whole scene library a page at a time, open a scene, and play
it inside the window. What's missing is everything that makes it a library client people use
every day instead of the Stash web UI:

- **Find**: search, every web UI filter, and the saved filters already stored in Stash.
- **The full scene page**: groups, galleries, markers, every file, rating, organized flag, and
  performers, tags, and studio that lead somewhere.
- **Play like the web UI remembers**: resume where you left off, markers as chapters, and play
  history (play count, time watched, resume point) saved back to Stash.
- **Previews**: moving previews on cards, a wall mode, and thumbnails on the seek bar.
- **A fallback** for the rare file the player can't decode: an explicit, visible, logged
  transcoded stream.

This finishes the "Browse & play" part of the roadmap for scenes: the stories 005 specified and
007 didn't build (005 delivered its first story, the paged grid, which 007 ported natively). It
is read-only except for play history, which the constitution requires to sync back to Stash
(Principles I and V), and the tag colour rules set in Settings (one UI-configuration key). All
editing arrives in Phase 2.

### Web UI capabilities covered (Principle VIII)

| Stash web UI capability | Where |
|---|---|
| Search box | US1 |
| Filter criteria for scenes, with their modifiers | US1: every criterion (FR-002) |
| Saved filters: loading and applying; the default filter | US1 (saving and deleting: Phase 2) |
| Scene page: details, performers, studio, tags, groups, galleries, markers, file info, rating, organized | US2 |
| Playback: resume point, play count, play duration, markers on the timeline | US3 |
| "Always start from beginning" / resume behaviour setting | US3 |
| Scene previews on hover, wall mode, scrubber sprite thumbnails | US4 (animated image previews; the UI plays no video, Principle V) |
| Transcoded playback when a file can't be played directly | US5 (explicit fallback only) |
| (Beyond the web UI) Tag colours by tag flag, set in Settings | US2 (constitution Principle XI) |
| Tagger, bulk edit, scene edit, O-counter, rating edits, marker creation | Not here: Phase 2 (editing) and Phase 3 (tagger) |

## Clarifications

### Carried over from 005 (2026-09-29 to 2026-10-01), still in force

- Play history is saved to Stash **on by default**, with a viewer-only switch in Settings →
  Playback, independent of the web UI's "track activity" setting.
- **Every** scene filter criterion the web UI has (about 50, custom fields included), not just
  the common ones. The semantic-tagging plugin's scene-filter building is prior art.
- The UI plays no media itself (Principle V): hover and wall previews use Stash's **animated
  image previews** shown as images, with the still thumbnail as the fallback.
- Play history applies to scenes only; switching the player from a scene to anything else counts
  as the end of that viewing.
- Paging, page sizes, keys, and per-tab state are as built in 005/007 and unchanged here.

### Session 2026-10-05

Focus: lean data fetching (the Stash web UI's scene page is a mess of N+1 queries; ask only for
what's shown).

- Q: How many requests to Stash may opening a scene page make? → A: Instant from the card's cached
  data, then one scoped request for what the page shows by default; bulky extras (every file's
  details, galleries, groups) load only when opened, one request each; never a request per
  related item.
- Q: What should the scene page fetch for each performer, tag, studio and marker it lists? → A:
  Only what each chip or row shows, plus small performer images on the chips (id + name for tags,
  studio, and groups; id, time, end, title, and primary tag name for markers); richer details
  (for example a performer's age and scene count, or a tag's description) load only when the user
  hovers or focuses that chip, one request for that item, cached.
- Q: When adding a filter for performers, tags or studios, how should the list of choices be
  found? → A: As in the Stash web UI: the full drop-down list, narrowing as letters are typed. The
  lists are cached per server (names and ids only, refreshed quietly) so the large lists don't
  load the Stash instance on every use. The filter menu also carries placeholders for the
  semantic-tagging plugin's filters, which are first-class in this UI.
- Q: While a scene plays, how often should play history be written to Stash? → A: When the viewing
  ends (however it ends), plus a checkpoint once a minute while actually playing (nothing while
  paused).
- Q: In wall mode, how many animated previews should load and play at once? → A: None by
  default: the wall shows stills, and a scene animates only while hovered or focused, as in the
  grid (the user has seen browsers crash with more than about 10 animating at once).
- Q: Should the ordinary tag lists (the Tags filter picker and the tag chips on scene pages) leave
  out semantic tags? → A: No. Ordinary tags stay everywhere the web UI shows them, because the
  tags on a scene (often a whole set from StashDB) are what the semantic-tagging plugin's
  recommendations read: a tag that is a semantic tag, or a common variant of one, is recommended
  for applying to the scene's performers. The viewer finds semantic tags by their per-type flag
  custom fields (`plugtag`, `isTrait`, `isRole`, `isAction`, `isLabel`, `isMiseEnScene`,
  `isVariation`; common variants by `variantOfTagId`), which are 0/1 values because Stash turns
  true/false into 0/1 unpredictably. The tag list is fetched as two separate cached lists that
  together make the full list: the semantic vocabulary (flag set, one request) and ordinary tags
  (every flag null, filtered by Stash), so neither list is ever scanned for the other.
- Q: How long must the pointer rest on a card before its animated preview starts? → A: 200 ms.
- Q: How long does the "Start from the beginning" control stay visible after a resumed start?
  → A: 8 seconds.

### Session 2026-10-06

Focus: tag colours (replacing the per-tag colour field and derived colours of the previous
session).

- Q: How are tags coloured? → A: Not per tag. A tag colour manager in Settings holds colour
  rules, each a tag custom field flag and a colour (for example `isRole` → purple), with default
  colours for the semantic types. A tag takes the colour of the first rule whose flag it has.
  Nothing is written onto tags: storing a colour on each tag would mean scanning tags to find
  them, while a flag rule finds exactly its tags with one Stash-side "flag is not null" filter
  (the crucial optimization).
- Q: Where should the tag colour manager save its colours? → A: In Stash's UI configuration under
  the viewer's own key (`semanticStashViewer.tagColours`), read with the UI-settings request the
  viewer already makes and written only when the user changes a colour; shared across machines
  and readable by the plugin.

### Session 2026-10-07

Focus: performer images on the scene page (20 px icons were too small to see).

- Q: How large should performer images on the scene page's chips be? → A: A 40 px round icon,
  decoded at its drawn size times the display scale (for example 80 px on a 2× display) from the
  480 px image already in the image cache; no extra request or download (measured: about 0.6 ms
  per performer, off the UI thread).
- Q: Should the performer details card show the performer's picture? → A: Yes, a portrait about
  240 px wide from the same cached image (no extra request or download; about 1 ms to decode, off
  the UI thread); none when the performer has no image.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Find scenes: search, filters, saved filters (Priority: P1)

A user types a few words to search, narrows the list with the same filters as the Stash web UI
(performers, tags, studio, rating, resolution, duration, date, organized, and so on), or applies
one of the saved filters they already made in the web UI.

**Why this priority**: a large library is only useful if you can find things. Saved filters let
people bring their existing web UI habits with them on day one. Everything else in this feature
works without it, but it's what users will reach for first.

**Independent Test**: search for a known title, add a performer filter and a resolution filter,
check the count and results match the web UI for the same criteria, then apply a saved filter
made in the web UI and check it gives the same scenes.

**Acceptance Scenarios**:

1. **Given** Scenes, **When** the user types in the search box, **Then** results update shortly
   after they stop typing, matching the web UI's search for the same text, and the view goes back
   to page 1.
2. **Given** Scenes, **When** the user adds filter criteria, **Then** each active criterion shows
   as a removable chip, the total count updates, and results match the web UI for the same
   criteria.
3. **Given** saved scene filters exist in Stash, **When** the user opens the saved filters menu,
   **Then** they're listed by name, and choosing one applies its search, criteria, sort, and
   display mode.
4. **Given** the server has a default scene filter, **When** the user opens Scenes in a new tab,
   **Then** that filter is applied.
5. **Given** a performer, tag, or studio shown on a scene page, **When** the user chooses it,
   **Then** Scenes opens filtered to that performer, tag, or studio (new tab with Ctrl+click or
   middle-click).
6. **Given** filters that match nothing, **When** results load, **Then** the view says no scenes
   match and offers to clear the filters.
7. **Given** active search and filters in a tab, **When** the user switches tabs and back, or
   restarts the app, **Then** the tab's search, filters, sort, page, page size, and scroll
   position come back exactly.

---

### User Story 2 - The full scene page (Priority: P2)

A user opens a scene and sees everything about it before (or while) playing: title, date,
details, studio, performers, tags, groups, galleries, markers, rating, play count, organized
flag, and every file's information (resolution, codec, size, duration, frame rate, bit rate,
path).

**Why this priority**: people decide what to watch from the details, and the scene page is where
playback starts. 007's scene view covers the basics; this completes it and makes its people,
tags, and studio lead somewhere.

**Independent Test**: open scenes with many performers and tags, with markers, with several files,
and with almost no metadata; check every field shown matches the web UI's scene page.

**Acceptance Scenarios**:

1. **Given** a scene, **When** its page opens, **Then** the title, cover, and Play appear within
   the navigation budget; the remaining details fill in without moving what's already shown.
2. **Given** a scene with performers, tags, studio, groups, and galleries, **When** the page
   shows, **Then** each appears as a chip or small card; performers, tags, and studios open
   filtered Scenes (US1), and groups and galleries are shown, opening once their own views exist.
3. **Given** a scene with markers, **When** the page shows, **Then** the markers are listed with
   their times, titles, and tags, and choosing one starts playback at that point.
4. **Given** a scene with more than one file, **When** the page shows, **Then** every file is
   listed with its details, and the primary file is marked as the one that plays.
5. **Given** a scene with no title, **When** its page or card shows, **Then** it's named by its
   file name, as in the web UI.
6. **Given** Settings → Tag colours, **When** the user changes the Role colour or adds a rule for
   another tag flag, **Then** every matching tag chip shows the new colour at once, and the web
   UI's configuration (and another viewer, after its refresh) holds the same rules.

---

### User Story 3 - Play with resume, chapters, and play history (Priority: P3)

A user plays a scene. If they watched part of it before, it picks up where they left off. Markers
appear as chapters on the seek bar and can be jumped between. When they stop, their resume
point, play count, and time watched are saved to Stash, so the web UI and other devices see the
same history.

**Why this priority**: resume and play history are what make a library client feel like a real
player, and the constitution requires the sync. Playback itself already works (006, 007); this
adds what depends on Stash's data.

**Independent Test** (test instance only): play a scene for a minute and stop; check in the Stash
web UI that the resume point, play duration, and (if the threshold was reached) play count
changed. Reopen it and check it resumes there. Jump between markers with the keyboard.

**Acceptance Scenarios**:

1. **Given** a scene with a saved resume point and "Resume where you left off" on, **When** the
   user presses Play, **Then** playback starts at the resume point, with a visible way to start
   over from the beginning.
2. **Given** that setting off, **When** the user presses Play, **Then** playback starts at the
   beginning.
3. **Given** a scene with markers, **When** it plays, **Then** the markers show on the seek bar
   with their titles, and next/previous chapter keys jump between them.
4. **Given** playback, **When** the user stops, closes the player or its tab, switches scene,
   switches server, or quits, **Then** the resume point and the time watched are saved to Stash.
5. **Given** the user has watched more than the play-count threshold, **When** they stop,
   **Then** the play count increases by one (once per viewing).
6. **Given** the user watched to (nearly) the end, **When** playback ends, **Then** the resume
   point is cleared, so the next play starts at the beginning.
7. **Given** play history can't be saved (server unreachable), **When** the connection returns in
   the same session, **Then** the history is saved then; if it still can't be saved, the
   notification centre says so.
8. **Given** a fresh install, **When** the user plays scenes, **Then** play history is saved (on
   by default).
9. **Given** the user turned off "Save play history" in Settings → Playback, **When** they play
   scenes, **Then** nothing is written to Stash; the web UI's own setting is unaffected.

---

### User Story 4 - Previews, wall mode, and seek-bar thumbnails (Priority: P4)

Hovering (or focusing) a card shows its short animated preview, a wall mode packs many scenes
densely (each animating while hovered), and hovering the seek bar shows a thumbnail of that
moment.

**Why this priority**: it makes browsing and seeking much quicker, but everything works without
it.

**Independent Test**: hover cards and check previews start and stop promptly; open wall mode,
scroll, and hover scenes there; hover the seek bar of a playing scene and check the thumbnail
matches the moment.

**Acceptance Scenarios**:

1. **Given** a card whose scene has a generated animated preview, **When** the user hovers or
   focuses it for a moment, **Then** the animated preview replaces the still thumbnail in place,
   and the still returns when the pointer or focus leaves.
2. **Given** wall mode, **When** the user scrolls, **Then** scenes show as stills, densely packed,
   and only the hovered or focused one animates, so at most one preview plays at a time.
3. **Given** a playing scene with generated sprites, **When** the user hovers the seek bar,
   **Then** a thumbnail of that moment shows above it with the time.
4. **Given** a scene without generated previews or sprites, **When** the user hovers, **Then** the
   still thumbnail stays and nothing breaks.

---

### User Story 5 - A visible fallback when a file can't play (Priority: P5)

For the rare file the player can't decode, the viewer says so and offers to play it through
Stash's transcoded stream instead, clearly marked as a fallback.

**Why this priority**: every format tried so far plays directly (002, 006, 007: WMV, AVI, FLV,
VP9, AV1, HEVC, 4K), so this is rarely needed, but the constitution requires the fallback to
exist and be explicit.

**Independent Test** (test instance): point a scene at a file the player can't decode; check the
error offers the fallback, that choosing it plays the transcoded stream with a visible
"transcoded" marker, and that the log records it.

**Acceptance Scenarios**:

1. **Given** a scene the player can't decode, **When** playback fails, **Then** the error explains
   it and offers "Play transcoded".
2. **Given** the user chooses "Play transcoded", **When** it plays, **Then** the player shows it's
   a transcoded stream, and the fallback is logged with the scene and the reason.
3. **Given** a normal scene, **When** it plays, **Then** it never uses a transcoded stream.

---

### Edge Cases

- **A filter with no matches, or an empty library**: a clear empty state with a way back (clear
  filters), never a blank grid.
- **Colour rules**: a rule for a flag no tag has colours nothing (shown with a count of 0); a
  tag with a flag of 0 isn't coloured by that rule; when another machine or the plugin changes
  the rules, this viewer picks them up with its quiet refresh of the UI settings; saving while
  offline says the colours couldn't be saved and keeps the previous ones.
- **A server without the semantic-tagging plugin** (no tag carries a type flag): the semantic
  vocabulary is empty, ordinary tags are the whole list, and nothing is marked; the semantic
  filter placeholders still show.
- **A semantic tag with a flag of 0** (cleared by the plugin, or turned from `false` by Stash):
  0 is not null, so Stash returns it with the semantic vocabulary and leaves it out of the
  ordinary list; the viewer treats a vocabulary entry whose flag isn't set (FR-003c) as an
  ordinary tag and shows it in the ordinary group, so it never disappears from the picker.
- **A common variant whose canonical tag isn't semantic, or no longer exists**: it has no type,
  so it is shown as an ordinary tag (it arrives with the vocabulary, not the ordinary list).
- **Search typed quickly, then changed**: only the latest search's results ever show; an earlier,
  slower response never replaces a newer one.
- **Saved filters using criteria the viewer doesn't know** (saved by a newer Stash): applied as
  far as possible, with a notice naming what was left out, rather than silently showing different
  results.
- **The default filter changes in the web UI**: new tabs use the new default once the viewer's
  quiet refresh of the server's UI settings picks it up (on the cache's schedule, or at the next
  session); tabs already open
  keep what they show.
- **Scenes without titles, dates, studios, previews, or sprites**: the file name as title, missing
  fields left out rather than shown blank, and the still thumbnail (or placeholder) instead of a
  preview.
- **Very long titles, many performers, tags, or markers**: truncated or wrapped without breaking
  the layout; the full text on the scene page. A scene with 40 performers still makes one request
  for its page; their images load through the image cache.
- **Very large choice lists** (tens of thousands of tags or performers): the drop-down opens from
  the cache at once and narrows smoothly as letters are typed; the first fetch of a list happens
  once per server, not on every open.
- **Sweeping the pointer across many chips**: no details are fetched until the pointer rests on
  one; each item's details are fetched at most once per session unless the cache refreshes them.
- **Offline**: cached pages, scene pages, and saved filters still show; a search or filter that
  isn't cached says the server can't be reached, with the controls still working; play history is
  kept and saved when the connection returns (US3).
- **A scene deleted elsewhere while its page is open**: the page says it no longer exists.
- **Resume point near the end** (98% of the duration or more): treated as finished; playback starts from
  the beginning.
- **A resume point beyond the file's length** (the file was replaced): playback starts from the
  beginning.
- **Several files per scene**: the primary file plays; the others are listed.
- **The same scene playing and its page open in another tab**: one player; the other tab shows
  the now-playing bar (007).
- **Play history while another device plays the same scene**: the latest save wins, as in the web
  UI.
- **Quitting or switching server mid-playback**: the history is saved before the player stops; if
  the save can't finish in time, it's kept and saved at the next session for that server.
- **Previews in very large pages or wall mode**: nothing animates on its own; only the hovered or
  focused scene does (one at a time), so a 1000-scene page or a wall never loads previews in bulk.
- **Moving the pointer quickly across the wall**: a preview starts only after a short rest on a
  scene; scenes only passed over fetch nothing.
- **A transcoded stream that also fails**: the error says both failed, and nothing retries in a
  loop.

## Requirements *(mandatory)*

### Functional Requirements

**Finding (US1)**

- **FR-001**: Scenes MUST have a search box whose results match the web UI's scene search for the
  same text, updating shortly after typing stops and returning to page 1.
- **FR-002**: Users MUST be able to filter by **every** scene filter criterion the web UI offers
  in the minimum supported Stash version (about 50: text fields such as title, code, details,
  director, URL, path, and folder; file facts such as resolution, orientation, duration, frame
  rate, bit rate, codecs, file count, hashes, and duplicates; activity such as rating, organized,
  O-counter, play count, play duration, resume time, and last played; relationships such as
  performers, performer tags, age, and favourites, tags, tag count, studios, studio tags, groups,
  galleries, markers, and StashIDs; dates and timestamps; interactive and captions; "is missing";
  and custom fields), each with the modifiers the web UI offers for it (equals, includes,
  excludes, greater/less than, between, is null, and so on).
- **FR-003**: Active criteria MUST show as removable chips, with a way to clear them all, and the
  total count MUST reflect them.
- **FR-003a**: Choosing performers, tags, studios, groups, or galleries for a filter MUST work as
  in the Stash web UI: a drop-down of the full list that narrows as the user types (matching
  names and aliases). The lists MUST come from the local cache when present (ids, names, and
  aliases only; performer images through the image cache, for visible rows only), be fetched in
  full at most once per server until the cache refreshes them quietly, and never be shared
  across servers.
- **FR-003b**: The filter menu MUST include the semantic-tagging plugin's filters as a first-class
  group, shown as placeholders until Phase 4 brings them (Trait, Trait Category, Label, Role, Role
  Category, Pluggable, Action with variation, Mise-en-scène and Location, Timeline relations such
  as same time / before / after, Archetype, and Look): visible and described, not yet usable, and
  never silently ignored if a saved filter or restored tab carries one.
- **FR-003c**: The tag list MUST be fetched as two disjoint lists, each cached per server like the
  other choice lists: the **semantic vocabulary** (tags with any per-type flag set, plus common
  variants with `variantOfTagId`, in one request that asks Stash for "flag is not null" per type)
  and **ordinary tags** (tags with every type flag null, filtered by Stash, not by the viewer).
  The Tags picker MUST show both together (as the full list the web UI shows), with semantic tags
  grouped by type above ordinary ones. A flag counts as set when it is 1 (or `"1"`, `true`,
  `"true"`, which Stash may hand back); anything the viewer writes later MUST be 0 or 1.
- **FR-003d**: Tag chips MUST show every tag on the scene, ordinary and semantic. A semantic tag's
  chip MUST carry its type badge. Every tag that matches a colour rule (FR-003e) MUST show that
  rule's colour; a common variant takes its canonical tag's colour; a tag matching several rules
  takes the first in the manager's order. Chip text MUST be dark or light, whichever gives at
  least 4.5:1 contrast. The same colours apply wherever a tag appears in this feature (scene page
  chips, filter chips, the Tags picker). The scene's full tag list (ids) MUST stay in the scene
  details so Phase 4's recommendations need no extra request.
- **FR-003e**: Tag colours MUST come from **colour rules**, each a tag custom field flag and a
  `#rrggbb` colour ([contracts/tag-colour.md](contracts/tag-colour.md)), stored in Stash's UI
  configuration under `semanticStashViewer.tagColours` and read with the UI settings. The tags a
  rule colours MUST be found with one Stash-side "flag is not null" filter per rule, in the same
  request as the semantic vocabulary (the semantic flags already are), never by reading every
  tag. Colours MUST NOT be written onto tags. When the key is absent, the default rules apply: one
  per semantic type flag, each with its own colour from the RPG palette.
- **FR-003g**: A Settings → Tag colours page MUST list the colour rules in order with a swatch and
  the number of tags each colours; let the user change a rule's colour (the RPG and app palettes,
  or a hex value), add a rule for any tag custom field flag, remove an added rule, reorder rules,
  and reset to the defaults. A change MUST be saved to Stash as one write of the whole
  `semanticStashViewer.tagColours` value (leaving the rest of the UI configuration untouched) and
  show in every open view within one frame of the confirmed save; a save that fails MUST say so and keep the previous colours.
- **FR-003f**: The search box, filter menu, criterion editors, pickers (move, choose, include or
  exclude, close), saved filters menu, chips, and wall mode MUST be fully usable from the
  keyboard (constitution Principle VI), with their keys listed in keyboard help.
- **FR-004**: Users MUST be able to list and apply the saved scene filters stored in Stash;
  applying one sets its search, criteria, sort, and display mode. New tabs open with the server's
  default scene filter when there is one. Saving, renaming, and deleting filters are out of scope
  (Phase 2).
- **FR-005**: A saved filter the viewer can't apply exactly MUST say which parts were left out.
- **FR-006**: Performers, tags, and studios shown in the viewer MUST open Scenes filtered to them,
  in the same tab or (Ctrl+click, middle-click) a new one.
- **FR-007**: Each tab MUST keep its search, filters, sort, page, page size, display mode, and
  scroll position across tab switches, back/forward, and restarts (constitution Principle IX).
- **FR-008**: Only the latest search or filter's results MUST ever show in a tab.

**Scene page (US2)**

- **FR-009**: The scene page MUST show the title (or file name), cover, date, details, studio,
  performers, tags, groups, galleries, markers, rating, play count, organized flag, and every
  file's details (resolution, codec, size, duration, frame rate, bit rate, path), with the
  primary file marked.
- **FR-010**: The page MUST show the title, cover, and Play at once from what the card already
  had (no request), then fill in the rest without moving what's already shown, from **one**
  request asking only for the fields shown by default. Bulky extras (every file's details beyond
  the primary file, galleries, groups) MUST load only when the user opens them, one request each.
  The page MUST NEVER make a request per performer, tag, marker, or other related item.
- **FR-011**: Choosing a marker on the page MUST start playback at that marker.
- **FR-011a**: Related items on the page MUST carry only what their chip or row shows: performers
  as id, name, and a 40 px round image (decoded at that size times the display scale, so it's
  sharp on high-density displays); tags, studio, and groups as id and name; markers as id, time,
  end, title, and primary tag name. Performer images come through the image cache like scene
  thumbnails, never as data in the page's request.
- **FR-011b**: Hovering or focusing a performer, tag, or studio chip for a moment MUST show a small
  details card (for example a performer's age, gender, and scene count, or a tag's description and
  scene count), fetched for that one item when first needed and cached; moving across chips
  quickly MUST NOT start requests for chips only passed over. A performer's card MUST also show
  their portrait (about 240 px wide) when they have an image, taken from the image the chip
  already cached: no extra request and no second download.

**Playback (US3)**

- **FR-012**: A Settings → Playback page MUST offer "Resume where you left off" (on by default)
  and "Save play history" (on by default). When resuming, the player MUST show a "Start from the
  beginning" control for 8 seconds.
- **FR-013**: Markers MUST appear on the seek bar with their titles, with keys to jump to the next
  and previous marker (listed in the keymap, keyboard help, and Settings → Keyboard).
- **FR-014**: When playback of a scene ends for any reason (stop, close, scene change, tab close,
  server switch, quit, disconnect), the viewer MUST save the resume point and time watched to
  Stash; a resume point at 98% of the duration or more is cleared instead. While a scene is
  actually playing (not paused), the viewer MUST also save a checkpoint once a minute, so a crash
  loses at most a minute of history; each save is one small write.
- **FR-015**: The play count MUST increase by one per viewing once the watched share passes the
  play-count threshold (the server's setting where it has one).
- **FR-016**: Play history that can't be saved MUST be kept and retried when the connection
  returns (in the same session, or the next session for that server), and reported in the
  notification centre if it still fails.
- **FR-017**: "Save play history" belongs to the viewer (independent of the web UI's "track
  activity" setting); when it's off, nothing is written to Stash.
- **FR-018**: Apart from play history (FR-014 to FR-017) and saving the colour rules (FR-003g,
  one `semanticStashViewer.tagColours` UI setting), this feature MUST NOT write to Stash.

**Previews (US4)**

- **FR-019**: Cards MUST show the scene's **animated image preview** (Stash's generated animated
  preview image) after the pointer or focus rests on it for 200 ms, and return to the still when the pointer or focus
  leaves. Previews are images, never video: the UI toolkit plays no media (constitution
  Principle V).
- **FR-020**: A wall display mode MUST show scenes as a dense grid of stills; like the grid, a scene
  animates only while hovered or focused, so at most one preview plays at a time. The mode is kept
  per tab like grid and list.
- **FR-020a**: An animated preview MUST be fetched only when its scene is hovered or focused, and
  kept in the disk cache (within its size limit) so hovering it again costs no request.
- **FR-021**: The seek bar MUST show a thumbnail of the hovered moment when the scene has
  generated sprites.
- **FR-022**: Scenes without previews or sprites MUST fall back to the still thumbnail quietly.

**Fallback (US5)**

- **FR-023**: When the player can't decode a scene, the error MUST offer "Play transcoded"; the
  viewer MUST NOT use a transcoded stream otherwise.
- **FR-024**: Transcoded playback MUST be visibly marked in the player and logged with the scene
  and the reason.

**Performance and data use**

- **FR-025**: Scene lists MUST fetch only the fields cards need, a page at a time, with no request
  per card (constitution Principle IV); searches, filters, and saved filters MUST reuse the same
  paging.
- **FR-026**: Filtered pages, saved filters, and scene pages MUST use the local cache: shown at
  once when cached, refreshed quietly, and never shared across servers.

### Key Entities

- **Scene query**: what a tab shows: search text, filter criteria (each with its modifier and
  value), sort field and direction (plus the random seed), page, page size, display mode, and
  scroll position.
- **Filter criterion**: one condition from the web UI's list, or (as a placeholder until Phase 4)
  from the semantic-tagging plugin's: the field, the modifier, and the value (text, number,
  range, date, a set of performers, tags, or studios, and so on).
- **Choice list**: every performer, tag, studio, group, or gallery a filter can pick from (id,
  name, aliases), cached per server and refreshed quietly. Tags come as two disjoint lists:
  ordinary tags and the semantic vocabulary.
- **Semantic vocabulary**: every tag carrying a semantic type flag (id, name, type), plus common
  variants (id, name, canonical tag id), plus the ids of the tags each extra colour rule matches,
  cached per server; used to group the Tags picker and to mark and colour tag chips.
  Recommendations themselves are Phase 4.
- **Colour rule**: a tag custom field flag and a colour, in an ordered list stored in Stash's UI
  configuration (`semanticStashViewer.tagColours`); defaults cover the semantic types. Shared
  across machines and with the plugin.
- **Saved filter**: a named scene query stored in Stash, possibly the server's default; read-only
  here.
- **Scene details**: everything on the scene page by default: the card's fields plus details,
  performers (id, name, 40 px image), tags, studio, and groups (id, name), markers (id, time, end,
  title, primary tag name), rating, play count, resume point, organized flag, and the primary
  file. Every file's details and the galleries load separately, when opened.
- **Chip details**: the small card shown on hovering a performer, tag, or studio chip, fetched
  for that one item and cached.
- **Marker**: a titled point (and optional end) in a scene, with its primary tag.
- **Play history**: per scene, the resume point, total time watched, and play count, owned by
  Stash; the viewer reports each viewing, and keeps unsaved reports until they're saved.
- **Playback preferences**: "Resume where you left off" and "Save play history", per viewer
  install.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: For the same search, criteria, and sort, the viewer's count and first 100 results
  match the web UI's exactly, checked on 10 varied queries including 3 saved filters.
- **SC-002a**: Opening a performer, tag, or studio drop-down shows the full list in under 150 ms
  when cached, narrowing as each letter is typed within one frame budget, for lists of 20,000
  names; after the first fetch, opening it again makes no request to Stash until the cache
  refreshes.
- **SC-002**: A search or filter change shows its first page in under 1 second on a LAN server
  for a library of 30,000+ scenes, and under 150 ms when that page is cached.
- **SC-003**: A user who knows the web UI can find a named scene by search, and by a
  performer-plus-tag filter, each in under 30 seconds on first use.
- **SC-004**: A scene page shows its title, cover, and Play in under 150 ms from the grid, and
  opening it makes at most one request to Stash until the user opens an extra section (counted by
  the tests and the harness).
- **SC-005**: After watching part of a scene and stopping, the web UI shows the updated resume
  point and time watched within 5 seconds; reopening the scene resumes within 2 seconds of the
  saved point.
- **SC-006**: No play history is lost: across 20 viewings ended in every way (stop, close, tab
  close, scene change, server switch, quit, disconnect and reconnect), all 20 reach Stash; after
  a forced kill mid-viewing, Stash has the history to within one minute. A 10-minute viewing makes
  at most 11 play-history writes.
- **SC-007**: A card's animated preview starts within 300 ms of the 200 ms hover delay ending (on a LAN
  server; instantly when cached), at most one preview plays at a time, and wall mode scrolls a
  250-scene page within the same smoothness budget as the grid.
- **SC-008**: Leaving a filtered Scenes tab (opening a scene, switching tabs, or restarting) and
  coming back returns to the same search, filters, page, and scroll position in 10 of 10 tries.
- **SC-009**: Every interaction and playback budget measured in 007 still passes, and the
  performance harness covers the new rows (search, filter, scene page, preview start).

## Assumptions

- **Server and testing**: Stash v0.31.1 or newer. Anything that writes play history is tested
  only against the disposable test instance; the user's production library is used read-only (no
  play history writes during testing).
- **Previews and sprites** are whatever Stash has already generated; generating them is a Stash
  task (Phase 3). Hover and wall previews need Stash's animated preview images; scenes with only
  video previews show their still thumbnail (Principle V).
- **Transcoding** uses Stash's own transcoded stream; nothing new is needed on the server.
- **Play-count threshold and resume clearing** match the Stash web UI's player: the play counts once
  the share watched reaches the server's "minimum play percent" setting (0 when unset, so the first
  save counts it), and a resume point at 98% of the duration or more is cleared (plan research
  R6).
- **Filters** are Stash's scene filter criteria, applied by the server; the viewer doesn't
  re-implement filtering locally. The criteria list and each one's modifiers come from the Stash
  web UI source for the minimum supported version (the local Stash checkout), and from the
  semantic-tagging plugin's scene-filter building.
- **Other entity views** (performers, studios, tags, groups, galleries) are later Phase 1 specs;
  until then performers, tags, and studios open filtered Scenes, and groups and galleries are
  shown without opening anything.
- **Home page rows and global search** are the `home-and-global-search` spec; Home is unchanged
  here.
- **The two performance rows accepted at 007's sign-off** (grid scrolling at 144 Hz with 1000
  cards; page-change p95 on a cold change) are tracked separately; this feature mustn't make them
  worse.
- **Semantic-tagging filters** are placeholders here (FR-003b); making them work needs the
  plugin's data in the core (Phase 4).
- **Semantic tag flags** are the plugin's per-type tag custom fields (`plugtag`, `isTrait`,
  `isRole`, `isAction`, `isLabel`, `isMiseEnScene`, `isVariation`) and the common-variant link
  `variantOfTagId`; this feature reads them and writes none. Recommending semantic tags from a
  scene's ordinary tags is Phase 4. The design documents colour tag chips by type (roadmap
  Milestone 1D §4), the same model as the colour rules; the plugin can read
  `semanticStashViewer.tagColours` from the UI configuration to show the same colours
  (constitution Principle II).
- **Plugin parity** (constitution Principle II): this feature builds on these sections of the
  semantic-tagging plugin's design documents
  (`/home/bbulder/Projects/Stash-Plugins/semantic-tagging/`), read at commit `3c82c85` plus the
  working-tree edits present on 2026-10-06:
  - `semantic-query-engine-roadmap.md`, "Preserve the lessons from the existing plugin": semantic
    tag categories are marked with custom-field flags and found with narrowly scoped
    custom-field filters, never by loading every tag (FR-003c).
  - `semantic-query-engine-roadmap.md`, Milestone 1D §2 "Looks → Variation" and §4 "Variation
    joins every other tag type's shared tooling": the seven per-type flags (`plugtag`,
    `isAction`, `isRole`, `isTrait`, `isLabel`, `isMiseEnScene`, `isVariation`), Variation as a
    full peer type, and per-type chip colouring (FR-003c, FR-003d).
  - `semantic-query-engine-roadmap.md`, Milestone 1C "Tag Recommendations & Common Variants":
    `variantOfTagId` on the variant tag pointing at its canonical tag, and the recommendations
    Phase 4 builds on the scene's ordinary tags (FR-003c, FR-003d).
  - `semantic-query-engine-roadmap.md`, Milestones 2–4 (query engine, general search, matrix
    integration): the semantic filters shown here only as placeholders (FR-003b).

  Two details the documents don't state come from the plugin's current behaviour and are
  recorded as implementation notes, not design sources: a flag counts as set when it is `1`,
  `"1"`, `true`, or `"true"` (the documents and the owner say flags are 0/1), and the default
  colour-rule order follows the plugin's current type priority. Compatibility with the shared
  data contract is verified with a scrubbed fixture of the vocabulary query's result captured
  read-only from Production (ids, flags, and variant links kept; names replaced), parsed by the
  core's tests.
- **Out of scope**: all editing (scene metadata, ratings, O-counter, markers, organized flag, bulk
  edit), saving or deleting saved filters, the tagger, and generating previews or sprites.
