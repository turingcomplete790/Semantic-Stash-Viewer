---

description: "Task list for 008-finish-browse-play"
---

# Tasks: Finish Browse and Play

**Input**: Design documents from `specs/008-finish-browse-play/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/query-budget.md](contracts/query-budget.md),
[contracts/play-history.md](contracts/play-history.md),
[contracts/tag-colour.md](contracts/tag-colour.md), [quickstart.md](quickstart.md)

**Tests**: Included. The query budget is enforced by tests (research R2: selection snapshots,
request counts, live audit rows), and play history must lose nothing (SC-006). Write each story's
tests first and check that they fail.

**Rules that apply to every task**:
- **The 007 architecture holds**: the UI is a hierarchical state machine (enums are exclusive
  states, `Step { effects, up }`); effects are data; no I/O, decoding, or blocking in `update` or
  `view`; results carry a generation and stale ones are dropped.
- **Stash I/O, the filter model, the semantic vocabulary, and play-history rules live in
  `stash-core`** (Principle III); `native-ui` holds states, views, and effect wiring.
- **Query budget** ([contracts/query-budget.md](contracts/query-budget.md)): every new operation
  is named, has a golden selection file in `crates/stash-core/graphql/operations/`, and selects
  only what its contract lists. No request per related item, ever.
- **Writes**: only `SaveActivity`, `AddPlay`, and `SaveTagColours`, only against Testing (`localhost:9998`) during
  development and measurement. Production (`localhost:9999`) is read-only.
- **Secrets**: the API key only in headers; never log or print `apikey=` values, scene titles, or
  file paths from the user's library.
- **Each story** ends with its quickstart check with the user and the gate (`cargo fmt --all
  --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`).
  Commit only after the user has looked.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependencies on unfinished tasks)
- **[Story]**: the user story the task belongs to (US1–US5)

---

## Phase 1: Setup

**Purpose**: module skeletons and test data on Testing.

- [X] T001 Create `crates/stash-core/graphql/operations/` with a `README.md` (one golden `.graphql`
  file per named operation; how to regenerate with `UPDATE_GOLDEN=1`) and the first golden file
  `ScenesPage.graphql` holding the current `find_scenes_page` operation text from
  `crates/stash-core/src/adapter/scenes.rs`
- [X] T002 [P] Add empty modules and register them: `crates/stash-core/src/adapter/{filters,choices,
  chips,activity,semantic}.rs` in `crates/stash-core/src/adapter/mod.rs`;
  `crates/stash-core/src/scenes/filter/mod.rs` in `crates/stash-core/src/scenes/mod.rs`;
  `crates/stash-core/src/semantic.rs`, `crates/stash-core/src/previews.rs`,
  `crates/stash-core/src/sprites.rs`, and `crates/stash-core/src/playback/{mod,history}.rs` in
  `crates/stash-core/src/lib.rs`
- [X] T003 [P] Prepare Testing (`localhost:9998`) through its web UI, and record the ids (not
  titles or paths) in [quickstart.md](quickstart.md) under "Prerequisites": three saved scene
  filters (one with performers, one with tags plus a resolution, one with a custom field), a
  default scene filter, a scene with markers, a scene with several files, a scene with sprites
  and an animated preview, two tags flagged as semantic (`isTrait: 1`, `plugtag: 1`), one tag
  with `variantOfTagId` pointing at one of them, one with `isTrait: 0`, and one ordinary tag with
  a non-semantic flag `ssvTestFlag: 1` (for an added colour rule). Then capture the vocabulary
  result read-only from Production by sending the `SemanticVocabulary` request text from
  [contracts/query-budget.md](contracts/query-budget.md) directly (curl, API key in a header;
  the viewer's operation doesn't exist until T027) into
  `crates/stash-core/tests/fixtures/semantic/vocabulary.json`, scrubbed (ids, flags, and variant
  links kept; names and aliases replaced with `tag-<id>`; no other custom fields kept), as the
  plugin compatibility fixture (constitution Principle II)

---

## Phase 2: Foundational (blocking prerequisites)

**Purpose**: the query-budget machinery every story's tests use, the shared UI settings read,
the session file version, and the player's start position.

**⚠️ CRITICAL**: no user story work can begin until this phase is complete.

- [X] T004 Add a request recorder to `StashClient` in `crates/stash-core/src/adapter/mod.rs`: a
  `RequestRecord { operation: String, request_bytes: usize, response_bytes: usize, duration:
  Duration, kind: GraphQl | Get }`, a `Recorder` trait (`fn record(&self, RequestRecord)`), and
  `StashClient::with_recorder(Arc<dyn Recorder>)`; record every call through
  `StashClient::graphql` (operation name from the Cynic operation) and every plain GET (operation
  name is the URL path's kind, for example `thumbnail`, `preview`, `sprite`, never the full URL)
- [X] T005 Create `crates/stash-core/tests/query_budget.rs` with a mock-server helper that installs a
  counting recorder and asserts `(operation, count)` lists per action; first case: showing one
  Scenes page records exactly `[("ScenesPage", 1)]`
- [X] T006 Create `crates/stash-core/tests/graphql_operations.rs`: for each named operation, build
  its query text and compare it with `crates/stash-core/graphql/operations/<Name>.graphql`
  (rewrite the file when `UPDATE_GOLDEN=1`); start with `ScenesPage`. Add the writes rule: no
  golden file other than `SaveActivity.graphql`, `AddPlay.graphql`, and `SaveTagColours.graphql`
  may contain `mutation` (FR-018)
- [X] T007 Add tier rules to `crates/stash-core/tests/graphql_operations.rs`: card and chip tier
  operations (`ScenesPage`, `PerformerCard`, `TagCard`, `StudioCard`, every `ChoiceList*`) MUST NOT
  select `paths.stream`, `sceneStreams`, `scene_markers`, `galleries`, `files.path`,
  `files.fingerprints`, or `image_path`, and only `TagCard` may select `description`;
  `SceneDetails` MUST NOT select `galleries`, `sceneStreams`, `paths.stream`, or any file `path`;
  no operation except `SemanticVocabulary` may select `custom_fields`
- [X] T008 [P] Add cache key constructors in `crates/stash-core/src/cache/mod.rs` for the new keys:
  `choices:<kind>`, `semantic:vocabulary`, `scene:details:<id>`, `scene:files:<id>`,
  `scene:galleries:<id>`, `chip:<kind>:<id>`, `ui:settings`, `saved-filters:scenes`,
  `preview:<id>`, `sprite:<id>`, with a unit test that keys from different kinds never collide
- [X] T009 [P] Implement `UiConfig` in `crates/stash-core/src/adapter/filters.rs`: one
  `configuration { ui }` read returning `UiSettings { default_scene_filter: Option<raw saved
  filter JSON>, minimum_play_percent: f64 (default 0), web_track_activity: bool, colour_rules:
  Vec<ColourRule> }` (keys `defaultFilters.scenes`, `minimumPlayPercent`, `trackActivity`,
  `semanticStashViewer.tagColours`; `ColourRule { field: "a tag custom field key: letters,
  digits, _; 1–64 characters", colour: #rrggbb read case-insensitively }`, ordered; invalid rules
  skipped and logged once at warn level, field name only; an absent or unreadable key gives the
  seven default rules of [contracts/tag-colour.md](contracts/tag-colour.md)), cached as `ui:settings`, at
  most once per session plus the cache's quiet refresh (so a changed default filter reaches new
  tabs); golden file `UiConfig.graphql`; tests in `crates/stash-core/tests/query_budget.rs` (1
  cold, 0 cached; missing keys give the defaults)
