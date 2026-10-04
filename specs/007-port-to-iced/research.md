# Research: Port the Viewer to iced

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md) | **Date**: 2026-10-04 (revised
after the 2026-10-03 clarifications and the library analysis)

Starting point: feature 006's `native-ui` crate (iced 0.14, the zero-copy mpv video path, the
player screen, a copy of the demo's app glue). The Tauri demo and specs 001–005 supply the
**capabilities** and the **behaviours that stay** (spec Overview); screens, flows, saved formats,
and code structure are designed fresh. Nothing is kept for compatibility with the demo.

## R1. The UI is a hierarchical state machine built from iced's own state

- **Decision**: the whole UI is one **hierarchical state machine (HSM)** expressed as iced state:
  nested Rust types where an enum is an exclusive choice of sub-states and a struct is a set of
  regions active at the same time (statecharts' orthogonal regions). There's no separate state
  machine runtime: iced's `App` owns the tree, `update` performs the transitions, `view` renders
  the active leaves.

  ```text
  App ─┬─ Onboarding                         (no saved server yet)
       └─ Session (one active server) ─┬─ connection: Connecting | Connected | Offline
                                       │               | AuthFailed | Failed
                                       ├─ shell: Shell ─┬─ tabs: [Tab] + selected
                                       │                └─ overlay: None | KeyboardHelp
                                       │                            | Notifications | ServerMenu
                                       └─ playback: Idle | Opening | Playing | Ended | Error
                                                        (owner tab; controls Shown | Hidden;
                                                         fullscreen)
  Tab ── history: [Screen] + cursor
  Screen = Home | Scenes(ScenesState) | Scene(SceneState) | Settings(SettingsState)
  ScenesState ── query, size, mode, scroll, page: Loading | Ready | Unreachable
  SettingsState ── page: Servers(ServersState) | Keyboard | Troubleshooting | About
  ```

  Connection, shell, and playback are orthogonal regions of Session: going offline doesn't tear
  down the shell (cached screens keep working, 003), and playback outlives tab switches (004).
- **Messages mirror the tree** (`Message::Session(SessionMsg::Shell(ShellMsg::Tab(id,
  TabMsg::Screen(ScreenMsg::Scenes(…)))))`), and each level's `update` returns a `Step`:
  - the **effects** to run (R2),
  - and optionally an **event for its parent**: anything a state doesn't handle bubbles up. For
    example, Scenes emits "open scene 123"; the Tab handles it by pushing a Scene screen; a
    Settings server change bubbles to Session, which switches servers.
- **Entry and exit**: each state has `enter` and `exit` functions returning effects. Entering
  Scenes asks for its page; exiting a Scene screen that owns playback hands the player back.
  Transitions are functions that run the old state's `exit` and the new state's `enter`.
- **No routes**: a tab's history holds **whole `Screen` states**, so back and forward return to the
  exact state left behind ("a tab never resets" holds by construction, not by re-loading a
  route's saved view state, which was where the demo's reset bugs came from). Opening a view is a
  transition that pushes a new `Screen`.
- **Rationale**: Rust's enums and structs are already a statechart notation. iced's
  update and view are the transition and output functions. Keeping one owner of state follows
  iced's model, needs no locks between the machine and the view, and makes every transition
  a plain function that can be tested.
- **Alternatives rejected**:
  - **hsmc** (0.6.0, statecharts via a proc macro): it runs its own async runtime and event queue
    and keeps state in a context it mutates, which competes with iced's `update`/`Task` model. It
    has no serde (so FR-007's full restore would be hand-built) and no history states. It's
    pre-1.0, created in April 2026, with about 300 downloads, and aimed at embedded systems.
  - **statig** (0.4, mature HSM library): workable, but it adds a second state model alongside
    iced's. **Kept as the fallback** if hand-written transitions become error-prone.
  - **Named routes with saved view state** (the demo's design): rejected per the clarification.

## R2. Effects as data: work leaves `update` through action triggers

- **Decision**: `update` never does I/O, decoding, or blocking work. Transitions and entry/exit
  actions return **`Effect` values** (data such as `LoadScenesPage { query, page, size }`,
  `LoadThumbnail { id, version }`, `OpenScene { id }`, `SaveSession`, `ClearCache`). One
  `effects` module at the root turns each into an iced `Task` that calls the service layer; the
  core fetches **and deserializes** off the UI thread and returns a typed message (`PageLoaded`,
  `ThumbnailReady`, …). That is "offloading deserialization to action triggers": the trigger is
  the transition, the work runs in a task, and only typed results re-enter the machine.
- Stale results are dropped by a generation number carried in the effect and checked when its
  message arrives (as the demo's page loader did).
- **Testing**: transitions are tested as pure functions, giving a state and a message, then checking
  the new state and the effects. For example, entering Scenes emits exactly one `LoadScenesPage`;
  back emits none. No iced runtime, no server.
- **Rationale**: it makes constitution VI ("the UI thread never blocks") checkable, keeps
  `update` deterministic, and turns iced's opaque `Task`s into assertable data.

## R3. Saved session: a snapshot of the state tree

- **Decision**: the session's tabs are saved as a **versioned serde snapshot of the state tree**
  (`session.json`, contract [session-snapshot.md](contracts/session-snapshot.md)): every tab, its
  full history of `Screen` states, the cursor, the selected tab, and each screen's persistent
  fields, scroll positions included (spec FR-007, clarified: restore everything). Loaded data
  (page items, image handles, details) is marked transient and isn't saved; on restore, each
  active screen's `enter` re-requests it, through cache first (003), so restored tabs show at
  once.
- A snapshot with another version, or one that fails to parse, is discarded and the app opens a
  fresh Home tab. There are no migrations; per the clarification, convenience never outranks the
  design.
- Saved 500 ms after a change that affects persistent fields, and on quit. Never during `view`.

## R4. The native app's own files

- **Decision**: all of the native app's data lives under its own directories, named by its app
  id **`dev.semantic-stash-viewer`**, under each platform directory
  (`~/.config/dev.semantic-stash-viewer/`, `~/.local/share/…`, `~/.cache/…` on Linux). These are
  separate from the demo's `semantic-stash-viewer/` folders, which it never reads (spec FR-005).
  - `profiles.json`, through the core's `ProfileStore` (a core-owned format, not the demo's UI).
  - `session.json` (R3).
  - `notifications.json`, through the core's `NotificationCenter`.
  - `cache/<profile>/cache.sqlite3`, through the core's view cache.
  - `logs/`.
- After the demo is removed, its folders can be deleted by hand; the README says so.

## R5. GraphQL in the core: Cynic replaces `graphql_client`

- **Decision**: `stash-core`'s adapter moves from `graphql_client` (one generated type set per
  `.graphql` file, 7 operations today) to **Cynic** 3.14 (code-first: queries and fragments are
  Rust structs checked against the schema at compile time):
  - **Schema**: registered in `stash-core/build.rs` from Stash **v0.31.1**'s SDL (the
    constitution's minimum version, from `stash/graphql/schema/` in the Stash source), committed
    to the repo with the fetch steps documented; `schema.json` stays only if a test needs it.
  - **Fragments per view tier** (constitution IV): `SceneCardFields` (grid card),
    `PlayableSceneFields` (player), shared by the queries that need them; new views reuse or add
    fragments instead of growing queries.
  - **Transport**: the adapter keeps its own `reqwest` client (headers, API key, the lenient-TLS
    option of Principle VII) and sends Cynic's operation as the JSON body, decoding Cynic's
    typed response. Cynic's optional `http-reqwest` helper is skipped so nothing changes in how
    requests are made.
  - **Custom scalars**: Stash's `Map`/`Any` (`custom_fields`, which the semantic plugin will need)
    map to `serde_json::Value`; timestamps to strings as today.
  - **Subscriptions**: the hand-written `graphql-transport-ws` jobs client stays. Checked in T007:
    `graphql-ws-client` could run over a WebSocket the core opens itself (so the lenient-TLS
    connector would still work), but the existing client is small and tested, and switching would
    add a dependency without adding a capability.
  - **Done (T005–T006, 2026-10-04)**: schema from tag `v0.31.1` (`graphql/stash-v0.31.1.graphql`);
    all six queries on Cynic; fragments `SceneCardFields`, `PlayableSceneFields`, `SceneRowFields`;
    scalars as newtypes (`Time`, `Int64`, `JsonMap`, `JsonAny`). Stash's snake_case fields carry
    explicit `rename`s. Cynic aliases use the Rust field name, so the test-set query's aliases
    became snake_case; the one recorded fixture and one inline test body that used the old
    camelCase aliases were updated (key names only). All 159 core tests pass.
- **Proof**: the core's existing tests (about 185, with recorded fixtures) pass unchanged; the
  fixtures still decode; request bodies stay card-sized (constitution IV).
- **Licence**: Cynic is MPL-2.0. That's file-level copyleft, which only applies to modified copies
  of Cynic's own files, so using it unmodified from this MIT project is fine (recorded in
  Complexity Tracking as a dependency note).
- **Rationale**: fragments become real shared Rust types; mutations get typed inputs before the
  writes arrive (spec clarification); the schema check catches drift at build time.
- **Alternative rejected**: keep `graphql_client` (per-query types; no shared fragment structs).

## R6. Keyboard and focus

- **Decision**: one keymap table (chord, which machine level it belongs to, action, label) serves
  the keyboard help overlay and Settings → Keyboard. Key events come from iced's event
  subscription with their capture status. Events a focused field captured (typing) are ignored.
  The rest go down the state tree like any message: the innermost active state that binds the
  chord handles it, otherwise it bubbles up (player keys while playing, grid keys in Scenes, shell
  keys anywhere). The player keeps 002's shortcuts exactly (a behaviour that stays).
- Every interactive element is reachable with Tab / Shift+Tab, and each state sets a sensible
  initial focus on entry (an entry effect).

## R7. Text input: fields, secrets, paste, IME

- **Decision**: iced 0.14's `text_input` for the server address, API key (`secure`, with
  show/hide), the page number, and Settings fields. The first US1 task checks typing, paste, and
  IME composition on KDE Wayland and records the result. If IME misbehaves, plain typing and paste
  still work, and IME goes in the known gaps rather than blocking.

