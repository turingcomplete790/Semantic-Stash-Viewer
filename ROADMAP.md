# Roadmap

This roadmap orders the work to build Semantic Stash Viewer. It starts with a read-only viewer,
adds editing, then covers the rest of the Stash web UI, and finally builds the semantic model the
`semantic-tagging` plugin's design documents describe, on an embedded semantic engine
(constitution Principles II and XII). Every phase must follow the
[constitution](.specify/memory/constitution.md).

Each checkbox group is meant to become one or more Spec Kit features
(`/speckit-specify` → `/speckit-plan` → `/speckit-tasks` → `/speckit-implement`). The
**Suggested specs** under each phase are the recommended way to slice it.

## Overview

| Phase | Theme | Result |
|---|---|---|
| 0 | Foundation (done) | The app connects to Stash securely, and the core, the adapter, and mpv are proven |
| 1 | Browse & play | The app shell (navigation bar, tabs, notifications), then read-only browsing of the whole library, with mpv playback |
| 2 | Tags & editing | Everyday editing: tags, metadata, ratings, markers |
| 3 | Advanced Stash features | Scrapers, StashBox, tasks, and settings. Full web UI parity (Principle VIII) |
| 4 | Semantic engine & tagging | The semantic model from the plugin's design documents, built on an embedded engine (Principles II, XII) |
| 5 | Semantic query language | The engine's query language for users: search bar, Matrix as a query producer, CONSTRUCT, rules |

**Platform (2026-10-05):** features 001–005 were first built as a Tauri/SolidJS app. Feature 007
([specs/007-port-to-iced](specs/007-port-to-iced/)) rebuilt everything natively in Rust with
iced, and the Tauri app was removed. Phase 0 below records the foundation as it was built; the
core, the player, and every capability it lists carried over, and Phase 1 continues natively.

The phases are mostly sequential. Phase 4 needs the adapter, tag editing, and the caching work
from Phases 0–2. Parts of Phase 3 (scrapers, tasks) can run alongside Phase 4 if needed.

---

## Phase 0: Foundation

**Goal:** a Tauri app that connects to a real Stash server, with the architecture from
Principles III, IV, V, and VII in place before any features are built on it.

- [x] Project scaffold: Tauri v2 workspace, a headless Rust core crate that doesn't depend on
      Tauri (Principle III), and a frontend with its framework chosen and justified
- [x] Stash adapter: one Rust module owning all GraphQL I/O, with typed operations generated from
      Stash's schema and recorded fixtures for tests
- [x] Connection profiles: endpoint URL, optional API key saved in the local config, and multiple
      saved servers
- [x] Version check on connect: refuse servers older than **Stash v0.31.1** with a clear message
- [x] TLS: validation off by default, a per-profile setting to turn strict validation on, and a
      connection-security indicator (unencrypted / unverified / verified)
- [x] Cache layer: discardable local cache, mutation-driven invalidation, and a "clear cache"
      troubleshooting control in Settings
      ([specs/003-complete-foundation](specs/003-complete-foundation/))
      - Per-server SQLite cache sized to 5% of free disk (64 MB–10 GB), never shared across
        servers; screens show cached data at once and refresh quietly. Targeted invalidation is
        ready for Phase 2's writes
- [x] mpv spike: embed libmpv in the Tauri window (or a managed mpv process as fallback) playing
      a Stash direct stream authenticated with the API key
      - Decision: libmpv's OpenGL render API into a GTK GL area under the transparent webview;
        every success criterion passed, no fallback needed
        ([decision record](specs/002-mpv-playback-spike/decision.md))
- [x] Performance harness: a way to measure Principle VI's budgets (input latency, first paint,
      scroll fps, cold start) (003)
      - `cargo run --bin perf-harness -- --profile "<name>"`: every budget passes on the reference
        machine, about 2 minutes per run, repeatable within 10%
