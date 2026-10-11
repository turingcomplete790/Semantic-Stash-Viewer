# Contract: Tag colour rules

How tags get colours, where the colours are kept, and what each read and write costs (spec
FR-003d, FR-003e, FR-003g; constitution Principles II, IV, and XI; [research R12](../research.md)).

Colours belong to **tag flags**, not to tags. A rule says "tags with the custom field `isRole`
set are purple". Nothing is ever written onto a tag: a colour per tag would mean scanning tags to
find their colours, while a rule finds exactly its tags with one Stash-side "flag is not null"
filter.

## Where the rules live

Stash's UI configuration (`configuration { ui }`), key `semanticStashViewer.tagColours`:

```json
{ "version": 1,
  "rules": [ { "field": "isLabel",  "colour": "#5b8fd4" },
             { "field": "isAction", "colour": "#d44040" },
             … ] }
```

- `rules` is ordered; a tag matching several rules takes the **first**.
- `field`: a tag custom field key (letters, digits, `_`; 1–64 characters). `colour`: `#rrggbb`
  (lower case when written; read case-insensitively).
- A rule whose `field` or `colour` is invalid is skipped and logged once per session at warn
  level (field name only). A missing key, or an unreadable value, means the default rules.

## Default rules (when the key is absent)

The seven tag types of the design documents (roadmap Milestone 1D §4), in the plugin's current
type priority (an implementation note: the documents don't rank types), highest first, with
colours from the RPG palette (constitution Principle XI):

| Order | Type | Field | Colour |
|---|---|---|---|
| 1 | Label | `isLabel` | `#5b8fd4` (blue) |
| 2 | Action | `isAction` | `#d44040` (red) |
| 3 | Role | `isRole` | `#8840c0` (purple) |
| 4 | Trait | `isTrait` | `#3a9e50` (green) |
| 5 | Pluggable | `plugtag` | `#c8952a` (gold) |
| 6 | Mise-en-scène | `isMiseEnScene` | `#b08858` (leather) |
| 7 | Variation | `isVariation` | `#7dd87a` (light green) |

## Matching

- A tag matches a rule when its custom field `field` is set: `1`, `"1"`, `true`, or `"true"` (the
  plugin's test). `0`, `"0"`, `false`, or null don't match.
- A common variant (`variantOfTagId`) that matches no rule itself takes its canonical tag's colour.
- A tag matching no rule has the app's normal chip style.
- Chip text is `#0d0a05` or `#e8e0c8`, whichever contrasts more; when neither reaches 4.5:1 (the
palette's red `#d44040` reaches 4.33 and purple `#8840c0` 4.498), pure black or white, whichever
contrasts more (red: black 4.61; purple: white 5.93).

## Reading (no extra requests)

- **The rules**: in the `UiConfig` request the viewer already makes (once per session, cached as
  `ui:settings`, quietly refreshed).
- **Which tags each rule colours**: in the `SemanticVocabulary` request. The semantic flags
  already have an alias each; every other rule adds one alias `rule_<n>: findTags(filter:
  {per_page:-1}, tag_filter:{custom_fields:{field:"<field>", modifier:NOT_NULL}}){tags{id
  custom_fields}}` (ids and the flag value only). Whenever the set of rule fields changes (saved
  here, or picked up from another machine or the plugin by the quiet refresh of the UI
  settings), the vocabulary is refetched once; a colour-only change refetches nothing.

## Writing (Settings → Tag colours only)

- `SaveTagColours`: `configureUISetting(key: "semanticStashViewer.tagColours", value: <the
  whole object>)`. It replaces only that key (Stash sets the nested key and keeps the rest of the
  UI configuration), so removing a rule really removes it.
- One write per save; on success the cached `ui:settings` is patched and every open view
  recolours; on failure the previous rules stay and the page says the colours weren't saved.

## Plugin adoption

The design documents colour chips by type (roadmap Milestone 1D §4), and the plugin does so
today with fixed colours of its own. To show the same colours it
reads `semanticStashViewer.tagColours` from the UI configuration (one read, no tag scan) and falls
back to its own colours when the key is absent.
