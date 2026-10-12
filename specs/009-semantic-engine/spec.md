# Feature Specification: Semantic Engine

**Feature Branch**: `009-semantic-engine`

**Created**: 2026-10-11

**Status**: Draft

**Input**: User description: "semantic-engine with all recommendations from speckit-analyze" (the
2026-10-11 roadmap analysis: build the semantic engine first, as roadmap Phase 4a, from the plugin's
design documents rather than by porting the plugin's code)

## Overview

The viewer exists to be the best home for semantic tagging (constitution Principle II). Every
semantic feature still to come (the Search Matrix, Quick Pairings, recommendations, Label
evaluation, archetype inheritance, and later the query language, CONSTRUCT, and rules) must be a
query or rule on one **semantic engine** (Principle XII), not a separate hand-written filter. This
feature builds that engine and nothing that depends on it:

- **A graph of semantic facts built from the library**: scenes and images, performers, the
  semantic tag types, and the relationships the plugin's design documents define.
- **Kept current and cheap to start**: built once, in the background, with progress; kept up to
  date as the library changes; reused on the next launch instead of rebuilt.
- **Answering queries**: SPARQL-style `SELECT` queries in the documents' vocabulary, giving the
  answers the documents define.
- **Visible and manageable**: a Settings page shows what the engine holds, lets the user rebuild or
  clear it, and offers a query box to ask it directly.

The engine is a derived index (Principle I): Stash stays the system of record, nothing is written
to Stash by this feature, and deleting the engine's data loses nothing.

**Sources** (Principle II: design documents, not code). Read at plugin commit `3c82c85`
(2026-09-10) in `~/Projects/Stash-Plugins/semantic-tagging/`:

- `semantic-query-engine-roadmap.md`: "Vision", "Target Architecture", "Core Design Decisions §1
  Stash remains the system of record" and "§2 The semantic graph is the abstraction boundary"
  (including its "Vocabulary corrections"), "Query Language Direction", "Query Features: Phase 1 —
  Minimal", "Where To Start: Step 4 — Build the semantic index", "Performance Strategy" (including
  "Preserve the lessons from the existing plugin"), "Milestone 1 — Semantic Core", "Milestone 1D —
  Variation System & Label Overhaul", and "Milestone 2 — Query Engine".
- `archetype-system-plan.md`: "Data model" and "Hierarchy index — inheritance algorithm" (the
  archetype parent relationship the graph carries).
- `README.md`: "Features" (the semantic tag types and what each is attached to).

Where the plugin's code and these documents disagree, the documents win and the difference is
noted for the plugin.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - The engine builds from the library and stays current (Priority: P1)

The first time the user connects to a server after this feature, the viewer builds the semantic
engine's graph in the background. The notification centre shows the build's progress with a
Cancel button, and the app stays fully usable meanwhile. On later launches the engine is ready
almost at once from its stored copy. When tags, performers, scenes, or images change (in the
viewer, the Stash web UI, or the plugin), the engine catches up without a full rebuild.

**Why this priority**: Nothing semantic can be built until there is a graph to query, and a graph
that's slow to start or goes stale would make every later feature slow or wrong.

**Independent Test**: Connect to a server with a semantically tagged library: the build runs with
progress, finishes, and the Settings page shows its counts. Relaunch: the engine is ready within
the start-up budget without rebuilding. Change a tag assignment in the Stash web UI: the engine
reflects it within the freshness target.

**Acceptance Scenarios**:

1. **Given** a server the engine hasn't seen, **When** the viewer connects, **Then** the build
   starts in the background, the notification centre shows its progress (items processed of the
   total), and browsing, playback, and every other screen stay within their budgets.
2. **Given** a build in progress, **When** the user chooses Cancel, **Then** it stops within 2
   seconds, nothing half-built is used, and the engine page says it isn't built yet, with a
   button to build it.
