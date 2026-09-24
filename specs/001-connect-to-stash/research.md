# Research: Connect to Stash

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md) | **Date**: 2026-09-23

Findings marked **(observed)** were checked against the developer's local Stash instance
(`http://localhost:9999`, v0.31.1, no authentication, 27,045 scenes / 25,632 images / 739
galleries / 542 performers) with `curl`. Everything else is a design decision.

---

## R1. Frontend framework (project-wide)

- **Decision**: SolidJS + TypeScript, built with Vite. Tests use Vitest with
  `@solidjs/testing-library`. The package manager is npm (the only one installed).
- **Rationale**: Chosen by the project owner. Fine-grained reactivity with no virtual DOM gives
  the cheapest updates for Principle VI's budgets (< 50 ms input, 60 fps virtualized grids over
  27k+ scenes in later phases). The runtime is small, which keeps cold start under 2 s.
- **Alternatives considered**: Svelte 5 (similar speed, larger ecosystem); React (largest
  ecosystem, matches the plugin, but needs more re-render tuning); Leptos (all Rust, but younger
  tooling and slower iteration).

## R2. Application shell and workspace layout

- **Decision**: A Tauri v2 app in a Cargo workspace:
  - `crates/stash-core`: headless library (no Tauri dependency) holding the Stash adapter,
    connection logic, version gate, and profile store.
  - `src-tauri`: the Tauri binary. Thin command/event layer over `stash-core`.
  - `ui/`: the SolidJS frontend.
- **Rationale**: Principle III requires domain logic that doesn't depend on Tauri and can be
  tested headlessly. A separate crate enforces this at compile time: `stash-core` can't reach
  Tauri APIs even by accident.
- **Alternatives considered**: a single `src-tauri` crate with internal modules (rejected: the
  boundary would only be enforced by convention); several core crates now (rejected: premature
  for one feature, and it can be split later when the semantic engine arrives in Phase 4).

## R3. Typed command boundary between UI and core

- **Decision**: `tauri-specta` (with `specta`) generates TypeScript bindings for every Tauri
  command, event, and DTO from the Rust definitions. The generated file is committed and checked
  for drift in CI.
- **Rationale**: Principle III calls for *typed* Tauri commands. Generating the types means the
  UI can't drift from the core's contract, and the list of exposed commands is easy to review.
