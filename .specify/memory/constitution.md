# Semantic Stash Viewer Constitution

Semantic Stash Viewer is an opinionated, native desktop frontend for the
[Stash](https://github.com/stashapp/stash) media server, built entirely in Rust with the iced
toolkit. Its goals:
(1) build the semantic model described in the `semantic-tagging` plugin's design documents
(`~/Projects/Stash-Plugins/semantic-tagging/`) around a semantic search engine, and deliver the
plugin's features (and the ones its documents plan) through that engine; (2) deliver a
responsive, near-native browsing experience with optimized GraphQL usage and mpv as the one
media player for video and images, avoiding the Stash web UI's expensive transcoding; and (3) cover every capability of the
default Stash web UI, so the viewer can fully replace it. A user points it at any Stash endpoint
(plus API key, if one is configured) and uses their library the way Jellyfin desktop clients let
users use a Jellyfin server.

## Core Principles

### I. Stash Is the System of Record

- Stash owns media, scenes, images, galleries, performers, tags, metadata, and persistence. The
  viewer MUST NOT introduce its own authoritative database for library data.
- Every write (tag assignments, semantic relationships, timeline entries, Variations, the
  archetype hierarchy, ratings, metadata edits) MUST be persisted to Stash through its GraphQL
  API — typically as tags or `custom_fields` on Tags, Scenes, Images, and Performers.
- Local storage is permitted only for caches, derived indexes, UI preferences, and connection
  profiles. Any local cache MUST be safely discardable: deleting it may cost time, never data.
- The viewer MUST NOT require a server-side component beyond a stock Stash instance (plus the
  semantic-tagging plugin's data conventions, which live in Stash itself).

**Rationale**: Users already trust Stash with their library. A client that forks the source of
truth creates sync bugs and lock-in; a client that is a pure lens on Stash can be installed,
removed, or run on several machines without risk.

### II. The Semantic Model Comes from the Plugin's Design Documents (NON-NEGOTIABLE)

- **Design documents, not code.** The semantic features are defined by the semantic-tagging
  plugin's design documents (today `README.md`, `semantic-query-engine-roadmap.md`, and
  `archetype-system-plan.md` in the plugin repository, plus any the owner adds). The viewer
  builds what those documents describe; it MUST NOT port the plugin's JavaScript implementation,
  which started as a small role-and-action tagger and is being decoupled and reworked toward the
  same documents.
- **Living documents.** The documents change (for example the Variation, Label, and Archetype
  reform of Milestone 1D). Each feature spec touching semantic data MUST cite the document
  sections it builds on and the plugin commit (or date) it read them at, so the work can be
  re-checked when they change. Where the plugin's code and its documents disagree, the documents
  win, and the disagreement is noted for the plugin.
- **One data contract.** What the documents put in Stash is a shared contract: the viewer MUST read
  and write the same tag flags and `custom_fields` as the documents (and the plugin, where it
  implements them) so both can work on the same library concurrently without corrupting each
  other's data. The documents hold the current list; at this amendment it includes per-type tag
  flags (`plugtag`, `isAction`, `isRole`, `isTrait`, `isLabel`, `isMiseEnScene`, `isVariation`,
  as 0/1 values), Variations as real tags carrying their own reverse-indexed `custom_fields`,
  common variants (`variantOfTagId`), Label formulas, Timeline data, and Archetypes as
  performers with parents-only adjacency.
- **Data scopes** are kept as the documents define them (for example scene- and image-scoped
  Roles, Actions, Pluggables, and Timeline data; performer-global Traits and Labels; a
  Variation's facts on its own tag; archetype children derived in memory).
- Any divergence from the contract MUST be deliberate, documented in the feature spec, and come
  with a migration or compatibility shim — never an accidental side effect.
- **Tag colours** are part of the shared semantic data. The plugin colours semantic tags by type
  today; the viewer colours tags by **colour rules** keyed on tag flags (Principle XI), with a
  distinct default colour for each semantic type, stored in Stash in a documented format the
  plugin can read, so both show the same colours for the same library. Colours MUST NOT be
  written onto tags.
- **Query semantics** (semantic search, the Search Matrix, Quick Pairings, recommendations, Label
  evaluation, archetype inheritance, and the SPARQL-style queries the roadmap plans) MUST follow
  the documents' definitions, so the same library state gives the same answers in the viewer
  and in a plugin that implements the same documents.

**Rationale**: The viewer's reason to exist is to be the best home for semantic tagging. The
plugin's code reflects where it started, not where it is going; its documents are the design both
are converging on. Sharing the data contract keeps the two safe to use side by side, and building
from the documents keeps the viewer from inheriting the plugin's early shortcuts.

### III. Brain/UI Separation

- Domain logic — Stash I/O, semantic vocabulary, index/graph building, query evaluation,
  archetype inheritance, caching — MUST live in Rust crates that have no dependency on the UI
  toolkit (iced), its renderer, or any windowing code.
- The UI MUST reach Stash only through the core's typed, asynchronous API (calls that return typed
  results, run off the UI thread, and come back to the UI as messages); it MUST NOT issue its own
  GraphQL requests or open its own connections to Stash.
- Layering inside the core: a single Stash adapter owns all network I/O; the semantic index and
  engine (Principle XII) consume the adapter; the UI consumes the core. Cross-layer shortcuts are
  violations.
- Core modules MUST be unit-testable headlessly (no window or GPU, and ideally no running Stash,
  via recorded/fixture GraphQL responses).

**Rationale**: The plugin's roadmap already established that "the brain" must be independent of
React. Carrying that into Rust makes the engine fast, testable, and reusable (CLI tools, future
background indexing) without redesign. With the UI in Rust too, the boundary is a crate boundary
rather than a serialisation bridge: views get typed data from the core and its cache directly, with
no command layer or generated bindings to keep in sync.

### IV. Lean, Batched GraphQL

- Prefer "bulk fetch → local semantic index → many local queries" over request-per-interaction.
  N+1 query patterns (a request per card, per performer, per tag) are prohibited in list and grid
  views.
- Every GraphQL operation MUST select only the fields the consuming view needs; shared fragments
  are defined per view tier (card, detail, editor) rather than one "fetch everything" fragment.
- Queries against semantic vocabulary MUST be narrowly scoped using tag flags / `custom_fields`
  filters evaluated by Stash: semantic tags of a type are found with "flag is not null" (and
  ordinary tags with "every flag is null"), never by loading every tag and testing flags locally.
  The viewer MUST NOT load the entire Stash tag universe when only the bounded semantic
  vocabulary is needed.
- **Paged lists**: list and grid views of library items MUST be paged: one page of results at a
  time, with page navigation (previous, next, first, last, and jump to a page), the total count,
  and a page size the user can change, defaulting to **50** items per page. This applies in
  particular to the Scenes and Galleries grids, and to the items inside a gallery (Principle X).
  The viewer fetches only the page being shown (server-side pagination) and MAY prefetch the
  adjacent pages so moving between pages is instant. Infinite (continuous) scrolling MUST NOT be
  the default for library listings.
- Responses SHOULD be cached with explicit invalidation: a mutation the viewer performs MUST
  invalidate or patch exactly the cache entries it affects; a manual "refresh / clear cache"
  control MUST exist.
- New GraphQL operations MUST be reviewed for request count and payload size as part of the
  plan's Constitution Check.

**Rationale**: The Stash web UI and naïve plugins pay for chatty, over-fetching queries on every
navigation. Batching and indexing locally is where a native client earns its speed.

### V. mpv Is the Media Player (Native Over Transcoding)

- **mpv is the one media player.** Video playback, full-resolution image viewing, slideshows, and
  mixed sequences of images and videos (Principle X) MUST use mpv (libmpv embedded, or a managed
  mpv process where embedding is not viable on a platform), decoding locally with hardware
  acceleration where available.
- **Video** MUST play Stash's direct stream endpoint. The viewer MUST NOT request Stash's
  transcoded streams (HLS/DASH/MP4 transcode endpoints) by default. Transcoding MAY be used only as
  an explicit, user-visible fallback when mpv cannot play a source, and the fallback MUST be
  logged.
- **Images** opened for viewing MUST display in mpv at full resolution, loaded on demand; the
  viewer MAY prefetch the items next to the current one. Zoom, pan, rotation, fit, and animated
  images (GIF, WebP, and the like) are handled by mpv.
- **mpv's picture is part of the app window.** Video and full-resolution images MUST be drawn
  inside the viewer's own window, with the viewer's controls on top, never in a separate player
  window (except a pop-out a feature spec adds per Principle IX).
- **The UI toolkit plays no media.** The UI MUST NOT decode or play video or audio itself, or use
  any second playback pipeline, including for previews. Grids and lists MUST show appropriately
  sized thumbnails as images; moving previews MUST be image formats (for example Stash's animated
  WebP previews), with the still thumbnail as the fallback.
- Streams and images MUST be fetched with the profile's configured API key, sent in a header and
  never placed in URLs or logs.
- Features that depend on Stash data (resume position, play count, O-counter, markers,
  Timeline slots/Stages as chapters) MUST sync back to Stash per Principle I.

**Rationale**: Transcoding is the single most expensive thing the Stash web UI asks of a server.
mpv handles virtually every video and image format natively, fast and colour-accurately; pushing
decode to the client makes playback instant and makes low-power Stash hosts viable. One player for
both media types is what lets galleries mix images and scenes seamlessly (Principle X), and keeping
the UI out of playback avoids a second, weaker pipeline with different formats and behaviour.

### VI. Responsive, Near-Native UI

- The UI thread (the event loop that updates and draws the UI) MUST never block on network or
  disk. All Stash I/O, image decoding, and heavy computation run in the Rust core asynchronously
  or on worker threads, with loading states rendered immediately.
- Performance budgets (on a mid-range desktop against a LAN Stash with a warm cache):
  - Input acknowledgement (hover, press, focus) < 50 ms; view navigation first paint < 150 ms.
  - Scrolling within a page keeps up with the display. Measured on a 144 Hz display, scrolling
    the scene grid misses at most 3% of frames, and other scrolled views (lists, the wall) at
    most 1%; moving to another page meets the navigation budget. Paging (Principle IV) bounds
    what is rendered; virtualization is needed only where a view renders more items at once
    than it can keep within budget (for example, a large page size the user chose).
  - Local semantic queries over an already-indexed library return in < 200 ms.
  - Cold start to an interactive home view < 2 s (excluding first-run index build, which MUST
    show progress and remain cancellable).
  - Playback shows every frame mpv renders: dropped-frame budgets count frames that never reached
    the screen, not only frames mpv failed to decode or render, and measurements MUST check the
    rate frames are actually displayed.
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
- Parity never means imitation. Where the web UI's design is weak, the viewer SHOULD redesign it
  rather than copy it; the gallery viewer is the prime example and follows Principle X.
- Parity is a floor, not a ceiling: capabilities the web UI lacks (tag colours, Principle XI; the
  semantic engine, Principle XII; the
  semantic features, Principle II; mixed galleries, Principle X) are welcome, under the same
  principles as everything else.
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
  state (page, page size, display mode, scroll position, filters, selection) while in the
  background and across back/forward, so leaving a view and returning never resets it, and
  switching back meets Principle VI's navigation budget without reloading. Opening, closing,
  and switching tabs MUST be keyboard-accessible.
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

### X. Galleries Beyond Stash: Images and Scenes Together

- Galleries MUST treat **scenes as first-class members** alongside images. A gallery's view shows
  its images and the scenes related to it in one browsable sequence, and opening any item plays or
  shows it in the same viewer (mpv, Principle V), moving between images and scenes without leaving
  the viewer.
- The gallery experience (the grid, the viewer, navigation, slideshows, chapters) MUST be designed
  for this viewer from first principles. It MUST NOT copy the Stash web UI's gallery viewer's
  layout or interaction model; Stash is consulted only for its data model and to confirm no
  capability is lost (Principle VIII).
- Every gallery capability Stash offers (browsing and filtering galleries, the images inside a
  gallery, covers, chapters, zip and folder galleries, image details) MUST still be available.
  An images-only way to view a gallery MUST remain available for users who want Stash's behaviour.
- Which scenes belong with a gallery, and any user-defined ordering of a mixed sequence, MUST come
  from or be persisted to Stash (relationships, tags, or `custom_fields`) per Principle I, never
  kept only in a local database.
- The gallery grid, and the images and scenes inside a gallery, are paged like every library
  listing (Principle IV: default 50 per page, user-selectable).
- The viewer MUST meet Principle VI's budgets: moving to the next or previous item acknowledges
  input within 50 ms and shows the item within 150 ms on a LAN server with neighbours prefetched.

**Rationale**: In Stash, galleries are image-only and the gallery viewer is weak, while real
collections mix photo sets and videos of the same shoot or subject. A native client with one fast
media player can present them together, which is a reason to choose this viewer over the web UI.
Designing it fresh, instead of porting Stash's viewer, avoids inheriting its problems.

### XI. Visual Identity & Tag Colours

- **App palette**: the app uses one defined palette (dark blue base; rosewood, yale blue,
  cerulean, frosted blue, and icy aqua accents, as set by the user in 007), applied through the
  theme, never hard-coded per screen.
- **Semantic features wear a light RPG theme.** The semantic manager and the semantic-tagging
  features (Roles, Actions, Traits, Labels, Pluggables, Variations, Mise-en-scène, the Timeline,
  Archetypes, semantic search and the Search Matrix) MUST be lightly themed with the RPG palette below: accents,
  panel frames, headers, and highlights within those features, not a reskin of the whole app.
  Text MUST stay legible (at least 4.5:1 contrast for body text) on both palettes.

  | Role | Colours |
  |---|---|
  | Shadows and panels (dark to light) | `#0d0a05` `#1c1508` `#2e2010` `#4a3420` |
  | Leather and parchment | `#7a5c38` `#b08858` `#d4b880` `#f0e0b0` |
  | Gold (emphasis, selection) | `#c8952a` |
  | Greens (positive, growth) | `#7dd87a` `#3a9e50` |
  | Blues (information, links) | `#5b8fd4` `#2a5090` |
  | Red (danger, conflict) | `#d44040` |
  | Purple (rare, arcane) | `#8840c0` |
  | Light text | `#e8e0c8` |

- **Role-playing icons**: because semantic tagging is built on roles and relationships, the
  semantic manager's icons MUST be a consistent role-playing-inspired set (for example shields,
  scrolls, crowns, masks, swords, bonds), drawn the same way as the app's other icons (vector, tinted
  by the theme) and carrying their licence notices. Icons outside the semantic features keep the
  app's general set.
