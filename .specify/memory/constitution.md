# Semantic Stash Viewer Constitution

Semantic Stash Viewer is an opinionated, native desktop frontend for the
[Stash](https://github.com/stashapp/stash) media server, built in Rust with Tauri. Its goals:
(1) bring every feature of the evolving `semantic-tagging` Stash plugin
(`~/Projects/Stash-Plugins/semantic-tagging/`) to a first-class client; (2) deliver a
responsive, near-native browsing experience with optimized GraphQL usage and mpv-based playback
that avoids the Stash web UI's expensive transcoding; and (3) cover every capability of the
default Stash web UI, so the viewer can fully replace it. A user points it at any Stash endpoint
(plus API key, if one is configured) and uses their library the way Jellyfin desktop clients let
users use a Jellyfin server.

## Core Principles

### I. Stash Is the System of Record

- Stash owns media, scenes, images, galleries, performers, tags, metadata, and persistence. The
  viewer MUST NOT introduce its own authoritative database for library data.
- Every write (tag assignments, semantic relationships, timeline entries, archetype recipes,
  ratings, metadata edits) MUST be persisted to Stash through its GraphQL API — typically as tags
  or `custom_fields` on Tags, Scenes, Images, and Performers.
- Local storage is permitted only for caches, derived indexes, UI preferences, and connection
  profiles. Any local cache MUST be safely discardable: deleting it may cost time, never data.
- The viewer MUST NOT require a server-side component beyond a stock Stash instance (plus the
  semantic-tagging plugin's data conventions, which live in Stash itself).

**Rationale**: Users already trust Stash with their library. A client that forks the source of
truth creates sync bugs and lock-in; a client that is a pure lens on Stash can be installed,
removed, or run on several machines without risk.

### II. Semantic-Tagging Parity & Data Compatibility (NON-NEGOTIABLE)

- The semantic-tagging plugin is the reference implementation and its on-Stash data format is a
  shared contract. The viewer MUST read and write the exact same tag flags and `custom_fields`
  schemas (e.g. Pluggables, Actions + variations, Roles + categories, Traits, Labels / Field
  Rules, Mise-en-scène, Timeline slots and Stages, performer-to-performer pairings, Archetypes
  with parents-only adjacency, Looks) so that the plugin and the viewer can operate on the same
  library concurrently without corrupting each other's data.
- Data-scope rules from the plugin MUST be preserved: Scene/Image-scoped data (Roles, Actions,
  Pluggables, Timeline) stays on the Scene/Image; performer-global data (Traits, Labels) stays on
  the performer; archetype hierarchy stays parents-only with children derived in memory.
- Any schema divergence MUST be deliberate, documented in the feature spec, and accompanied by a
  migration or compatibility shim — never an accidental side effect.
- The plugin keeps evolving. Each feature spec touching semantic data MUST name the plugin
  module(s) it mirrors (e.g. `src/ui/timeline.js`, `src/semantic/graph.js`) so parity can be
  re-checked when the plugin changes. A parity gap is tracked work, not an acceptable end state.
- Semantic vocabulary, graph model, and query semantics (Semantic Search Matrix, Quick Pairings,
  graph SELECT/WHERE/FILTER queries, and the roadmap's SPARQL-inspired direction) MUST produce
  the same results as the plugin for the same library state.

**Rationale**: The viewer's reason to exist is to be the best home for semantic tagging. If the
two diverge, the user loses the ability to use either one safely.

### III. Brain/UI Separation

- Domain logic — Stash I/O, semantic vocabulary, index/graph building, query evaluation,
  archetype inheritance, caching — MUST live in Rust crates/modules that have no dependency on
  Tauri, the webview, or the frontend framework.
- The frontend MUST reach Stash only through typed Tauri commands/events exposed by the Rust
  core; it MUST NOT issue its own GraphQL requests.
- Layering inside the core mirrors the plugin: a single Stash adapter owns all network I/O; the
  semantic engine consumes the adapter; the UI consumes the engine. Cross-layer shortcuts are
  violations.
- Core modules MUST be unit-testable headlessly (no running webview, and ideally no running
  Stash, via recorded/fixture GraphQL responses).

**Rationale**: The plugin's roadmap already established that "the brain" must be independent of
React. Carrying that into Rust makes the engine fast, testable, and reusable (CLI tools, future
background indexing) without redesign.

### IV. Lean, Batched GraphQL

- Prefer "bulk fetch → local semantic index → many local queries" over request-per-interaction.
  N+1 query patterns (a request per card, per performer, per tag) are prohibited in list and grid
  views.
- Every GraphQL operation MUST select only the fields the consuming view needs; shared fragments
  are defined per view tier (card, detail, editor) rather than one "fetch everything" fragment.
- Queries against semantic vocabulary MUST be narrowly scoped using tag flags / `custom_fields`
  filters; the viewer MUST NOT load the entire Stash tag universe when only the bounded semantic
  vocabulary is needed.
- List views MUST paginate server-side and prefetch the next page ahead of the scroll position.
- Responses SHOULD be cached with explicit invalidation: a mutation the viewer performs MUST
  invalidate or patch exactly the cache entries it affects; a manual "refresh / clear cache"
  control MUST exist.
- New GraphQL operations MUST be reviewed for request count and payload size as part of the
  plan's Constitution Check.

**Rationale**: The Stash web UI and naïve plugins pay for chatty, over-fetching queries on every
navigation. Batching and indexing locally is where a native client earns its speed.

### V. Native Playback Over Transcoding

- Video playback MUST use mpv (libmpv embedded, or a managed mpv process where embedding is not
  viable on a platform) playing Stash's direct stream endpoint, so the original file is decoded
  locally with hardware acceleration.
- The viewer MUST NOT request Stash's transcoded streams (HLS/DASH/MP4 transcode endpoints) by
  default. Transcoding MAY be used only as an explicit, user-visible fallback when mpv cannot play
  a source, and the fallback MUST be logged.
- Streams MUST authenticate with the profile's configured API key.
- Playback features that depend on Stash data (resume position, play count, O-counter, markers,
  Timeline slots/Stages as chapters) MUST sync back to Stash per Principle I.
- Images and image decks MAY render in the webview but MUST use appropriately sized
  thumbnails/previews for grids and load full resolution only on demand.

**Rationale**: Transcoding is the single most expensive thing the Stash web UI asks of a server.
mpv handles virtually every codec natively; pushing decode to the client makes playback instant
and makes low-power Stash hosts viable.

### VI. Responsive, Near-Native UI

- The UI thread MUST never block on network or disk. All Stash I/O and heavy computation run in
  the Rust core asynchronously, with loading states rendered immediately.
- Performance budgets (on a mid-range desktop against a LAN Stash with a warm cache):
  - Input acknowledgement (hover, press, focus) < 50 ms; view navigation first paint < 150 ms.
  - Grid/list scrolling sustains 60 fps; large collections MUST use virtualization.
  - Local semantic queries over an already-indexed library return in < 200 ms.
  - Cold start to an interactive home view < 2 s (excluding first-run index build, which MUST
    show progress and remain cancellable).
- The UI MUST be fully keyboard-navigable and SHOULD support remote/controller-style navigation
  for a "10-foot" mode, in the spirit of Jellyfin clients.
- The product is opinionated: when a Stash web UI behaviour conflicts with speed or clarity, the
  viewer MAY redesign how it is presented, but MUST NOT drop the capability itself
  (Principle VIII). Navigation, tabs, and notifications follow Principle IX.

**Rationale**: "Near-native" is a measurable promise. Budgets make regressions visible and keep
the experience from sliding back toward a sluggish web page in a window.

### VII. User-Owned Connection

- The user supplies the Stash endpoint URL and optional API key; the viewer MUST work with
  API-key-less instances and with remote (non-LAN, HTTPS) instances. Multiple saved server
  profiles SHOULD be supported.
- The API key is ordinary profile configuration. It is stored with the rest of the profile in the
  viewer's local config file, and the UI MAY display and edit it. The viewer MUST NOT add OS
  keyring integration, session-only key modes, or other key-hiding machinery unless this
  principle is amended.
- The viewer MUST NOT phone home. Any network destination other than the user's configured Stash
  server(s) requires an explicit feature spec and user opt-in.
- TLS certificate validation is OFF by default, because typical self-hosted Stash instances use
  plain HTTP or self-signed certificates. Each server profile MUST offer a setting to turn strict
  validation on, and the connection UI MUST show plainly whether the current connection is
  unencrypted, unverified, or verified.
- Destructive operations against the library (deleting media, bulk tag removal, schema
  migrations) MUST require confirmation and SHOULD be previewable.

**Rationale**: The viewer is a personal client for the user's own Stash server. Protecting the API
key from the user's own machine isn't a goal. Plain config keeps setup simple and portable, and
avoids platform-specific keyring failures. TLS validation defaults to off for the same reason:
connecting to a typical home-lab Stash should just work, while the security state stays visible
and strict checking stays available per server.

### VIII. Stash Web UI Feature Parity

- The viewer MUST support every user-facing capability of the default Stash web UI in the
  minimum supported Stash version, so a user never has to go back to the browser for routine
  library work. This includes at minimum:
  - Browsing, filtering, sorting, and saved filters for Scenes, Images, Galleries, Groups/Movies,
    Performers, Studios, Tags, and Markers, including the Gallery view.
  - Viewing and editing metadata on every entity type, including tag editing (create, rename,
    merge, parents/children, aliases) and bulk edits.
  - Scene markers: creating, editing, deleting, and jumping to them during playback.
  - Scrapers: scraping by URL, name, and fragment, plus StashBox identify and tagger workflows,
    with a review step before scraped data is applied.
  - Ratings, O-counter, play history, resume points, and organized flags.
  - Library tasks exposed by the web UI (scan, generate, identify, auto-tag, clean) and their
    job progress.
- Parity is judged by capability, not pixel layout. The viewer MAY present a feature differently,
  combine screens, or improve workflows, but a capability MUST NOT be silently dropped.
- Server administration settings (Stash's System/Settings pages) are in scope unless a feature
  spec explicitly defers them, stating why and linking them to the web UI instead.
- Capabilities MUST be built on Stash's public GraphQL API, following Principles I and IV. Each
  feature spec MUST list the web UI capabilities it covers so overall parity can be tracked.
- When a new Stash stable release adds a UI capability, closing the gap is tracked backlog work,
  as with Principle II.

**Rationale**: Semantic tagging is the reason to choose this viewer, but people will only live in
it if it fully replaces the web UI. A fast client that still sends users to the browser to add a
marker or run a scraper fails the "Jellyfin-style client" promise.

### IX. One App Shell: Navigation, Tabs, and Notifications

- Every feature MUST live inside one consistent app shell. A new feature fits into the shell as a
  navigation section, a kind of tab, a settings page, or a notification. It MUST NOT add its own
  navigation scheme or a stand-alone screen that bypasses the shell.
- **Navigation bar**: a persistent navigation bar MUST give one-click (and keyboard) access to
  each top-level section, modelled on the Stash web UI's (Scenes, Images, Galleries, Groups,
  Markers, Performers, Studios, Tags), plus Settings and the notification centre. Sections are
  added to it as their features land.
- **Tabs**: users MUST be able to keep several views open at once inside the main window, e.g. a
  scene list, a gallery, and a performer, and switch between them. Each tab MUST keep its own
  state (scroll position, filters, selection) while in the background, so switching back meets
  Principle VI's navigation budget without reloading. Opening, closing, and switching tabs MUST
  be keyboard-accessible.
- **Notification centre**: one place MUST collect events the user may need to know about:
  - connection alerts (server unreachable, reconnected, authentication failures);
  - failures from background work;
  - Stash jobs (scan, generate, identify, auto-tag, clean, plugin tasks), with live progress and
    cancellation where Stash allows it.

  Important events MAY also appear briefly as a toast, but every notification MUST stay in the
  centre until dismissed, and a badge MUST show unread or active items. Notifications MUST NOT
  steal focus or block interaction.
- **Quiet infrastructure**: internal machinery (caches, indexes, prefetching) MUST work without
  the user managing it. It MUST NOT show its own banners or prompts on the main screens. It
  surfaces only through the notification centre when the user needs to act, plus a
  troubleshooting control in Settings where one is needed.
- **Windows**: the main window with tabs is the primary model. Additional windows (for example a
  pop-out player on a second monitor) MAY be added by a feature spec, provided they stay part of
  the same shell and state.

**Rationale**: The viewer will grow to cover the whole Stash web UI plus semantic tagging.
Without one shell, each feature invents its own navigation and alerts, and the app turns into a
maze. A Stash-like navigation bar keeps it familiar to Stash users. Tabs make moving between
scenes, galleries, and performers instant and stateful, which fits a desktop client better than
browser-style back-and-forth. A single notification centre gives connection problems and
long-running Stash jobs one predictable home, and keeps infrastructure out of the way.

## Technology & Platform Constraints

- **Application shell**: Tauri (v2 or later) with a Rust backend. Business logic lives in Rust
  per Principle III.
- **Frontend**: a web UI rendered in the Tauri webview. The specific framework is chosen in the
  first implementation plan and MUST satisfy Principle VI's budgets; it performs no direct
  network I/O.
- **Stash API**: GraphQL only, via a single Rust adapter module. Typed operations (e.g. generated
  from Stash's schema) are preferred over hand-built query strings.
- **Playback**: mpv/libmpv as the sole default video pipeline (Principle V).
- **Local persistence**: limited to caches, derived semantic indexes, preferences, and profile
  profiles, including their API keys (Principles I and VII).
- **Target platforms**: Linux is the primary development and release platform; Windows and macOS
  SHOULD be supported where Tauri and mpv allow, and platform-specific gaps MUST be documented.
- **Stash compatibility**: the minimum supported version is **Stash v0.31.1** (latest stable at
  ratification). It provides `custom_fields` on Tags, Scenes, Images, and Performers. The viewer
  MUST check the server version on connect and refuse to proceed, with a clear message, when the
  server is older. Raising the minimum is a MINOR amendment to this constitution.
- **Dependencies**: each new crate or npm package MUST be justified in the plan (what it replaces,
  its size, its maintenance status). Prefer mature, widely used crates.

## Development Workflow & Quality Gates

- **Spec-driven flow**: features proceed through `/speckit-specify` → `/speckit-plan` →
  `/speckit-tasks` → `/speckit-implement`. Every plan MUST include a Constitution Check that
  explicitly addresses Principles I–IX, and any deviation MUST be recorded in the plan's
  Complexity Tracking table with justification.
- **Parity check**: specs touching semantic data MUST cite the semantic-tagging plugin modules
  they mirror and describe how compatibility with the plugin's data is verified (Principle II).
- **Testing**:
  - Rust core: unit tests for the adapter (against recorded GraphQL fixtures), semantic parsing,
    index building, query evaluation, and archetype inheritance.
  - Compatibility tests: fixtures captured from a real library tagged by the plugin MUST
    round-trip through the viewer's read → write path unchanged.
  - Integration/E2E tests against a disposable Stash instance for connection, auth, and write
    paths where practical.
- **Performance gates**: changes to list views, GraphQL operations, or the index MUST state their
  expected request count and be checked against Principle VI's budgets before merge.
- **Hygiene**: `cargo fmt`, `cargo clippy` (no warnings), and the frontend's formatter/linter MUST
  pass. Commit after every working change; small, reviewable commits are preferred over large
  multi-feature drops.
- **Secrets**: no API keys, endpoints of real private servers, or captured media in the repo or
  test fixtures; fixtures MUST be scrubbed.

## Governance

- This constitution supersedes other project practices and conventions. Where a guidance file
  (README, CLAUDE.md, agent instructions) conflicts with it, the constitution wins until amended.
- **Amendments**: proposed via `/speckit-constitution`, with a Sync Impact Report describing the
  change, its rationale, and any affected specs or plans. The project owner approves amendments.
- **Versioning** (semantic):
  - MAJOR — a principle is removed or redefined in a backward-incompatible way.
  - MINOR — a principle or section is added, or guidance is materially expanded.
  - PATCH — clarifications, wording, or typo fixes with no change in meaning.
- **Compliance review**: every `/speckit-plan` Constitution Check and every code review MUST
  verify compliance with Principles I–IX. Unjustified violations block merge. Justified
  exceptions are recorded in the plan's Complexity Tracking and revisited when the related
  feature is next touched.
- **Periodic review**: when the semantic-tagging plugin gains a significant new feature or data
  structure, Principle II's parity expectations MUST be re-evaluated and parity work added to the
  backlog.

**Version**: 3.1.0 | **Ratified**: 2026-09-23 | **Last Amended**: 2026-09-27