3. **Given** a built engine, **When** the viewer is restarted and reconnects, **Then** the engine is
   ready from its stored copy within the start-up budget, with no full rebuild.
4. **Given** a built engine, **When** a performer gains a semantic tag, a scene's Role or Action
   assignments change, or a tag's flag changes (anywhere: viewer, web UI, or plugin), **Then** the
   engine reflects it within the freshness target, by updating only what changed.
5. **Given** two saved servers, **When** the user switches between them, **Then** each has its own
   engine, never mixed with the other's facts.

---

### User Story 2 - Ask the engine a question (Priority: P1)

On Settings → Semantic engine, the user types a SPARQL-style `SELECT` query in the documents'
vocabulary (for example: scenes where one performer has the Role `:Dom` and another `:Sub`) and
gets the bindings as a table, where scenes, images, performers, and tags open their pages. A
query the engine can't run says why, pointing at the problem.

**Why this priority**: It's the only way to see and check what the engine knows before the
features built on it exist, and the first, smallest form of the documents' general semantic
search.

**Independent Test**: Run the documents' three example queries ("Basic query", "Multiple
relationships", "Multiple bound performers") against a library tagged with those Roles and Traits:
each returns exactly the matching scenes and performers; a query with a typo returns an error
naming the line and what's wrong.

**Acceptance Scenarios**:

1. **Given** a built engine, **When** the user runs the documents' "Basic query"
   (`?scene :hasPerformer ?person . ?person :hasRole :Dom .`), **Then** the result lists every
   scene or image in which some performer has the Role Dom, and only those.
2. **Given** the documents' "Multiple bound performers" query with `FILTER (?dom != ?sub)`,
   **When** it runs, **Then** it returns only scenes where two different performers hold the two
   Roles in that same scene, never a pairing assembled from two different scenes.
3. **Given** a result row, **When** the user chooses a scene, image, performer, or tag in it,
   **Then** that item's page opens (in a new tab with Ctrl, as elsewhere).
4. **Given** a query with a syntax error or an unknown term, **When** it runs, **Then** no results
   show, and a message names the position and the problem (for example an unknown predicate,
   suggesting the closest known one).
5. **Given** a query whose results are large, **When** it runs, **Then** the first results show
   within the query budget and the rest page in, without freezing the window.

---

### User Story 3 - The engine gives the answers the documents define (Priority: P2)

For a library the plugin has tagged, the engine holds the same facts the design documents
describe and answers queries the way they define, so later features built on it can be trusted
and the viewer and plugin can be used on the same library.

**Why this priority**: Correctness is what makes the engine worth building, but it's checked
mostly by fixtures and tests rather than by a new screen, so it follows the two stories that make
the engine visible.

**Independent Test**: Load a scrubbed fixture captured read-only from a plugin-tagged library and
run a set of queries whose expected answers are derived from the documents: every answer matches.

**Acceptance Scenarios**:

1. **Given** the fixture, **When** the engine is built from it, **Then** every semantic fact the
   documents define for that data is present (media entities, performers, the six flagged tag
   types and Variations, Role and Action instances with their variation and time, categories,
   tag variations, parent tags, Label formulas, archetype parents) and nothing else.
2. **Given** a scene-scoped predicate (`hasRole`, `hasAction`, `hasPluggable`), **When** a query
   binds it, **Then** it only matches within the scene or image the performer was bound through,
   as the documents' "Vocabulary corrections" require.
3. **Given** a tag flagged with a value of 0, or a common variant (`variantOfTagId`), **When** the
   graph is built, **Then** they're treated as the documents define (a 0 flag isn't that type; a
   variant points at its canonical tag).
4. **Given** the same library state, **When** the plugin's current code would answer a query
   differently from the documents, **Then** the engine follows the documents and the difference is
   recorded for the plugin.

---

### User Story 4 - See and manage the engine (Priority: P3)