- **Alternatives considered**: `ts-rs` (types only, doesn't cover command signatures);
  handwritten TS types (drift risk).

## R4. HTTP client and TLS

- **Decision**: `reqwest` with the `rustls-tls` backend (webpki roots plus native roots via
  `rustls-native-certs`) and JSON support. Each profile gets its own client:
  - strict **off** (the default, FR-018): `danger_accept_invalid_certs(true)` and
    `danger_accept_invalid_hostnames(true)`;
  - strict **on** (FR-019): default verification.
- Redirects are followed (at most 5). The **final** `Response::url()` scheme decides the
  security state (FR-020).
- The security state is derived from the final scheme plus the strict setting:
  `http` → `Unencrypted`; `https` with strict off → `EncryptedUnverified`; `https` with strict
  on and a successful handshake → `EncryptedVerified`.
- **Rationale**: rustls avoids an OpenSSL system dependency and behaves the same on every
  platform. With strict off, the viewer honestly reports "not verified" even if the certificate
  happens to be valid, because nothing was checked. This matches spec US3 exactly.
- **Alternatives considered**: `native-tls` (behaviour depends on the platform); checking the
  certificate even with strict off in order to report "verified" opportunistically (rejected:
  it complicates the model, and the spec defines "verified" only with strict on).

## R5. Detecting Stash, authentication failures, and "not Stash"

**(observed)** behaviour of Stash v0.31.1:

| Request | Response |
|---|---|
| `POST /graphql` `{ version { version } }`, no key, no auth configured | `200`, `{"data":{"version":{"version":"v0.31.1"}}}` |
| Same request with `ApiKey: bogus`, **no auth configured on the server** | `401`, `Www-Authenticate: FormBased`, empty body |
| `GET /healthz` | `200`, `text/plain`, body `.`; needs no authentication |

- **Decision**: the connection probe sequence for one candidate base URL is:
  1. `POST {base}/graphql` with the probe query (below), sending the `ApiKey` header only if a
     key was entered.
  2. `200` with `data.version.version` → **Stash, authenticated**. Continue to the version gate
     (R6).
  3. `401` with `Www-Authenticate: FormBased` → **it's Stash, but authentication failed**:
     - if no key was sent → `ApiKeyRequired`;
     - if a key was sent → retry once *without* the key. If that returns `200`, the result is
       `ApiKeyInvalidButNotRequired` (the server has no auth configured, but the key is wrong).
       The UI offers "connect without a key". Otherwise → `ApiKeyRejected`.
  4. Connection refused, DNS failure, or TLS handshake failure → `Unreachable` (or
     `CertificateNotVerified` when strict is on and the failure is a certificate error).
  5. Any other status, a non-JSON body, or JSON without `data.version` → `NotStash`.
- **Rationale**: the `FormBased` challenge header is specific to Stash, so it tells "Stash that
  wants a key" apart from "some other server returning 401". The retry in step 3 exists because
  **(observed)** Stash rejects an invalid key even when no authentication is configured, which
  the spec's edge case "API key given but not needed is accepted" did not anticipate. A *valid*
  key on an open server is accepted. The spec edge case has been updated to cover the invalid
  case.
- `/healthz` is used for the cheap periodic reachability check (R8), not for detecting Stash:
  the body is too generic.
- **Alternatives considered**: sniffing `/` or `/login` HTML for "Stash" (fragile, depends on UI
  build); relying on `/healthz` alone (can't tell Stash from other servers or check auth).

## R6. Minimum-version gate (v0.31.1)

- **Probe query** (one round trip, also fills the library summary for FR-007):

  ```graphql
  query ConnectProbe {
    version { version hash }
    systemStatus { appSchema status }
    stats { scene_count image_count gallery_count performer_count }
  }
  ```

  **(observed)**: v0.31.1 reports `appSchema: 85`, `status: "OK"`.
- **Decision**:
  - Remove the leading `v` and parse with the `semver` crate.
  - **Release** (`0.31.1`): supported if `>= 0.31.1`.
  - **git-describe development build** (`0.31.1-12-gabc1234`): treat the build as *newer than*
    its base tag. Compare only `major.minor.patch` against the minimum, and mark it as
    `DevelopmentBuild` so the UI shows a compatibility warning. (A naive semver comparison would
    rank it *below* 0.31.1 because of the pre-release suffix, which is wrong.)
  - **Unparseable**: fall back to a capability check. Supported with a warning if
    `systemStatus.appSchema >= 85`; otherwise refused as unsupported.
  - `systemStatus.status` other than `OK` (for example `NEEDS_MIGRATION`, `SETUP`) → refuse
    with a specific message ("finish setup or migration in Stash's web UI first").
- **Rationale**: this meets FR-004 and SC-006 (never refuse ≥ v0.31.1 because of its version,
  always refuse older) and the spec's development-build edge case.
- **Alternatives considered**: comparing `appSchema` alone (not user-meaningful for the error
  message the spec requires); refusing development builds (too strict for the edge case).

## R7. GraphQL typing

- **Decision**: the `graphql_client` crate with Stash's schema checked in at
  `crates/stash-core/graphql/schema.json`, fetched by introspection from a v0.31.1 server. The
  schema contains no secrets or library data, so it is safe to commit. Each operation lives in its own `.graphql`
  file next to the adapter, and only the fields the view needs are selected (Principle IV).
- **Rationale**: typed operations are required by the constitution's technology constraints and
  stop field drift from reaching runtime. The introspection file makes future Stash upgrades a
  reviewable diff.
- **Alternatives considered**: `cynic` (more powerful, heavier macro setup, unnecessary for now);
  handwritten query strings (not allowed by the constitution).

## R8. Session state machine, health checks, and backoff

- **Decision**:
  - A single `ConnectionManager` in `stash-core` owns the active session and runs on the
    `tokio` runtime that Tauri provides.
  - Connection attempts are wrapped in a 15 s `tokio::time::timeout` (FR-006) and cancelled with
    a `tokio_util::sync::CancellationToken`.
  - While connected, the manager checks `GET /healthz` every **5 s**. A failure moves the state
    to `Offline` right away, which meets "within 10 seconds" (SC-005). Any failed adapter request
    also reports to the manager and triggers the same transition.
  - While offline, it probes with exponential backoff: 1, 2, 4, 8, 16, 32, then every 60 s
    (FR-016). On success it runs the full probe again (R5 + R6) before returning to `Connected`,
    so a downgraded server or a revoked key is caught (US2 AS4/AS5).
  - `AuthFailed` stops the retries (FR-017) until the user updates the key.
  - Every state change is emitted to the UI as an event (contract in
    [contracts/tauri-commands.md](contracts/tauri-commands.md)).
- **Rationale**: a 5 s cadence against the unauthenticated `/healthz` costs nothing and gives
  headroom within the 10 s budget. A full re-probe on recovery reuses one code path for first
  connect and reconnect.
- **Alternatives considered**: checking with a GraphQL query (heavier; auth problems are caught
  by the re-probe and by ordinary requests anyway); a fixed retry interval (hammers a server
  that's down, and the spec asks for increasing intervals).

## R9. Address normalisation

- **Decision**: parse with the `url` crate after light pre-processing:
  - Trim whitespace. If there is no `scheme://`, create **two candidates**, `https://…` then
    `http://…`, and try them in that order (FR-002). The first that yields a Stash response
    wins.
  - Remove a trailing `/graphql`, `/graphql/`, `/playground`, or trailing slashes from the path,
    keeping any other sub-path (reverse-proxy deployments such as `https://host/stash`).
  - Reject addresses that have no host, or schemes other than http/https, before any network
    call.
  - Store the **final** base URL after redirects in the profile, and show it in the UI.
- **Rationale**: covers every address format in the spec's edge cases and supports
  reverse-proxy sub-paths.
- **Alternatives considered**: http first (wrong for remote servers, and it would silently
  downgrade security).

## R10. API key handling

- **Decision**: the API key is ordinary profile configuration (constitution v3.0.0, Principle
  VII). It is saved in `profiles.json` next to the rest of the profile (R11), sent to the UI in
  profile DTOs so the form can show and edit it as plain text, and held by the adapter as a plain
  `String` used for the `ApiKey` header.
- **Rationale**: the project owner ruled that protecting the key on the user's own machine isn't
  a goal. Plain config removes a platform-specific dependency (libsecret / Keychain / Credential
  Manager), a failure mode (no secret service running), and a whole UI flow (session-only keys).
  It also makes profiles portable: copying `profiles.json` is enough (SC-004).
- **Alternatives considered**: the OS keyring through the `keyring` crate, and an encrypted vault
  through `tauri-plugin-stronghold`. Both were rejected by the project owner (the constitution now
  prohibits key-hiding machinery unless VII is amended).

## R11. Profile persistence

- **Decision**: one JSON file, `profiles.json`, in the platform config directory
  (`$XDG_CONFIG_HOME/semantic-stash-viewer/` on Linux), found by `src-tauri` and passed as a
  path to `stash-core`. It holds a schema `version` field, the list of profiles, and
  `last_used_profile_id`. Each profile includes its API key (R10). Writes are atomic (temp
  file, then rename).
- **Rationale**: tiny data, readable by people, no database dependency, and Principle I
  (config, not library data).
- **Alternatives considered**: `tauri-plugin-store` (ties persistence to Tauri, which breaks the
  headless core); SQLite (overkill for a handful of profiles; a local database will be judged on
  its own merits for the Phase 0 cache spec).

## R12. Testing strategy

- **Decision**:
  - `stash-core` unit tests: normalisation, the version gate (release, development, and
    unparseable cases), state-machine transitions with a fake clock, and the probe
    classification. Network fixtures are replayed through `wiremock` using responses captured
    from the local v0.31.1 server, including the observed `401 FormBased` case.
  - An integration test (`#[ignore]`, opt-in through `STASH_TEST_URL`) runs against a real Stash.
  - A persistence test (SC-004) saves a profile with a key, reopens the store from the same
    file, and checks the key and settings come back unchanged.
  - UI: Vitest component tests for the connection form, indicator, and profile list against
    mocked command bindings.
  - Manual end-to-end steps in [quickstart.md](quickstart.md). An automated WebDriver end-to-end
    suite (`tauri-driver`) is deferred until Phase 1 has more screens to justify it.
- **Rationale**: follows the constitution's testing gate (the core tested headlessly against
  recorded fixtures) while keeping this first feature focused.

## R13. Toolchain prerequisites (observed)

| Tool / library | Found | Note |
|---|---|---|
| rustc / cargo | 1.94.0 | OK for Tauri v2 |
| node / npm | v26.8.1 / 12.0.2 | OK |
| webkit2gtk-4.1 | 2.52.6 | Tauri v2 Linux webview |
| mpv / libmpv | 0.41.0 / 2.5.0 | not used by this feature (mpv-playback-spike) |
| tauri CLI | 2.11.5 | installed on 2026-09-24 |
