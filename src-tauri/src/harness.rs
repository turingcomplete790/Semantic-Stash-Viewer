//! Switches for the performance harness (003 US2, research R8). Debug builds only; in release
//! builds every switch reads as off.
//!
//! - `SSV_HARNESS_PROFILE=<display name>`: connect to that profile instead of the last-used one.
//! - `SSV_HARNESS_CLEAR_CACHE=1`: clear its cache first (cold start with a cleared cache).
//! - `SSV_HARNESS_EXIT=1`: exit once the run's measurements are printed: the playback run
//!   (`SSV_MEASURE`), else the UI bench (`SSV_DEBUG_BENCH`), else the cold-start mark.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use tauri::AppHandle;

/// A run that can't reach its first measurement in this time is reported invalid.
const INTERACTIVE_TIMEOUT: Duration = Duration::from_secs(30);

static START: OnceLock<Instant> = OnceLock::new();
static APP: OnceLock<AppHandle> = OnceLock::new();
static MARKED: AtomicBool = AtomicBool::new(false);

/// What finishes a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finish {
    Interactive,
    Bench,
    Measure,
}

fn var(name: &str) -> Option<String> {
    if !cfg!(debug_assertions) {
        return None;
    }
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// Record the process start. Call first thing in `run()`.
pub fn record_start() {
    START.get_or_init(Instant::now);
}

pub fn profile() -> Option<String> {
    var("SSV_HARNESS_PROFILE")
}

pub fn clear_cache() -> bool {
    var("SSV_HARNESS_CLEAR_CACHE").is_some()
}

fn exit_requested() -> bool {
    var("SSV_HARNESS_EXIT").is_some()
}

fn expected() -> Finish {
    if var("SSV_MEASURE").is_some() {
        Finish::Measure
    } else if var("SSV_DEBUG_BENCH").is_some() {
        Finish::Bench
    } else {
        Finish::Interactive
    }
}

/// Print one `MEASURE` line.
pub fn emit(value: &serde_json::Value) {
    println!("MEASURE {value}");
}

/// Keep the app handle for exiting, and stop runs that never become interactive.
pub fn install(app: &AppHandle) {
    let _ = APP.set(app.clone());
    if exit_requested() && expected() == Finish::Interactive {
        std::thread::spawn(|| {
            std::thread::sleep(INTERACTIVE_TIMEOUT);
            if !MARKED.load(Ordering::SeqCst) {
                emit(&serde_json::json!({"invalid": "not connected"}));
                finished(Finish::Interactive);
            }
        });
    }
}

/// The run reached `what`; exit if that's what this run was waiting for.
pub fn finished(what: Finish) {
    if !exit_requested() || what != expected() {
        return;
    }
    if let Some(app) = APP.get().cloned() {
        std::thread::spawn(move || {
            // Let the last lines reach the harness.
            std::thread::sleep(Duration::from_millis(300));
            app.exit(0);
        });
    }
}

/// Debug builds only: the UI calls this once Home first paints with server info (live or
/// cached). Prints `MEASURE {"coldStartMs": …}` the first time (research R8).
#[tauri::command]
#[specta::specta]
pub fn debug_mark_interactive(visible: bool) {
    if !cfg!(debug_assertions) || MARKED.swap(true, Ordering::SeqCst) {
        return;
    }
    let Some(start) = START.get() else {
        return;
    };
    let ms = (start.elapsed().as_secs_f64() * 1000.0).round();
    if visible {
        emit(&serde_json::json!({"coldStartMs": ms}));
    } else {
        emit(&serde_json::json!({"coldStartMs": ms, "invalid": "window hidden"}));
    }
    finished(Finish::Interactive);
}
