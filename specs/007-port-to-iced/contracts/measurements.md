# Contract: Measurements (native build, perf-harness)

Extends [006's measurement contract](../../006-native-ui-spike/contracts/measurements.md): same
environment variables, same `MEASURE {json}` lines, same harness rules (feature 003). Changes:

## Harness

- Moves to its own crate, `crates/perf-harness`, binary `perf-harness`
  (`cargo run -p perf-harness -- --profile "<name>" [--app web|native] [--quick]`). It never
  depended on Tauri.
- `--app` defaults to **`native`** from this feature on; `web` stays available until retirement,
  then the option is removed.
- Reports keep the `app` field, and comparisons stay within one app.

## UI bench lines (native)

Rewritten for the new UI; the line names stay the demo's so reports compare.

| Line | What | Budget (spec) |
|---|---|---|
| `nav-first-paint` | navigation to a section, until drawn | < 150 ms (SC-003) |
| `control-press` | a control press until its change is drawn | < 50 ms (SC-003) |
| `tab-switch-20-tabs` | switching among 20 tabs, one playing; `videoSized` for the playing tab | < 150 ms (SC-003) |
| `now-playing-appears`, `back-to-scene` | as the web build | info |
| `scroll-frame-time` | the 003 reference scroll | < 1% missed |
| `scenes-page-change` | next/previous, thumbnails drawn, 1 s glance between | < 150 ms p95 (SC-004) |
| `scenes-page-jump` | until the target page's cards show | < 1 s p95 (SC-004) |
| `scenes-scroll-1000-{grid,list}-cold` | thumbnails still arriving | < 1% missed (SC-005) |
| `scenes-scroll-1000-{grid,list}` | thumbnails cached | < 1% missed (SC-005) |
| `scenes-thumbs-1000-{grid,list}` | how long the page's thumbnails took to arrive before the cached scroll | info |
| `scenes-page-1000` | cards on the 1000-per-page page (fewer when the library is smaller) | info |

Scrolling moves 3,840 px per second (the web bench's 64 px per frame at 60 Hz), whatever the
display's refresh rate, and runs inside the scrollable with no messages per frame; each line
carries `pxPerSecond`. A frame counts as missed when it takes over 1.5 times the display's idle
frame time. `scroll-frame-time` (the 003 reference list) isn't in the native app and reports
"not measured".

Playback lines as 006 (with `rendered_fps` / `displayed_fps`; frames not shown count as
dropped).

## Memory and CPU (both apps)

`memory-idle-scenes-50`, `memory-playing-1080p`, `cpu-idle-scenes-50`, `cpu-playing-1080p`: the
app's whole process tree (for the web build, including WebKit's processes), sampled from `/proc`
after the bench settles and during the long playback. Info rows.

## Baseline

SC-008 compares against the web build's numbers from a run on the **Testing** profile on the same
machine, taken at the start of this feature (its earlier full runs used Production's spinning
disks).