- [X] T010 [P] Bump the session file to version 2 in `native-ui/src/session/snapshot.rs`
  (`VERSION = 2`; a version 1 file starts fresh with Home, no migration) with a test in
  `native-ui/tests/snapshot.rs`
- [X] T011 [P] Add an optional start position to the player's open request in
  `crates/player/src/commands.rs` and `crates/player/src/session.rs` (mpv `start` option on load;
  `None` starts at 0; a start beyond the duration starts at 0), carried through the `native-ui`
  play effect in `native-ui/src/effects.rs` and `native-ui/src/session/playback.rs`; test in
  `native-ui/tests/playback.rs`
- [X] T012 Wire the recorder into the bench: `native-ui/src/measure/bench.rs` installs a recorder
  when measuring and writes per-scenario `gql-*` records (requests, response bytes, server time
  median and p95); `crates/perf-harness/src/report.rs` gains `gql-*` rows with a request budget
  each (fail when exceeded) and flags a payload more than 20% larger than the previous report's

**Checkpoint**: recorder, snapshot tests, UI settings, session v2, and start position are in place.

---

## Phase 3: User Story 1 - Find scenes: search, filters, saved filters (Priority: P1) 🎯 MVP

**Goal**: search, every web UI scene criterion, the full cached pickers (with the semantic
vocabulary grouped by type), saved and default filters, and filtered views that restore exactly.

**Independent Test**: quickstart V1: search a known title, add performer and resolution filters,
check the count and first 100 against Testing's web UI; apply each saved filter; a new tab gets
the default filter; restart restores the tab; the performer picker on Production opens at once
after the first time.

### Tests for User Story 1 ⚠️ write first, check they fail

- [X] T013 [P] [US1] Criterion conversion tests in `crates/stash-core/tests/scene_filters.rs`:
  table-driven, one row per `CriterionKind` × allowed `Modifier`, checking the generated
  `SceneFilterType` variables; validation: "the modifier is one the kind allows; `Between` has two
  values; `IsNull`/`NotNull` carry no value; `Items` are non-empty unless the modifier is a null
  check"; semantic placeholder kinds are never sent
- [X] T014 [P] [US1] Saved-filter decoding tests in `crates/stash-core/tests/scene_filters.rs` using
  fixture JSON in `crates/stash-core/tests/fixtures/saved_filters/` copied from Testing's saved
  filters (the saved criterion format: `{items: [{id, label}], excluded, depth}`,
  `find_filter`, `ui_options`); an unknown key lands in `left_out`, never silently dropped
- [X] T015 [P] [US1] Choice list and vocabulary tests in `crates/stash-core/tests/choices.rs`:
  `ChoiceListTags` sends one `custom_fields` list with `IS_NULL` for every type flag and
  `variantOfTagId`; flag values `1`, `"1"`, `true`, `"true"` are set and `0`, `"0"`, `false`, null
  are not; a tag with several flags takes the fixed priority (Label > Action > Role > Trait >
  Pluggable > MiseEnScene > Variation); `kind_of` resolves a common variant through its canonical
  tag; a vocabulary entry with a flag of 0 and a variant whose canonical tag has no kind are
  grouped as ordinary; `colour_of` gives the first matching rule's colour in rule order, a
  flag of 0 matches no rule, a common variant without its own match takes its canonical tag's
  colour, a tag matching no rule has none, and an added rule (`ssvTestFlag`) colours its tags;
  the Production fixture `crates/stash-core/tests/fixtures/semantic/vocabulary.json` parses with
  every flagged tag typed; group aliases split on commas; local search is case-insensitive and
  accent-folded over name and aliases