## R8. Connection and failure messages

- **Decision**: the Onboarding state (no server) and the Session's connection region drive the
  first-run form, the indicator (state plus unencrypted / unverified / verified), and the
  details panel, designed fresh. Plain-language messages for every `ConnectFailure` and
  `AppError` are an exhaustive `match` in `native-ui/src/messages.rs`, so a new variant fails to
  compile until it has a message. Server management uses the core's `profiles::service`, and
  "last used" is recorded on connect.

## R9. Notifications and jobs

- **Decision**: the core's `NotificationCenter` (with its `watch` subscription) and `JobsWatcher`
  (started with the session) feed the Session; the Shell's Notifications overlay and the toast
  layer render them. Toasts never take focus. Playback failures post an entry.

## R10. Scenes, the scene view, and playback

- **Decision**: Scenes follows 006's research (R3, R4): page loading through `read_cached` (cache
  keys are the core's), neighbour prefetch, thumbnails from the core's `ThumbService` as image
  handles in an LRU (at most 2,000) with next-page warming, a plain grid with row windowing only
  if the 1000-card scroll misses its budget, paging rules unchanged (a behaviour that stays). The
  scene view (title, cover, details, Play) is designed fresh; the cover comes from the core's
  screenshot fetch through the cache. Playback is the Session's playback region around 006's
  player; the video widget is in the tree only in the owner tab's active screen; the now-playing
  bar appears in other tabs.