The Settings → Semantic engine page shows, for the current server: whether the engine is built,
current, updating, or stale; how many facts it holds by kind (scenes, images, performers, each tag
type, assignments); when it last updated; and the space its stored copy uses. The user can rebuild
it or clear it; clearing the app's cache clears it too.

**Why this priority**: Useful for trust and troubleshooting, but the engine works without anyone
opening this page.

**Independent Test**: Open the page on a built engine: the counts match the library; choose
Rebuild: it rebuilds with progress; choose Clear: the stored copy is gone and the page offers to
build it again.

**Acceptance Scenarios**:

1. **Given** a built engine, **When** the page opens, **Then** it shows the state, the counts by
   kind, the last update time, and the stored size, without asking Stash for anything.
2. **Given** the stored copy is missing, damaged, or from an older format, **When** the viewer
   starts, **Then** it rebuilds in the background (with progress), and the page says why.
3. **Given** the user clears the cache in Settings → Troubleshooting, **When** it's done, **Then**
   the engine's stored copy for that server is gone too, and it rebuilds on next use.

---

### Edge Cases

- **A library with no semantic tagging**: the graph holds the media entities and performers with
  no semantic facts; queries over semantic predicates return nothing (not an error); the page says
  no semantic tags were found.
- **Malformed semantic data** (a `custom_fields` value that doesn't parse, an assignment pointing
  at a deleted tag): that one fact is skipped and counted on the engine page ("N facts couldn't be
  read"), and the rest of the graph is built.
- **Offline**: the stored engine still answers queries; it catches up when the connection returns.
- **The server changes a lot at once** (a bulk edit or a plugin migration): the engine catches up
  in the background; if catching up would cost more than rebuilding, it rebuilds, with progress.
- **A build interrupted by quitting or a server switch**: nothing half-built is kept; the build
  starts again next time.
- **A very large query result**: results page in; a query can be stopped; a query taking longer
  than 30 seconds is stopped with a message.
- **Two viewers or the plugin editing at the same time**: the engine never writes to Stash, so it
  can't conflict; it reflects edits as they land.
- **Not enough disk for the stored copy**: the engine still works in memory for the session, and
  the page says it couldn't be stored.

## Requirements *(mandatory)*

### Functional Requirements

**The graph (US1, US3)**

- **FR-001**: The engine MUST hold a graph of semantic facts built from the current server's
  library, using the vocabulary in the design documents' "§2 The semantic graph is the
  abstraction boundary" with its "Vocabulary corrections" and the Milestone 1D changes: Scene and
  Image as media entities; performers (archetypes as performers); the tag types Role, Action,
  Pluggable, Trait, Label, MiseEnScene, and Variation; Role and Action assignments as instances
  carrying their tag, chosen variation, and timeline time; tag categories and declared variations;
  parent tags; Label formulas (threshold and weighted Trait, Field, and Label-dependency rules);
  and archetype parents.
- **FR-002**: Facts MUST keep the documents' scopes: Role, Action, and Pluggable assignments and
  Mise-en-scène are tied to one scene or image; Traits, Labels, and Variations held by a performer
  are global to that performer; a Variation's facts sit on its own tag.
- **FR-003**: The engine MUST read the shared data contract exactly as the documents define it:
  the per-type tag flags (`plugtag`, `isAction`, `isRole`, `isTrait`, `isLabel`, `isMiseEnScene`,
  `isVariation`, as 0/1 values, where only 1 means that type), common variants
  (`variantOfTagId`), and the assignment and formula data in `custom_fields`.
- **FR-004**: Discovery MUST fetch only the semantic vocabulary and the data the graph needs, with
  flag-filtered queries evaluated by Stash ("Preserve the lessons from the existing plugin";
  Principle IV), never by reading every tag; requests come in bulk, never one per item.
- **FR-005**: A fact that can't be read MUST be skipped and counted, never stop the build or the
  rest of the graph.

**Building and staying current (US1, US4)**

- **FR-006**: The engine MUST build in the background, off the window's work, with progress (items
  done of the total) shown in the notification centre and a Cancel control.