- **Tag colours come from colour rules, not from tags.** A colour rule pairs a tag flag (a tag
  custom field such as `isRole`) with a colour; a tag takes the colour of the first rule, in the
  user's order, whose flag it has set. Every semantic type MUST have a default rule with its own
  colour from the RPG palette, and the user MUST be able to change colours, add rules for other
  flags, reorder, and reset them in Settings.
  - **Storage**: the rules are library data, stored in Stash per Principle I (the UI
    configuration, under the viewer's own key), never only on one machine. Colours MUST NOT be
    written onto individual tags.
  - **Lookup**: the tags a rule colours MUST be found with a server-side "flag is not null" filter
    (Principle IV), never by reading every tag to find colours. Reading rules and matches MUST
    add no request per tag or per chip.
  - **Display**: wherever a tag appears (chips on cards and scene pages, filter chips and pickers,
    the Timeline, the semantic manager) it MUST show its rule's colour, with chip text kept
    legible (dark or light text chosen for at least 4.5:1 contrast).

**Rationale**: colour-coded tags are how the user reads semantic data at a glance, a capability
the plugin already relies on and the web UI lacks. A light, consistent RPG identity makes the
semantic features recognisable as their own space without making the rest of the app feel like a
game, and keeping colours in Stash means every client shows the same thing. Colours belong to
flags because flags are what Stash can filter on cheaply: a rule finds exactly its tags with one
"not null" filter and changing a colour is one small write, while a colour on every tag would need
a scan to find and a write per tag to change.

### XII. The Semantic Engine Is the Foundation

- The core MUST build a **semantic search engine**: a graph of semantic facts derived from Stash
  (scenes, images, performers, tags, and the relationships the design documents define), queried
  with a SPARQL-style language as the plugin's roadmap describes (`SELECT` with variables and
  joins first; `CONSTRUCT` and rules later).
