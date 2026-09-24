# Feature Specification: Connect to Stash

**Feature Branch**: `001-connect-to-stash`

**Created**: 2026-09-23

**Status**: Draft

**Input**: User description: "connect-to-stash" (Roadmap Phase 0: connection profiles, API key
storage, minimum-version check, TLS behaviour and connection-security indicator)

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Connect to a Stash server for the first time (Priority: P1)

A user opens the viewer for the first time. They enter their Stash server's address and, if
their server needs one, an API key. The viewer checks the server, confirms it is a supported
Stash instance, and shows that it is connected: the server's name/address, its Stash version, and
a short library summary (for example, numbers of scenes, images, galleries, and performers). The
connection is saved so the user does not have to enter it again.

**Why this priority**: Nothing else in the viewer works without a connection. This is the
smallest slice that proves the viewer can talk to a real Stash library.

**Independent Test**: On a fresh install, enter the address of a running Stash v0.31.1+ server
(with and without an API key, as that server requires). Confirm the viewer reports it as
connected and shows the correct version and library counts.

**Acceptance Scenarios**:

1. **Given** a fresh install and a running Stash server that has no API key configured,
   **When** the user enters only the server address and chooses Connect, **Then** the viewer
   shows it as connected, with the server's Stash version and library counts.
2. **Given** a Stash server that requires an API key, **When** the user enters the address and a
   valid API key, **Then** the viewer connects and shows the version and library counts.
3. **Given** a Stash server that requires an API key, **When** the user enters the address
   without a key, or with a wrong key, **Then** the viewer does not connect and tells the user
   an API key is required or the key was rejected, without clearing what they typed.
4. **Given** a Stash server older than v0.31.1, **When** the user tries to connect, **Then** the
   viewer refuses, names the server's version and the minimum supported version (v0.31.1), and
   does not save the profile as usable.
5. **Given** an address that does not answer, or that answers but is not a Stash server,
   **When** the user tries to connect, **Then** the viewer says which of the two happened in
   plain language and suggests what to check (address, port, http vs https).
6. **Given** a successful connection, **When** the user chooses to save it, **Then** the profile,
   including its API key, is saved and is available on the next launch without re-entering
   anything.

---

### User Story 2 - Reconnect automatically and see connection health (Priority: P2)

A returning user opens the viewer and it reconnects to the server they used last, without asking
for anything. While they use the viewer, a connection indicator always shows whether the server
can be reached. If the server goes offline or the key stops working, the user is told
straight away and the viewer keeps trying to reconnect.

**Why this priority**: Once a user has connected, every later session starts here. Reconnecting
without friction and failing clearly are what make the viewer feel like an app rather than a
web page.

**Independent Test**: Save a profile, close the viewer, reopen it, and confirm it reconnects
without prompting. Then stop the Stash server while the viewer is open and confirm the indicator
changes and the viewer reconnects on its own once the server is back.

**Acceptance Scenarios**:

1. **Given** a saved profile that was last used, **When** the user launches the viewer, **Then**
   it connects to that profile without prompting and shows the connected state.
2. **Given** a connected session, **When** the server becomes unreachable, **Then** the
   connection indicator changes to "offline" within 10 seconds of the next failed request or
   health check, and a non-blocking notice says the server can't be reached.
3. **Given** an offline session, **When** the server becomes reachable again, **Then** the viewer
   reconnects automatically, without the user doing anything, and the indicator returns to
   "connected".
4. **Given** a saved profile whose API key has been changed or revoked on the server, **When**
   the viewer tries to connect, **Then** it says the key was rejected and offers to update the
   key, instead of retrying forever.
5. **Given** a saved profile whose server has been downgraded below v0.31.1, **When** the viewer
   connects, **Then** it refuses with the same minimum-version message as User Story 1.

---

### User Story 3 - Understand and control connection security (Priority: P2)

The user can always see how secure their current connection is: **unencrypted** (plain http),
**encrypted but unverified** (https with certificate checks off), or **encrypted and verified**
(https with certificate checks on). By default the viewer does not verify certificates, so
self-hosted servers with self-signed certificates just work. For each saved server, the user can
turn on strict certificate checking.

**Why this priority**: The constitution sets TLS checking off by default for convenience, and
requires in return that the security state is always visible and that stricter checking is
available per server. This is required before the viewer is used over untrusted networks, but a
LAN-only user can get value from Story 1 without it.

