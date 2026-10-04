# Quickstart: Port the Viewer to iced

How to build, run, and validate the native app. Measurements use the **Testing** profile
(`localhost:9998`, NVMe); Production (`localhost:9999`) is for manual checks only. The native app
keeps its own files under `dev.semantic-stash-viewer/` (research R4) and never touches the demo's.

## Build and run

```bash
cargo run -p semantic-stash-viewer-native        # the native app
cargo test --workspace                            # all tests, transition tests included
cargo run -p perf-harness -- --profile "Testing"  # the harness (native by default)
```

## Validation

Each story records its scenarios and results in [capabilities.md](capabilities.md).

### V0: Baseline and core switch
- `perf-harness --profile "Testing" --app web`: the demo's numbers on the same library (SC-008).
- After moving the core to Cynic (research R5): `cargo test -p stash-core` passes unchanged, and
  the demo and the 006 native build still browse and play.

### V1: Servers (US1)
First launch with no native files: add Testing with its key, connect; add Production; switch;
edit; remove the last and land back in onboarding; relaunch. Check the security state on each,
and the plain-language message for a wrong address, a wrong key, and an old server.

### V2: Shell (US2)
Sections, 20 tabs, back and forward (every screen comes back exactly), overlays, keyboard only,
the now-playing bar; quit and relaunch: every tab, history, scroll position, and the selected tab
come back. Corrupt `session.json`: the app starts fresh with Home.

### V3: Scenes (US3)
50 and 1000 per page, grid and list, sorts, sizes, go to page, keyboard, open and back; offline
with cached pages. Harness: page change, jump, 1000-card scroll cold and cached.

### V4: Scene view and playback (US4)
Every codec in Testing's `ssv-native-spike` set; every control and the 002 keys; switch tabs while
playing; fullscreen; quit while playing; a file that can't play. Clippy passes with the
unsafe-code lints.

### V5: Notifications (US5)
Stop and restart the Testing server; start a scan from the Stash web UI on Testing; open a broken
file.

### V6: Settings and retirement (US6)
Every Settings page; clear the cache (library unchanged). Full harness for both apps; the 20-cycle
stability loop; capability checklist complete; **user sign-off**. Then remove the demo and check a
fresh clone builds, tests, runs, and measures with only Rust (SC-009).

## Gate (every story)

`cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings` (with the
unsafe-code lints once R11 lands); `cargo test --workspace`. While the demo exists, its bindings
check and UI lint, typecheck, and tests too.

## Results

### Baseline (demo on Testing), 2026-10-04 (T001)

`perf-harness --profile "Testing" --app web`, the frozen Tauri demo, same machine as 006
(report `~/.local/share/semantic-stash-viewer/perf/20261004-004124.md`). SC-008's reference.

| Measurement | Median | p95 | Max | Samples |
|---|---|---|---|---|
| cold-start-warm | 914 ms | 1114 ms | 1114 ms | 3 |
| cold-start-cleared | 990 ms | 1006 ms | 1006 ms | 3 |
| nav-first-paint | 37 ms | 72 ms | 72 ms | 20 |
| input-ack | 32 ms | 49 ms | 49 ms | 10 |
| scroll-missed-frames | 0% | | | |
| tab-switch-20-tabs | 32 ms | 43 ms | 46 ms | 40 |
| now-playing-appears | 49 ms | 50 ms | 50 ms | 10 |
| back-to-scene | 50 ms | 134 ms | 134 ms | 10 |
| scenes-page-change | 89 ms | 98 ms | 98 ms | 3 (Testing's library is small: few pages) |
| scenes-page-jump | 65 ms | 68 ms | 68 ms | 3 |
| scenes-scroll-1000 grid / list | 0% / 0% missed | | | |
| playback-open | 120 ms | 164 ms | 164 ms | 3 |
| seek-to-frame | 155 ms | 515 ms | 563 ms | 30 |
| dropped frames per min (1080p) | 0 | | | |
| playback main-thread CPU | 8% | | | |

**Caveat**: Testing holds about 200 scenes, so the "1000-card" rows scrolled a page of about 200
cards and the page rows had few pages. For SC-005 the 1000-card comparison uses Production (both
apps, same day) or more scenes copied to Testing.

`playback-displayed-fps` has no demo samples (the demo presents every frame mpv renders; only
the native build reports it).
