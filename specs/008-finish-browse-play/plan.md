# Implementation Plan: Finish Browse and Play

**Branch**: `008-finish-browse-play` | **Date**: 2026-10-05 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/008-finish-browse-play/spec.md`

## Summary

Add finding (search, every web UI filter, saved and default filters, semantic-tagging
placeholders), the full scene page, resume and play history, previews, and the transcoding
fallback to the native app, with **strictly budgeted GraphQL**: every user action has a written
budget of named operations with fixed selection sets (one request per page, one per scene page,
lazy extras, chip details only on rest, cached choice lists, at most one play-history write a
minute), enforced by selection snapshot tests, request-count tests, and live audit rows in the
performance harness ([research R1–R2](research.md), [contracts/query-budget.md](contracts/query-budget.md)).

**Amendment 2026-10-07 (performer picture versions)**: performer pictures are cached under the
version Stash puts in each performer's `image_path` (`?t=<updated_at>`), as scene thumbnails
already are, instead of a fixed `0`, so a picture changed in Stash replaces the old one; the
picker's performer list carries `image_path` too, which also stops it downloading pictures for
performers that have none ([research R13](research.md#r13-performer-picture-versions-fr-011c)).

## Technical Context

**Language/Version**: Rust 1.88 (workspace).

**Primary Dependencies**: iced 0.14 (UI), Cynic 3.14 (typed GraphQL against the pinned v0.31.1
schema), libmpv via `player`, the `image` crate already in the build (animated WebP frames),
turbojpeg (thumbnails), rusqlite (view cache). No new dependencies.

**Storage**: the per-server view cache (003; new keys `choices:<kind>`, `semantic:vocabulary`
(semantic tags, common variants, and colour-rule matches), `scene:files:<id>`,
`scene:galleries:<id>`, `chip:<kind>:<id>`, `ui:settings`, `saved-filters:scenes`, `preview:<id>`,
`sprite:<id>`); the session file (version 2); a play-history queue file; a settings file for
playback preferences.

**Testing**: `cargo test` (core tests on a mock server, Cynic selection snapshots, native
transition tests and `iced_test` flows), the opt-in live test against Testing, and perf-harness
audit rows.

**Target Platform**: Linux desktop (as 007).

**Project Type**: desktop app (workspace: `stash-core`, `player`, `native-ui`, `perf-harness`).

**Performance Goals**: spec SC-001–SC-009: filtered first page < 1 s on LAN (< 150 ms cached);
cached picker of 20,000 names opens < 150 ms and narrows within a frame; scene page title, cover,
and Play < 150 ms with at most one request; preview starts ≤ 300 ms after the hover delay; every
007 budget still passes.

**Constraints**: request budgets per action (contract); read-only except play history and the
colour rules setting; previews
are images (Principle V); API key only in headers; no per-item requests in any list or page.

**Scale/Scope**: libraries of 30,000–50,000 scenes and 20,000 performers or tags (Production);
about 50 filter criteria; 5 user stories.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | Check | Result |
|---|---|---|
| I. Stash is the system of record | Writes are play history (required by the constitution) and the colour rules, one UI-configuration key (`semanticStashViewer.tagColours`) changed only from Settings → Tag colours; saved filters are read, not written; tag colours live in Stash, not on this machine | Pass |
| II. Semantic-tagging parity | The plugin's filters are a first-class (placeholder) group in the filter model and menu. The plugin's per-type tag flags and `variantOfTagId` are read exactly as the plugin reads them (modules cited in the spec's Plugin parity assumption), checked with a scrubbed fixture from Production; nothing is written to tags. Colours are flag rules in Stash's UI configuration, the same model as the plugin's type colours, documented in [contracts/tag-colour.md](contracts/tag-colour.md) so the plugin can read them. | Pass |
| III. Brain/UI separation | Filter model, saved-filter decoding, queries, play-history rules and queue live in `stash-core`; the UI holds state machines and views | Pass |
| IV. Lean, batched GraphQL | Per-action budgets with named operations and fixed selections (contract); no N+1 (chips, markers, performers inside one request; images through the cache); choice lists fetched once per server and cached; play-history writes patch the cache. **Operation review** below | Pass |
| IV. Semantic vocabulary scoping | Semantic tags load with Stash-side "flag is not null" filters per type; ordinary tags with "every flag is null"; the two lists are disjoint and neither is scanned to find the other (FR-003c) | Pass |
| XI. Visual identity & tag colours | Tag chips show their flag rule's colour (defaults from the RPG palette); rules stored in Stash (UI configuration), found by Stash-side `NOT_NULL` filters in the existing vocabulary request; chip text picked for ≥ 4.5:1 contrast; Settings → Tag colours manages them. | Pass |
| VI. Keyboard | Search, filter menu, editors, pickers, saved filters, chips, and wall mode are keyboard-operable (FR-003f) | Pass |
| V. mpv is the player | Direct stream by default; transcoding only as the explicit, marked, logged fallback; previews are animated WebP images decoded to frames, never video; keys in headers | Pass |
| VI. Responsive UI | Budgets as 007's plus new harness rows; previews one at a time; pickers filter a cached list locally | Pass |
| VII. User-owned connection | Only the configured server; nothing new leaves the machine | Pass |
| VIII. Web UI parity | Search, every criterion, saved/default filters, scene page, resume, history, markers, previews, wall, scrubber, fallback; editing stays Phase 2 (spec table) | Pass |
| IX. One app shell | Filtered views open in tabs (Ctrl/middle-click), restore exactly; chip details and pickers follow the overlay rules (Escape, click outside) | Pass |
| X. Galleries | Gallery chips listed, opening once galleries exist | Pass |
| C4 unsafe-code gate | No new `unsafe` expected; `player` gains a transcoded open flag in safe code | Pass |

**Operation review (Principle IV)**:

| Operation | When | Requests | Payload (estimate) |
|---|---|---|---|
| `ScenesPage` (+ criteria) | each page | 1 | as 007 (~25 KB for 50 cards) |
| `SceneDetails` | scene page | 1 | ~3–8 KB (40 performers ≈ +4 KB) |
| `SceneFiles` / `SceneGalleries` | section opened | 1 each | < 2 KB / < 5 KB |
| `PerformerCard` / `TagCard` / `StudioCard` | chip rest | 1 per item per session | < 1 KB |
| `ChoiceList<Kind>` | picker, first time per server | 1 per kind | ≤ ~1 MB for 20,000 names, once; performers ≈ +70 B each for `image_path` (≈ +70 KB per 1,000; R13) |
| `SemanticVocabulary` | Tags picker or first scene page, first time per server | 1 | small (the plugin's vocabulary plus tags matched by extra colour rules, a few hundred; includes their `custom_fields`) |
| `SavedSceneFilters` / `UiConfig` | menu / new tab | 1 per session each | < 50 KB / < 50 KB |
| `SaveActivity` / `AddPlay` | viewing | ≤ 1/min + 1 / ≤ 1 | < 1 KB |
| `SaveTagColours` | Settings → Tag colours save | 1 (+ 1 vocabulary refetch when a rule's field changes) | < 2 KB |

Post-design re-check: the data model and contracts keep every budget; no violations.

## Project Structure

### Documentation (this feature)

```text
specs/008-finish-browse-play/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── query-budget.md
│   └── play-history.md
├── checklists/requirements.md
└── tasks.md            # /speckit-tasks
```

### Source Code (repository root)

```text
crates/stash-core/
├── graphql/operations/          # NEW: golden selection files, one per operation
├── src/adapter/
│   ├── mod.rs                   # request recorder hook in StashClient::graphql and GETs
│   ├── gql.rs                   # SceneFilterType and criterion input objects; detail, chip, choice fragments
│   ├── scenes.rs                # ScenesPage with criteria; SceneDetails, SceneFiles, SceneGalleries; transcode URL
│   ├── filters.rs               # NEW: SavedSceneFilters, UiConfig, SaveTagColours
│   ├── choices.rs               # NEW: ChoiceList<Kind> (ordinary tags only for Tags)
│   ├── semantic.rs              # NEW: SemanticVocabulary (flags, variants, tag colours)
│   ├── chips.rs                 # NEW: PerformerCard, TagCard, StudioCard
│   └── activity.rs              # NEW: SaveActivity, AddPlay
├── src/semantic.rs              # NEW: SemanticKind, flag parsing, priority, kind_of, colour rules and colour_of
├── src/previews.rs              # NEW: animated WebP fetch and decode to frames
├── src/sprites.rs               # NEW: VTT parsing and sprite lookup
├── src/scenes/
│   ├── filter/                  # NEW: criterion model, modifiers, saved-filter decoding, semantic placeholders, local search
│   └── query.rs                 # SceneQuery gains criteria (cache hash includes them)
├── src/playback/history.rs      # NEW: viewing rules (checkpoints, threshold, 98%) and the queue
└── tests/
    ├── graphql_operations.rs    # NEW: snapshots, tier rules, writes rule
    ├── query_budget.rs          # NEW: request counts per action (mock server)
    ├── scene_filters.rs         # NEW: criteria → SceneFilterType, saved-filter decoding
    ├── choices.rs               # NEW: choice lists, vocabulary, colours (scrubbed plugin fixture)
    ├── previews.rs              # NEW: WebP frames, VTT
    └── play_history.rs          # NEW: rules and the queue

