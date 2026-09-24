---

description: "Task list for 001-connect-to-stash"
---

# Tasks: Connect to Stash

**Input**: Design documents from `specs/001-connect-to-stash/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: Included. The spec doesn't ask for them, but the constitution's Development Workflow
gate *requires* Rust core unit tests against recorded GraphQL fixtures. Write the test tasks in
each story first, and make sure they fail before implementing.

**Organization**: Tasks are grouped by user story so each can be implemented and tested on its
own.

**API key handling** (constitution v3.0.0, Principle VII): the key is a plain `api_key` string on
the profile. It is saved in `profiles.json`, sent to the UI, and shown and edited as plain text.
Don't add a keyring, session-only mode, masking, or log redaction.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on unfinished tasks)
- **[Story]**: The user story the task belongs to (US1–US4)

## Path Conventions

Cargo workspace at the repo root (see [plan.md](plan.md#source-code-repository-root)):
`crates/stash-core/` (headless core, **must not depend on `tauri`**), `src-tauri/` (thin Tauri
shell), and `ui/` (SolidJS + TypeScript).

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project skeleton, toolchain, and quality gates.

- [X] T001 Create the root Cargo workspace in `Cargo.toml` with members `crates/stash-core` and `src-tauri`, `resolver = "2"`, `rust-version = "1.80"`, and shared `[workspace.dependencies]` for `tokio` (features `rt-multi-thread`, `macros`, `time`, `sync`), `serde` (derive), `serde_json`, `thiserror`, `tracing`, `uuid` (v4, serde), `url` (serde)
- [X] T002 Create the `crates/stash-core/Cargo.toml` library crate depending on workspace deps plus `reqwest` (`default-features = false`, features `rustls-tls`, `rustls-tls-native-roots`, `json`), `graphql_client`, `tokio-util`, `semver`, `chrono` (serde); dev-deps `wiremock`, `tokio` (test-util), `tempfile`. Add an empty `crates/stash-core/src/lib.rs`. **Do not add `tauri` as a dependency.**
- [X] T003 Scaffold the Tauri v2 app in `src-tauri/` (`Cargo.toml` depending on `tauri` v2, `tauri-specta` v2, `specta`, `tracing-subscriber`, and `stash-core` by path; `src/main.rs`; `tauri.conf.json` with identifier `dev.semantic-stash-viewer`, product name `Semantic Stash Viewer`, `build.frontendDist = "../ui/dist"`, `build.devUrl = "http://localhost:5173"`, `beforeDevCommand = "npm --prefix ../ui run dev"`, `beforeBuildCommand = "npm --prefix ../ui run build"`)
- [X] T004 [P] Scaffold the SolidJS + TypeScript + Vite frontend in `ui/` (`package.json` with scripts `dev`, `build`, `test` (vitest run), `typecheck` (`tsc --noEmit`), `lint`; deps `solid-js`, `@tauri-apps/api` v2; dev-deps `vite`, `vite-plugin-solid`, `typescript`, `vitest`, `jsdom`, `@solidjs/testing-library`, `eslint`, `prettier`), plus `ui/vite.config.ts` (port 5173, `strictPort: true`, vitest `environment: "jsdom"`), `ui/tsconfig.json` (strict), `ui/index.html`, `ui/src/index.tsx`, and `ui/src/App.tsx`
- [X] T005 [P] Add `rustfmt.toml` and workspace clippy config (`[workspace.lints.clippy]` in `Cargo.toml`: `all = "warn"`, `unwrap_used = "warn"` for non-test code) and `ui/.eslintrc.cjs` + `ui/.prettierrc`
- [X] T006 [P] Restrict webview permissions in `src-tauri/capabilities/default.json`: `core:default` and `core:event:default` only, **no `http:*` permission** (contract invariant 3, Principle III)
- [X] T007 [P] Add a CI workflow in `.github/workflows/ci.yml` that runs `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, `npm --prefix ui ci`, `npm --prefix ui run lint`, `npm --prefix ui run typecheck`, `npm --prefix ui run test`, and a bindings drift check (regenerate `ui/src/bindings.ts` and fail on `git diff --exit-code`). Install the Linux dep `libwebkit2gtk-4.1-dev`
- [X] T008 [P] Fetch the Stash v0.31.1 GraphQL introspection schema from `http://localhost:9999/graphql` into `crates/stash-core/graphql/schema.json` (for example with `graphql-client introspect-schema`), and add `crates/stash-core/graphql/README.md` recording the Stash version and the command used
- [X] T009 [P] Capture scrubbed response fixtures from Stash into `crates/stash-core/tests/fixtures/stash-v0.31.1/` as listed in [contracts/stash-http.md](contracts/stash-http.md#fixtures): `probe-ok.json` (real response), `probe-401-formbased.http` (status line, `Www-Authenticate: FormBased`, empty body), and hand-edited variants `probe-needs-migration.json` (`status: "NEEDS_MIGRATION"`), `probe-dev-version.json` (`version: "v0.31.1-12-gabc1234"`), `probe-old-version.json` (`version: "v0.30.1"`, `appSchema: 80`), `healthz-ok.txt` (`.`). Counts and versions only, no library content

**Checkpoint**: `cargo build --workspace`, `npm --prefix ui run build`, and `cargo tauri dev` open an empty window.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Types, adapter plumbing, and the command/binding pipeline that every story uses.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T010 [P] Implement `ConnectFailure` in `crates/stash-core/src/connection/failure.rs` with the variants from [data-model.md](data-model.md#connectfailure-enum-maps-11-to-fr-005-messages): `InvalidAddress { reason }`, `Unreachable { tried: Vec<String> }`, `Timeout`, `NotStash { status: Option<u16> }`, `ApiKeyRequired`, `ApiKeyRejected`, `ApiKeyInvalidButNotRequired`, `UnsupportedVersion { found, minimum }`, `ServerNotReady { status }`, `CertificateNotVerified`, `DuplicateProfile { existing_id }`. Derive `thiserror::Error`, `serde::Serialize` with `#[serde(tag = "kind", rename_all = "camelCase")]`, and `specta::Type` behind a `specta` cargo feature. "No variant carries a raw response body." Wire the modules in `crates/stash-core/src/connection/mod.rs` and `crates/stash-core/src/lib.rs`
- [X] T011 [P] Implement the profile types in `crates/stash-core/src/profiles/model.rs`: `ServerProfile { id: Uuid, display_name, base_url: Url, strict_tls: bool, api_key: Option<String>, created_at, last_used_at: Option }`, `ProfileDraft { display_name: Option<String>, address: String, api_key: Option<String>, strict_tls: bool }`, and `ProfileSummary` (the DTO in [contracts/tauri-commands.md](contracts/tauri-commands.md#dtos-exposed-to-the-ui), including `api_key`). Enforce: `display_name` "1–64 chars after trim; defaults to the host (e.g. `192.168.1.10:9999`)"; `strict_tls` "Default `false` (FR-018)"; `id` "Generated on create; immutable"; `api_key` "Stored as entered, after trimming whitespace; `null` or absent means no key"; draft `api_key` "an empty string is treated as no key"
- [X] T012 Implement the profile persistence in `crates/stash-core/src/profiles/store.rs` (depends on T011): `ProfileStore::open(path)` reading and writing `ProfilesFile { version: 1, profiles, last_used_profile_id }`. Writes are atomic (temp file in the same directory + `rename`). "Unknown future versions are refused rather than overwritten". "A corrupt file is renamed to `profiles.json.bak` and the user starts with an empty list and a notice". Expose `list`, `get`, `insert`, `replace`, `remove`, `reorder`, `set_last_used`, and `find_by_base_url` (case-insensitive on scheme and host). Wire in `crates/stash-core/src/profiles/mod.rs`
- [X] T013 [P] Write tests for the store in `crates/stash-core/tests/profile_store.rs`: a round-trip that includes `api_key` (SC-004: the reopened store returns the same key and settings), atomic write leaves no temp files, a future `version` is refused, a corrupt file goes to `.bak`, and `find_by_base_url` matches `HTTP://LocalHost:9999` against `http://localhost:9999`
- [X] T014 Implement the adapter base in `crates/stash-core/src/adapter/mod.rs`: `StashClient::new(base_url: Url, strict_tls: bool, api_key: Option<String>)` building a `reqwest::Client` with rustls, `redirect::Policy::limited(5)`, a 15 s total timeout, `danger_accept_invalid_certs(!strict_tls)` and `danger_accept_invalid_hostnames(!strict_tls)`, sending the `ApiKey` header only when a key is present. A helper classifies transport errors into `Unreachable` / `Timeout` / `CertificateNotVerified` (a certificate error while `strict_tls` is true). This is the **only** module in the workspace allowed to make network calls
- [X] T015 Wire `tauri-specta` in `src-tauri/src/main.rs` and a new `src-tauri/src/commands.rs` (depends on T012): create the `tauri_specta::Builder`, export the bindings to `ui/src/bindings.ts` in debug builds, and put a `ProfileStore` at `<app_config_dir>/profiles.json` in shared app state. Commit the generated `ui/src/bindings.ts`
- [X] T016 [P] Set up `tracing-subscriber` in `src-tauri/src/main.rs`, with a log file in the app log directory and stderr output in debug builds
- [X] T017 [P] Create the plain-language failure messages in `ui/src/messages/failures.ts`: one function mapping every `ConnectFailure` `kind` to a title, an explanation, and a suggested next step, using the wording in [quickstart.md](quickstart.md#manual-validation-scenarios) (for example `unsupportedVersion` → "Stash {found} is older than the minimum supported {minimum}"). Add `ui/src/__tests__/failures.test.ts` asserting every variant from the generated bindings has a message (SC-003)
- [X] T018 [P] Create the app layout in `ui/src/App.tsx` with a top bar containing a slot for the connection indicator and a main area. It routes to the connection screen when there is no active profile and to a "Connected" summary view otherwise (views are filled in by the stories)

**Checkpoint**: the workspace builds; `cargo test -p stash-core` runs the store tests; `ui/src/bindings.ts` is generated.

---

## Phase 3: User Story 1 - Connect to a Stash server for the first time (Priority: P1) 🎯 MVP

**Goal**: enter an address and an optional key, get a precise success or failure, see the version and library counts, and save the profile (key included) to `profiles.json`.

**Independent Test**: fresh install → enter `localhost:9999` → connected, showing `v0.31.1` and the real counts. Quickstart V1–V4.

### Tests for User Story 1 ⚠️ (write first, they must fail)

- [X] T019 [P] [US1] Address normalisation tests in `crates/stash-core/tests/address.rs`: `localhost:9999` → candidates `[https://localhost:9999, http://localhost:9999]`; trailing `/`, `/graphql`, `/graphql/`, `/playground` stripped; `https://host/stash/graphql` → `https://host/stash`; `ftp://x`, an empty string, and `http://` → `InvalidAddress`
- [X] T020 [P] [US1] Version gate tests in `crates/stash-core/tests/version_gate.rs`: `v0.31.1` → Supported; `v0.32.0` → Supported; `v0.30.1` → `UnsupportedVersion { found: "v0.30.1", minimum: "v0.31.1" }`; `v0.31.1-12-gabc1234` → `DevelopmentBuild` (must **not** be refused); `garbage` with appSchema 85 → `UnknownButCompatible`; `garbage` with appSchema 80 → `UnsupportedVersion`; status `NEEDS_MIGRATION` → `ServerNotReady`
- [X] T021 [P] [US1] Probe classification tests in `crates/stash-core/tests/probe_classification.rs` using `wiremock` with the T009 fixtures: 200 → `ServerInfo` with counts 27045/25632/739/542; 401 FormBased with no key → `ApiKeyRequired`; 401 FormBased with a key whose retry without the key gets 200 → `ApiKeyInvalidButNotRequired`; 401 FormBased on both → `ApiKeyRejected`; 404 HTML → `NotStash { status: 404 }`; 200 non-JSON → `NotStash`; a closed port → `Unreachable`; a response delayed past the timeout (use a shortened test timeout) → `Timeout`; the https candidate fails and the http candidate succeeds → success with the `http://` final URL; a 301 to another path → final URL reported
- [X] T022 [P] [US1] Connection form component test in `ui/src/__tests__/ConnectionForm.test.tsx` with mocked bindings: submits the draft, with the API key entered in a plain text field; shows the Connecting state immediately with a working Cancel; shows the failure message from `failures.ts` and **keeps the typed address and key**; on `apiKeyInvalidButNotRequired` shows a "Connect without a key" action; on success shows the version and the four counts

### Implementation for User Story 1

- [X] T023 [P] [US1] Implement address normalisation in `crates/stash-core/src/connection/address.rs` per research R9, returning `Vec<Url>` candidates or `ConnectFailure::InvalidAddress` before any network call (FR-002)
- [X] T024 [P] [US1] Implement the version gate in `crates/stash-core/src/connection/version.rs` per research R6: strip the leading `v`, parse with `semver`, compare the `major.minor.patch` core against `0.31.1`, treat a git-describe suffix as a `DevelopmentBuild`, fall back to `appSchema >= 85` when the version can't be parsed, and map `systemStatus.status != "OK"` to `ServerNotReady`. Define `MINIMUM_STASH_VERSION = "0.31.1"` and `MINIMUM_APP_SCHEMA = 85` as constants (FR-004)
- [X] T025 [P] [US1] Add the `ConnectProbe` operation in `crates/stash-core/graphql/connect_probe.graphql` selecting exactly `version { version hash }`, `systemStatus { appSchema status }`, and `stats { scene_count image_count gallery_count performer_count }` (Principle IV), and the `ServerInfo` type in `crates/stash-core/src/connection/mod.rs` (`version`, `version_status: Supported | DevelopmentBuild | UnknownButCompatible`, `app_schema`, `counts { scenes, images, galleries, performers }`)
- [X] T026 [US1] Implement the probe in `crates/stash-core/src/adapter/probe.rs` (depends on T014, T025): a `graphql_client`-typed `POST {base}/graphql`, and response classification exactly as in [contracts/stash-http.md](contracts/stash-http.md#1-connect-probe-post-basegraphql), including the single retry without the key on a 401 `Www-Authenticate: FormBased` when a key was sent (research R5). Returns `(final_url, ServerInfo)` or `ConnectFailure`
- [X] T027 [US1] Implement `test_connection` in `crates/stash-core/src/connection/connect.rs` (depends on T023, T024, T026): try each candidate URL in order, stop at the first Stash response (success *or* an auth/version failure, since both prove it's Stash), apply the version gate, enforce the 15 s overall budget with `tokio::time::timeout`, and support cancellation through a `CancellationToken` (FR-003, FR-006)
- [X] T028 [US1] Implement profile creation in `crates/stash-core/src/profiles/service.rs` (depends on T012, T027): `create_profile(draft)` re-runs `test_connection`, rejects duplicates with `DuplicateProfile { existing_id }`, stores the **final** URL as `base_url` and the trimmed `api_key` on the profile, persists through `ProfileStore`, and sets `last_used_profile_id` (FR-008, FR-009)
- [X] T029 [US1] Expose the Tauri commands in `src-tauri/src/commands.rs` (depends on T015, T028): `list_profiles`, `test_connection(draft, request_id)`, `cancel_request(request_id)` (a map of request id to `CancellationToken` in app state), and `create_profile(draft)` with the signatures in [contracts/tauri-commands.md](contracts/tauri-commands.md#commands). Regenerate `ui/src/bindings.ts`
- [X] T030 [US1] Build the connection screen in `ui/src/components/ConnectionForm.tsx` (depends on T029, T017): address field, a plain text API key field, optional display name, Connect/Cancel, a result panel (address used, version, counts), failure display from `failures.ts`, "Connect without a key" for `apiKeyInvalidButNotRequired`, and a "Save" that calls `create_profile` (FR-001, FR-010)
- [X] T031 [US1] Build a minimal connected summary view in `ui/src/components/ConnectedSummary.tsx`, routed from `ui/src/App.tsx`, showing the server address, the Stash version (with a warning badge for `developmentBuild`/`unknownButCompatible`), and the four library counts (FR-007)

**Checkpoint**: US1 works on its own. Quickstart V1–V4 pass; `cargo test -p stash-core` is green.

---

## Phase 4: User Story 2 - Reconnect automatically and see connection health (Priority: P2)

**Goal**: auto-connect on launch, an always-visible health indicator, detecting offline within 10 s, self-healing reconnects with backoff, stopping on auth failure, and an "Update key" fix-up.

**Independent Test**: save a profile → relaunch → connected with no prompt within 2 s → stop Stash → Offline within 10 s → start Stash → reconnected within 60 s. Quickstart V5, V6, and V9 step 1.

### Tests for User Story 2 ⚠️

- [X] T032 [P] [US2] State machine tests in `crates/stash-core/tests/connection_manager.rs` using `tokio::time::pause()` and wiremock, covering every row of the [transition table](data-model.md#connectionstate-and-transitions): Idle→Connecting→Connected; a `/healthz` failure → Offline; the backoff delays are exactly 1, 2, 4, 8, 16, 32, 60, 60 s; recovery re-runs the full probe; a 401 on the re-probe → AuthFailed with **no further retries**; a re-probe reporting `v0.30.1` → Failed(UnsupportedVersion); a launch auto-connect to an unreachable server that has connected before → Offline (not Failed); `cancel` → Idle; every transition emits exactly one snapshot
- [X] T033 [P] [US2] `update_profile` tests in `crates/stash-core/tests/profile_service.rs` with wiremock: a failing validation with `force=false` leaves the old settings unchanged; with `force=true` saves; the draft's `api_key` replaces the stored key, and `null` or an empty string clears it (FR-013)
- [X] T034 [P] [US2] Connection indicator component test in `ui/src/__tests__/ConnectionIndicator.test.tsx`: renders connecting / connected / offline (with a retry countdown) / authFailed from mocked `connection-state` events; authFailed shows an "Update key" action

### Implementation for User Story 2

- [X] T035 [P] [US2] Implement the health check in `crates/stash-core/src/adapter/health.rs`: `GET {base}/healthz`, where any non-2xx or transport error is a failure (research R8, [contracts/stash-http.md](contracts/stash-http.md#2-health-check-get-basehealthz))
- [X] T036 [US2] Implement `ConnectionManager` in `crates/stash-core/src/connection/manager.rs` (depends on T027, T035): owns `ConnectionSession { profile_id, state, security, final_url, server, last_contact_at, retry }`; `connect(profile_id, is_launch)`, `disconnect()`, `report_request_failure(kind)`; a 5 s health-check loop while Connected; the backoff sequence 1, 2, 4, 8, 16, 32 s, then 60 s repeating while Offline (FR-016); a full re-probe on recovery; retries stop on AuthFailed (FR-017); publishes `ConnectionSnapshot` through a `tokio::sync::watch` channel. Updates `last_used_at` / `last_used_profile_id` on success
- [X] T037 [US2] Add the `ConnectionSnapshot` DTO (exact shape in [contracts/tauri-commands.md](contracts/tauri-commands.md#dtos-exposed-to-the-ui)) in `crates/stash-core/src/connection/snapshot.rs`, with `specta::Type` behind the `specta` feature
- [X] T038 [US2] Add `update_profile(id, draft, force)` to `crates/stash-core/src/profiles/service.rs` (depends on T028): validate by running `test_connection` with the merged settings before replacing (FR-013), and save anyway when `force` is true. If the profile is active, reconnect through `ConnectionManager` with the new settings
- [X] T039 [US2] Bridge the manager to Tauri in `src-tauri/src/events.rs` (depends on T036, T037, T038): forward every `watch` update as a `connection-state` event, add the commands `connect(profile_id, request_id)`, `disconnect()`, `get_connection_snapshot()`, and `update_profile(profile_id, draft, force)` to `src-tauri/src/commands.rs`, and regenerate `ui/src/bindings.ts`
- [X] T040 [US2] Auto-connect on startup in `src-tauri/src/main.rs` (depends on T039): in Tauri's `setup` hook, read `last_used_profile_id` and call `ConnectionManager::connect(id, is_launch = true)` without blocking window creation (FR-014, SC-002)
- [X] T041 [P] [US2] Create the connection store in `ui/src/state/connection.ts`: on load, hydrate with `getConnectionSnapshot()`, then subscribe to `connection-state`, exposing a Solid store used by all components
- [X] T042 [US2] Build `ui/src/components/ConnectionIndicator.tsx` (depends on T041) and mount it in the top-bar slot in `ui/src/App.tsx`: states connecting, connected, offline (a non-blocking notice plus a "retrying in Ns" countdown), and authFailed (FR-015). The UI stays usable while offline (the launch-offline edge case)
- [X] T043 [US2] Build `ui/src/components/KeyPrompt.tsx` (depends on T042, T039): for authFailed, an "Update key" dialog, prefilled with the current key in a plain text field, that calls `update_profile` with the new `apiKey` and then `connect` (US2 AS4)

**Checkpoint**: US1 and US2 work. Quickstart V5, V6, and V9 step 1 pass.

---

## Phase 5: User Story 3 - Understand and control connection security (Priority: P2)

**Goal**: always show unencrypted / encrypted not verified / encrypted verified; TLS verification off by default; per-profile strict mode with a clear refusal when the certificate can't be verified.

**Independent Test**: connect over http, over self-signed https, and over publicly trusted https (strict on) → the indicator shows the correct state each time; turning strict on for the self-signed server → refused with an explanation. Quickstart V7.

### Tests for User Story 3 ⚠️

- [ ] T044 [P] [US3] Security derivation tests in `crates/stash-core/tests/security.rs`: `http` → Unencrypted; `https` + strict off → EncryptedUnverified (even when the certificate is valid); `https` + strict on → EncryptedVerified; an http→https redirect uses the **final** scheme (FR-020)
- [ ] T045 [P] [US3] TLS behaviour tests in `crates/stash-core/tests/tls.rs`: start a local HTTPS wiremock/rustls server with a self-signed certificate generated by `rcgen` (add `rcgen` as a dev-dependency in `crates/stash-core/Cargo.toml`); strict off → probe succeeds; strict on → `ConnectFailure::CertificateNotVerified`

### Implementation for User Story 3

- [ ] T046 [P] [US3] Implement `SecurityState { Unencrypted, EncryptedUnverified, EncryptedVerified }` and `derive_security(final_url, strict_tls)` in `crates/stash-core/src/connection/security.rs` (research R4), and include `security` in `TestResult` and `ConnectionSnapshot` (update `crates/stash-core/src/connection/connect.rs` and `crates/stash-core/src/connection/snapshot.rs`)
- [ ] T047 [US3] Make sure `crates/stash-core/src/adapter/mod.rs` maps rustls certificate/hostname verification errors to `ConnectFailure::CertificateNotVerified` only when `strict_tls` is true, and to `Unreachable` otherwise (FR-019)
- [ ] T048 [US3] Add a "Verify certificate (strict)" toggle, default **off**, to `ui/src/components/ConnectionForm.tsx`, with helper text explaining the default. The `certificateNotVerified` message in `ui/src/messages/failures.ts` must explain how to turn strict checking back off (US3 AS3)
- [ ] T049 [US3] Show the security state in `ui/src/components/ConnectionIndicator.tsx` (icon + label: "Unencrypted", "Encrypted, not verified", "Encrypted, verified") and build `ui/src/components/ConnectionDetails.tsx`, a popover opened from the indicator showing the server address, the security state, and whether strict checking is on for the profile (FR-020, FR-021)

**Checkpoint**: US1–US3 work. Quickstart V7 passes.

---

## Phase 6: User Story 4 - Manage several Stash servers (Priority: P3)

**Goal**: add, rename, edit, switch, reorder, and delete profiles; validate edits before saving; warn on duplicates.

**Independent Test**: two profiles for two servers → switching changes the counts → edit one key → delete the other → it's gone after a relaunch. Quickstart V8 and V9 steps 2–3.

### Tests for User Story 4 ⚠️

- [ ] T050 [US4] Extend `crates/stash-core/tests/profile_service.rs` (after T033): `delete_profile` removes the profile and clears `last_used_profile_id` when it matched; creating a duplicate `base_url` → `DuplicateProfile { existing_id }`; reorder persists across a store reopen
- [ ] T051 [P] [US4] Profile manager component test in `ui/src/__tests__/ProfileManager.test.tsx`: lists profiles, with the API key visible in edit mode; delete asks for confirmation before calling `delete_profile`; a duplicate error offers "Open existing"; switching calls `connect` with the chosen id

### Implementation for User Story 4

- [ ] T052 [US4] Add `delete_profile(id)` and `reorder_profiles(ids)` to `crates/stash-core/src/profiles/service.rs` (FR-011, FR-012). If the deleted profile is active, disconnect through `ConnectionManager` and clear `last_used_profile_id`
- [ ] T053 [US4] Expose `delete_profile` and `reorder_profiles` in `src-tauri/src/commands.rs`, emit `profiles-changed` from `src-tauri/src/events.rs` after every mutation (create, update, delete, reorder), and regenerate `ui/src/bindings.ts`
- [ ] T054 [US4] Build `ui/src/components/ProfileManager.tsx` (opened from the indicator's details popover and from the connection screen): a list with display name, address, and security setting; add (reuses `ConnectionForm`), edit with the API key shown as plain text (with the "save anyway" choice when validation fails), rename, a delete confirmation dialog, drag or up/down reordering, and switch-to. On a `duplicateProfile` failure, offer "Open existing" (US4 AS1–AS4, FR-010)
- [ ] T055 [US4] Handle deletion of the active profile in `ui/src/App.tsx`: return to the connection screen, or show the profile picker when other profiles exist (US4 AS3)

**Checkpoint**: all user stories work on their own. Quickstart V8 and V9 pass.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [ ] T056 [P] Add the opt-in live integration test in `crates/stash-core/tests/live_stash.rs` (`#[ignore]`, reads `STASH_TEST_URL` and optional `STASH_TEST_API_KEY` from the environment): test the connection, assert the version is ≥ v0.31.1 and the counts are > 0
- [ ] T057 [P] Set the window title to "Semantic Stash Viewer — {display name}" in `src-tauri/src/events.rs` on connect
- [ ] T058 [P] Keyboard accessibility pass on `ui/src/components/ConnectionForm.tsx`, `ConnectionIndicator.tsx`, `ConnectionDetails.tsx`, `ProfileManager.tsx`, and `KeyPrompt.tsx`: tab order, Enter submits, Esc closes popovers and dialogs, visible focus (Principle VI)
- [ ] T059 Measure the relaunch-to-connected time (SC-002, ≤ 2 s) and the offline detection time (SC-005, ≤ 10 s) against the local Stash, and record the results in `specs/001-connect-to-stash/quickstart.md` under a new "Measured results" section
- [ ] T060 Update `README.md` Development section with the prerequisites (Tauri CLI, `libwebkit2gtk-4.1`), `cargo tauri dev`, and the test commands from [quickstart.md](quickstart.md#automated-checks)
- [ ] T061 Run every scenario in [quickstart.md](quickstart.md) (V1–V9), and tick off the matching items in `ROADMAP.md` Phase 0 ("Connection profiles", "Version check on connect", "TLS")

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies.
- **Foundational (Phase 2)**: depends on Setup. **Blocks all stories.**
- **US1 (Phase 3)**: depends on Foundational. This is the MVP.
- **US2 (Phase 4)**: depends on US1's `test_connection` (T027) and profile service (T028); `ConnectionManager` wraps them. US2 also adds `update_profile` for the "Update key" prompt.
- **US3 (Phase 5)**: depends on US1 (probe, form) and on US2's indicator (T042) for display. The core tasks T044–T047 can start right after US1.
- **US4 (Phase 6)**: depends on US1 (profile service, form) and US2 (`update_profile`, and `ConnectionManager` for switching and active-profile handling).
- **Polish (Phase 7)**: after the desired stories.

### User Story Dependencies

```text
Setup ─► Foundational ─► US1 (MVP) ─┬─► US2 ─┬─► US4
                                    │        └─► US3 (UI part: T048–T049)
                                    └─► US3 (core part: T044–T047)
```

The stories build on each other through the shared connection path, so they are **not** fully
parallel. Each is still independently *testable* at its checkpoint.

### Within Each Story

Tests first (they must fail) → core types → core services → Tauri commands and bindings
regeneration → UI components → checkpoint validation.

### Parallel Opportunities

- Setup: T004–T009 in parallel after T001–T003.
- Foundational: T010, T011, T014, T016, T017, T018 in parallel; T012 after T011; T013 after
  T012; T015 after T012.
- US1: tests T019–T022 in parallel; T023, T024, T025 in parallel; then T026 → T027 → T028 →
  T029 → T030/T031.
- US2: T032, T033, T034, T035, T041 in parallel.
- US3: T044, T045, T046 in parallel.
- US4: T050 and T051 in parallel.
- Polish: T056–T058 in parallel.

---

## Parallel Example: User Story 1

```bash
# Tests first, all in different files:
Task: "Address normalisation tests in crates/stash-core/tests/address.rs"
Task: "Version gate tests in crates/stash-core/tests/version_gate.rs"
Task: "Probe classification tests in crates/stash-core/tests/probe_classification.rs"
Task: "ConnectionForm component test in ui/src/__tests__/ConnectionForm.test.tsx"

# Then the independent core modules:
Task: "Address normalisation in crates/stash-core/src/connection/address.rs"
Task: "Version gate in crates/stash-core/src/connection/version.rs"
Task: "ConnectProbe operation in crates/stash-core/graphql/connect_probe.graphql"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1 Setup → Phase 2 Foundational.
2. Phase 3 (US1).
3. **Stop and validate**: quickstart V1–V4; `cargo test -p stash-core` green.
4. The viewer can now connect to a real library and remember it, which is the base for
   `mpv-playback-spike` and Phase 1 browsing.

### Incremental Delivery

1. Setup + Foundational → skeleton builds.
2. US1 → connect and save (MVP).
3. US2 → auto-reconnect, health indicator, and "Update key".
4. US3 → security indicator and strict mode (required before using the viewer over untrusted
   networks).
5. US4 → multiple servers.
6. Polish → measured budgets, docs, full quickstart run.

Commit after each task or logical group (constitution: "Commit after every working change").

---

## Notes

- [P] tasks = different files, no dependencies on unfinished tasks.
- `crates/stash-core` must never gain a `tauri` dependency. If a task seems to need one, the
  logic belongs in `src-tauri`.
- The API key is plain profile config (constitution v3.0.0). Don't add keyring, session-only,
  masking, or redaction code.
- Regenerate `ui/src/bindings.ts` whenever a command or DTO changes. CI fails on drift.
