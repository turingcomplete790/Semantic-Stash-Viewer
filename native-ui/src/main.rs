//! Semantic Stash Viewer, drawn with iced (007). It reuses `stash-core` and `player` directly.

use std::sync::OnceLock;

use semantic_stash_viewer_native::{app, logging, measure, services};

/// The core's runtime (connection manager, cache refreshers, player tasks). iced runs its own
/// executor for UI tasks; both are tokio, so core futures run on either.
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn main() -> iced::Result {
    measure::record_start();
    measure::install_panic_hook();
    // The video path needs wgpu on Vulkan (research R1), which it prefers on Linux; `cargo run`
    // and the harness pin it with `WGPU_BACKEND=vulkan` (007 T045). If anything else is chosen,
    // the player screen says the video path is unavailable.

    let Some(paths) = services::Paths::resolve() else {
        eprintln!("can't find the home directory");
        std::process::exit(1);
    };
    let _log = logging::init(&paths.logs());

    let runtime = RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("ssv-core")
            .build()
            .expect("couldn't start the async runtime")
    });
    let services = match services::Services::open(paths, runtime.handle().clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("couldn't open the saved profiles: {e}");
            std::process::exit(1);
        }
    };
    app::run(services)
}
