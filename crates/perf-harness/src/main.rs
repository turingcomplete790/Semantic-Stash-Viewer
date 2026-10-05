//! One command for every Phase 0 performance budget (003 US2, research R8).
//!
//! `cargo run -p perf-harness -- --profile "<name>" [--app native|web] [--quick]` builds the
//! debug app (`native`, the default, or the frozen web demo until it's removed; 007
//! contracts/measurements.md), starts
//! the UI dev server if it isn't running, launches the app several times with the harness
//! switches, collects its `MEASURE` lines, and writes a report to
//! `~/.local/share/semantic-stash-viewer/perf/`. Exits non-zero if a budget failed.
//!
//! The harness never talks to Stash and never sees an address: the app picks its own scenes.

mod report;

use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::Value;

use report::{summarise, Check, Measurement, Report, Unit};

const DEV_PORT: u16 = 5173;
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(300);
const LONG_PLAYBACK_SECS: u32 = 60;

/// Which build to measure (006): the Tauri web build, or the native spike build.
#[derive(Clone, Copy, PartialEq, Eq)]
enum App {
    Web,
    Native,
}

impl App {
    fn package(self) -> &'static str {
        match self {
            App::Web => "semantic-stash-viewer",
            App::Native => "semantic-stash-viewer-native",
        }
    }

    fn name(self) -> &'static str {
        match self {
            App::Web => "web",
            App::Native => "native",
        }
    }
}

struct Options {
    profile: String,
    quick: bool,
    app: App,
}

fn parse_args() -> Result<Options, String> {
    let mut profile = None;
    let mut quick = false;
    let mut app = App::Native;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--profile" => profile = args.next(),
            "--quick" => quick = true,
            "--app" => {
                app = match args.next().as_deref() {
                    Some("web") => App::Web,
                    Some("native") => App::Native,
                    _ => return Err("--app takes web or native".into()),
                }
            }
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(Options {
        profile: profile.ok_or("--profile <display name> is required")?,
        quick,
        app,
    })
}

fn main() -> ExitCode {
    let options = match parse_args() {
        Ok(o) => o,
        Err(message) => {
            if !message.is_empty() {
                eprintln!("{message}");
            }
            eprintln!(
                "usage: perf-harness --profile \"<display name>\" [--app native|web] [--quick]"
            );
            return ExitCode::from(2);
        }
    };
    match run(&options) {
        Ok(report) => {
            if report.failed() {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(e) => {
            eprintln!("perf-harness: {e}");
            ExitCode::from(2)
        }
    }
}

fn workspace_root() -> PathBuf {
    // crates/perf-harness → the workspace root.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/perf-harness sits two levels below the workspace root")
        .to_path_buf()
}

fn run(options: &Options) -> Result<Report, String> {
    let started = Instant::now();
    let root = workspace_root();

    step(&format!("Building the debug app ({})", options.app.name()));
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = Command::new(cargo)
        .args(["build", "-p", options.app.package()])
        .current_dir(&root)
        .status()
        .map_err(|e| format!("couldn't run cargo: {e}"))?;
    if !status.success() {
        return Err("the build failed".into());
    }
    let target =
        std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), PathBuf::from);
    let app = target.join("debug").join(options.app.package());

    // Only the web build loads its UI from the dev server.
    let _dev_server = match options.app {
        App::Web => Some(DevServer::start(&root)?),
        App::Native => None,
    };

    let base = [
        ("SSV_HARNESS_PROFILE", options.profile.clone()),
        ("SSV_HARNESS_EXIT", "1".into()),
    ];
    let reps = if options.quick { 1 } else { 3 };

    step("Warm-up launch (discarded)");
    launch(&app, &base, &[])?;

    let mut cleared = Runs::default();
    for i in 1..=reps {
        step(&format!("Cold start, cleared cache ({i}/{reps})"));
        cleared.push(launch(&app, &base, &[("SSV_HARNESS_CLEAR_CACHE", "1")])?);
    }
    let mut warm = Runs::default();
    for i in 1..=reps {
        step(&format!("Cold start, warm cache ({i}/{reps})"));
        warm.push(launch(&app, &base, &[])?);
    }

    step("UI bench");
    let bench = launch(&app, &base, &[("SSV_DEBUG_BENCH", "1")])?;

    step("Playback");
    let long_secs = LONG_PLAYBACK_SECS.to_string();
    let playback = launch(
        &app,
        &base,
        &[
            ("SSV_MEASURE", "auto"),
            ("SSV_MEASURE_LONG", "auto"),
            ("SSV_MEASURE_LONG_SECS", &long_secs),
        ],
    )?;

    let mut measurements = Vec::new();
    measurements.push(cold_start("cold-start-warm", Some(2000.0), &warm));
    measurements.push(cold_start("cold-start-cleared", None, &cleared));
    measurements.extend(bench_measurements(&bench));
    measurements.extend(scenes_measurements(&bench));
    measurements.extend(playback_measurements(&playback));

    let dir = perf_dir();
    let previous = newest_report(&dir, options.app.name());
    let mut report = Report {
        created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        machine: machine(),
        profile: options.profile.clone(),
        app: options.app.name().to_owned(),
        measurements,
    };
    report.compare(previous.as_ref());

    std::fs::create_dir_all(&dir).map_err(|e| format!("couldn't create {}: {e}", dir.display()))?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    let markdown = report.to_markdown();
    std::fs::write(dir.join(format!("{stamp}.json")), json).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(format!("{stamp}.md")), &markdown).map_err(|e| e.to_string())?;

    println!("\n{markdown}");
    println!(
        "Report: {}  ({} s)",
        dir.join(format!("{stamp}.md")).display(),
        started.elapsed().as_secs()
    );
    Ok(report)
}

