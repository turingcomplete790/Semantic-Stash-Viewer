# Implementation Plan: Connect to Stash

**Branch**: `001-connect-to-stash` | **Date**: 2026-09-23 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/001-connect-to-stash/spec.md`

## Summary

Let a user connect the viewer to any Stash v0.31.1+ server with an address and an optional API
key. The viewer detects Stash and classifies failures precisely, enforces the minimum version,
and shows the version and library counts. It saves server profiles (API key included, as plain
config) and reconnects on launch. A persistent indicator shows connection health
and security (unencrypted / encrypted not verified / encrypted verified); TLS verification is
off by default, with per-profile strict mode.

Technically, this feature also creates the project skeleton the constitution requires:
- a Cargo workspace with a headless `stash-core` crate that doesn't depend on Tauri, holding the
  Stash adapter, connection manager, version gate, and profile store;
- a thin `src-tauri` command layer with typed bindings (`tauri-specta`);
- a SolidJS + TypeScript UI that never touches the network.

One GraphQL round trip (`ConnectProbe`) gets identity, version, readiness, and counts. The
unauthenticated `/healthz` drives offline detection. See [research.md](research.md).

## Technical Context

**Language/Version**: Rust 1.94 (edition 2021; workspace minimum 1.80); TypeScript 5.x

**Primary Dependencies**:
- Core (`stash-core`): `reqwest` (rustls), `graphql_client`, `tokio`, `tokio-util`, `url`,
  `semver`, `serde`/`serde_json`, `uuid`, `thiserror`, `tracing`
- Shell (`src-tauri`): Tauri v2, `tauri-specta`/`specta`
- UI (`ui/`): SolidJS, Vite, `@tauri-apps/api` v2

**Storage**: `profiles.json` (profiles, including their API keys) in the platform config
directory. No library data is stored.

**Testing**: `cargo test` with `wiremock` replaying fixtures captured from Stash v0.31.1; an
opt-in `#[ignore]` integration test against a real Stash; Vitest + `@solidjs/testing-library`
for the UI; manual end-to-end steps in [quickstart.md](quickstart.md)

**Target Platform**: Linux desktop first (webkit2gtk-4.1); Windows and macOS should
build, but aren't validated in this feature

**Project Type**: Desktop app (Tauri: Rust core + web UI)

**Performance Goals**: connected state within 2 s of the window appearing on relaunch (SC-002);
offline detected within 10 s (SC-005); the UI never blocks during a connection attempt
(Principle VI)

**Constraints**: 15 s connection timeout; retry backoff capped at 60 s; the webview has no HTTP
permissions

**Scale/Scope**: a handful of profiles per user; the reference library is 27k scenes / 25k
images (only counts are read in this feature); ~5 UI surfaces
([contracts/tauri-commands.md](contracts/tauri-commands.md))

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | How this plan complies | Status |
|---|---|---|
| **I. Stash is the system of record** | Only connection config (`profiles.json`, keys included) is stored locally. No library data is cached or persisted. No mutations against Stash. | ✅ Pass |
| **II. Semantic-tagging parity** | Not applicable: no semantic data is read or written. | ✅ N/A |
| **III. Brain/UI separation** | All I/O and logic live in `stash-core` (no Tauri dependency, enforced by the crate boundary). The UI uses only typed commands and events; the webview has no HTTP capability. The core is tested headlessly against fixtures. | ✅ Pass |
| **IV. Lean, batched GraphQL** | One typed `ConnectProbe` query per connect selects only the fields it needs. Health checks use the zero-cost `/healthz`. The request budget is documented in [contracts/stash-http.md](contracts/stash-http.md). | ✅ Pass |
| **V. Native playback** | Not applicable: no playback in this feature (mpv-playback-spike). | ✅ N/A |
| **VI. Responsive UI** | Connection work is async on the core runtime with a cancellable 15 s timeout. The UI renders the Connecting state immediately and follows state events. Relaunch-to-connected target ≤ 2 s (within the ≤ 2 s cold-start budget). | ✅ Pass |
| **VII. User-owned connection** | API key saved as plain profile config and editable in the UI; no keyring, session-only mode, or key hiding (as v3.0.0 requires); TLS off by default with per-profile strict mode and an always-visible security state; profile deletion confirmed; no connections to anywhere except the configured server. | ✅ Pass |
| **VIII. Web UI parity** | Covers the web UI's server-connection prerequisite. No web UI capability is dropped. | ✅ Pass |
| **Tech constraints** | Tauri v2 + Rust; frontend framework chosen and justified (SolidJS, R1); GraphQL only through one adapter with typed operations (R7); minimum Stash v0.31.1 enforced on connect (R6); each new dependency justified in research.md. | ✅ Pass |
| **Workflow gates** | Tests for the adapter, version gate, and state machine; `cargo fmt`/`clippy`/UI lint in CI; fixtures scrubbed to counts and versions only. | ✅ Pass |

