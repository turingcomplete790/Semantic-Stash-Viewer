# Roadmap

This roadmap orders the work to build Semantic Stash Viewer. It starts with a read-only viewer,
adds editing, then covers the rest of the Stash web UI, and finally brings over the
`semantic-tagging` plugin's features. Every phase must follow the
[constitution](.specify/memory/constitution.md).

Each checkbox group is meant to become one or more Spec Kit features
(`/speckit-specify` → `/speckit-plan` → `/speckit-tasks` → `/speckit-implement`). The
**Suggested specs** under each phase are the recommended way to slice it.

## Overview

| Phase | Theme | Result |
|---|---|---|
| 0 | Foundation | The app connects to Stash securely, and the core, the adapter, and mpv are proven |
| 1 | Browse & play | The app shell (navigation bar, tabs, notifications), then read-only browsing of the whole library, with mpv playback |
| 2 | Tags & editing | Everyday editing: tags, metadata, ratings, markers |
| 3 | Advanced Stash features | Scrapers, StashBox, tasks, and settings. Full web UI parity (Principle VIII) |
| 4 | Semantic tagging | Parity with the semantic-tagging plugin (Principle II) |
| 5 | Semantic query engine | Follows the plugin's own roadmap: query language, CONSTRUCT, rules |

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
- [ ] Cache layer: discardable local cache, mutation-driven invalidation, and a "clear cache"
      troubleshooting control in Settings (in progress:
      [specs/003-complete-foundation](specs/003-complete-foundation/))
- [x] mpv spike: embed libmpv in the Tauri window (or a managed mpv process as fallback) playing
      a Stash direct stream authenticated with the API key
      - Decision: libmpv's OpenGL render API into a GTK GL area under the transparent webview;
        every success criterion passed, no fallback needed
        ([decision record](specs/002-mpv-playback-spike/decision.md))
- [ ] Performance harness: a way to measure Principle VI's budgets (input latency, first paint,
      scroll fps, cold start) (in progress: 003)
- [ ] CI: `cargo fmt`, `cargo clippy`, the frontend linter, and core unit tests (a check already
      runs on every push; 003 confirms coverage and adds a README status badge)

**Exit criteria:** you can add a server profile, connect, see server info, and play one scene in
mpv.

**Suggested specs:** `connect-to-stash` (done: [specs/001-connect-to-stash](specs/001-connect-to-stash/)),
`mpv-playback-spike` (done: [specs/002-mpv-playback-spike](specs/002-mpv-playback-spike/)),
`complete-foundation` (in progress: [specs/003-complete-foundation](specs/003-complete-foundation/))

---

## Phase 1: Browse & play (read-only)

**Goal:** browse the whole library faster than the web UI, and play videos without transcoding.

### Scenes
- [ ] Scene grid and list views: virtualized, server-side pagination, prefetch
- [ ] Sorting, filtering (the web UI's filter criteria), and search
- [ ] Loading and applying saved filters
- [ ] Scene detail page: metadata, performers, studio, tags, groups, galleries, files
- [ ] mpv playback: direct stream, hardware decode, resume position, markers as chapters
- [ ] Play count, play duration, and resume position sync back to Stash (Principle V)
- [ ] Resume where you left off: a Settings → Playback option (as in the Stash web UI) to start
      scenes from their saved resume point instead of the beginning
- [ ] Scene previews and sprite/scrubber thumbnails
- [ ] Transcoding fallback: explicit, user-visible, and logged, only when mpv can't play a file

### Images
- [ ] Image grid (sized thumbnails), filters, and sorting
- [ ] Full-screen image viewer: keyboard/mouse navigation, zoom, slideshow, full resolution
      loaded on demand

### Galleries
- [ ] Gallery grid, filters, and sorting
- [ ] Gallery view: the images inside a gallery, opening in the image viewer
- [ ] Gallery chapters

### Other entities
- [ ] Performers: grid, filters, and detail page (scenes, images, galleries, groups)
- [ ] Studios: grid, parent/child studios, and detail page
- [ ] Tags: grid, tag hierarchy browsing, and detail page
- [ ] Groups (formerly Movies): grid and detail page with scene ordering
- [ ] Markers: marker wall/list, filtered by tag, playing from the marker position

### App shell (Principle IX)
- [ ] Navigation bar modelled on the Stash web UI's sections (Scenes, Images, Galleries, Groups,
      Markers, Performers, Studios, Tags), plus Settings and notifications. Sections are added
      as their features land.
- [ ] Tabs: several views open at once in the main window, each keeping its scroll position,
      filters, and selection; keyboard shortcuts to open, close, and switch tabs
- [ ] Notification centre: connection alerts (server unreachable, reconnected, authentication
      failures) and background failures, with a badge for unread items, and toasts for important
      events. Stash jobs arrive in Phase 3.
- [ ] Settings page: viewer preferences and troubleshooting controls (e.g. clear cache)
- [ ] Home page with configurable rows (recently added, recently played, saved filters)
- [ ] Global search across entity types
- [ ] Keyboard navigation throughout; groundwork for a 10-foot (remote/controller) mode

**Exit criteria:** every entity type can be browsed, filtered, and opened. Scenes play in mpv
and resume state syncs back. Principle VI's budgets are met on a large library.

**Suggested specs:** `app-shell` (navigation bar, tabs, notification centre, Settings; first,
because every other screen lives inside it), `browse-scenes-and-play`, `browse-images-and-galleries`,
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