- **Features are clients of the engine.** Semantic search, the Search Matrix, Quick Pairings,
  recommendations, Label evaluation, and archetype inheritance MUST be expressed as engine
  queries (or rules), not as separate hand-written filters per feature. A feature that needs a
  fact the engine lacks adds it to the engine.
- **A derived index** (Principle I): the engine's store is built from Stash, kept up to date as
  Stash changes, and safe to delete (rebuilt on demand). It never becomes the source of truth,
  and writes still go to Stash.
- **Built on an existing engine: Oxigraph, with Redland as the fallback.** The viewer embeds
  **Oxigraph** (Rust; SPARQL 1.1 Query and Update, SPARQL 1.2 behind a feature) rather than writing
  its own engine. **Redland** (librdf with Rasqal, C, through FFI under the unsafe-code gate) is
  the fallback, used only if Oxigraph proves unable to meet this principle or Principle VI's
  budgets, and only by an amendment that records why. The engine sits behind a core interface so
  it can be replaced without touching features. It runs in-process: no server to install
  (Principle I).
- **Disk store as a cache.** Oxigraph's on-disk store (RocksDB) MAY keep the engine's index
  between launches so it isn't rebuilt every time the app starts. It is a cache under Principle I:
  kept in the cache folder, one per server profile and never shared across servers, rebuilt when
  missing, damaged, or from an older format, and cleared with the app's other caches.