- [x] CI: `cargo fmt`, `cargo clippy`, the frontend linter, and core unit tests (003)
      - Every push runs all checks in about 4–6 minutes, with a timeout, superseded runs
        cancelled, and a README status badge

**Exit criteria:** you can add a server profile, connect, see server info, and play one scene in
mpv.

**Suggested specs:** `connect-to-stash` (done: [specs/001-connect-to-stash](specs/001-connect-to-stash/)),
`mpv-playback-spike` (done: [specs/002-mpv-playback-spike](specs/002-mpv-playback-spike/)),
`complete-foundation` (done: [specs/003-complete-foundation](specs/003-complete-foundation/))

---

## Phase 1: Browse & play (read-only)

**Goal:** browse the whole library faster than the web UI, and play videos without transcoding.

### Scenes
- [x] Scene grid and list views: paged (default 50 per page, 20–1000 selectable, per tab),
      server-side pagination, neighbouring pages prefetched (constitution IV)
      ([specs/005-browse-scenes-and-play](specs/005-browse-scenes-and-play/), P1)
- [x] Sorting, filtering (every web UI filter criterion), and search; the plugin's semantic
      filters are placeholders until Phase 4
      ([specs/008-finish-browse-play](specs/008-finish-browse-play/), US1)
- [x] Loading and applying saved filters and the server's default filter (008 US1; creating and
      editing them is Phase 2)
- [x] Scene detail page: metadata, performers (with pictures and detail cards), studio, tags
      (marked by semantic type, coloured by flag rules set in Settings → Tag colours), groups,
      markers, every file, galleries (008 US2)
- [x] mpv playback: direct stream, hardware decode, resume position, markers as chapters
      (006–008)
- [x] Play count, play duration, and resume position sync back to Stash (Principle V), queued on
      disk so nothing is lost (008 US3)
- [x] Resume where you left off: a Settings → Playback option (as in the Stash web UI) to start
      scenes from their saved resume point instead of the beginning (008 US3)
- [x] Scene previews (animated, on hover), a wall view, and sprite/scrubber thumbnails (008 US4)
- [x] Transcoding fallback: explicit, user-visible, and logged, only when mpv can't play a file
      (008 US5)
- [x] Queue view: the list on one side and the selected scene's player and details on the
      other, so a run of scenes can be clicked through without leaving the list, with Play next
      across pages and full keyboard operation (010)
- [x] Queue playlist controls: clicking a scene plays it; Autoplay (the queue starts on its own
      and moves on), Shuffle (every scene once per round, from every page), and Repeat (all or
      one), decided by a pure state machine in the core (011)

### Images
- [ ] Image grid (sized thumbnails), filters, and sorting
- [ ] Full-screen image viewer in mpv (Principle V): keyboard/mouse navigation, zoom, pan,
      rotation, fit/actual size, animated GIF/WebP, slideshow, and full resolution loaded on
      demand with the neighbouring items prefetched (next/previous shown within 150 ms)

### Galleries (Principle X: designed fresh, not copied from Stash)

Stash's galleries are image-only and its gallery viewer is weak. Here galleries hold **images and
scenes together**, in one mpv viewer. Stash is used for its data and to check no capability is
lost, not as a design to copy.

- [ ] Gallery grid, filters, and sorting (every Stash gallery filter and sort)
- [ ] Mixed gallery view: the gallery's images and its related scenes in one browsable sequence,
      with image and scene cards side by side
- [ ] Unified viewer: opening any item shows the image or plays the scene in mpv, moving between
      images and scenes (next/previous, slideshow) without leaving the viewer; playback history
      still syncs for scenes (Principle V)
- [ ] Which scenes belong with a gallery, and any custom ordering of the mixed sequence, come from
      or are saved to Stash (relationships, tags, or `custom_fields`), never kept only locally
- [ ] An images-only view of a gallery, for users who want Stash's behaviour
- [ ] A queue-style gallery view: the gallery's items listed beside the viewer (see the Scenes
      queue view)
