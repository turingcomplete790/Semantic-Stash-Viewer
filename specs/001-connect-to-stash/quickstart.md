# Quickstart & Validation: Connect to Stash

**Feature**: [spec.md](spec.md) | **Contracts**: [contracts/](contracts/) |
**Model**: [data-model.md](data-model.md)

This guide proves the feature works end to end. It isn't an implementation guide; that's
`tasks.md`.

## Prerequisites

| Need | Check |
|---|---|
| Rust ≥ 1.80, Node ≥ 20, npm | `rustc --version`, `node --version` |
| Tauri CLI v2 | `cargo tauri --version` (2.11.5 installed) |
| Linux webview library | `pkg-config --modversion webkit2gtk-4.1` |
| Docker (for the disposable test servers) | `docker --version` |
| Your everyday Stash | `http://localhost:9999`, v0.31.1, no auth |

### Disposable test servers

These cover the authentication, old-version, and https scenarios without touching your real
library. Suggested setup (the ports are only examples):

| Name | What | Suggested setup |
|---|---|---|
| **S-open** | v0.31.1, no auth | your local Stash at `:9999` |
| **S-auth** | v0.31.1 with username/password set (so an API key is required) | `stashapp/stash:v0.31.1` container on `:9998`; set credentials in its web UI → Settings → Security, then copy the API key |
| **S-old** | v0.30.x | `stashapp/stash:v0.30.1` container on `:9997` |
| **S-tls-self** | https, self-signed certificate, in front of S-open | a Caddy container with `tls internal` reverse-proxying to `:9999`, on `:9443` |
| **S-tls-public** | https with a publicly trusted certificate | any Stash behind a real domain, if available; otherwise skip US3 AS4 and mark it "not verified in dev" |

Test containers get an empty library, which is fine. This feature only reads counts.

## Automated checks

```bash
# Core: normalisation, version gate, probe classification, state machine, profile store
cargo test -p stash-core

# Opt-in integration test against a real Stash
STASH_TEST_URL=http://localhost:9999 cargo test -p stash-core -- --ignored

# Lints and formatting (constitution quality gates)
cargo fmt --check && cargo clippy --workspace -- -D warnings

# UI component tests and type check, including drift in the generated bindings
npm --prefix ui run test && npm --prefix ui run typecheck
```

**Expected**: all tests pass.

## Manual validation scenarios

Run the app with `cargo tauri dev`. Reset between runs by deleting
`~/.config/semantic-stash-viewer/profiles.json`.

### V1: First connection, no auth (US1, SC-001, FR-007)
1. Fresh start. The connection screen appears.
2. Enter `localhost:9999` (no scheme) and choose Connect.
3. **Expect**: connected in under 1 minute from launch. The address is shown as
   `http://localhost:9999` (https was tried first and fell back to http). Version `v0.31.1`.
   Counts: 27,045 scenes, 25,632 images, 739 galleries, 542 performers (or your current
   numbers). The indicator shows **Unencrypted**.

### V2: API key required, wrong, and correct (US1 AS2–3)
1. Add S-auth with no key. **Expect**: "This server requires an API key". The typed address is
   kept.
2. Enter a wrong key. **Expect**: "The API key was rejected".
3. Enter the correct key. **Expect**: connected, and the profile is saved.
4. `~/.config/semantic-stash-viewer/profiles.json` contains the profile with its `api_key`, and
   the key is shown in the profile's settings.

### V3: Invalid key on an open server (research R5, edge case)
1. Add S-open with the key `bogus`.
2. **Expect**: "This server doesn't need an API key, but the one entered is wrong", with a
   "Connect without a key" action that succeeds.

### V4: Old version and not Stash (US1 AS4–5, SC-006)
1. Add S-old. **Expect**: refused with "Stash v0.30.1 is older than the minimum supported
   v0.31.1". No usable profile is saved.
2. Add `http://localhost:1` (nothing listening). **Expect**: "Nothing answered at this address",
   with a hint about address, port, and http/https.
3. Add any non-Stash web server (for example `https://example.com`). **Expect**: "Something
   answered, but it isn't Stash".

### V5: Auto-reconnect and offline handling (US2, SC-002, SC-005)
1. With the S-open profile saved, quit and relaunch. **Expect**: connected within 2 s of the
   window appearing, with no prompts.
2. Stop Stash. **Expect**: the indicator shows **Offline** within 10 s. The interface stays
   responsive. Retry attempts show an increasing countdown.
3. Start Stash. **Expect**: back to **Connected** within 60 s, with no action.
4. Launch the app while Stash is stopped. **Expect**: the app opens in the Offline state, not an
   error dialog; switching profiles still works.