- **Fast and incremental**: building and querying run off the UI thread; the first build shows
  progress and can be cancelled (Principle VI); later updates are incremental, fetched with the
  lean, flag-filtered queries of Principle IV.

**Rationale**: The plugin began as a simple tagger and grew feature by feature, each with its own
filtering code; its roadmap concludes that a semantic layer with one query engine is the right
shape. Rust can host a real RDF/SPARQL engine in-process, so the viewer builds that foundation
first and lets every semantic feature, including the plugin's, sit on top of it.

## Technology & Platform Constraints

- **Native UI**: the whole application is Rust. The UI is built with **iced** (0.14 or later),
  drawn on the GPU through wgpu (Vulkan on Linux), with no web engine, webview, or JavaScript.
  Business logic lives in the core crates per Principle III; UI code holds view state and talks
  to the core only through its API. Moving to another native Rust toolkit is a MAJOR amendment.
- **Stash API**: GraphQL only, via a single Rust adapter module. Typed operations (e.g. generated
  from Stash's schema) are preferred over hand-built query strings.
- **Media**: mpv/libmpv as the only media player: video, full-resolution images, and mixed
  image/video sequences (Principles V and X), embedded through libmpv's render API into GPU images
  the UI draws (zero-copy, with hardware decoding where available). The UI renders thumbnails and
  animated image previews only.
- **Local persistence**: limited to caches, derived semantic indexes, preferences, and profile
  profiles, including their API keys (Principles I and VII).
- **Target platforms**: Linux (Wayland first, X11 where it works) is the primary development and
  release platform; Windows and macOS SHOULD be supported where iced, wgpu, and mpv allow, and
  platform-specific gaps (for example the GPU path that shares mpv's frames) MUST be documented.
- **Stash compatibility**: the minimum supported version is **Stash v0.31.1** (latest stable at
  ratification). It provides `custom_fields` on Tags, Scenes, Images, and Performers. The viewer
  MUST check the server version on connect and refuse to proceed, with a clear message, when the
  server is older. Raising the minimum is a MINOR amendment to this constitution.
- **Semantic engine**: Oxigraph, embedded and in-process, with its RocksDB disk store as a
  rebuildable cache (RocksDB is a system requirement, like libmpv); Redland (librdf, Rasqal) is the
  fallback, through FFI under the unsafe-code rules below (Principle XII).
- **Dependencies**: each new crate MUST be justified in the plan (what it replaces, its size, its
  maintenance status). Prefer mature, widely used crates, and crates already in the dependency tree.
- **Transition from the Tauri build**: the Tauri/webview build (features 001–005) was replaced
  by the native app and removed in feature 007. Specs 001–005 keep their user-facing
  requirements; their implementation plans are superseded.

## Development Workflow & Quality Gates

- **Spec-driven flow**: features proceed through `/speckit-specify` → `/speckit-plan` →
  `/speckit-tasks` → `/speckit-implement`. Every plan MUST include a Constitution Check that
  explicitly addresses Principles I–XII, and any deviation MUST be recorded in the plan's
  Complexity Tracking table with justification.
- **Parity check**: specs touching semantic data MUST cite the design-document sections they
  build on (with the plugin commit or date they were read at) and describe how compatibility
  with the shared data contract is verified (Principle II).
- **Testing**:
  - Rust core: unit tests for the adapter (against recorded GraphQL fixtures), semantic parsing,
    index building, query evaluation, and archetype inheritance.
  - Engine: query tests over fixture graphs, including the worked examples in the design
    documents, so a document's example query and its expected answer are a test.
  - Compatibility tests: fixtures captured from a real library tagged by the plugin MUST
    round-trip through the viewer's read → write path unchanged.
  - Integration/E2E tests against a disposable Stash instance for connection, auth, and write
    paths where practical.
  - UI: view state and its transitions (paging, keyboard moves, tabs, history) are plain Rust
    and unit-tested; screen flows are tested headlessly with iced's test support, without a
    server.
- **Performance gates**: changes to list views, GraphQL operations, or the index MUST state their
  expected request count and be checked against Principle VI's budgets before merge.
- **Hygiene**: `cargo fmt` and `cargo clippy` (no warnings) MUST pass for the whole workspace,
  UI included. Commit after every working change; small, reviewable commits are preferred over large
  multi-feature drops.
- **Unsafe code**:
  - `unsafe` is allowed only at foreign-function boundaries: libmpv, EGL and OpenGL, Vulkan
    interop through wgpu-hal, libwayland, and similar C libraries. It lives in small modules
    behind safe wrapper types (owning handles freed on drop), so UI and feature code never
    writes `unsafe`.
  - Core crates with no foreign-function work (`stash-core`) MUST declare
    `#![forbid(unsafe_code)]`.
  - Every `unsafe` block MUST carry a `// SAFETY:` comment naming the invariant it relies on,
    and SHOULD perform one unsafe operation. Every `unsafe fn` MUST document its preconditions
    under `# Safety`.
  - Crates that contain `unsafe` MUST deny `clippy::undocumented_unsafe_blocks`,
    `clippy::multiple_unsafe_ops_per_block`, and `unsafe_op_in_unsafe_fn`; an exception is
    allowed only on a single item, with a comment saying why.
  - A plan that adds `unsafe` MUST list it in its Constitution Check: where it is, which
    boundary it serves, and the safe type that hides it.
  - Lifetime tricks (`transmute` to `'static`, raw pointers between Rust objects) are not a
    foreign-function need: use ownership instead (an owner that outlives the borrowers, or a
    self-referential cell crate).
  - The frozen Tauri build is exempt until it's removed.
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
  verify compliance with Principles I–XII. Unjustified violations block merge. Justified
  exceptions are recorded in the plan's Complexity Tracking and revisited when the related
  feature is next touched.
- **Periodic review**: when the plugin's design documents gain a significant new feature or data
  structure (or change an existing one), Principle II's expectations MUST be re-evaluated and the
  work added to the backlog.

**Version**: 5.1.0 | **Ratified**: 2026-09-23 | **Last Amended**: 2026-10-10