- [X] T016 [P] [US1] Request-count tests in `crates/stash-core/tests/query_budget.rs`: a page with
  search and criteria is 1 `ScenesPage`; `SavedSceneFilters` 1 cold, 0 cached; each
  `ChoiceList<Kind>` 1 cold, 0 cached; `SemanticVocabulary` 1 cold, 0 cached; opening the Tags
  picker cold is at most 2 (`ChoiceListTags` + `SemanticVocabulary`)
- [X] T017 [P] [US1] Transition tests in `native-ui/tests/scenes_find.rs`: typing updates
  `search_draft` only; 300 ms after the last key one `LoadScenesPage` with page 1; an older
  generation's results are dropped; adding, editing, removing a chip, clearing, and applying a
  saved filter each reset to page 1 with one load; a saved filter with `left_out` shows the
  notice; a new Scenes tab applies the default filter; no matches shows the empty state with
  "Clear filters"; a semantic placeholder is shown disabled and kept on restore; keyboard
  (FR-003f): focus search, open the filter menu, move through criteria and editors, picker
  arrows/Enter/include-exclude/Escape, saved filters menu, and removing a focused chip all work
  without a pointer
- [X] T018 [P] [US1] Snapshot round-trip test in `native-ui/tests/snapshot.rs`: a tab with search,
  criteria (including an `Items` criterion with labels and a semantic placeholder), applied saved
  filter, sort, page, page size, and scroll comes back identical without a request

### Implementation for User Story 1

- [X] T019 [US1] Criterion model in `crates/stash-core/src/scenes/filter/mod.rs`: `Criterion {
  kind, modifier, value }`, `CriterionKind` (one per v0.31.1 `SceneFilterType` field in research
  R3, plus `Semantic(SemanticKind)` placeholders), `Modifier` (`Equals, NotEquals, GreaterThan,
  LessThan, Between, NotBetween, Includes, IncludesAll, Excludes, IsNull, NotNull, MatchesRegex,
  NotMatchesRegex`), `CriterionValue` (`Text | Int | IntRange | Duration | Date | DateRange |
  Timestamp | Bool | Resolution | Orientation(Set) | Items { items, excluded, depth } | StashId {
  endpoint, id } | CustomField { field, value }`), validation as quoted in T013, serde for the
  session file
