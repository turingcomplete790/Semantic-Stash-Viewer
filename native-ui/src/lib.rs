//! Native UI spike (006): the viewer's hard screens drawn with iced instead of a webview. The
//! library holds everything; `main.rs` only starts it, so `tests/` can reach the logic.

pub mod app;
pub mod logging;
pub mod measure;
pub mod player;
pub mod services;