- [ ] Gallery chapters, covers, zip and folder galleries, and image details (Stash parity)

### Other entities
- [ ] Performers: grid, filters, and detail page (scenes, images, galleries, groups)
- [ ] Studios: grid, parent/child studios, and detail page
- [ ] Tags: grid, tag hierarchy browsing, and detail page
- [ ] Groups (formerly Movies): grid and detail page with scene ordering
- [ ] Markers: marker wall/list, filtered by tag, playing from the marker position

### App shell (Principle IX)
- [x] Navigation bar modelled on the Stash web UI's sections (Scenes, Images, Galleries, Groups,
      Markers, Performers, Studios, Tags), plus Settings and notifications. Sections are added
      as their features land.
- [x] Tabs: several views open at once in the main window, each keeping its scroll position,
      filters, and selection; keyboard shortcuts to open, close, and switch tabs
- [x] Notification centre: connection alerts (server unreachable, reconnected, authentication
      failures) and background failures, with a badge for unread items, and toasts for important
      events. Running Stash jobs are shown read-only with live progress; starting and cancelling
      them arrive in Phase 3.
- [x] Settings page: viewer preferences and troubleshooting controls (e.g. clear cache)
      (done: [specs/004-app-shell](specs/004-app-shell/) for the four items above)
- [ ] Home page with configurable rows (recently added, recently played, saved filters)
- [ ] Global search across entity types
- [ ] Keyboard navigation throughout; groundwork for a 10-foot (remote/controller) mode
- [ ] Adjustable panel sizes: drag the dividers between panels (e.g. the scene view's picture and
      its details side panel) and keep the sizes per view; the defaults use the width of
      widescreen displays (side panels rather than stacked ones)

**Exit criteria:** every entity type can be browsed, filtered, and opened. Scenes play in mpv
and resume state syncs back. Principle VI's budgets are met on a large library.

**Suggested specs:** `app-shell` (done: [specs/004-app-shell](specs/004-app-shell/)), `browse-scenes-and-play` (in progress: [specs/005-browse-scenes-and-play](specs/005-browse-scenes-and-play/)), `browse-images-and-galleries` (mpv image viewer and mixed image/scene galleries, Principles V
and X),
`browse-performers-studios-tags-groups`, `markers-wall`, `home-and-global-search`

---

## Phase 2: Tags & editing

**Goal:** the everyday edits people currently open the web UI for.

### Tags
- [ ] Add and remove tags on scenes, images, galleries, performers, groups, and markers, with
      fast type-ahead
- [ ] Create a tag inline while tagging
- [ ] Tag management: create, rename, description, aliases, image, parent/child tags, and
      merging tags
- [ ] Deleting tags, with confirmation (Principle VII)

### Metadata & state
- [ ] Edit metadata on every entity type (title, date, details, URLs, studio, performers, codes)
- [ ] Ratings, O-counter (increment, decrement, history), and the organized flag
- [ ] Bulk edit for multi-selections (tags, performers, studio, rating, organized)
- [ ] Create, rename, and delete saved filters, and set the default filter per view

### Markers
- [ ] Create a marker at the current mpv position (title, primary tag, extra tags)
- [ ] Edit, delete, and re-time markers

### Other creation
- [ ] Create and edit performers, studios, and groups (including cover images)
- [ ] Add and remove scenes in galleries and groups; gallery cover and ordering

**Exit criteria:** a normal tagging and curation session can be done entirely in the viewer.
Every write goes to Stash, and the cache updates without a manual refresh.

**Suggested specs:** `tag-editing`, `tag-management`, `metadata-editing-and-bulk-edit`,
`scene-markers-editing`, `entity-creation`

---

## Phase 3: Advanced Stash features (web UI parity)

**Goal:** close the remaining gaps so the viewer can fully replace the web UI (Principle VIII).

### Scrapers & StashBox
- [ ] Scraping scenes, performers, galleries, groups, and images by URL, name, and fragment
- [ ] Reviewing scraped data before it's applied: per-field accept/reject, new vs. existing
      performers, studios, and tags
