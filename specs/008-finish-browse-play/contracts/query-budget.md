# Contract: Query budget (GraphQL performance QA)

Every request this feature makes to Stash, what it may select, and how that's checked
([research R1, R2](../research.md)). Constitution Principle IV: no N+1, each operation selects only
what its view shows, and every new operation's request count and payload is reviewed here.

## Operations

Each operation is named (the name the recorder and the harness report). Selections are the
complete list: anything not listed MUST NOT be selected.

### `ScenesPage` (card tier; exists since 005, gains the filter)

Variables: `filter: FindFilterType` (q, page, per_page, sort, direction), `scene_filter:
SceneFilterType` (the criteria).
Selects: `count`, and per scene `id title date files{basename duration width height}
studio{name} paths{screenshot}` (007's card fields, unchanged).
Budget: **1 per page shown**, whatever the page size, search, or criteria. Neighbour prefetch: 1
per neighbour (as 007).

### `SceneDetails` (detail tier; replaces 007's)

Variables: `id`.
Selects: `id title code details director date rating100 organized play_count resume_time
play_duration urls studio{id name} performers{id name image_path} tags{id name}
groups{group{id name} scene_index} scene_markers{id title seconds end_seconds primary_tag{name}}
files(first file only){basename duration width height video_codec audio_codec format frame_rate
bit_rate size} paths{screenshot webp vtt sprite}`.
Must not select: galleries, every file's path, performer details beyond id/name/image path, tag
images or descriptions, markers' scenes or streams, `sceneStreams`, `paths.stream`.
Budget: **1 per scene page opened** (0 when cached and fresh); the title, cover, and Play show
before it from the card.

### `SceneFiles` / `SceneGalleries` (opened sections)

`SceneFiles`: `files{id basename path size duration width height video_codec audio_codec format
frame_rate bit_rate fingerprints{type value}}`.
`SceneGalleries`: `galleries{id title image_count}`.
Budget: **1 each, only when the section is opened**.

### `PerformerCard` / `TagCard` / `StudioCard` (chip details)

`PerformerCard(id)`: `id name disambiguation gender birthdate country favorite scene_count`.
`TagCard(id)`: `id name description scene_count`.
`StudioCard(id)`: `id name parent_studio{name} scene_count`.
Budget: **1 per item per session**, after the pointer or focus rests 400 ms; none for chips only
passed over.

### `ChoiceList<Kind>` (picker lists)

`ChoiceListPerformers`: `findPerformers(filter:{per_page:-1, sort:"name"}){performers{id name
alias_list}}`; likewise `Studios` (`id name aliases`), `Groups` (`id name aliases`, one string split on commas), `Galleries`
(`id title`).
`ChoiceListTags` (ordinary tags only): `findTags(filter:{per_page:-1, sort:"name"},
tag_filter:{custom_fields:[{field:"plugtag", modifier:IS_NULL}, … one per type flag,
{field:"variantOfTagId", modifier:IS_NULL}]}){tags{id name aliases}}`. Common variants are left
out here because they arrive in the vocabulary (they are still offered in the picker).
Budget: **0 when cached**; otherwise **1 per kind per server**, then only when the cache refreshes
it.

### `SemanticVocabulary` (semantic tags; FR-003c)

One request with one aliased `findTags(filter:{per_page:-1}, tag_filter:{custom_fields:{field:
"<flag>", modifier:NOT_NULL}}){tags{id name aliases custom_fields}}` per flag: `plugtag`,
`isTrait`, `isRole`, `isAction`, `isLabel`, `isMiseEnScene`, `isVariation`, and `variantOfTagId`
(common variants), plus one `rule_<n>: findTags(… {field:"<field>", modifier:NOT_NULL}){tags{id
custom_fields}}` per colour rule whose field isn't a semantic flag ([contracts/tag-colour.md](tag-colour.md)).
`custom_fields` is selected only here, to read the flag value (1, `"1"`, `true`, `"true"` count as
set) and the canonical id.
Must not: fetch the full tag list to find semantic tags (constitution IV), or select tag images,
descriptions, parents, or children.
Budget: **0 when cached**; otherwise **1 per server**, refreshed with the tag choice list. Scene
chips are typed from this cache with no request.

### `SavedSceneFilters`, `UiConfig`

`SavedSceneFilters`: `findSavedFilters(mode: SCENES){id name find_filter{q page per_page sort
direction} object_filter ui_options}`.
`UiConfig`: `configuration{ui}` (read keys: `defaultFilters`, `minimumPlayPercent`,
`trackActivity`, `semanticStashViewer.tagColours`).
Budget: **0 when cached**; otherwise **1 per session** each, plus the cache's quiet refresh on
its schedule (so a changed default filter reaches new tabs without a restart).

### `SaveTagColours` (a settings write)

`configureUISetting(key: "semanticStashViewer.tagColours", value: {version, rules})`.
Budget: **1 per save** in Settings → Tag colours, then 1 `SemanticVocabulary` refetch if a rule's
field was added or changed (0 for a colour change only).

### `SaveActivity`, `AddPlay` (the play-history writes)

`SaveActivity`: `sceneSaveActivity(id, resume_time, playDuration)`.
`AddPlay`: `sceneAddPlay(id){count}`.
Budget per viewing: `SaveActivity` once per minute of actual playing plus once at the end (a
10-minute viewing: at most 11); `AddPlay` at most once. None when "Save play history" is off.

### Plain GETs (not GraphQL)

Thumbnails (as 007); performer chip images (resized, cached); the hover preview (`paths.webp`,
cached); the VTT and sprite (`paths.vtt`, `paths.sprite`, cached); covers (as 007). Each is
fetched at most once per item until the cache drops it, with the API key in a header.

## Checks

| Check | Where | Fails when |
|---|---|---|
| Selection snapshot | stash-core test per operation, against `crates/stash-core/graphql/operations/<Name>.graphql` | an operation's text differs from its golden file |
| Tier rules | stash-core test | a card or chip operation selects a forbidden field |
| Writes | stash-core test | any golden file other than `SaveActivity`, `AddPlay`, and `SaveTagColours` contains `mutation` (FR-018) |
| Request counts | stash-core tests on a mock server, native transition tests on effects | an action makes more requests (or emits more load effects) than its budget |
| Live audit | perf-harness rows, Testing (read-only except the simulated viewing's writes on Testing) | a scenario exceeds its request budget; payload sizes are recorded and compared with the previous report (more than 20% larger is flagged) |

## Live audit rows

`gql-page-50`, `gql-page-1000`, `gql-filter-apply`, `gql-scene-page`, `gql-scene-files`,
`gql-scene-galleries`, `gql-chip-rest`, `gql-chip-sweep` (expected 0), `gql-picker-cold`,
`gql-picker-cached` (expected 0), `gql-vocabulary-cold` (1), `gql-tag-colours-save` (1 + at most 1), `gql-tags-picker-cold` (≤ 2:
ordinary tags plus the vocabulary), `gql-viewing-10min-writes` (≤ 11 + 1). Each reports requests,
response bytes, and server time (median, p95).
