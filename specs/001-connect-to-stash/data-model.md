# Data Model: Connect to Stash

**Feature**: [spec.md](spec.md) | **Research**: [research.md](research.md)

All types live in `crates/stash-core` unless noted. DTOs that cross into the UI are exported to
TypeScript through `tauri-specta` ([contracts/tauri-commands.md](contracts/tauri-commands.md)).
Nothing in this model is library data. Stash stays the system of record (Principle I).

---

## ServerProfile (persisted)

One saved Stash server, stored in `profiles.json` (research R11), API key included (R10).

| Field | Type | Rules |
|---|---|---|
| `id` | UUID v4 | Generated on create; immutable |
| `display_name` | string | 1–64 chars after trim; defaults to the host (for example `192.168.1.10:9999`) |
| `base_url` | URL | Normalised **final** base URL after redirects (R9): scheme http/https, host required, no trailing slash, no `/graphql` suffix; may keep a reverse-proxy sub-path |
| `strict_tls` | bool | Default `false` (FR-018) |
| `api_key` | string, optional | Stored as entered, after trimming whitespace; `null` or absent means no key. Sent to the UI as-is (FR-010) |
| `created_at` | timestamp (UTC) | Set on create |
| `last_used_at` | timestamp (UTC), optional | Updated on each successful connect |

**Uniqueness**: `base_url` is unique across profiles, compared case-insensitively on scheme and
host (US4 AS4). A duplicate triggers a `DuplicateProfile { existing_id }` error.

## ProfilesFile (persisted container)

| Field | Type | Rules |
|---|---|---|
| `version` | integer | Schema version, starting at `1`; unknown future versions are refused rather than overwritten |
| `profiles` | `ServerProfile[]` | Order = user order in the profile list |
| `last_used_profile_id` | UUID, optional | Used for auto-connect on launch (FR-014); cleared if that profile is deleted |

Written atomically (temp file + rename). A corrupt file is renamed to `profiles.json.bak` and the
user starts with an empty list and a notice. It's never silently overwritten.

## ProfileDraft (transient input)

What the connection form submits for `test_connection`, `create_profile`, and `update_profile`.

| Field | Type | Rules |
|---|---|---|
| `display_name` | string, optional | Same rules as the profile |
| `address` | string | Raw user input; normalised per R9 |
| `api_key` | string, optional | Whitespace-trimmed; an empty string is treated as no key |
| `strict_tls` | bool | |

## ConnectionSession (in memory, one at a time)

Owned by `ConnectionManager` (R8).

| Field | Type | Notes |
|---|---|---|
| `profile_id` | UUID | Active profile |
| `state` | `ConnectionState` | See the state machine below |
| `security` | `SecurityState`, optional | Known after the first successful handshake |
| `final_url` | URL, optional | After redirects |
| `server` | `ServerInfo`, optional | From the last successful probe |
| `last_contact_at` | timestamp, optional | Last successful health check or request |
| `retry` | `RetryInfo`, optional | Present while `Offline`: attempt number, next attempt time |

### SecurityState (enum)

`Unencrypted` (http) · `EncryptedUnverified` (https, strict off) · `EncryptedVerified` (https,
strict on). Derived from the **final** URL scheme and the profile's `strict_tls` (R4, FR-020).

### ConnectionState and transitions

```text
Idle ──connect──► Connecting ──ok──────────► Connected ──network failure──► Offline
                     │  ├──401─────────────► AuthFailed ◄──────401──────────┤
                     │  ├──other failure───► Failed(reason) ◄─version/not-ready─┤
                     │  └──unreachable on launch auto-connect──► Offline     │
                     └──cancel──► Idle                  Offline ──re-probe ok──► Connected
```

The table below is the authoritative definition; the sketch is a summary.

| From | Event | To |
|---|---|---|
| Idle | `connect` | Connecting |
| Connecting | probe OK + version gate OK | Connected |
| Connecting | 401 (`ApiKeyRequired`/`ApiKeyRejected`/`ApiKeyInvalidButNotRequired`) | AuthFailed |
| Connecting | unreachable / timeout, **and** this is a launch auto-connect of a profile that has connected before | Offline (edge case: "launch with last server offline") |
| Connecting | unreachable / timeout / NotStash / UnsupportedVersion / ServerNotReady / CertificateNotVerified otherwise | Failed(reason) |
| Connecting | `cancel` | Idle |
| Connected | health check or request fails at the network level | Offline |
| Connected | any request returns 401 | AuthFailed |
| Offline | backoff probe succeeds (full R5 + R6) | Connected |
| Offline | backoff probe → 401 | AuthFailed (retries stop, FR-017) |
| Offline | backoff probe → UnsupportedVersion / ServerNotReady | Failed(reason) |
| any | `disconnect` or switching profile | Idle (then Connecting for the new profile) |
| AuthFailed / Failed | `connect` (for example after editing the profile) | Connecting |

**Backoff** (Offline): delays 1, 2, 4, 8, 16, 32 s, then 60 s repeating (FR-016). The health
check interval while Connected is 5 s (R8).

## ConnectFailure (enum, maps 1:1 to FR-005 messages)

| Variant | Payload | User-facing meaning |
|---|---|---|
| `InvalidAddress` | reason | The address can't be used (no host, bad scheme) |
| `Unreachable` | tried URLs | Nothing answered at this address |
| `Timeout` | | No response within 15 s |
| `NotStash` | http status | Something answered, but it isn't Stash |
| `ApiKeyRequired` | | The server requires an API key |
| `ApiKeyRejected` | | The API key was rejected |
| `ApiKeyInvalidButNotRequired` | | The server doesn't need a key, but this key is wrong |
| `UnsupportedVersion` | found, minimum | The server's version is older than v0.31.1 |
| `ServerNotReady` | status | Stash needs setup or migration in its web UI |
| `CertificateNotVerified` | | Strict checking is on and the certificate couldn't be verified |
| `DuplicateProfile` | existing_id | A profile for this server already exists |

No variant carries a raw response body.

## ServerInfo (in memory; DTO to UI)

| Field | Type | Source |
|---|---|---|
| `version` | string | `version.version` (for example `v0.31.1`) |
| `version_status` | `Supported` · `DevelopmentBuild` · `UnknownButCompatible` | Version gate (R6) |
| `app_schema` | integer | `systemStatus.appSchema` |
| `counts` | `{ scenes, images, galleries, performers }` | `stats` (FR-007) |
