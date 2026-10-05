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

### Both apps, same day (T058), 2026-10-05

Testing profile (1,020 scenes since 2026-10-05), same machine (Xeon W-2135, Radeon RX 9060 XT,
144 Hz display), debug builds as the harness runs them (dependencies optimised, research R16),
playback rows pinned to the same scenes for both apps: `perf-harness --profile <Testing>
--app native|web --scenes 185,192,194 --long 185` (1080p H.264, AV1, HEVC; long: 1080p H.264).
Reports: `perf/20261005-155615.md` (native), `perf/20261005-161035.md` (demo).

| Measurement | Native | Demo | Baseline (demo, T001) | Budget |
|---|---|---|---|---|
| cold-start-warm | 488 ms | 944 ms | 914 ms | 2 s |
| cold-start-cleared | 480 ms | 1029 ms | 990 ms | info |
| nav-first-paint (median) | 14 ms | 35 ms | 37 ms | 150 ms |
| control press (median) | 15 ms | 34 ms | 32 ms | 50 ms |
| tab-switch-20-tabs, one playing (median) | 14 ms (`videoSized` 8/8) | 32 ms | 32 ms | info |
| now-playing-appears / back-to-scene | 14 / 14 ms | 48 / 51 ms | 49 / 50 ms | info |
| scenes-page-change median / p95 | 21 / 340 ms **fail** | 113 / 394 ms **fail** | 89 / 98 ms (3 pages) | 150 ms p95 |
| scenes-page-jump median / p95 | 28 / 70 ms | 120 / 166 ms | 65 / 68 ms | 1 s p95 |
| scenes-scroll-1000 grid (missed) | 5.9% at 144 Hz **fail** | 0.7% at 60 Hz | 0% (≈200 cards) | 1% |
| scenes-scroll-1000 list (missed) | 0.1% at 144 Hz | 0% at 60 Hz | 0% (≈200 cards) | 1% |
| playback-open (median) | 143 ms | 124 ms | 120 ms | 1.5 s |
| seek-to-frame median / p95 | 83 / 238 ms | 81 / 238 ms | 155 / 515 ms | 1 s |
| dropped frames per min (1080p) | 0 | 0 | 0 | 1 |
| playback displayed fps | 29.9 (every frame) | – | – | info |
| playback main-thread CPU | 3% | 10% | 8% | info |
| memory / CPU at rest on Scenes (50) | 736 MB / 0% | 760 MB / 0% (3 processes) | – | info |
| memory / CPU playing 1080p | 778 MB / 8.7% | 1179 MB / 18% (3 processes) | – | info |

**Reading it**:
- The native app is faster on every interaction row, and plays the same (0 dropped, every frame
  shown) with half the CPU and two-thirds of the memory while playing.
- **Page change** fails in both apps on its p95 only: one cold change in 20 (the harness clears
  the cache in its earlier runs); medians are 21 ms (native) and 113 ms (demo).
- **Grid scrolling**: the demo is judged at WebKit's 60 Hz (16.7 ms per frame); the native app at
  the display's 144 Hz (6.9 ms per frame), so it moves 2.4 times the frames per second. It misses
  5.9% of 144 Hz frames with 1000 cards (list mode: 0.1%), which the user found smoother than the
  demo in use (V3). Remaining cause: re-laying out the rows around the view as new rows enter
  (research R16).
- The baseline (T001) had about 200 scenes on Testing, so its page and 1000-card rows aren't
  comparable to today's; its playback rows played other (older) scenes.


### Stability (T059), 2026-10-05

20 scripted cycles on Testing (`SSV_MEASURE_QUIT_WHILE_PLAYING=185`, every other one fullscreen):
launch, connect, restore the saved tabs, open scene 185 in a tab, play it, and close the window
while it plays. Result: **20 of 20 exited cleanly; no crashes, no hangs; no tabs lost** (the
saved tabs grew by one per cycle, 2 → 21, as each cycle opened its scene before the saved tabs
arrived).

Found and fixed on the way:
- Quitting while fullscreen crashed about one run in three (mpv's decoder teardown raced the
  render context being freed): playback is now stopped and mpv left idle first.
- A tab used before the saved tabs arrived (a click right at launch) was replaced by the
  restore: it's now kept as an extra tab, selected, with playback still attached.

The 5 cycles by hand are part of the user's V6 check.