- [X] T020 [US1] Per-kind metadata in `crates/stash-core/src/scenes/filter/kinds.rs`: label, group
  (as the web UI's filter menu), allowed modifiers, and value type, taken from the Stash web UI
  source for v0.31.1 (`/home/bbulder/stash-source/stash/ui/v2.5/src/models/list-filter/scenes.ts`
  and its criteria files)
- [X] T021 [US1] Semantic placeholders in `crates/stash-core/src/scenes/filter/semantic.rs`: Trait,
  Trait Category, Label, Role, Role Category, Pluggable, Action with variation, Mise-en-scène and
  Location, Timeline relation, Archetype, Look, each with a one-line description; excluded from
  conversion (never sent)
- [X] T022 [US1] Cynic input objects for `SceneFilterType` and its criterion inputs in
  `crates/stash-core/src/adapter/gql.rs`, and the conversion in
  `crates/stash-core/src/scenes/filter/to_stash.rs`
- [X] T023 [US1] `SceneQuery` gains `criteria: Vec<Criterion>` in
  `crates/stash-core/src/scenes/query.rs` ("each kind at most once except custom fields; order
  kept for chips"); `normalized()` sorts criteria by kind and drops empty search and criteria;
  `cache_hash()` includes them; `find_scenes_page` in `crates/stash-core/src/adapter/scenes.rs`
  sends `scene_filter`; regenerate `ScenesPage.graphql` (selection unchanged, new variable only)
- [X] T024 [US1] Saved-filter decoding in `crates/stash-core/src/scenes/filter/saved.rs`:
  `SavedFilter { id, name, query: SceneQuery, page_size: Option<u32>, mode: Option<Mode>,
  left_out: Vec<String> }` from `find_filter` + `object_filter` + `ui_options`; also used for the
  default filter from `UiSettings`
- [X] T025 [US1] `SavedSceneFilters` in `crates/stash-core/src/adapter/filters.rs`
  (`findSavedFilters(mode: SCENES){id name find_filter{q page per_page sort direction}
  object_filter ui_options}`), cached as `saved-filters:scenes`, golden file
- [X] T026 [P] [US1] Choice lists in `crates/stash-core/src/adapter/choices.rs`:
  `ChoiceListPerformers` (`id name alias_list`), `ChoiceListTags` (ordinary tags only: `id name
  aliases` with every type flag and `variantOfTagId` `IS_NULL`), `ChoiceListStudios`,
  `ChoiceListGroups` (`id name aliases`), `ChoiceListGalleries` (`id title`), all `per_page: -1,
  sort: name`, cached per server as `choices:<kind>`, registered with the quiet refresher in
  `crates/stash-core/src/cache/refresh.rs`; golden files; local search in
  `crates/stash-core/src/scenes/filter/search.rs`
- [X] T027 [P] [US1] Semantic vocabulary in `crates/stash-core/src/semantic.rs` (`SemanticKind`:
  `Pluggable (plugtag) | Trait (isTrait) | Role (isRole) | Action (isAction) | Label (isLabel) |
  MiseEnScene (isMiseEnScene) | Variation (isVariation)`; flag parsing; priority; `kind_of`;
  `colour_of(tag id, rules)` per [contracts/tag-colour.md](contracts/tag-colour.md)) and the
  `SemanticVocabulary` operation in `crates/stash-core/src/adapter/semantic.rs` (one request:
  one aliased `findTags` per flag and for `variantOfTagId` with `NOT_NULL`, selecting `id name
  aliases custom_fields`, plus one `rule_<n>` alias per colour rule whose field isn't a semantic
  flag, selecting `id custom_fields`; the request text is built from the current rules), cached
  as `semantic:vocabulary` (refetched whenever the set of rule fields changes, whether saved
  here or picked up by the quiet refresh of the UI settings) and refreshed with the tag
  choice list; golden file for the default rules
- [X] T028 [US1] Effects in `native-ui/src/effects.rs` and services in
  `native-ui/src/services/choices.rs` (`LoadChoices { kind }`, `LoadVocabulary`) and
  `native-ui/src/services/filters.rs` (`LoadSavedFilters`, `LoadUiSettings`), registered in
  `native-ui/src/services/mod.rs`, each answered from the cache first, results as typed messages
  in `native-ui/src/messages.rs`
- [X] T029 [US1] `ScenesState` in `native-ui/src/screens/scenes/state.rs`: `query.search`,
  `query.criteria`, `applied_saved_filter: Option<(id, name)>` ("cleared when the user edits the
  criteria"), transient `search_draft` and `picker: Option<PickerState>`; the transitions in T017
  (300 ms debounce through a timer effect, page 1 on change, generations)
- [X] T030 [P] [US1] Search box in `native-ui/src/screens/scenes/search.rs`, placed in the controls
  bar (`native-ui/src/screens/scenes/controls.rs`), with a key to focus it added to
  `native-ui/src/shell/keymap.rs` and the keyboard help
- [X] T031 [US1] Criterion chips and "Clear all" in `native-ui/src/screens/scenes/filters.rs`, plus
  the filter menu: every criterion in the web UI's groups, a value editor per value type (text,
  number, range, duration, date, resolution, orientation, bool, items, StashID, custom field), and
  the Semantic group shown described and disabled; tag chips in the filter bar use `colour_of`;
  every part keyboard-operable (FR-003f), keys listed in keyboard help
- [X] T032 [US1] Picker in `native-ui/src/screens/scenes/picker.rs`: the full cached list, narrowing
  on each key within a frame (windowed rows with `keyed_column`, as the grid), names and aliases;
  performer images only for visible rows through the thumbnail service; the Tags picker shows the
  vocabulary grouped by kind (type badge, each tag in its `colour_of` colour) above ordinary
  tags (ordinary tags matched by an added rule in that rule's colour); include/exclude and modifier; `Loading` until
  the list first arrives; arrows move, Enter chooses, a key toggles include/exclude, Escape and
  click outside close it
- [X] T033 [US1] Saved filters menu in `native-ui/src/screens/scenes/filters.rs` (listed by name,
  apply sets search, criteria, sort, page size, display mode; `left_out` shown as a notice;
  opened and chosen from the keyboard), and
  the default filter for new Scenes tabs in `native-ui/src/shell/mod.rs` (tabs already open keep
  what they show)
- [X] T034 [P] [US1] Empty state with "Clear filters", and the offline message for an uncached
  search or filter, in `native-ui/src/screens/scenes/grid.rs`
- [X] T035 [US1] Performer, tag, and studio chips on the scene page open Scenes filtered to them
  (same tab; Ctrl+click or middle-click opens a new tab) in `native-ui/src/screens/scene.rs`
- [X] T036 [US1] Save and restore search, criteria, and applied saved filter with the tab in
  `native-ui/src/session/snapshot.rs`
- [X] T037 [US1] Bench rows in `native-ui/src/measure/bench.rs` and budgets in
  `crates/perf-harness/src/report.rs`: `gql-page-50` (1), `gql-page-1000` (1), `gql-filter-apply`
  (1), `gql-picker-cold` (1), `gql-picker-cached` (0), `gql-vocabulary-cold` (1),
  `gql-tags-picker-cold` (≤ 2); add the SC-002a picker timing row (open < 150 ms cached for a
  20,000-name list, narrowing within a frame)
- [X] T038 [US1] Opt-in live test in `crates/stash-core/tests/live_stash.rs` (SC-001): every saved
  filter on Testing decodes with an empty `left_out`; for 10 varied queries (listed in
  [quickstart.md](quickstart.md) V1: search only, performer, tag with depth, studio, resolution,
  duration range, organized, rating, custom field, and the 3 saved filters) a hand-written
  `SceneFilterType` JSON per query in `crates/stash-core/tests/fixtures/sc001/` (written from
  the web UI's network requests for the same filter, independent of the viewer's converter)
  and the viewer's converted criteria give the same count and the same first 100 ids
- [X] T039 [US1] Run quickstart V1 with the user (Testing, and Production read-only for the
  picker), then the gate

**Checkpoint**: US1 works on its own (MVP).

---

## Phase 4: User Story 2 - The full scene page (Priority: P2)

**Goal**: the scene page with every field, lazy "All files" and "Galleries", markers, chip
details on rest, tag chips marked by type and coloured by the colour rules, and Settings → Tag
colours: 0 requests for the first paint, then 1.

**Independent Test**: quickstart V2: scenes with many performers, markers, several files, and
almost no metadata match the web UI; chip hover shows details; sweeping chips loads nothing;
changing a rule's colour on Testing recolours chips at once.

### Tests for User Story 2 ⚠️ write first, check they fail

- [X] T040 [P] [US2] Golden files and tier rules in `crates/stash-core/tests/graphql_operations.rs`
  for the new `SceneDetails` selection, `SceneFiles`, `SceneGalleries`, `PerformerCard`,
  `TagCard`, `StudioCard` (selections exactly as in the contract)
- [X] T041 [P] [US2] Request-count tests in `crates/stash-core/tests/query_budget.rs`: opening a scene
  page is 1 `SceneDetails` (0 when cached and fresh), also for a scene with 40 performers;
  `SceneFiles` and `SceneGalleries` 1 each, only when requested; each chip card 1 per item per
  session
- [X] T042 [P] [US2] Transition tests in `native-ui/tests/scene.rs`: title, cover, and Play show
  from the card before details arrive and keep their place after; `Section` goes `Closed →
  Loading → Ready | Failed` and loads once; a chip hover resting 400 ms emits one
  `LoadChipDetails`, a sweep emits none, a second rest on the same chip emits none; choosing a
  marker plays from its seconds; a scene without a title is named by its file's basename; a
  deleted scene shows "no longer exists"; a semantic tag chip carries its kind (and a common
  variant its canonical tag's kind) and its `colour_of` colour; an ordinary tag matched by an
  added rule shows that rule's colour

### Implementation for User Story 2

- [X] T043 [US2] Replace `SceneDetails` in `crates/stash-core/src/adapter/gql.rs` and
  `crates/stash-core/src/adapter/scenes.rs` with the contract's selection (`id title code details
  director date rating100 organized play_count resume_time play_duration urls studio{id name}
  performers{id name image_path} tags{id name} groups{group{id name} scene_index}
  scene_markers{id title seconds end_seconds primary_tag{name}}` first file's facts, `paths{
  screenshot webp vtt sprite}`), cached as `scene:details:<id>`
- [X] T044 [P] [US2] `SceneFiles` (`files{id basename path size duration width height video_codec
  audio_codec format frame_rate bit_rate fingerprints{type value}}`) and `SceneGalleries`
  (`galleries{id title image_count}`) in `crates/stash-core/src/adapter/scenes.rs`, cached as
  `scene:files:<id>` and `scene:galleries:<id>`
- [X] T045 [P] [US2] `PerformerCard`, `TagCard`, `StudioCard` in
  `crates/stash-core/src/adapter/chips.rs`, cached as `chip:<kind>:<id>`
- [X] T046 [US2] Effects `LoadSceneFiles`, `LoadSceneGalleries`, `LoadChipDetails` in
  `native-ui/src/effects.rs`, and small performer chip images through the thumbnail service
  (`native-ui/src/screens/scenes/thumbs.rs`), cached like scene thumbnails
- [X] T047 [US2] `SceneState` in `native-ui/src/screens/scene.rs`: full `details`; `files:
  Section<Vec<FileFacts>>`, `galleries: Section<Vec<GalleryChip>>` where `Section = Closed |
  Loading | Ready | Failed`; `chip_hover: Option<(ChipRef, since)>`; fetched chip cards
- [X] T048 [P] [US2] Tag chip styling in `native-ui/src/widgets/theme.rs`: a badge per
  `SemanticKind`; a chip style from `colour_of` (the matching rule's colour); chip text `#0d0a05`
  or `#e8e0c8`, whichever gives at least 4.5:1 contrast (constitution Principle XI), with a unit
  test over every default rule colour and a few user-picked colours
- [X] T049 [P] [US2] `SaveTagColours` in `crates/stash-core/src/adapter/filters.rs`:
  `configureUISetting(key: "semanticStashViewer.tagColours", value: { version: 1, rules })`
  (rules written with lower-case `#rrggbb`), golden file; on success patch `ui:settings`; tests
  in `crates/stash-core/tests/query_budget.rs` (1 write per save; a refetch of
  `SemanticVocabulary` only when a rule's field was added or changed) and against a mock server
  that the request replaces only that key
- [X] T050 [US2] Settings → Tag colours page in `native-ui/src/screens/settings/tag_colours.rs`,
  registered in `native-ui/src/screens/settings/mod.rs`: `TagColoursPage { rules, counts, editing,
  saving: Idle | Saving | Failed(message) }`; the ordered rules with swatch and tag count; change
  a colour (RPG and app palettes, or hex), add a rule for a tag custom field key (letters,
  digits, _; 1–64 characters), remove an added rule, reorder, reset to defaults; save as one
  `SaveTagColours`; every open view recolours within one frame of the confirmed save; a failed save says so and keeps the
  previous rules; keyboard-operable; transition tests in `native-ui/tests/settings.rs`
- [X] T051 [US2] Bench row `gql-tag-colours-save` (1 + at most 1, Testing only) in
  `native-ui/src/measure/bench.rs` and `crates/perf-harness/src/report.rs`
- [X] T052 [US2] Scene page view in `native-ui/src/screens/scene.rs`: details, code, director, date,
  rating, organized, play count, resume point, URLs; performer chips with images; tag chips (all
  tags, semantic ones marked by kind and every matched tag coloured by `colour_of` from the
  cached vocabulary and rules, FR-003d); studio; groups with
  scene index; markers with time, title, and tag; primary file facts marked as the one that
  plays; "All files" and "Galleries" sections; missing fields left out; long text truncated in
  chips and full on the page
- [X] T053 [US2] Chip details card in `native-ui/src/screens/scene.rs` with the overlay rules in
  `native-ui/src/shell/overlays.rs` (shown after a 400 ms rest or focus; Escape and click outside
  close it)
- [X] T054 [US2] Choosing a marker starts playback at its seconds (T011's start position) in
  `native-ui/src/session/playback.rs`
- [X] T055 [US2] Bench rows `gql-scene-page` (1), `gql-scene-files` (1), `gql-scene-galleries` (1),
  `gql-chip-rest` (1), `gql-chip-sweep` (0) in `native-ui/src/measure/bench.rs` and
  `crates/perf-harness/src/report.rs`
- [X] T056 [US2] Run quickstart V2 with the user (the tag colours part on Testing only), then the gate
- [X] T056a [US2] Performer images (FR-011a/b, clarified 2026-10-07): 40 px round chip icons in
  `native-ui/src/screens/scene.rs`, decoded at drawn size × display scale (not the fixed 64 px in
  `native-ui/src/session/mod.rs` `thumbnails()`); a ≈ 240 px portrait in the performer card under
  its own UI key (for example `portrait:<id>`) in `native-ui/src/screens/scenes/thumbs.rs` and
  `native-ui/src/effects.rs` `fetch_thumb`, mapped to the same `ThumbService` entry
  (`performer`, id, version `0`) so it never downloads again; tests in `native-ui/tests/scene.rs`
  (icon and portrait handles are separate; showing the portrait emits no new download); the
  `gql-chip-rest` bench row also reports image downloads (expected 0) and the portrait decode time
- [X] T056b [US2] Performer picture versions in the core (FR-011c; research R13; data-model
  `Choice`, `SceneDetails`): add `image: Option<String>` to `Choice` in
  `crates/stash-core/src/adapter/choices.rs` ("performers only, the picture version from
  `image_path`'s `t`; `None` when the path says `default=true` or for other kinds") and replace
  `PerformerChip.has_image` with `image: Option<String>` in `crates/stash-core/src/scenes/mod.rs`;
  both fields required when deserializing (`#[serde(deserialize_with = "Option::deserialize")]`)
  so entries cached before the change fail to parse and refetch once. Add
  `picture_version(path: Option<&str>) -> Option<String>` next to `screenshot_version` (`None` for
  no path or `default=true`, otherwise the `t` value) and use it in `adapter/choices.rs`
  (`ChoiceListPerformers` selects `image_path`) and `adapter/scene_page.rs`. Regenerate
  `crates/stash-core/graphql/operations/ChoiceListPerformers.graphql` (`UPDATE_GOLDEN=1`). Tests:
  `picture_version` (`?t=1791406802` → `1791406802`, `default=true` → none, no path → none) in
  `crates/stash-core/src/scenes/mod.rs`; performer choices carry their version and other kinds
  `None` in `crates/stash-core/tests/choices.rs`; a cached `PerformerChip` with `has_image` and no
  `image` fails to parse; `crates/stash-core/tests/query_budget.rs` still counts one
  `ChoiceListPerformers` request
- [X] T056c [US2] Picture versions in the UI (FR-011c): `performer_key(id, version)` and
  `portrait_key(id, version)` in `native-ui/src/screens/scenes/picker.rs`, the version as the
  `ThumbKey`'s second part (so `thumb_source` in `native-ui/src/effects.rs` still maps both to
  the same `ThumbService` entry, now `thumb:performer:<id>:<version>`); callers in
  `native-ui/src/session/mod.rs` `thumbnails()` (scene page performers, and picker rows only for
  choices with `image: Some`), `native-ui/src/screens/scene.rs` (`performer_chip`,
  `portrait_wanted` returning the id and version), the picker row view in `picker.rs`, and
  `native-ui/src/measure/bench.rs`. Tests in `native-ui/tests/scene.rs` and
  `native-ui/tests/picker.rs`: a new version is a new key and one `LoadThumbnails`; icon and
  portrait still share one source; picker rows for performers without a picture ask for nothing
- [X] T056d [US2] Validate FR-011c: the bench on Testing (`gql-picker-cold-payload` records the
  list's new size; `gql-chip-rest-downloads` and `gql-chip-portrait-downloads` stay 0), then
  quickstart V2a with the user on Testing (replace performer 2's picture, remove performer 3's),
  then the gate

**Checkpoint**: US1 and US2 both work on their own.

---

## Phase 5: User Story 3 - Play with resume, chapters, and play history (Priority: P3)

**Goal**: resume, markers as chapters, and play history saved to Stash by the web UI's rules,
with a queue on disk so nothing is lost.

**Independent Test**: quickstart V3 on Testing only.

### Tests for User Story 3 ⚠️ write first, check they fail

- [X] T057 [P] [US3] Rule tests in `crates/stash-core/tests/play_history.rs`: a checkpoint after
  every 60 s of playing (paused time doesn't count) with `play_duration` since the previous save;
  a final report on end; resume point 0 when the position is at 98% of the duration or more;
  `add_play` once per viewing, on the first report whose watched share is at least
  `minimumPlayPercent` (0 when unset); nothing when "Save play history" is off
- [X] T058 [P] [US3] Queue tests in `crates/stash-core/tests/play_history.rs`: append before send,
  remove on confirm, atomic write; two pending reports for the same scene merge (durations add,
  the latest resume point wins, `add_play` kept if either had it); per profile; sent in order;
  the 2 s flush deadline leaves the rest queued
- [X] T059 [P] [US3] Request-count test in `crates/stash-core/tests/query_budget.rs`: a simulated
  10-minute viewing makes at most 11 `SaveActivity` and 1 `AddPlay`; with history off, 0
- [X] T060 [P] [US3] Transition tests in `native-ui/tests/playback.rs`: Play with a resume point and
  resume on starts there and offers "Start from the beginning"; resume off starts at 0; every way
  a viewing ends (stop, close, tab close, scene change, server switch, quit, disconnect) emits
  the final report; next/previous chapter keys seek to markers; after 3 failed sends one
  notification per server, updated, not one per report

### Implementation for User Story 3

- [X] T061 [US3] Viewing rules in `crates/stash-core/src/playback/history.rs`:
  `PlayHistoryReport { scene_id, resume_time: f64 (0 when ≥ 98% complete), play_duration_delta:
  f64, add_play: bool, ended: bool, recorded_at }` and a pure `Viewing` accumulator fed by
  play/pause/position/end events
- [X] T062 [US3] The queue in `crates/stash-core/src/playback/history.rs`: the file
  `<data>/dev.semantic-stash-viewer/play-history.json` in the contract's format (`version: 1`,
  `pending` by profile uuid), merging, in-order sending, and `flush(deadline)`
- [X] T063 [P] [US3] `SaveActivity` (`sceneSaveActivity(id, resume_time, playDuration)`) and
  `AddPlay` (`sceneAddPlay(id){count}`) in `crates/stash-core/src/adapter/activity.rs` with golden
  files; after a confirmed save, patch `scene:details:<id>` (resume time, play duration, play
  count) instead of invalidating it
- [X] T064 [P] [US3] Playback preferences `{ resume: bool (default true), save_history: bool
  (default true) }` in a settings file in the config folder, in
  `native-ui/src/services/settings.rs`
- [X] T065 [US3] Viewing region in `native-ui/src/session/playback.rs` (`{ scene_id, started_at,
  playing_since, played_seconds, played_since_save, add_play_sent, last_checkpoint }`): feeds the
  core accumulator, emits `SavePlayHistory` effects at checkpoints and on every kind of end
- [X] T066 [US3] Queue flushing in `native-ui/src/effects.rs` and `native-ui/src/session/connection.rs`:
  at once, on reconnect, when a session for that server starts; on quit (`native-ui/src/app.rs`)
  and server switch, queue the final report and flush for up to 2 s; the failure notification
  through the notification centre
- [X] T067 [US3] Resume on Play (T011's start position from `resume_time` when resume is on) and a
  "Start from the beginning" control shown for 8 seconds in `native-ui/src/player/controls.rs`
- [X] T068 [US3] Markers as chapters on the seek bar (titles on hover) in
  `native-ui/src/player/controls.rs`; next/previous chapter keys (PageUp/PageDown) at the player
  level in `native-ui/src/shell/keymap.rs`, listed in keyboard help and Settings → Keyboard
- [X] T069 [P] [US3] Settings → Playback page in `native-ui/src/screens/settings/playback.rs`
  ("Resume where you left off", "Save play history"), registered in
  `native-ui/src/screens/settings/mod.rs`
- [X] T070 [US3] Bench row `gql-viewing-10min-writes` (≤ 11 + 1, Testing only) in
  `native-ui/src/measure/bench.rs` and `crates/perf-harness/src/report.rs`
- [X] T071 [US3] Run quickstart V3 with the user on Testing, then the gate

**Checkpoint**: US1–US3 work on their own.

---

## Phase 6: User Story 4 - Previews, wall mode, and seek-bar thumbnails (Priority: P4)

**Goal**: animated image previews on hover (one at a time), a wall mode of stills, and sprite
thumbnails on the seek bar.

**Independent Test**: quickstart V4.

### Tests for User Story 4 ⚠️ write first, check they fail

- [X] T072 [P] [US4] Core tests in `crates/stash-core/tests/previews.rs`: an animated WebP fixture in
  `crates/stash-core/tests/fixtures/` decodes to frames at the requested width with delays; the
  VTT parser reads `start --> end` cues with `#xywh=x,y,w,h` and looks up a time; a missing
  preview or sprite is a quiet "none"
- [X] T073 [P] [US4] Transition tests in `native-ui/tests/previews.rs`: `PreviewState` goes `Waiting
  { since } → (after a 200 ms rest) Loading → Playing { frames, frame index, next at } → gone`;
  keyboard focus starts a preview the same way and arrows move through the wall; only one exists across
  the session; a sweep fetches nothing; leaving drops the frames; no `paths.webp` keeps the
  still; the wall mode is saved with the tab; the frame timer runs only while a preview plays

### Implementation for User Story 4

- [X] T074 [P] [US4] Previews in `crates/stash-core/src/previews.rs`: fetch `paths.webp` (API key in
  a header) through the disk cache as `preview:<id>`, decode the animated WebP with the `image`
  crate into RGBA frames at card width, off the UI thread
- [X] T075 [P] [US4] Sprites in `crates/stash-core/src/sprites.rs`: fetch `paths.vtt` and
  `paths.sprite` once (cached as `sprite:<id>`), parse the VTT, `lookup(time) -> Option<Rect>`
- [X] T076 [US4] Preview service in `native-ui/src/services/previews.rs`: one preview at a time,
  cancelled when the pointer or focus leaves; effects `StartPreview`, `StopPreview`
- [X] T077 [US4] Hover and focus previews on grid and list cards in
  `native-ui/src/screens/scenes/preview.rs` (after a 200 ms rest; the frame replaces the still in place; a frame-timer
  subscription only while playing, so idle CPU is unchanged)
- [X] T078 [US4] Wall mode in `native-ui/src/screens/scenes/wall.rs` and
  `native-ui/src/screens/scenes/layout.rs` (dense stills, windowed like the grid, hovered one
  animates; keyboard moves like the grid's), the mode switch in `native-ui/src/screens/scenes/controls.rs`, saved with the tab in
  `native-ui/src/session/snapshot.rs`
- [X] T079 [US4] Seek-bar thumbnail and time on hover in `native-ui/src/player/controls.rs` (crop of
  the sprite sheet's image handle)
- [X] T080 [US4] Bench rows in `native-ui/src/measure/bench.rs` and
  `crates/perf-harness/src/report.rs`: preview start (≤ 300 ms after the 200 ms hover delay on LAN),
  wall scroll of a 250-scene page within the grid's smoothness budget (SC-007)
- [X] T081 [US4] Run quickstart V4 with the user, then the gate

**Checkpoint**: US1–US4 work on their own.

---

## Phase 7: User Story 5 - A visible fallback when a file can't play (Priority: P5)

**Goal**: an explicit, marked, logged transcoded stream only after a decode failure.

**Independent Test**: quickstart V5 on Testing.

### Tests for User Story 5 ⚠️ write first, check they fail

- [X] T082 [P] [US5] Player tests in `crates/player/src/commands.rs` (`#[cfg(test)]`): the
  direct-stream guard rejects a non-direct URL without the `transcoded` flag and accepts it with
  the flag; a decode failure is classified apart from a network failure
- [X] T083 [P] [US5] Transition tests in `native-ui/tests/playback.rs`: a decode error offers "Play
  transcoded" (a network error doesn't); choosing it opens the transcode with the badge; a
  transcode that also fails shows both reasons and doesn't retry; a normal scene never opens a
  transcode

### Implementation for User Story 5

- [X] T084 [US5] The `transcoded` open flag through the guard in `crates/player/src/commands.rs` and
  error classification in `crates/player/src/error.rs`
- [X] T085 [P] [US5] The transcode URL (`/scene/<id>/stream.mp4` from the profile's base URL, API key
  in a header) in `crates/stash-core/src/adapter/scenes.rs`
- [X] T086 [US5] The offer, the "Transcoded" badge, and the log line (scene id and mpv's reason, no
  title or path) in `native-ui/src/session/playback.rs` and `native-ui/src/player/view.rs`
- [X] T087 [US5] Run quickstart V5 with the user on Testing, then the gate

**Checkpoint**: all five stories work.

---

## Phase 8: Polish & cross-cutting concerns

- [ ] T088 [P] Update `README.md` and `ROADMAP.md` (browse and play done for scenes; semantic
  recommendations and tag colour editing still ahead)
- [ ] T089 [P] Add a requirement-to-test table (FR-001–FR-026, SC-001–SC-009 → test names and
  harness rows) to [quickstart.md](quickstart.md)
- [ ] T090 Full perf-harness run on Testing: every 007 row still passes (SC-009) and every `gql-*`
  row is within budget; record the summary in [quickstart.md](quickstart.md)
- [ ] T091 Final gate: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test --workspace`

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (Phase 1)**: none.
- **Foundational (Phase 2)**: after Setup; blocks every story.
- **US1 (P1)**: after Foundational. The MVP.
- **US2 (P2)**: after Foundational. Two touch points with US1: T035 (chips open filtered Scenes)
  lives in US1, and T052's semantic chip marking reads the vocabulary from T027. Without US1 the
  page still works with unmarked chips. The colour manager (T049–T051) needs T009's colour rules
  and T027's rule aliases, and writes Stash's UI configuration, so check it on Testing only.
  The picture versions run in order T056b → T056c → T056d (the UI keys need the core's
  versions); T056b changes T026's performer list, so its payload row grows once.
- **US3 (P3)**: after Foundational (uses T011's start position and T009's `minimumPlayPercent`).
  Chapters read markers from US2's `SceneDetails` (T043); without US2 the seek bar has no
  chapters.
- **US4 (P4)**: after Foundational; reads `paths.webp`/`vtt`/`sprite` (T043 in US2 adds them to
  `SceneDetails`; the card tier would need them otherwise, so do US2 first).
- **US5 (P5)**: after Foundational; independent of the others.
- **Polish**: after the stories you ship.

### Within each story

Tests first (failing) → core models → core operations (with golden files) → effects and services
→ UI states → views → bench rows → quickstart check with the user → gate.

### Parallel opportunities

- Setup: T002 and T003.
- Foundational: T008, T009, T010, T011 together after T004–T007.
- Each story's test tasks marked [P] together.
- US1: T026 and T027 (choice lists and vocabulary) together; T030 and T034 alongside the state
  work.
- US2: T044, T045, T048, T049 together.
- US3: T063, T064, T069 together.
- US4: T074 and T075 together.
- With more than one developer, US5 can run beside any other story.

## Parallel Example: User Story 1

```bash
# Tests together:
T013 criterion conversion   (crates/stash-core/tests/scene_filters.rs)
T015 choices and vocabulary (crates/stash-core/tests/choices.rs)
T016 request counts         (crates/stash-core/tests/query_budget.rs)
T017 find transitions       (native-ui/tests/scenes_find.rs)
T018 snapshot round-trip    (native-ui/tests/snapshot.rs)

# Then core operations together:
T026 choice lists           (crates/stash-core/src/adapter/choices.rs)
T027 semantic vocabulary    (crates/stash-core/src/semantic.rs, adapter/semantic.rs)
```

## Implementation Strategy

### MVP first (US1 only)

1. Phase 1 and Phase 2 (the query-budget machinery is what every later check stands on).
2. US1: search, filters, pickers, saved and default filters.
3. Stop: run quickstart V1 and the `gql-*` rows with the user; commit when they're happy.

### Incremental delivery

US1 → US2 → US3 → US4 → US5, each ending with its quickstart check, the gate, and a commit after
the user has looked. Every story keeps every earlier budget passing (SC-009).