- [ ] StashBox search and "Scene Tagger" workflow (match, review, apply)
- [ ] StashBox performer and studio tagger
- [ ] Submitting drafts and fingerprints to StashBox

### Library tasks & jobs
- [ ] Scan, Generate (previews, sprites, phashes, and so on), Auto Tag, Identify, Clean
- [ ] Job queue in the notification centre (Principle IX): live progress, cancellation, and
      completion or failure notifications
- [ ] Running plugin tasks and scripts registered with Stash

### Library maintenance
- [ ] Duplicate checker (phash) and scene merge
- [ ] Multi-file scenes: choosing the primary file, splitting files off
- [ ] Deleting scenes, images, and galleries, with an option to delete files (confirmation
      required)

### Settings & system
- [ ] Stash server settings: library paths, scraping, StashBox endpoints, tasks, security,
      interface defaults that apply to the viewer, and plugin settings
- [ ] Stats page
- [ ] Logs viewer
- [ ] Viewer-only preferences: theme, player behaviour, keybindings, 10-foot mode

**Exit criteria:** a written parity checklist against the v0.31.1 web UI shows no unexplained
gaps. Any deferred item (for example a rarely used setting) is recorded in its spec with a
reason, per Principle VIII.

**Suggested specs:** `scrapers`, `stashbox-tagger`, `library-tasks-and-jobs`,
`duplicates-and-scene-merge`, `server-settings`, `web-ui-parity-audit`

---

## Phase 4: Semantic engine & tagging

**Goal:** build the semantic model that the `semantic-tagging` plugin's design documents describe
(`README.md`, `semantic-query-engine-roadmap.md`, `archetype-system-plan.md` in
`~/Projects/Stash-Plugins/semantic-tagging/`) on an embedded semantic engine (Principle XII), reading
and writing the same Stash data as the plugin so both can be used on the same library (Principle
II). The plugin's JavaScript is not ported: each spec cites the document sections it builds on and
the plugin commit (or date) it read them at, and every semantic feature is a query or rule on the
engine, not a hand-written filter.

The documents' own engine plan (`semantic-query-engine-roadmap.md`, "Target Architecture" and
Where To Start Steps 4–6: the semantic index, Traqula as parser and algebra, a hand-written
evaluator) maps onto the viewer like this: Oxigraph provides the parsing, algebra, and evaluation,
and the engine's graph is the semantic index.

### 4a: Semantic engine (foundation; 009, specified)
- [ ] Oxigraph embedded in the core behind a semantic-engine interface (Redland as the fallback,
      only by a constitution amendment recording why), running in-process
- [ ] The semantic graph built from Stash with the documents' vocabulary ("Core Design
      Decisions §2: The semantic graph is the abstraction boundary" and its "Vocabulary
      corrections"; Milestone 1D for Variations and Labels): Scene and Image media entities,
      performers, the six flagged tag types plus Variations, reified Role and Action instances,
      tag categories and variations, parent tags, Label formulas, archetype parents
- [ ] The data contract read exactly as the documents define it: per-type tag flags (`plugtag`,
      `isAction`, `isRole`, `isTrait`, `isLabel`, `isMiseEnScene`, `isVariation`), `variantOfTagId`,
      scene- and image-scoped assignments, performer-global Traits and Labels