## Phase 4: Semantic tagging (plugin parity)

**Goal:** bring every feature of `~/Projects/Stash-Plugins/semantic-tagging/` over, reading and
writing the same data so the plugin and the viewer can safely be used on the same library
(Principle II). Each spec names the plugin module it mirrors.

### 4a: Semantic core (Rust)
- [ ] Vocabulary: a port of `src/semantic/vocabulary.js` (classes, predicates, scope annotations)
- [ ] Semantic parsing of `custom_fields` for Tags, Scenes, Images, and Performers, following
      `src/semantic/engine.js`
- [ ] Semantic index and graph with reverse indexes, following `src/semantic/graph.js`
- [ ] Bounded vocabulary discovery: flag-filtered tag queries only, never loading every tag
      (Principle IV)
- [ ] Compatibility fixtures: data captured from a library tagged by the plugin must round-trip
      through the viewer unchanged

### 4b: Tagging dimensions
- [ ] Pluggables: per scene/image, per performer (`src/ui/pluggables.js`)
- [ ] Actions with variations, own and inherited (`src/ui/actions.js`)
- [ ] Roles with categories (`src/ui/roles.js`)
- [ ] Traits: performer-global, curated, with categories and variations (`src/ui/traits.js`)
- [ ] Labels: computed from Field Rules and label formulas (`src/ui/labels.js`)
- [ ] Mise-en-scène: scene/image only, with categories and variations
      (`src/ui/mise-en-scene.js`)
- [ ] Performer-to-performer pairings for Roles and Actions
- [ ] Looks: saved Trait/Pluggable combinations applied to a performer
- [ ] Performer card badges and tag-chip colour-coding by category
- [ ] Tag metadata management pages for each dimension (`src/ui/tag-metadata.js`)

### 4c: Timeline
- [ ] Timeline editor: Action and Role instances at timestamps, integrated with mpv playback
      (`src/ui/timeline.js`)
- [ ] Stages: scene headings carrying Mise-en-scène tags that apply to the slots after them
- [ ] Generating Stash markers from timeline slots, including the active stage's tags
- [ ] Timeline slots and Stages shown as mpv chapters

### 4d: Archetypes
- [ ] Archetype index: parents-only DAG with nearest-ancestor-wins inherited recipes
      (`src/archetypes/index.js`)
- [ ] Bank manager: creating and configuring archetypes and their recipes and parents
      (`src/ui/archetypes.js`)
- [ ] Hierarchy graph editor: node/edge DAG, link editing, saved node positions
      (`src/ui/archetype-hierarchy-graph.js`)
- [ ] Scene/image archetype quick-add (`src/ui/archetype-quick-add.js`)
- [ ] Applying an archetype to a regular performer, with precise re-apply reconciliation
      (`src/ui/archetype-apply.js`)
- [ ] Archetype Compare (union/intersect)

### 4e: Tag recommendations & common variants
- [ ] Marking common variants (`variantOfTagId`) against canonical tags
- [ ] Recommendations in assignment dropdowns and the Stage picker

### 4f: Semantic search
- [ ] Semantic Search Matrix: Side A/B criteria, Mise-en-scène filter, timeline relation
      constraints, recursive archetype filter, results for both scenes and images
      (`src/ui/matrix.js`)
- [ ] Quick Pairings
- [ ] Graph Query Preview (SELECT/WHERE/FILTER against the local index)
- [ ] Gallery results view and a native image deck with scene playback in mpv (replacing the
      plugin's `src/image-deck/` and video.js player)

**Exit criteria:** for the same library, the viewer and the plugin show the same semantic data
and return the same search results. Editing in one is immediately valid in the other.

**Suggested specs:** `semantic-core`, `semantic-tagging-dimensions`, `semantic-timeline`,
`archetypes`, `tag-recommendations`, `semantic-search-matrix`, `image-deck`

---

## Phase 5: Semantic query engine (following the plugin roadmap)

**Goal:** keep pace with the plugin's `semantic-query-engine-roadmap.md`. The plugin is still
the reference implementation (Principle II), so these items are built alongside the plugin, or
right after it, so that both give the same results.

- [ ] Query language: SPARQL-inspired SELECT, triple patterns, variables, joins, projection,
      filters (plugin Milestone 2)
- [ ] General semantic search bar with query errors and result bindings (plugin Milestone 3)
- [ ] Matrix as a query producer: Matrix state translated into query patterns, with one shared
      matching path (plugin Milestone 4)
- [ ] CONSTRUCT: previewing derived facts and applying derived tags through Stash, with
      provenance recorded (plugin Milestone 5)
- [ ] Saved queries and rules: rule management, manual or scheduled runs, explaining derived
      tags (plugin Milestone 6)
- [ ] Optimization: evaluator profiling, join ordering, query caching, memory footprint (plugin
      Milestone 7). The Rust core may reach this sooner than the JS plugin.

---

## Maintaining this roadmap

- Tick items off when their spec is implemented, and link the spec folder (`specs/NNN-name/`)
  next to the item.
- When the semantic-tagging plugin adds a feature, add it to Phase 4 or 5 (Principle II).
- When a new stable Stash release adds a web UI capability, add it to the relevant phase
  (Principle VIII).
- Reordering phases is fine. Changes that conflict with the constitution need a constitution
  amendment first.