crates/player/src/               # transcoded open flag through the direct-stream guard; start position
native-ui/src/
├── screens/scenes/
│   ├── search.rs, filters.rs, picker.rs   # NEW: search box, chips, filter menu, pickers
│   ├── wall.rs, preview.rs                 # NEW: wall mode, hover previews (frames on a timer)
├── screens/scene.rs             # full detail tier, sections, chip hover cards, markers
├── screens/settings/playback.rs # NEW: Settings → Playback
├── session/playback.rs          # viewings, checkpoints, resume, chapters, fallback offer
├── player/                      # seek-bar chapters and sprite thumbnails; "Transcoded" badge
├── services/                    # NEW files: choices.rs, filters.rs, settings.rs (playback preferences), previews.rs; history queue flushing
├── screens/settings/tag_colours.rs # NEW: Settings → Tag colours (colour rules manager)
├── widgets/theme.rs             # tag chip colours from colour rules, chip text contrast
└── measure/bench.rs             # gql-* audit rows
native-ui/tests/                 # scenes_find.rs, scene.rs, playback.rs, previews.rs, snapshot.rs
crates/perf-harness/             # gql-* rows and budgets
```

**Structure Decision**: the 007 workspace layout, unchanged. Stash I/O, the filter model, and
play-history rules go in `stash-core` (Principle III); the UI adds screens and views.

## Performer picture versions (amendment 2026-10-07, FR-011c)

**Problem**: performer pictures are cached as `thumb:performer:<id>:0`. The version never changes,
so a picture replaced in Stash keeps showing the old one until the disk cache evicts it. Scene
thumbnails don't have this problem: their key carries the `t` stamp from the screenshot path.

**Design** ([research R13](research.md#r13-performer-picture-versions-fr-011c)):

- The version is the `t` query value of the performer's `image_path` (Stash sets it to the
  performer's `updated_at`), parsed by the same function as scene screenshots. A path marked
  `default=true` means no picture.
- `PerformerChip` (scene page) and the performer `Choice` (picker) carry `image: Option<String>`,
  the version, replacing `has_image`. `SceneDetails` already selects `image_path`;
  `ChoiceListPerformers` adds it ([contracts/query-budget.md](contracts/query-budget.md)).
- Icon and portrait keys (`performer:<id>`, `portrait:<id>`) take that version, so both still
  share one cached image, and a new version is one download.
- The serialized field is required: scene details and choice lists cached before the change fail
  to parse, count as misses, and refetch once (the cache already treats unreadable entries that
  way).
- Older versions are never read again and leave through the disk cache's least-recently-used
  eviction; nothing deletes them explicitly.
- A changed picture appears when the entry carrying the version refreshes (the scene page's
  details or the picker's list, on the cache's existing schedule); no new polling or requests.

**Technical context delta**: no new dependencies; files touched are `stash-core`
`adapter/choices.rs`, `adapter/scene_page.rs`, `scenes/mod.rs` and the UI's `picker.rs`,
`scene.rs`, `session/mod.rs`; golden operation `ChoiceListPerformers.graphql` changes.

**Constitution re-check**: IV (lean GraphQL): one field added to one cached list (≈ 70 bytes per
performer, Stash doesn't compress responses), no new request; in exchange the picker stops one
wasted GET per visible performer without a picture each session. Tier rules unchanged (a
performer's `image_path` isn't scene-scoped). VI (responsive UI): unchanged. I (system of record):
read only. Pass.

## Complexity Tracking

No constitution violations to justify.