**Independent Test**: Connect to one server over http, one over https with a self-signed
certificate, and one over https with a publicly trusted certificate. Confirm the indicator shows
the correct state for each, and that turning on strict checking for the self-signed server makes
the connection fail with an explanation.

**Acceptance Scenarios**:

1. **Given** a connection over plain http, **When** the user looks at the connection indicator,
   **Then** it shows "unencrypted".
2. **Given** an https server with a self-signed certificate and strict checking off (the
   default), **When** the user connects, **Then** the connection succeeds and the indicator shows
   "encrypted, not verified".
3. **Given** the same server, **When** the user turns strict checking on for that profile and
   reconnects, **Then** the connection is refused with a message explaining the certificate
   could not be verified, and the user is told how to turn strict checking back off.
4. **Given** an https server with a valid, publicly trusted certificate and strict checking on,
   **When** the user connects, **Then** the indicator shows "encrypted, verified".
5. **Given** any connection state, **When** the user opens the indicator's details, **Then** they
   see the server address, the security state, and whether strict checking is on for this
   profile.

---

### User Story 4 - Manage several Stash servers (Priority: P3)

A user with more than one Stash library (for example a home server and a travel or test server)
saves each as its own profile, gives it a friendly name, switches between them, edits the
address, key, or security setting, and deletes profiles they no longer need.

**Why this priority**: Useful for a subset of users; a single profile already delivers the core
value.

**Independent Test**: Create two profiles for two different Stash servers, switch between them,
and confirm the library summary changes to match each server. Edit one profile's key, delete the
other, and confirm the deleted profile is gone after a relaunch.

**Acceptance Scenarios**:

1. **Given** one saved profile, **When** the user adds a second profile and switches to it,
   **Then** the viewer disconnects from the first, connects to the second, and shows the second
   server's details.
2. **Given** a saved profile, **When** the user edits its address, API key, name, or strict
   checking setting, **Then** the change is validated by reconnecting before it is saved, and a
   failed check leaves the previous working settings in place unless the user explicitly saves
   anyway.
3. **Given** a saved profile, **When** the user deletes it and confirms, **Then** the profile and
   its API key are both removed, and if it was the active profile the viewer goes back to
   the connection screen (or to another saved profile the user picks).
4. **Given** the user enters an address that matches an existing profile, **When** they try to
   save it as a new profile, **Then** the viewer warns that a profile for this server already
   exists and offers to open that one instead.

---

### Edge Cases

- **Address formats**: the user enters an address without a scheme (`192.168.1.10:9999`), with a
  trailing slash, or with a path such as `/graphql` pasted from elsewhere. The viewer normalises
  these to the server's base address. When no scheme is given, it tries https first, falls back
  to http, and shows which one it used.
- **Redirects**: if the server redirects (for example http to https), the viewer follows it,
  shows the final address, and computes the security state from the final connection.
- **Unparseable or development versions**: a server whose version can't be read, or is a
  development build, is allowed with a warning that compatibility is not guaranteed, as long as
  it has the capabilities the viewer checks for.
- **API key given but not needed**: a *valid* key entered for a server that doesn't need one is
  accepted and kept (the server may require one later). Stash rejects an *invalid* key even
  when it has no authentication configured. In that case the viewer explains that the server
  doesn't need a key but the one entered is wrong, and offers to connect without it.
- **Slow servers**: a connection attempt that takes longer than 15 seconds is abandoned with a
  timeout message. The user can cancel an attempt at any time.
- **Launch with the last server offline**: the viewer opens in the offline state for that profile
  (Story 2) rather than blocking on a modal, and still lets the user switch profiles.

## Requirements *(mandatory)*

### Functional Requirements

**Connecting**

- **FR-001**: Users MUST be able to connect by entering a Stash server address and an optional
  API key.
- **FR-002**: The viewer MUST normalise entered addresses (missing scheme, trailing slash, pasted
  API paths) to the server's base address, trying https before http when no scheme is given.
- **FR-003**: The viewer MUST confirm the server is a Stash instance and read its version before
  reporting a successful connection.
- **FR-004**: The viewer MUST refuse to connect to servers older than Stash v0.31.1 and MUST name
  both the server's version and the minimum supported version in the message.
- **FR-005**: The viewer MUST tell apart and report, in plain language, at least these failures:
  server unreachable, not a Stash server, API key required, API key rejected, API key invalid
  but not required, unsupported version, server not ready (setup or migration pending),
  certificate not verified (strict mode), and timeout.
- **FR-006**: A connection attempt MUST be cancellable and MUST time out after 15 seconds.
- **FR-007**: On success, the viewer MUST show the server address, its Stash version, and a
  library summary with counts of scenes, images, galleries, and performers.

