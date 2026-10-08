# Data Model: Finish Browse and Play (008)

Builds on 007's model (screens as whole states in tab histories, effects as data). New and changed
entities only. Field lists for Stash data are exactly the selections in
[contracts/query-budget.md](contracts/query-budget.md).

## Core (stash-core)

### SceneQuery (changed)

| Field | Rules |
|---|---|
| `search` | Trimmed; empty means none (as 007) |
| `sort`, `direction`, `seed` | As 007 |
| `criteria: Vec<Criterion>` | New; each kind at most once except custom fields; order kept for chips |

Normalised form (cache key): criteria sorted by kind; empty search and empty criteria drop out.
The cache key hashes the normalised query (as 007's `cache_hash`), so the same filter shares
cached pages.

### Criterion

`{ kind: CriterionKind, modifier: Modifier, value: CriterionValue }`.

- `CriterionKind`: one per v0.31.1 `SceneFilterType` field the web UI offers (research R3), plus
  the `Semantic(SemanticKind)` placeholders (never sent).
- `Modifier`: `Equals, NotEquals, GreaterThan, LessThan, Between, NotBetween, Includes,
  IncludesAll, Excludes, IsNull, NotNull, MatchesRegex, NotMatchesRegex` (each kind lists the ones
  the web UI offers for it).
- `CriterionValue`: `Text(String) | Int(i64) | IntRange(i64, i64) | Duration(f64) | Date(..) |
  DateRange | Timestamp | Bool | Resolution(ResolutionEnum) | Orientation(Set) |
  Items { items: Vec<Item { id, label }>, excluded: Vec<Item>, depth: Option<i32> } |
  StashId { endpoint, id } | CustomField { field, value }`.
- Validation: the modifier is one the kind allows; `Between` has two values; `IsNull`/`NotNull`
  carry no value; `Items` are non-empty unless the modifier is a null check.
- `labels` on items are display-only (from the choice list or a saved filter); the request sends
  ids.

### SavedFilter (read-only)

`{ id, name, query: SceneQuery, page_size: Option<u32>, mode: Option<Mode>, left_out: Vec<String> }`
decoded from Stash's `find_filter` + `object_filter` (saved criterion format) + `ui_options`;
`left_out` names keys the viewer didn't recognise (FR-005).

### UiSettings (per session; the colour rules are written by Settings → Tag colours)

`{ default_scene_filter: Option<SavedFilter>, minimum_play_percent: f64 (default 0),
web_track_activity: bool, colour_rules: Vec<ColourRule> }` from `configuration { ui }`.

### ColourRule

`{ field: String (a tag custom field key: letters, digits, _; 1–64 characters), colour: Rgb }`,
ordered; stored as `semanticStashViewer.tagColours = { version: 1, rules: [...] }`. When absent,
the defaults: `isLabel`, `isAction`, `isRole`, `isTrait`, `plugtag`, `isMiseEnScene`,
`isVariation`, each with its RPG palette colour (contract). Invalid rules are skipped.

### ChoiceList

`{ kind: Performers | Tags | Studios | Groups | Galleries, entries: Vec<Choice { id, name,
aliases: Vec<String>, image: Option<String> }>, fetched_at }` (`image`: performers only, the
picture version from `image_path`'s `t`; `None` when the path says `default=true` or for other
kinds; R13) (a group's single `aliases` string is split on commas), cached per server (`choices:<kind>`). Search is local:
case-insensitive and accent-folded over name and aliases.

### SemanticVocabulary

`{ tags: map id → SemanticTag { name, aliases, kind: SemanticKind }, variants: map variant id →
VariantTag { name, aliases, canonical id }, rule_matches: map rule field → set of tag ids,
fetched_at }`, cached per
server (`semantic:vocabulary`).

- `SemanticKind`: `Pluggable (plugtag) | Trait (isTrait) | Role (isRole) | Action (isAction) |
  Label (isLabel) | MiseEnScene (isMiseEnScene) | Variation (isVariation)`.
- A flag is set when its value is 1, `"1"`, `true`, or `"true"`; any other value means not set.
- A tag with several flags takes one kind by a fixed priority order (the plugin's: Label over
  Action over Role over Trait over Pluggable; MiseEnScene and Variation, which the plugin doesn't
  rank, come below Pluggable).
- `kind_of(tag id)`: the tag's own kind, else its canonical tag's kind through `variants`, else
  none (ordinary).
- The Tags ChoiceList holds ordinary tags only (every flag null, filtered by Stash); the picker
  shows the vocabulary grouped by kind above it. A vocabulary entry whose flag isn't set (for
  example 0), and a variant whose canonical tag has no kind, are shown as ordinary tags.
- `colour_of(tag id, rules)`: the colour of the first rule (in order) whose flag the tag has set
  (`1`, `"1"`, `true`, `"true"`), else, for a common variant, its canonical tag's colour, else
  none ([contracts/tag-colour.md](contracts/tag-colour.md)).

### SceneDetails (changed: detail tier)

The fields of `SceneDetails` in the contract: details, code, director, date, rating, organized,
play count, resume time, play duration, URLs, studio, performers (`PerformerChip { id, name,
image: Option<String> }`, the picture version as in `Choice`; R13), tags,
groups (with scene index), markers (id, title, seconds, end seconds, primary tag name), the
primary file's facts, and paths (screenshot, webp, vtt, sprite). Cached as `scene:details:<id>`
and patched after play-history saves.

### SceneFiles, SceneGalleries, ChipDetails

Loaded on demand (contract), cached as `scene:files:<id>`, `scene:galleries:<id>`,
`chip:<kind>:<id>`.

### PlayHistoryReport (written)

`{ scene_id, resume_time: f64 (0 when ≥ 98% complete), play_duration_delta: f64, add_play: bool,
ended: bool, recorded_at }`. Queued on disk per profile until Stash confirms it
([contracts/play-history.md](contracts/play-history.md)).

## UI (native-ui)

### ScenesState (changed)

Adds to 007's fields (all saved with the tab, session file version 2):

| Field | Rules |
|---|---|
| `query.search`, `query.criteria` | As SceneQuery |
| `applied_saved_filter: Option<(id, name)>` | Shown above the chips; cleared when the user edits the criteria |
| `mode` | `Grid | List | Wall` (Wall new) |

Transient: `search_draft` (typed text before the debounce), `picker: Option<PickerState>`,
`hovered: Option<(card index, PreviewState)>`.

### PickerState (transient)

`{ kind, query: String, selection: Vec<Item>, modifier }` over the cached ChoiceList; `Loading`
until the list arrives the first time.

### PreviewState (transient)

`Waiting { since } → (200 ms rest) Loading → Playing { frames, frame index, next at } → (pointer leaves) gone`.
Only one exists at a time across the session (clarification Q5).

### SceneState (changed)

Adds: `details` with the full detail tier; `files: Section<Vec<FileFacts>>`, `galleries:
Section<Vec<GalleryChip>>` where `Section = Closed | Loading | Ready | Failed`; `chip_hover:
Option<(ChipRef, since)>`; `chip_cards: map of fetched ChipDetails` (session cache view).

### Viewing (playback region, changed)

`{ scene_id, started_at, playing_since: Option<Instant>, played_seconds, played_since_save,
add_play_sent, last_checkpoint }` driving checkpoints (once a minute of actual playing) and the
end-of-viewing report; resume start and the "start from the beginning" offer (shown 8 s) live in the
playback region.

### PlaybackPreferences

`{ resume: bool (default true), save_history: bool (default true) }`, saved in the app's
settings file (config folder), Settings → Playback.

### TagColoursPage (Settings → Tag colours)

`{ rules: Vec<ColourRule> (being edited), counts: map field → number of tags matched, editing:
Option<rule index>, saving: Idle | Saving | Failed(message) }`. Saving sends the whole list in one
write; on success the cached UI settings are patched and every view recolours.

## State transitions (key ones)

- **Search**: typing updates `search_draft`; 300 ms after the last keystroke the query's search
  is set, page goes to 1, and one `LoadScenesPage` is emitted; results for an older generation are
  dropped (FR-008, as 007).
- **Criterion change** (add, edit, remove a chip, clear, apply a saved filter): query changes,
  page 1, one `LoadScenesPage`.
- **Viewing**: `Opening → Playing ⇄ Paused → Ended`; every 60 s of `Playing` time a checkpoint
  report; on `Ended` (any cause) a final report; `add_play` set on the first report whose watched
  share passes the threshold.