## R11. Video-path hardening (constitution v4.1.0)

- **Decision**: as the 006 analysis's R1–R5, at the start of US4:
  - `Mpv` gets an owner that outlives every render context, removing the `transmute` to
    `'static`.
  - GPU resources get owning types, freed together per slot generation.
  - A safe `RenderTarget` is passed to `create_renderer`.
  - The large Vulkan block is split into one-operation blocks with `// SAFETY:` notes.
  - `stash-core` gets `#![forbid(unsafe_code)]`.
  - `native-ui` and `player` deny the three lints.
  - `WGPU_BACKEND` is set without `set_var`.

## R12. Testing

- **Decision**:
  - **Transition tests** (pure functions, R2) for every state machine level: Onboarding,
    connection region, shell and tabs (history, close, select), each screen, playback, overlays.
  - **Headless iced tests** (`iced_test`) for screen flows with fixture data.
  - **Snapshot tests**: a session round-trips through `session.json`; an unknown version starts
    fresh.
  - **Core tests** unchanged (with Cynic underneath, R5).
  - Each capability and each behaviour that stays maps to at least one test (spec FR-015), listed
    in the capability checklist.

## R13. Measurement

- **Decision**:
  - The harness moves to `crates/perf-harness` (it never needed Tauri) and defaults to the native
    app; `--app web` remains until retirement.
  - The UI bench is rewritten for the new UI, keeping the same line names so reports compare.
  - Memory and CPU rows are added for both apps.
  - The web baseline for SC-008 is re-run on the Testing profile first.

## R14. Retiring the demo

- **Decision**: after the user signs off the capability checklist, one change removes `ui/`,
  `src-tauri/`, the npm files, and CI's Node steps, plus core code only the demo used: the
  demo's tab store (`stash_core::shell::tabs`) and the screenshot data-URL helper. The workspace
  becomes `stash-core`, `player`, `native-ui`, `perf-harness`, on Rust 1.88. A fresh clone builds,
  passes the gate, runs, and measures with only Rust (SC-009).

## R15. Desktop identity

- **Decision**: Wayland `app_id` / X11 class `dev.semantic-stash-viewer`, matching the data
  directories (R4); the app icon moves from `src-tauri/icons` into `native-ui` before the demo
  is removed.