fn step(what: &str) {
    eprintln!("==> {what}");
}

// ---- Launching the app -------------------------------------------------------------------

/// The `MEASURE` lines of one launch, and why it stopped early if it did.
#[derive(Default)]
struct Launch {
    lines: Vec<Value>,
    problem: Option<String>,
}

impl Launch {
    /// Why none of this launch's measurements can be trusted, if so. Lines for one scene or
    /// one bench item carry their own `invalid` and are judged on their own.
    fn invalid(&self) -> Option<String> {
        let own = |l: &Value| {
            ["scene", "long", "bench"]
                .iter()
                .any(|k| l.get(k).is_some())
        };
        self.lines
            .iter()
            .filter(|l| !own(l))
            .find_map(|l| l.get("invalid").and_then(Value::as_str).map(str::to_owned))
            .or_else(|| self.problem.clone())
    }
}

#[derive(Default)]
struct Runs(Vec<Launch>);

impl Runs {
    fn push(&mut self, launch: Launch) {
        self.0.push(launch);
    }
}

fn launch(app: &Path, base: &[(&str, String)], extra: &[(&str, &str)]) -> Result<Launch, String> {
    let mut command = Command::new(app);
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    // The native app's video path needs wgpu on Vulkan (it doesn't set its own environment);
    // an explicit choice in the harness's environment wins.
    if std::env::var_os("WGPU_BACKEND").is_none() {
        command.env("WGPU_BACKEND", "vulkan");
    }
    for (k, v) in base {
        command.env(k, v);
    }
    for (k, v) in extra {
        command.env(k, v);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("couldn't start {}: {e}", app.display()))?;
    let stdout = child.stdout.take().expect("piped stdout");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(json) = line.strip_prefix("MEASURE ") {
                if let Ok(value) = serde_json::from_str::<Value>(json) {
                    eprintln!("    {}", describe_line(&value));
                    let _ = tx.send(value);
                }
            }
        }
    });

    let deadline = Instant::now() + LAUNCH_TIMEOUT;
    let mut result = Launch::default();
    loop {
        while let Ok(value) = rx.try_recv() {
            result.lines.push(value);
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                // Drain what's left after exit.
                std::thread::sleep(Duration::from_millis(100));
                result.lines.extend(rx.try_iter());
                if !status.success() {
                    result.problem = Some(format!("app exited with {status}"));
                }
                break;
            }
            Ok(None) if Instant::now() > deadline => {
                let _ = child.kill();
                let _ = child.wait();
                result.lines.extend(rx.try_iter());
                result.problem = Some("timed out after 5 minutes".into());
                break;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(format!("lost the app process: {e}")),
        }
    }
    Ok(result)
}

