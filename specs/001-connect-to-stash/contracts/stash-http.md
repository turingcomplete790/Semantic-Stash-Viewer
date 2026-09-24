# Contract: Stash HTTP/GraphQL calls (Rust core → Stash)

**Feature**: [../spec.md](../spec.md) | **Research**: [../research.md](../research.md) R5, R6, R8

These are the only requests this feature makes to Stash. All of them go through the single
adapter in `crates/stash-core` (Principle III). The request count is part of the Constitution
Check (Principle IV).

## Authentication

- Header `ApiKey: <key>` when the profile has a key. The key is never put in a query string.
- No cookies or session login (API-key-only, spec Assumptions).

## 1. Connect probe: `POST {base}/graphql`

One round trip gets the server's identity, version, readiness, and library summary.

```graphql
query ConnectProbe {
  version { version hash }
  systemStatus { appSchema status }
  stats { scene_count image_count gallery_count performer_count }
}
```

**Observed response (v0.31.1, no auth):**

```json
{"data":{"version":{"version":"v0.31.1"},
         "systemStatus":{"appSchema":85,"status":"OK"},
         "stats":{"scene_count":27045,"image_count":25632,"gallery_count":739,"performer_count":542}}}
```

| Response | Classification |
|---|---|
| `200` + `data.version.version` | Stash. Go to the version gate |
| `200` + `systemStatus.status != "OK"` | `ServerNotReady(status)` |
| `401` + `Www-Authenticate: FormBased`, no key sent | `ApiKeyRequired` |
| `401` + `Www-Authenticate: FormBased`, key sent | Retry once without the key: `200` → `ApiKeyInvalidButNotRequired`; otherwise `ApiKeyRejected` |
| Connect/DNS/TLS error | `Unreachable`, or `CertificateNotVerified` for a certificate error with strict on |
| No response within the overall 15 s budget | `Timeout` |
| Anything else | `NotStash(status)` |

**Requests per connect**: 1 (2 for a key sent to a server without auth; ×2 candidates when the
address had no scheme and https fails).

## 2. Health check: `GET {base}/healthz`

- Unauthenticated. **Observed**: `200 text/plain`, body `.`.
- Every 5 s while Connected. Any non-2xx or network error → Offline (R8).
- Not used to detect Stash (the body is too generic).

**Load**: 12 tiny requests per minute per connected client, with no database work on the server.

## 3. Reconnect probe

Same as (1), run after an Offline backoff probe of `/healthz` succeeds, so key revocation and
version changes are caught before returning to Connected.

## Explicitly not used by this feature

- Transcoding, stream, or image endpoints (the mpv-playback-spike spec).
- `/login` or cookie sessions.
- Any mutation. This feature is read-only against Stash.

## Fixtures

Captured responses for replay in `stash-core` tests live in
`crates/stash-core/tests/fixtures/stash-v0.31.1/`:

- `probe-ok.json`
- `probe-401-formbased.http` (status + headers, empty body)
- `probe-needs-migration.json`
- `probe-dev-version.json`
- `probe-old-version.json`
- `healthz-ok.txt`

Fixtures contain counts and versions only. There are no titles, paths, or other library content
(constitution: fixtures must be scrubbed).
