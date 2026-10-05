//! App shell state kept by the core (004): the notification centre, discardable UI state stored
//! under the viewer's local data directory (research R3). The native app saves its own tabs
//! (`native-ui/src/session/snapshot.rs`).

pub mod notifications;
pub mod store;

/// Most notifications kept (data-model "NotificationsFile").
pub const MAX_NOTIFICATIONS: usize = 200;