/// A short progress line (no addresses are ever in `MEASURE` lines).
fn describe_line(value: &Value) -> String {
    let text = value.to_string();
    if text.chars().count() > 160 {
        format!("{}…", text.chars().take(160).collect::<String>())
    } else {
        text
    }
}

/// The UI dev server, started here unless it's already running; stopped on drop.
struct DevServer(Option<Child>);

impl DevServer {
    fn start(root: &Path) -> Result<Self, String> {
        if port_open() {
            eprintln!("==> Using the UI dev server already on port {DEV_PORT}");
            return Ok(Self(None));
        }
        step("Starting the UI dev server");
        let mut command = Command::new("npm");
        command
            .args(["--prefix", "ui", "run", "dev"])
            .current_dir(root)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        let child = command
            .spawn()
            .map_err(|e| format!("couldn't start npm: {e}"))?;
        let server = Self(Some(child));
        let deadline = Instant::now() + Duration::from_secs(60);
        while !port_open() {
            if Instant::now() > deadline {
                return Err("the UI dev server didn't start within 60 s".into());
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        Ok(server)
    }
}

impl Drop for DevServer {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            // npm starts vite as a child: stop the whole process group.
            #[cfg(unix)]
            let _ = Command::new("kill")
                .args(["-TERM", "--", &format!("-{}", child.id())])
                .status();
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Vite listens on `localhost`, which may be IPv6 only (`::1`): try every address it has.
fn port_open() -> bool {
    use std::net::ToSocketAddrs;
    ("localhost", DEV_PORT)
        .to_socket_addrs()
        .map(|mut addrs| {
            addrs.any(|addr| TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok())
        })
        .unwrap_or(false)
}

// ---- Turning MEASURE lines into measurements ---------------------------------------------

fn number(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

fn cold_start(name: &str, budget: Option<f64>, runs: &Runs) -> Measurement {
    let mut samples = Vec::new();
    let mut invalid = None;
    for run in &runs.0 {
        match (
            run.invalid(),
            run.lines.iter().find_map(|l| number(l, "coldStartMs")),
        ) {
            (None, Some(ms)) => samples.push(ms),
            (reason, _) => invalid = invalid.or(reason).or(Some("no cold start mark".into())),
        }
    }
    // Any bad run makes the whole measurement untrustworthy (FR-016).
    Measurement::from_samples(name, Unit::Ms, budget, &samples, invalid)
}

/// A summary line from the UI bench (`{"bench": name, n, median, p95, max}`).
fn bench_line<'a>(launch: &'a Launch, name: &str) -> Option<&'a Value> {
    launch
        .lines
        .iter()
        .find(|l| l.get("bench").and_then(Value::as_str) == Some(name))
}

fn from_bench(
    launch: &Launch,
    bench: &str,
    name: &str,
    budget: Option<f64>,
    check: Check,
) -> Measurement {
    let line = bench_line(launch, bench);
    let summary = line.and_then(|l| {
        Some(report::Summary {
            median: number(l, "median")?,
            p95: number(l, "p95")?,
            max: number(l, "max")?,
            #[allow(clippy::cast_possible_truncation)]
            samples: number(l, "n")? as usize,
        })
    });
    let invalid = line
        .and_then(|l| l.get("invalid").and_then(Value::as_str).map(str::to_owned))
        .or_else(|| launch.invalid())
        .or_else(|| line.is_none().then(|| "not measured".to_owned()));
    Measurement::new(name, Unit::Ms, budget, check, summary, invalid)
}

fn bench_measurements(launch: &Launch) -> Vec<Measurement> {
    let scroll = bench_line(launch, "scroll-frame-time");
    let missed = scroll.and_then(|l| number(l, "missedPercent"));
    let fps = scroll.and_then(|l| number(l, "fps"));
    let scroll_invalid = scroll
        .and_then(|l| l.get("invalid").and_then(Value::as_str).map(str::to_owned))
        .or_else(|| launch.invalid())
        .or_else(|| missed.is_none().then(|| "not measured".into()));
    vec![
        from_bench(
            launch,
            "nav-first-paint",
            "nav-first-paint",
            Some(150.0),
            Check::Median,
        ),
        from_bench(
            launch,
            "control-press",
            "input-ack",
            Some(50.0),
            Check::Median,
        ),
        // 60 fps: under 1% of frames take over 1.5 display frames (research R8).
        Measurement::from_samples(
            "scroll-missed-frames",
            Unit::Percent,
            Some(1.0),
            &missed.into_iter().collect::<Vec<_>>(),
            scroll_invalid.clone(),
        ),
        from_bench(
            launch,
            "scroll-frame-time",
            "scroll-frame-time",
            None,
            Check::P95,
        ),
        // Frames drawn per second while scrolling (the display's rate when nothing is missed).
        Measurement::from_samples(
            "scroll-fps",
            Unit::Fps,
            None,
            &fps.into_iter().collect::<Vec<_>>(),
            scroll_invalid.clone(),
        ),
        from_bench(
            launch,
            "tab-switch-20-tabs",
            "tab-switch-20-tabs",
            None,
            Check::Median,
        ),
        from_bench(
            launch,
            "now-playing-appears",
            "now-playing-appears",
            None,
            Check::Median,
        ),
        from_bench(
            launch,
            "back-to-scene",
            "back-to-scene",
            None,
            Check::Median,
        ),
    ]
}

/// The paged Scenes view (005 R10): page change (< 150 ms at p95, 50 per page), page jump
/// (< 1 s at p95), and scrolling a 1000-card page (< 1% missed frames in each mode).
fn scenes_measurements(launch: &Launch) -> Vec<Measurement> {
    let mut out = vec![
        from_bench(
            launch,
            "scenes-page-change",
            "scenes-page-change",
            Some(150.0),
            Check::P95,
        ),
        from_bench(
            launch,
            "scenes-page-jump",
            "scenes-page-jump",
            Some(1000.0),
            Check::P95,
        ),
    ];
    for mode in ["grid", "list"] {
        let line = bench_line(launch, &format!("scenes-scroll-1000-{mode}"));
        let invalid = line
            .and_then(|l| l.get("invalid").and_then(Value::as_str).map(str::to_owned))
            .or_else(|| launch.invalid())
            .or_else(|| line.is_none().then(|| "not measured".to_owned()));
        let missed = line.and_then(|l| number(l, "missedPercent"));
        out.push(Measurement::from_samples(
            &format!("scenes-scroll-1000-{mode}-missed-frames"),
            Unit::Percent,
            Some(1.0),
            &missed.into_iter().collect::<Vec<_>>(),
            invalid,
        ));
    }
    out
}

fn playback_measurements(launch: &Launch) -> Vec<Measurement> {
    let run_invalid = launch.invalid();
    let scenes: Vec<&Value> = launch
        .lines
        .iter()
        .filter(|l| l.get("scene").is_some())
        .collect();
    let played: Vec<&&Value> = scenes
        .iter()
        .filter(|l| l.get("result").and_then(Value::as_str) == Some("played"))
        .collect();
    let opens: Vec<f64> = played.iter().filter_map(|l| number(l, "open_ms")).collect();
    let seeks: Vec<f64> = played
        .iter()
        .filter_map(|l| l.get("seek_ms").and_then(Value::as_array))
        .flatten()
        .filter_map(Value::as_f64)
        .collect();
    let failed = scenes.len() - played.len();
    let scene_invalid = run_invalid.clone().or_else(|| {
        (failed > 0).then(|| format!("{failed} of {} scenes didn't play", scenes.len()))
    });

    let long = launch.lines.iter().find(|l| l.get("long").is_some());
    let per_minute = long.and_then(|l| {
        let dropped = number(l, "dropped")? + not_shown(l);
        let seconds = number(l, "seconds")?;
        (seconds > 0.0).then(|| (dropped / (seconds / 60.0) * 100.0).round() / 100.0)
    });
    // The native build also reports the rate the UI put frames on screen (006): mpv's own counter
    // can't see a frame that was rendered but never shown.
    let displayed = long.and_then(|l| number(l, "displayed_fps"));
    let cpu = long.and_then(|l| number(l, "main_thread_cpu_percent"));
    let long_invalid = run_invalid
        .or_else(|| long.and_then(|l| l.get("invalid").and_then(Value::as_str).map(str::to_owned)))
        .or_else(|| long.and_then(|l| l.get("error").map(|e| format!("long run: {e}"))))
        .or_else(|| long.is_none().then(|| "not measured".into()));

    vec![
        Measurement::from_samples(
            "playback-open",
            Unit::Ms,
            Some(1500.0),
            &opens,
            scene_invalid.clone(),
        ),
        Measurement::from_samples(
            "seek-to-frame",
            Unit::Ms,
            Some(1000.0),
            &seeks,
            scene_invalid,
        ),
        Measurement::new(
            "dropped-frames-per-min-1080p",
            Unit::Count,
            Some(1.0),
            Check::Median,
            summarise(&per_minute.into_iter().collect::<Vec<_>>()),
            long_invalid.clone(),
        ),
        Measurement::from_samples(
            "playback-displayed-fps",
            Unit::Fps,
            None,
            &displayed.into_iter().collect::<Vec<_>>(),
            long_invalid.clone(),
        ),
        Measurement::from_samples(
            "playback-main-thread-cpu",
            Unit::Percent,
            None,
            &cpu.into_iter().collect::<Vec<_>>(),
            long_invalid,
        ),
    ]
}

/// Frames mpv rendered that the UI never showed, over the long run (native build lines only; the
/// web build presents every frame mpv renders, so it has no such fields).
fn not_shown(long: &Value) -> f64 {
    match (
        number(long, "rendered_fps"),
        number(long, "displayed_fps"),
        number(long, "seconds"),
    ) {
        (Some(rendered), Some(displayed), Some(seconds)) => {
            ((rendered - displayed) * seconds).max(0.0).round()
        }
        _ => 0.0,
    }
}

// ---- Reports on disk and the machine -----------------------------------------------------

fn perf_dir() -> PathBuf {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    data.join("semantic-stash-viewer").join("perf")
}

/// The newest earlier report (names are timestamps, so they sort in time order).
/// The newest earlier report for the same app (reports from before 006 are the web build's).
fn newest_report(dir: &Path, app: &str) -> Option<Report> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    files
        .iter()
        .rev()
        .filter_map(|p| serde_json::from_str::<Report>(&std::fs::read_to_string(p).ok()?).ok())
        .find(|r| r.app == app)
}

/// OS, CPU, and GPU, from what the system reports.
fn machine() -> String {
    let os = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|s| {
            s.lines().find_map(|l| {
                l.strip_prefix("PRETTY_NAME=")
                    .map(|v| v.trim_matches('"').to_owned())
            })
        })
        .unwrap_or_else(|| std::env::consts::OS.to_owned());
    let cpu = std::fs::read_to_string("/proc/cpuinfo").ok().and_then(|s| {
        s.lines().find_map(|l| {
            l.strip_prefix("model name")
                .map(|v| v.trim_start_matches([' ', '\t', ':']).to_owned())
        })
    });
    let gpu = Command::new("lspci").output().ok().and_then(|o| {
        String::from_utf8_lossy(&o.stdout)
            .lines()
            .find(|l| l.contains("VGA") || l.contains("3D controller"))
            .and_then(|l| l.split_once(": ").map(|(_, name)| name.to_owned()))
    });
    [Some(os), cpu, gpu]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::not_shown;
    use serde_json::json;

    #[test]
    fn frames_rendered_but_never_shown_count_as_dropped() {
        // The native build's first video path: mpv rendered 30 fps, the UI showed 0.1 fps.
        let slideshow =
            json!({"long": "1", "seconds": 20.0, "rendered_fps": 29.9, "displayed_fps": 0.1});
        assert_eq!(not_shown(&slideshow), 596.0);
        let smooth =
            json!({"long": "1", "seconds": 20.0, "rendered_fps": 29.9, "displayed_fps": 29.9});
        assert_eq!(not_shown(&smooth), 0.0);
        // The web build's lines have no frame rates.
        assert_eq!(not_shown(&json!({"long": "1", "seconds": 20.0})), 0.0);
    }
}
