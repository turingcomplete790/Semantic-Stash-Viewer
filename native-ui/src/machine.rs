//! The conventions every level of the UI's state machine follows (007 research R1, R2).
//!
//! The UI is one hierarchical state machine held in iced's state: enums are exclusive states,
//! structs are regions active at the same time. Each level has an `update` that takes its own
//! message and returns a [`Step`]:
//!
//! - **No I/O in `update` or `view`.** Nothing reads disk or network, decodes data, or blocks.
//!   A transition that needs work returns [`Effect`]s (data); `effects.rs` runs them as tasks over
//!   the service layer, and their typed results come back as messages.
//! - **Unhandled events bubble up.** A state that can't handle something returns it as
//!   `Step::up`, and its parent decides (a Scenes screen asks to "open scene 123"; its tab opens a
//!   new screen).
//! - **Entry and exit.** A state that starts work when it becomes active does it in an `enter`
//!   function returning effects; one that must stop work does it in `exit`. Transitions call the
//!   old state's `exit` and the new state's `enter`.
//!
//! Because steps and effects are plain data, every transition is tested as a pure function:
//! state + message → new state + effects + event for the parent.

use crate::effects::Effect;

/// What one level's `update` produced: work to run, and possibly an event for its parent.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct Step<Up> {
    pub effects: Vec<Effect>,
    pub up: Option<Up>,
}

impl<Up> Default for Step<Up> {
    fn default() -> Self {
        Self {
            effects: Vec::new(),
            up: None,
        }
    }
}

impl<Up> Step<Up> {
    /// Nothing to do.
    pub fn none() -> Self {
        Self::default()
    }

    pub fn effect(effect: Effect) -> Self {
        Self {
            effects: vec![effect],
            up: None,
        }
    }

    pub fn effects(effects: Vec<Effect>) -> Self {
        Self { effects, up: None }
    }

    /// Hand an event to the parent.
    pub fn up(up: Up) -> Self {
        Self {
            effects: Vec::new(),
            up: Some(up),
        }
    }

    /// Add an effect.
    pub fn with(mut self, effect: Effect) -> Self {
        self.effects.push(effect);
        self
    }

    /// Add several effects.
    pub fn with_all(mut self, effects: impl IntoIterator<Item = Effect>) -> Self {
        self.effects.extend(effects);
        self
    }

    /// Lift a child's step into this level: keep its effects, translate its event.
    pub fn map_up<U>(self, f: impl FnOnce(Up) -> U) -> Step<U> {
        Step {
            effects: self.effects,
            up: self.up.map(f),
        }
    }
}
