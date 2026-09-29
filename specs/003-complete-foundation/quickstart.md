# Quickstart & Validation: Complete Phase 0

**Feature**: [spec.md](spec.md) | **Contracts**: [contracts/](contracts/) |
**Model**: [data-model.md](data-model.md) | **Research**: [research.md](research.md)

## Prerequisites

| Need | Check |
|---|---|
| Everything from features 001, 002, and 004 | see their quickstarts |
| A saved profile for your library | `localhost:9999`, read-only use |
| A second profile (e.g. the test instance, `localhost:9998`) | for the "never across servers" check |

## Automated checks

```bash
cargo test -p stash-core   # ViewCache: get/put/touch, LRU eviction at the limit, targeted
                           # invalidation (SC-005), identity mismatch wipes, damaged file recreated,
                           # clear returns bytes freed; refresher: 5 s floor, offline skip,
                           # failure → background notification
npm --prefix ui test       # views render cached data then update in place on view-data-changed;
                           # Home shows the cached summary while connecting; Troubleshooting shows
                           # size and clears
```

The full gate is the same as CI: fmt, clippy with `-D warnings`, all tests, the bindings drift
check, UI lint and typecheck.

## Manual validation

### V1: Instant screens (US1, SC-001)
1. Connect, then open Scenes, and quit.
2. Relaunch. **Expect**: Home shows the server summary, and Scenes shows its lists, both at once
   (under 150 ms), followed by a quiet refresh.

### V2: Offline (US1 scenario 3, FR-003)
1. Stop Stash (or block its port), then relaunch. **Expect**:
   - Home and Scenes still show their cached content, with no cache wording;
   - the connection indicator and the notification centre say the server is unreachable;
   - opening a scene explains that the server can't be reached.

### V3: Changes outside the viewer (SC-003)
1. With Scenes open, add or remove a scene in the Stash web UI (test instance).
2. Revisit Scenes. **Expect**: the list updates in place after one refresh, and the scroll
   position is kept.

### V4: Clear cache (FR-005, SC-004)
1. Settings → Troubleshooting. **Expect**: the cache size.
2. Choose "Clear cache". **Expect**:
   - no confirmation;
   - the space freed is shown;
   - open views reload from the server;
   - the library counts on Home are unchanged.

### V5: Never across servers (FR-007)
1. Edit a profile's address to point at the other Stash instance, then connect. **Expect**: none
   of the first server's scenes appear; the views load from the new server.

### V6: Performance harness (US2, SC-006, SC-007)
```bash
cargo run --bin perf-harness -- --profile "Production"
```
**Expect**:
- it runs unattended in under 10 minutes;
- it prints a table with every measurement, marked pass, fail, info, or invalid;
- it writes `~/.local/share/semantic-stash-viewer/perf/<timestamp>.{json,md}`.

Run it again. **Expect**:
- the second report shows the change from the first, with anything more than 20% slower flagged;
- the medians agree within 10%.

Cover the window during a run. **Expect**: the frame measurements are marked invalid, not failed.

### V7: CI (US3, SC-008)
1. The README shows the CI badge for `main`, and it's green.
2. On a throwaway branch, push a commit with a formatting error. **Expect**: the run fails on
   "Rust format". Then delete the branch. This pushes, so do it only with the owner's go-ahead.
