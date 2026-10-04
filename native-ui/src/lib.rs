//! Semantic Stash Viewer's native UI (007): a hierarchical state machine in iced's state
//! (`machine`, `app`, `session`, `onboarding`), effects as data (`effects`), and the service layer
//! over the core (`services`). The library holds everything; `main.rs` only starts it, so
//! `tests/` can reach the logic.

pub mod app;
pub mod effects;
pub mod logging;
pub mod machine;
pub mod measure;
pub mod messages;
pub mod onboarding;
pub mod player;
pub mod services;
pub mod session;
pub mod shell;
pub mod widgets;
