# Capability Checklist: Port the Viewer to iced

Each row is a capability the native app must offer (spec Overview's table) or a behaviour that
stays exactly (spec "Behaviours that stay"). The acceptance scenario is written for the new UI
when its story is built; retirement of the demo (US6) needs every row to pass and the sign-off at
the end (spec FR-001, FR-003, SC-001).

## Capabilities

| # | From | Capability | Acceptance scenario (new UI) | Tests | Result | Date |
|---|---|---|---|---|---|---|
| C1 | 001 US1 | Add a first server and connect | First launch shows "Connect to Stash"; address (+ key, strict) → Connect tests and saves the server and enters the session; a failure keeps the form and explains why | `tests/onboarding.rs`, `tests/messages.rs` | pass (user, V1) | 2026-10-04 |
| C2 | 001 US2 | Reconnect automatically at launch; see connection health | With a saved server, launch goes straight into the session and connects; the indicator shows connecting, connected, offline (retrying), or the failure | `tests/connection.rs` | pass (user, V1) | 2026-10-04 |
| C3 | 001 US3 | See the connection's security; turn strict TLS on per server | The indicator and server menu always show unencrypted / encrypted-unverified / verified; strict checking is set when adding a server (editing it arrives with Settings, US6) | `tests/connection.rs` (state), manual | pass (user, V1) | 2026-10-04 |
| C4 | 001 US4 | Save several servers; switch, edit, remove, reorder | US1: the server menu lists saved servers, switches between them (leaving one session, entering the next), and adds another (the first-run form; the app switches to it once it connects); a rejected key opens a prompt that re-tests and saves the new key. Editing, removing, reordering: Settings → Servers (US6) | `tests/connection.rs` | partial (US6 completes) | |
| C5 | 002 US1 + 006 | Play a scene inside the window, hardware-decoded, every frame shown | | | | |
| C6 | 002 US2 | The viewer's own playback controls over the video | | | | |
| C7 | 003 US1 | Screens open from the local cache, offline included | | | | |
| C8 | 003 US2 | One command measures every performance budget | | | | |
| C9 | 003 US3 | Every push is checked automatically | | | | |
| C10 | 004 US1 | A navigation bar to every section, Settings, and notifications | | | | |
| C11 | 004 US2 | Several views open in tabs, with back and forward | | | | |
| C12 | 004 US3 | One notification centre: connection alerts, failures, Stash jobs with live progress | | | | |
| C13 | 004 US4 | Settings: servers, keyboard, troubleshooting (cache, logs), about | | | | |
| C14 | 005 US1 | Browse the whole library as a paged grid or list, sorted, with thumbnails | | | | |
| C15 | 004 / 005 | Open a scene into its own view (title, cover, details, Play) | | | | |

## Behaviours that stay

| # | Behaviour | Acceptance scenario (new UI) | Tests | Result | Date |
|---|---|---|---|---|---|
| B1 | A tab never resets: page, size, mode, scroll, selection kept, across back and forward | | | | |
| B2 | Tabs restore fully on relaunch (clarified 2026-10-03) | | | | |
| B3 | Paged lists: default 50, sizes 20–1000, first/previous/next/last, jump, total count | | | | |
| B4 | Everything reachable from the keyboard; the player keeps 002's shortcuts | | | | |
| B5 | mpv's video inside the window with controls on top; every frame shown | | | | |
| B6 | Plain-language, distinct connection failures; security state always visible | Every failure kind has its own title, detail, and hint; the indicator shows the security state at all times | `tests/messages.rs`, `tests/connection.rs` | pass (user, V1) | 2026-10-04 |
| B7 | One notification centre with a badge; toasts never take focus; infrastructure stays out of sight | | | | |
| B8 | The constitution's performance budgets (spec SC-002–SC-006, SC-008) | | | | |

## Sign-off

- [ ] Every row above passes in the native app. Signed off by the user: ____________ (date)