- [ ] Bounded, flag-filtered discovery: only the semantic vocabulary is fetched ("Performance
      Strategy: Preserve the lessons from the existing plugin"; Principle IV). Started in
      [specs/008-finish-browse-play](specs/008-finish-browse-play/) (the flag-filtered semantic
      vocabulary and colour rules)
- [ ] SPARQL `SELECT` over the graph: triple patterns, variables, joins, projection, basic
      filters ("Query Features: Phase 1 — Minimal"; Milestone 2), with the documents' scoping
      rules enforced (scene-scoped predicates only join within one scene)
- [ ] A derived index (Principle I): built off the UI thread, the first build showing progress
      and cancellable, kept current incrementally as Stash changes, safe to delete
- [ ] The engine's on-disk store as a cache: one per server profile, in the cache folder, rebuilt
      when missing, damaged, or from an older format, and cleared with the other caches
- [ ] Compatibility fixtures: data captured read-only from a library tagged by the plugin gives
      the same graph, and round-trips through the viewer unchanged

### 4b: Tagging dimensions (engine clients; README §Features; Milestone 1D)
- [ ] Pluggables: per scene/image, and per Variation (Milestone 1D §3)
- [ ] Actions with variations, own and inherited
- [ ] Roles with categories
- [ ] Traits: performer-global, curated, with categories and variations
- [ ] Labels: computed from their formula (Trait, Field, and Label-dependency rules with weights
      and a threshold; Milestone 1D §3), with "why does this performer have this Label?" answered
      by the engine
- [ ] Mise-en-scène: scene/image only, with categories and variations
- [ ] Variations as real tags (`isVariation`), replacing Looks: a performer's Variations and the
      Pluggables they carry (Milestone 1D §2, §4)
- [ ] Performer-to-performer pairings for Roles and Actions
- [ ] Performer card badges by semantic type and category. Tag-chip colours by flag rules are
      done ([specs/008-finish-browse-play](specs/008-finish-browse-play/), Principle XI); colours
      by category, if wanted, extend the rules
- [ ] Tag metadata management pages for each dimension (README §Features)

### 4c: Timeline (README §Features)
- [ ] Timeline editor: Action and Role instances at timestamps, integrated with mpv playback
- [ ] Stages: scene headings carrying Mise-en-scène tags that apply to the slots after them
- [ ] Generating Stash markers from timeline slots, including the active stage's tags
- [ ] Timeline slots and Stages shown as mpv chapters

### 4d: Archetypes (`archetype-system-plan.md`; Milestones 1B and 1D §1)
- [ ] Archetypes as performers with parents-only adjacency, and nearest-ancestor-wins inherited
      recipes computed by the engine (§Data model, §Hierarchy index)
- [ ] Bank manager: creating and configuring archetypes, their recipes, and parents (§Bank
      manager UI)
- [ ] Hierarchy graph editor: node/edge DAG, link editing, saved node positions
- [ ] Scene/image archetype quick-add
- [ ] Archetype Compare (union/intersect)
- (Applying an archetype to a regular performer was removed by Milestone 1D §1.)

### 4e: Tag recommendations & common variants (Milestone 1C)
- [ ] Marking common variants (`variantOfTagId`) against canonical tags
- [ ] Recommendations in assignment dropdowns and the Stage picker, as engine queries

### 4f: Semantic search (README §Features)
- [ ] Semantic Search Matrix: Side A/B criteria, Mise-en-scène filter, timeline relation
      constraints, recursive archetype filter, results for both scenes and images; each search an
      engine query
- [ ] Quick Pairings, as an engine query
- [ ] A query preview panel: shows the `SELECT` a search runs and its bindings (the documents'
      "Graph Query Preview"; the query language itself is 4a)
- [ ] Gallery results view and the Image Deck viewer (README §Features), on mpv

**Exit criteria:** the viewer reads and writes the same data contract as the plugin, so both are
safe on the same library at once, and for the same library state it gives the answers the design
documents define. Where the plugin's current code answers differently, the difference is
reported to the plugin rather than copied.

**Suggested specs:** `semantic-engine` (4a, first), `semantic-tagging-dimensions`,
`semantic-timeline`, `archetypes`, `tag-recommendations`, `semantic-search-matrix`, `image-deck`

---

## Phase 5: Semantic query language (the documents' Milestones 3–8)

**Goal:** give users the engine's query language, following the query-engine roadmap in the
plugin's design documents (the documents are the reference, Principle II). Each item extends the
4a engine; none adds a second matching path.

- [ ] General semantic search bar with query errors and result bindings (Milestone 3)
- [ ] Matrix as a query producer: Matrix state translated into query patterns, with one shared
      matching path (Milestone 4)
- [ ] Boolean logic and more SPARQL constructs in user queries ("Query Features: Phase 2 —
      Boolean logic", "Phase 3 — More SPARQL-style constructs")
- [ ] CONSTRUCT: previewing derived facts and applying derived tags through Stash, with
      provenance recorded (Milestone 5)
- [ ] Saved queries and rules: rule management, manual or scheduled runs, explaining derived
      tags (Milestone 6)
- [ ] Optimization: profiling, join ordering, query caching, memory footprint (Milestone 7);
      much of it comes with the engine
- [ ] Optional extraction (Milestone 8): see Phase 6, a shared engine cache as an optional
      helper, after the core functionality

---

## Phase 6: Shared semantic engine cache (optional helper; last)

**Goal:** let the clients of one Stash server (this viewer on several machines, and the
semantic-tagging plugin) share one semantic engine instead of each building and polling its own.
A helper is an optimisation, never a requirement: every client keeps its own in-process engine
(Principle XII) and works against stock Stash without it. This is the plugin documents'
Milestone 8 ("Optional Extraction … only if justified"), in Rust from this repo's engine crate
rather than as a Node service. Comes after Phases 1–5, once the engine works (2026-10-11
middleware analysis).

- [ ] Spike: the engine as a Stash plugin (Stash's "raw" plugin interface, running the 009 engine
      on the server). It keeps a snapshot (009 FR-023) current from Stash's plugin hooks (scenes,
      images, performers, tags saved) instead of every client polling, since Stash has no change
      subscription. Clients reach it only through Stash's GraphQL (`runPluginOperation`;
      `runPluginTask` for builds), so the network destination stays Stash (Principle VII).
      Measure and decide:
      - a client syncing from the snapshot vs building itself (time and size, against 009 SC-001)
      - `runPluginOperation` round trip (process start included) and the largest result it can
        return
      - how the snapshot reaches clients (an operation's result, or the plugin's asset folder that
        Stash serves) and how a client catches up from it
      - concurrent calls and the engine store's single-writer lock (one writer, read-only readers,
        or snapshot-only)
      - hook delivery and freshness when a tag is saved in the web UI, the plugin, or the viewer
      - the plugin's JavaScript asking the same engine through `runPluginOperation` (one engine,
        one set of answers: Principle II's query semantics)
