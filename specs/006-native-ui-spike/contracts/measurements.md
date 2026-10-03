# Contract: Measurements (native build ↔ performance harness)

The native build reports the same measurements as the web build so the harness can compare them
(spec FR-013, research R8). Everything here is debug-build behaviour; release builds ignore it.

## Environment (same names and meaning as the web build)

| Variable | Meaning |
|---|---|
| `SSV_HARNESS_PROFILE=<display name>` | Connect to this saved profile instead of the last-used one |
| `SSV_HARNESS_CLEAR_CACHE=1` | Clear that profile's view cache first (thumbnails included) |
| `SSV_HARNESS_EXIT=1` | Exit after the run's measurements are printed |
| `SSV_DEBUG_BENCH=1` | Run the UI bench (below) once Scenes is interactive |
| `SSV_MEASURE=<id>,<id>,…` | Open each scene in the player and measure open, seek, dropped frames |
| `SSV_MEASURE_LONG=<id>`, `SSV_MEASURE_LONG_SECS=<n>` | Play one scene for `n` seconds for dropped frames and CPU |

## Output

One line per result on stdout: `MEASURE ` followed by a JSON object. Names and fields match the web
build's lines, so the harness's existing parsing applies.

| Line (`bench` or key) | Fields | Budget (spec) |
|---|---|---|
| `{"coldStartMs": n}` | first interactive Scenes view after launch | < 2000 ms (SC-009) |
| `nav-first-paint` | n, median, p95, max (ms) | < 150 ms |
| `control-press` | input acknowledgement (ms) | < 50 ms (SC-004) |
| `tab-switch-20-tabs` | ms; plus `videoSized: bool` for the playing tab | < 150 ms (SC-008) |
| `scenes-page-change` | ms, next/previous with thumbnails cached, page size 50 | < 150 ms p95 (SC-005) |
| `scenes-page-jump` | ms until the target page's cards show | < 1000 ms p95 (SC-006) |
| `scenes-scroll-1000-{grid,list}` | n, median, p95, max, baselineMs, fps, missedPercent | < 1% missed (SC-007) |
| `scenes-scroll-1000-{grid,list}-cold` | same, with the page's thumbnails uncached | < 1% missed (SC-007) |
| playback lines (`open_ms`, `seek_ms`, `dropped`, `hwdec`, codec, container; the long run adds `rendered_fps` and `displayed_fps`) | as the web build's `SSV_MEASURE` run; frames rendered but not shown count as dropped | open < 1500 ms, seek < 1000 ms median, 0 dropped/min at 1080p (SC-001–SC-003) |
| `{"uiError": …}` | an unexpected error or panic in the UI, with its message | none expected |

Rules carried over from feature 003: missed frames are frames longer than 1.5 × an idle baseline
measured just before the scroll; the scroll moves 64 px per frame for 5 s; samples that time out are
dropped and counted.

## Harness

- `cargo run --bin perf-harness -- --profile "<name>" [--app web|native] [--quick]`; `web` is the
  default and behaves exactly as today.
- `--app native` builds and runs the `semantic-stash-viewer-native` debug binary with the same
  steps (cold starts with a cleared and a warm cache, UI bench, playback run).
- **New for both apps** (SC-011): after the UI bench and during the long playback, the harness
  samples resident memory (`VmRSS`, summed over the app's process tree) and CPU (user + system time
  per second, summed likewise) from `/proc`, reported as `memory-idle-scenes-50`,
  `memory-playing-1080p`, `cpu-idle-scenes-50`, `cpu-playing-1080p` (info rows, no budget).
- Reports land in the same folder with the app name in the header, so a web run and a native run
  can be read side by side.
