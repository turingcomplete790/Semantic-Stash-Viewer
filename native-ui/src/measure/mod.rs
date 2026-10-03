//! Measurement plumbing (006 T007, contracts/measurements.md): the web build's environment
//! variables and `MEASURE {json}` lines, so the performance harness reads both builds the same way.
//! Debug builds only; release builds ignore all of it.

pub mod playback;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();
static MARKED: AtomicBool = AtomicBool::new(false);

/// What a harness run waits for before exiting (`SSV_HARNESS_EXIT`).
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

/// Record the process start. Call first thing in `main`.
pub fn record_start() {
    START.get_or_init(Instant::now);
}

pub fn harness_profile() -> Option<String> {
    var("SSV_HARNESS_PROFILE")
}

pub fn clear_cache_requested() -> bool {
    var("SSV_HARNESS_CLEAR_CACHE").is_some()
}

fn exit_requested() -> bool {
    var("SSV_HARNESS_EXIT").is_some()
}

pub fn bench_requested() -> bool {
    var("SSV_DEBUG_BENCH").is_some()
}

/// Scene ids for the playback run (`SSV_MEASURE`).
pub fn measure_scenes() -> Option<Vec<String>> {
    var("SSV_MEASURE").map(|ids| {
        ids.split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect()
    })
}

/// The long playback (`SSV_MEASURE_LONG`, `SSV_MEASURE_LONG_SECS`, default 300 s as the web build).
pub fn measure_long() -> Option<(String, u64)> {
    let id = var("SSV_MEASURE_LONG")?;
    let secs = var("SSV_MEASURE_LONG_SECS")
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);
    Some((id, secs))
}

fn expected() -> Finish {
    if var("SSV_MEASURE").is_some() {
        Finish::Measure
    } else if bench_requested() {
        Finish::Bench
    } else {
        Finish::Interactive
    }
}

/// Print one `MEASURE` line.
pub fn emit(value: &serde_json::Value) {
    println!("MEASURE {value}");
}

/// The first interactive view was drawn: print `coldStartMs` once. Returns whether the app should
/// exit now (a harness cold-start run).
pub fn mark_interactive() -> bool {
    if !cfg!(debug_assertions) || MARKED.swap(true, Ordering::SeqCst) {
        return false;
    }
    let Some(start) = START.get() else {
        return false;
    };
    emit(&serde_json::json!({"coldStartMs": (start.elapsed().as_secs_f64() * 1000.0).round()}));
    should_exit(Finish::Interactive)
}

/// The run reached `what`: whether the app should exit (only when the harness asked for it and
/// this is what the run was waiting for).
pub fn should_exit(what: Finish) -> bool {
    exit_requested() && what == expected()
}

/// Report panics as `uiError` lines (the web build reports UI errors the same way).
pub fn install_panic_hook() {
    if !cfg!(debug_assertions) {
        return;
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        emit(&serde_json::json!({"uiError": info.to_string()}));
        previous(info);
    }));
}

/// Summary statistics in the web bench's shape: `n`, `median`, `p95`, `max` (rounded to `digits`
/// decimals). An empty set reports `null`s.
pub fn summary(samples: &[f64], digits: i32) -> serde_json::Value {
    let mut v: Vec<f64> = samples.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return serde_json::json!({"n": 0, "median": null, "p95": null, "max": null});
    }
    v.sort_by(f64::total_cmp);
    let scale = 10f64.powi(digits);
    let round = |x: f64| (x * scale).round() / scale;
    // Same index rule as the web bench: floor(q × n), clamped.
    let at = |q: f64| v[((q * v.len() as f64) as usize).min(v.len() - 1)];
    serde_json::json!({
        "n": v.len(),
        "median": round(at(0.5)),
        "p95": round(at(0.95)),
        "max": round(v[v.len() - 1]),
    })
}