- [ ] If the spike holds: amend the constitution (minor) before a feature spec. Draft wording:
      - Principle I, replacing "MUST NOT require a server-side component": "The viewer MUST work
        fully against a stock Stash instance. It MAY use an optional companion running inside
        Stash's plugin system (a shared semantic engine cache); without it, nothing is lost but
        time."
      - Principle XII, after "Disk store as a cache": "**Shared cache (optional).** A companion on
        the Stash server MAY keep a snapshot of the engine for that server, reached only through
        Stash's GraphQL. Clients MAY start from it and catch up, and still build on their own when
        it's missing, stale, from another server, or from another format. The snapshot is a cache
        under Principle I and never shared across servers."
- [ ] Out of scope for the helper: scrapers (they already run in Stash and any client uses them
      through Stash's scrape API, Phase 3) and sharing ordinary GraphQL response caches between
      clients (Stash is already fast to query; only the semantic index is costly to build)

---

## Maintaining this roadmap

- Tick items off when their spec is implemented, and link the spec folder (`specs/NNN-name/`)
  next to the item.
- When the semantic-tagging plugin's design documents add or change a feature or data structure,
  update Phase 4 or 5 and note the plugin commit read (Principle II).
- When a new stable Stash release adds a web UI capability, add it to the relevant phase
  (Principle VIII).
- Reordering phases is fine. Changes that conflict with the constitution need a constitution
  amendment first.