### V6: Key revoked (US2 AS4, FR-017)
1. Connected to S-auth, regenerate the API key in S-auth's web UI.
2. Trigger a reconnect (stop and start the container, or wait for the next request).
   **Expect**: "The API key was rejected", with an "Update key" prompt; automatic retries stop.

### V7: Connection security (US3, SC-007)
1. Connect to S-tls-self with defaults. **Expect**: connected; **Encrypted, not verified**.
2. Edit the profile, turn on strict checking, and save. **Expect**: validation fails with "The
   server's certificate couldn't be verified", and a hint on how to turn strict checking off.
   Choosing "save anyway" keeps the setting; not doing so leaves the old working settings.
3. (If available) S-tls-public with strict on. **Expect**: **Encrypted, verified**.
4. Open the indicator details. **Expect**: the address, security state, and strict setting.

### V8: Multiple profiles (US4)
1. With S-open and S-auth saved, switch between them. **Expect**: the counts change to match
   each server.
2. Add `http://localhost:9999/` again. **Expect**: the duplicate warning with "Open existing".
3. Delete S-auth and confirm. **Expect**: the profile is gone from the list and from
   `profiles.json`.

### V9: Saved and portable profiles (FR-009, FR-010, SC-004)
1. With the S-auth profile saved, quit and relaunch. **Expect**: it connects without asking for
   the address or key.
2. Open the S-auth profile's settings. **Expect**: the API key is shown in plain text and can be
   edited.
3. Copy `~/.config/semantic-stash-viewer/profiles.json` to a second machine (or a second user
   account) that can reach S-auth, and launch the viewer there. **Expect**: it connects with no
   setup.

## Measured results

Measured 2026-09-25 on the development machine (Linux, debug build) with the core's real
`StashProber` and default `ManagerConfig`, against the user's library (`:9999`, ~28k scenes)
and the disposable test instance (`:9998`, empty).

| Criterion | Target | Measured |
|---|---|---|
| SC-002: launch auto-connect → Connected (core) | ≤ 2 s | 56–102 ms on `:9999`, 3–39 ms on `:9998` over 30 runs each; one outlier of 3.8 s in an earlier batch did not reproduce in 40 further runs |
| SC-005: offline detected after the server stops | ≤ 10 s | 0.1–2.7 s over 3 outages (depends on where the 5 s health tick falls) |
| SC-005: reconnected after the server returns | ≤ 60 s | 0.7 s (3 s outage), 4.4 s (12 s outage), 23.1 s (40 s outage, waiting out the 32 s backoff step) |

Outages were simulated with a TCP proxy in front of the test instance that was stopped and
restarted, so no real server was touched. The SC-002 figure covers the connection only; window
creation and first paint aren't included (they weren't measured headlessly). The user confirmed
the relaunch reconnects without prompts on test and production servers.

## Validation run (2026-09-25)

| Scenario | Result | How |
|---|---|---|
| V1 First connection, no auth | ✅ | Live: `localhost:9999` → https tried, fell back to http; v0.31.1, real counts, Unencrypted. User confirmed in the app. |
| V2 API key required / wrong / correct | ✅ | User verified against the test instance after enabling a username/password (2026-09-25); also covered by probe tests with the captured `401 FormBased` response. |
| V3 Invalid key on an open server | ✅ | Live against `:9999`: "doesn't need a key, but the one entered is wrong". |
| V4 Old version / nothing listening / not Stash | ✅ (old version automated) | Live: dead port → Unreachable (both URLs listed); local non-Stash web server → NotStash (HTTP 501). Old version via fixture tests. |
| V5 Auto-reconnect and offline | ✅ | User confirmed relaunch reconnects. Offline/recovery measured through a proxy (see Measured results). |
| V6 Key revoked | ⚠️ Automated only | State-machine test (401 on re-probe → AuthFailed, retries stop) and KeyPrompt tests. Needs an auth-enabled server. |
| V7 Connection security | ⚠️ http live, https automated | User confirmed "Unencrypted". Self-signed https strict on/off and http→https redirect covered by TLS tests against a local self-signed server; no real https Stash available. |
| V8 Multiple profiles | ✅ | User confirmed switching, editing, and deleting across test and production servers. |
| V9 Saved and portable profiles | ✅ steps 1–2; step 3 not run | Relaunch reconnects (user); `profiles.json` holds the profiles with `api_key` fields; the key field is shown and edited as plain text (UI tests). Copying to a second machine not tried. |

Remaining manual checks: V6 (regenerate the key while connected) on the auth-enabled test
instance, V7 on a real https server, and V9 step 3.
