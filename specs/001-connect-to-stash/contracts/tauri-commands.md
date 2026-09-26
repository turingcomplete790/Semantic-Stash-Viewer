# Contract: Tauri Commands & Events (UI ↔ Rust core)

**Feature**: [../spec.md](../spec.md) | **Types**: [../data-model.md](../data-model.md)

This is the only interface between the SolidJS UI and the Rust core for this feature
(Principle III). Bindings are generated into `ui/src/bindings.ts` by `tauri-specta` (research
R3). Rust names are `snake_case`; the generated TypeScript uses `camelCase`.

## Invariants

1. **Commands never block the UI.** Long operations (`test_connection`, `connect`) are async
   and cancellable. Progress and results also arrive as `connection-state` events.
2. **Errors are typed.** Every fallible command returns `Result<T, ConnectFailure | AppError>`.
   The UI maps each variant to a plain-language message (FR-005). There are no stringly-typed
   errors.
3. **The UI performs no network I/O.** It issues no `fetch` or GraphQL calls. The Tauri
   capability config grants no HTTP permission to the webview.

## DTOs exposed to the UI

```ts
type SecurityState = "unencrypted" | "encryptedUnverified" | "encryptedVerified";

type ProfileSummary = {
  id: string;
  displayName: string;
  baseUrl: string;
  strictTls: boolean;
  apiKey: string | null;             // plain profile config (constitution VII)
  lastUsedAt: string | null;         // ISO 8601
};

type ServerInfo = {
  version: string;                   // "v0.31.1"
  versionStatus: "supported" | "developmentBuild" | "unknownButCompatible";
  appSchema: number;
  counts: { scenes: number; images: number; galleries: number; performers: number };
};

type ConnectionSnapshot = {
  profileId: string | null;
  state:
    | { kind: "idle" }
    | { kind: "connecting"; attemptUrl: string }
    | { kind: "connected" }
    | { kind: "offline"; attempt: number; nextRetryAt: string }
    | { kind: "authFailed"; failure: ConnectFailure }
    | { kind: "failed"; failure: ConnectFailure };
  security: SecurityState | null;
  finalUrl: string | null;
  server: ServerInfo | null;
  lastContactAt: string | null;
};

type ProfileDraft = {
  displayName?: string;
  address: string;
  apiKey: string | null;             // empty string is treated as null
  strictTls: boolean;
};

type TestResult = {
  normalizedUrl: string;             // final URL after redirects
  security: SecurityState;
  server: ServerInfo;
};
```

`ConnectFailure` is the enum in [data-model.md](../data-model.md#connectfailure-enum-maps-11-to-fr-005-messages),
serialized as `{ kind: "...", ...payload }`.

## Commands

| Command | Input | Output | Notes / requirements |
|---|---|---|---|
| `list_profiles` | — | `ProfileSummary[]` | Ordered as the user arranged them |
| `test_connection` | `ProfileDraft`, `requestId: string` | `TestResult` | Runs probe + version gate without saving (US1). 15 s timeout (FR-006) |
| `cancel_request` | `requestId: string` | `void` | Cancels an in-flight `test_connection` or `connect` |
| `create_profile` | `ProfileDraft` | `ProfileSummary` | Re-runs the probe; saves only on success. Fails with `DuplicateProfile` (US4 AS4) |
| `update_profile` | `profileId`, `ProfileDraft`, `force: boolean` | `ProfileSummary` | Validates by reconnecting before replacing settings (FR-013). With `force = true`, saves even if validation fails (US4 AS2) |
| `delete_profile` | `profileId` | `void` | UI confirms first (FR-012). If the profile was active, the session goes to Idle |
| `reorder_profiles` | `profileIds: string[]` | `void` | |
| `connect` | `profileId`, `requestId` | `ConnectionSnapshot` | Makes the profile active and updates `last_used_profile_id` on success |
| `disconnect` | — | `void` | Session goes to Idle |
| `get_connection_snapshot` | — | `ConnectionSnapshot` | For UI hydration on window load |

**Auto-connect (FR-014)** is not a command. On startup the core reads `last_used_profile_id`
and starts `connect` itself. The UI hydrates with `get_connection_snapshot` and then follows the
events.

## Events (core → UI)

| Event | Payload | Emitted when |
|---|---|---|
| `connection-state` | `ConnectionSnapshot` | Every state transition in [data-model.md](../data-model.md#connectionstate-and-transitions), including each Offline retry attempt |
| `profiles-changed` | `ProfileSummary[]` | After create, update, delete, or reorder |

## UI surfaces bound to this contract

| Surface | Commands / events | Spec |
|---|---|---|
| Connection screen (first run, add profile) | `test_connection`, `create_profile`, `cancel_request` | US1, FR-001…FR-010 |
| Connection indicator (always visible) + details popover | `connection-state`, `get_connection_snapshot` | US2, US3, FR-015, FR-020, FR-021 |
| "Key rejected: update key" prompt | `update_profile`, `connect` | US2 AS4, FR-017 |
| Server profiles manager | `list_profiles`, `update_profile`, `delete_profile`, `reorder_profiles`, `connect`, `profiles-changed` | US4, FR-011…FR-013 |