- **FR-007**: A cancelled or interrupted build MUST leave no partial graph in use.
- **FR-008**: The engine MUST keep a stored copy per server profile in the app's cache folder,
  reuse it on the next launch, and rebuild it when it's missing, damaged, or from an older format.
- **FR-009**: The engine MUST catch up with library changes by updating only what changed, within
  the freshness target (SC-004), using the same lean queries as FR-004; when catching up would
  cost more than a rebuild, it MUST rebuild instead.
- **FR-010**: Each server MUST have its own engine; switching servers never mixes facts.
- **FR-011**: Clearing the app's cache for a server (Settings → Troubleshooting) MUST clear that
  server's stored engine too.
- **FR-012**: The engine MUST NOT write to Stash.

**Queries (US2, US3)**

- **FR-013**: The engine MUST answer SPARQL-style `SELECT` queries with triple patterns, variables,
  constants, joins, projection, and basic filters (the documents' "Query Features: Phase 1 —
  Minimal" and Milestone 2), in the documents' vocabulary, using the documents' prefix
  conventions (`:hasRole`, `:Dom`).
- **FR-014**: Scene-scoped predicates MUST only match within the scene or image their performer
  was bound through, so a query never pairs facts from different scenes.
- **FR-015**: Named terms in a query (`:Dom`, `:Blonde`) MUST resolve to the library's tags and
  performers by name (and alias), as in the documents' examples; an ambiguous or unknown name MUST
  be reported, not guessed.
- **FR-016**: A query that can't run MUST return a message naming the position and the problem,
  suggesting the closest known term for an unknown one.
- **FR-017**: Results MUST page in, a running query MUST be stoppable, and a query running over 30
  seconds MUST stop with a message.
- **FR-018**: Every later semantic feature MUST be able to ask the engine through this one query
  path; this feature adds no second, hand-written matching path.

**Settings → Semantic engine (US2, US4)**

- **FR-019**: A Settings → Semantic engine page MUST show the current server's engine state (not
  built, building, current, catching up, stale, stored in memory only), the facts by kind, facts
  that couldn't be read, the last update time, and the stored copy's size, without a request to
  Stash.
- **FR-020**: The page MUST offer Rebuild and Clear, and a query box that runs a `SELECT` (FR-013)
  and shows the bindings as a table whose scenes, images, performers, and tags open their pages.
- **FR-021**: The page and its query box MUST be keyboard-operable, as the rest of the app.

**Ready for a shared helper later (2026-10-11 middleware analysis)**

- **FR-022**: The engine MUST be usable apart from the viewer's screens, settings, and connection
  handling: given a way to read from Stash and a folder for its stored copy, another program (a
  future helper on the Stash server, roadmap Phase 6) can run the same engine and get the same
  answers. This feature ships no such program.
- **FR-023**: The engine MUST be able to write its stored copy out as a snapshot in a standard,
  documented graph-exchange format, and to start from such a snapshot instead of a full build.
  A snapshot names the server it came from, the engine's format version, and how far through the
  library's changes it reflects, so a loaded snapshot catches up through FR-009 rather than
  rebuilding, and a snapshot from another server or an older format is refused and rebuilt.

### Key Entities *(include if feature involves data)*

- **Semantic graph**: the set of semantic facts for one server, built from its library; derived,
  never the source of truth.
- **Media entity**: a scene or an image; scene-scoped facts hang off it.
- **Semantic tag**: a tag flagged as one of the seven types (Role, Action, Pluggable, Trait, Label,
  MiseEnScene, Variation), with its categories, declared variations, parent tags, and (for Labels)
  formula; common variants point at their canonical tag.
- **Assignment instance**: one performer's Role or Action in one scene or image, with its chosen
  variation and timeline time.