**Post-design re-check (after Phase 1)**: ✅ still passing. Design notes:
- The contract confirms that the webview gets no HTTP permission (III).
- **Amended 2026-09-24** for constitution v3.0.0: the keyring, session-only keys, log redaction,
  and the key-leak test were removed. The API key is now a plain `api_key` field on the profile
  and in the DTOs.
- The 5 s `/healthz` check costs 12 tiny, unauthenticated requests per minute, no database work,
  so it stays within IV's intent.
- **Spec change found in research:** Stash rejects an invalid key even on servers without auth
  (observed). The spec was updated with a new failure type, `ApiKeyInvalidButNotRequired`, and
  its edge case. It doesn't affect any principle.

## Project Structure

### Documentation (this feature)

```text
specs/001-connect-to-stash/
├── plan.md               # This file
├── research.md           # Phase 0: decisions R1–R13
├── data-model.md         # Phase 1: profiles, session state machine, failures
├── quickstart.md         # Phase 1: validation guide (V1–V9)
├── contracts/
│   ├── tauri-commands.md # UI ↔ core commands, events, DTOs
│   └── stash-http.md     # core → Stash requests, classification, fixtures
├── checklists/
│   └── requirements.md   # Spec quality checklist
└── tasks.md              # Phase 2 (/speckit-tasks; not created here)
```

### Source Code (repository root)

```text
Cargo.toml                      # workspace: crates/*, src-tauri
crates/
└── stash-core/                 # headless; MUST NOT depend on tauri
    ├── Cargo.toml
    ├── graphql/
    │   ├── schema.json         # Stash v0.31.1 introspection
    │   └── connect_probe.graphql
    ├── src/
    │   ├── lib.rs
    │   ├── adapter/            # the only module doing network I/O (Principle III)
    │   │   ├── mod.rs          # StashClient: per-profile reqwest client, TLS mode, ApiKey header
    │   │   ├── probe.rs        # ConnectProbe + response classification (R5)
    │   │   └── health.rs       # /healthz check
    │   ├── connection/
    │   │   ├── address.rs      # normalisation + scheme candidates (R9)
    │   │   ├── version.rs      # minimum-version gate (R6)
    │   │   ├── security.rs     # SecurityState derivation (R4)
    │   │   ├── manager.rs      # ConnectionManager state machine, backoff, events (R8)
    │   │   └── failure.rs      # ConnectFailure
    │   ├── profiles/
    │   │   ├── model.rs        # ServerProfile, ProfileDraft, ProfileSummary
    │   │   ├── store.rs        # profiles.json atomic persistence (R11)
    │   │   └── service.rs      # create / update / delete / reorder with validation
    └── tests/
        ├── fixtures/stash-v0.31.1/
        ├── probe_classification.rs
        ├── version_gate.rs
        ├── connection_manager.rs
        ├── profile_store.rs
        └── live_stash.rs       # #[ignore], STASH_TEST_URL
src-tauri/
├── Cargo.toml
├── tauri.conf.json
├── capabilities/default.json   # no http permission for the webview
└── src/
    ├── main.rs
    ├── commands.rs             # tauri-specta commands → stash-core
    └── events.rs               # connection-state, profiles-changed
ui/
├── package.json
├── vite.config.ts
├── tsconfig.json
└── src/
    ├── bindings.ts             # generated by tauri-specta (committed; drift-checked)
    ├── index.tsx
    ├── App.tsx
    ├── state/connection.ts     # store fed by connection-state events
    ├── components/
    │   ├── ConnectionIndicator.tsx
    │   ├── ConnectionDetails.tsx
    │   ├── ConnectionForm.tsx
    │   ├── ProfileManager.tsx
    │   └── KeyPrompt.tsx
    ├── messages/failures.ts    # ConnectFailure → plain-language text (FR-005)
    └── __tests__/
```

**Structure Decision**: a Tauri v2 desktop app in a Cargo workspace. The Tauri-free
`crates/stash-core` library holds every piece of domain logic and network I/O. `src-tauri` is a
thin shell (commands and events), and `ui/` is the SolidJS frontend. Later features
add modules to `stash-core` (cache, browsing queries) or new crates (the Phase 4 semantic
engine), rather than putting logic in `src-tauri` or `ui/`.

## Complexity Tracking

No constitution violations to justify.
