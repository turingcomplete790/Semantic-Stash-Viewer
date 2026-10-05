//! Semantic Stash Viewer's native UI (007): a hierarchical state machine in iced's state
//! (`machine`, `app`, `session`, `onboarding`), effects as data (`effects`), and the service layer
//! over the core (`services`). The library holds everything; `main.rs` only starts it, so
//! `tests/` can reach the logic.

// The unsafe-code gate (constitution C4): every unsafe block documented, one unsafe operation per
// block, and unsafe functions' bodies checked like any other code.
#![deny(
    unsafe_op_in_unsafe_fn,
    clippy::undocumented_unsafe_blocks,
    clippy::multiple_unsafe_ops_per_block
)]

pub mod app;
pub mod effects;
pub mod logging;
pub mod machine;
pub mod measure;
pub mod messages;
pub mod onboarding;
pub mod player;
pub mod screens;
pub mod services;
pub mod session;
pub mod shell;
pub mod widgets;