- **Label formula**: a Label's threshold and its weighted rules (Trait, Field, Label-dependency).
- **Stored engine**: the per-server stored copy of the graph, with its format version and the
  point in the library's history it reflects.
- **Query**: a `SELECT` in the documents' vocabulary; its result is a list of bindings.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On the owner's largest library (Production, read-only), the first build finishes
  within 5 minutes, with progress throughout, and browsing and playback stay within every budget
  the constitution sets while it runs.
- **SC-002**: With a stored copy, the engine is ready to answer within 2 seconds of the app
  starting (within the constitution's cold-start budget), with no full rebuild.
- **SC-003**: Queries of the documents' Phase 1 kind (up to five triple patterns) return their
  first results in under 200 ms on that library (the constitution's local semantic query budget).
- **SC-004**: A change made in Stash (by the viewer, the web UI, or the plugin) is reflected in the
  engine within 60 seconds while connected, and at reconnection otherwise.
- **SC-005**: On the plugin-tagged fixture, 100% of the expected answers derived from the documents
  match, including the scene-scoping cases (no cross-scene pairings).
- **SC-006**: Building and keeping the engine current sends Stash only bulk, flag-filtered
  requests: no request per item, and no request that reads every tag, checked by the request
  budget tests and harness rows as in 008.
- **SC-007**: Cancelling a build takes effect within 2 seconds; clearing removes the stored copy at
  once; a damaged stored copy is detected and rebuilt on the next start every time.

## Assumptions

- **The engine choice is the constitution's**: an existing in-process engine (Principle XII names
  Oxigraph, with Redland as a fallback only by amendment), its on-disk store used only as a cache.
  This spec states what the engine must do; the plan records how.
- **Change tracking**: Stash has no change feed, so catching up relies on the "updated since"
  timestamps Stash keeps on scenes, images, performers, and tags, checked on the cache's refresh
  schedule and after the viewer's own edits. Deletions are found by comparing identifiers on a
  slower schedule.
- **"Largest library"** is the owner's Production server (read-only for testing); its exact size
  is measured during planning and recorded with SC-001.
- **Query language scope**: Phase 1 of the documents' query features (plus the basic filters
  Milestone 2 lists). Boolean logic, more SPARQL constructs, CONSTRUCT, and rules are roadmap
  Phase 5.
- **Name resolution**: query terms like `:Dom` match a tag's or performer's name, then its aliases,
  case-insensitively and accent-folded, as the 008 pickers search.
- **No writes**: this feature never writes to Stash; Label materialization, CONSTRUCT, and rules
  that write come later, through the existing write paths.
- **Fixture**: a scrubbed capture from Production (read-only), like 008's vocabulary fixture, with
  names replaced and only semantic data kept; expected answers are derived from the documents and
  recorded with the fixture.
- **Out of scope**: the Search Matrix, Quick Pairings, recommendations, Label evaluation and
  materialization screens, archetype screens, timeline editing, and the general search bar. Each
  is a later feature that queries this engine (roadmap Phase 4b–4f and Phase 5).
- **008's tag-type marking and colour rules** keep their own flag-filtered discovery: they aren't
  among Principle XII's engine-client features. The engine uses the same discovery queries, so
  the two never disagree about which tags are semantic.
- **The middleware analysis (2026-10-11)**: the engine stays in-process in this feature
  (constitution XII). FR-022 and FR-023 only keep a later shared helper cheap: an optional
  program on the Stash server that keeps a snapshot current and lets clients sync from it
  instead of building (roadmap Phase 6). Nothing here depends on that helper existing.
- **The roadmap analysis (2026-10-11)** is applied: sources are design-document sections with the
  commit read, not the plugin's modules; the engine comes first and later semantic features are
  its clients; the documents, not the plugin's code, define correct answers; the Milestone 1D
  vocabulary (Variations replacing Looks, the Label formula) is the one used.
