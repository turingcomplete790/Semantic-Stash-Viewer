//! Logging: a file in the app's log directory, plus stderr in debug builds.

use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// Keeps the non-blocking log writer alive for the life of the app.
pub struct LogGuard(#[allow(dead_code)] Option<WorkerGuard>);

/// Initialise the global subscriber. Honours `RUST_LOG`; defaults to `info`.
pub fn init(log_dir: &Path) -> LogGuard {
    let filter = || EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let (file_layer, guard) = match std::fs::create_dir_all(log_dir) {
        Ok(()) => {
            let appender = tracing_appender::rolling::daily(log_dir, "semantic-stash-viewer.log");
            let (writer, guard) = tracing_appender::non_blocking(appender);
            let layer = fmt::layer()
                .with_writer(writer)
                .with_ansi(false)
                .with_filter(filter());
            (Some(layer), Some(guard))
        }
        Err(e) => {
            eprintln!("could not create log directory {}: {e}", log_dir.display());
            (None, None)
        }
    };

    let stderr_layer = cfg!(debug_assertions).then(|| fmt::layer().with_filter(filter()));

    let _ = tracing_subscriber::registry()
        .with(file_layer)
        .with(stderr_layer)
        .try_init();
    LogGuard(guard)
}