**Profiles**

- **FR-008**: The viewer MUST save a successful connection as a server profile containing a
  display name, address, strict-checking setting, and API key (if any).
- **FR-009**: Profiles, including their API keys, MUST be saved in the viewer's local config file
  so they are available on the next launch without re-entry.
- **FR-010**: The API key MUST be viewable and editable as plain text in the connection form and
  the profile settings.
- **FR-011**: Users MUST be able to add, rename, edit, switch between, and delete server
  profiles.
- **FR-012**: Deleting a profile MUST require confirmation.
- **FR-013**: Edits to a profile's address, key, or security setting MUST be checked by
  reconnecting before they replace the previous working settings.

**Session behaviour**

- **FR-014**: On launch, the viewer MUST automatically connect to the most recently used profile
  without prompting.
- **FR-015**: The viewer MUST show a persistent connection indicator with at least these states:
  connecting, connected, offline, and authentication failed.
- **FR-016**: When the server becomes unreachable during a session, the viewer MUST switch to the
  offline state without blocking the interface, and MUST keep retrying automatically, with
  increasing intervals capped at 60 seconds, until it reconnects or the user switches profiles.
- **FR-017**: When authentication fails on a saved profile, the viewer MUST stop automatic
  retries and prompt the user to update the key.

**Connection security**

- **FR-018**: Certificate verification MUST be off by default for new profiles.
- **FR-019**: Each profile MUST have a setting to turn strict certificate verification on; with it
  on, connections to servers whose certificate can't be verified MUST be refused with an
  explanation.
- **FR-020**: The connection indicator MUST always show the security state: unencrypted,
  encrypted not verified, or encrypted and verified, computed from the final connection after
  any redirects.
- **FR-021**: The indicator's details view MUST show the server address, security state, and
  whether strict checking is on for this profile.

### Key Entities

- **Server Profile**: one saved Stash server the user can connect to. Attributes: display name,
  normalised base address, strict-certificate-checking setting, API key (optional), last-used
  time.
- **Connection Session**: the live connection to the active profile. Attributes: state
  (connecting, connected, offline, authentication failed), security state (unencrypted,
  encrypted not verified, encrypted verified), final address after redirects, last successful
  contact time.
- **Server Info**: facts read from the server on connect. Attributes: Stash version,
  whether it meets the minimum (v0.31.1), and the library summary counts (scenes, images,
  galleries, performers).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A new user connects to a LAN Stash server, from first launch to seeing the library
  summary, in under 1 minute.
- **SC-002**: On relaunch with a reachable saved server, the connected state is shown within 2
  seconds of the window appearing, with no prompts.
- **SC-003**: 100% of the failure types in FR-005 produce a distinct, plain-language message;
  none of them shows a raw technical error as the only explanation.
- **SC-004**: After saving a profile with an API key, 100% of relaunches connect without asking
  the user to re-enter the address or key, and copying the config file to another machine is
  enough to connect from there.
- **SC-005**: When the server goes offline, the indicator reflects it within 10 seconds, and the
  viewer reconnects within 60 seconds of the server coming back, with no user action.
- **SC-006**: Connections to servers below v0.31.1 are refused 100% of the time, and connections
  to v0.31.1 and newer are never refused because of their version.
- **SC-007**: In testing against http, self-signed https, and publicly trusted https servers, the
  security indicator shows the correct state in every case.

## Assumptions

- **API key only.** Authentication uses a Stash API key, or none. Signing in with Stash's
  username and password is out of scope for this feature. Stash issues an API key once
  credentials are configured, and the web UI shows it in Settings → Security.
- **One active server at a time.** The viewer connects to one profile at a time; viewing several
  libraries at once is out of scope.
- **Remote servers work the same way.** Remote servers over the internet are supported with the
  same flow; no reverse-proxy-specific authentication (for example single sign-on in front of
  Stash) is handled in this feature.
- **Library summary is a status readout.** The counts only confirm the connection works;
  browsing the library is Roadmap Phase 1.
- **Dependencies.** This feature is the foundation for every later feature. Video playback
  (`mpv-playback-spike`) and the shared cache layer are separate Phase 0 specs that build on the
  connection established here.
- **Constitution.** Principles III (the interface never talks to Stash directly), VI (the
  interface never blocks on the network), and VII (API key as plain profile config, TLS default
  off with a visible indicator) apply directly and are reflected in the requirements above.
