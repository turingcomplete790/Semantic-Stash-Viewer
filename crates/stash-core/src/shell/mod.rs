//! App shell state kept by the core (004): tab sets and the notification centre.
//!
//! Both are discardable UI state stored under the viewer's local data directory (research R3).

pub mod store;

/// Most history entries kept per tab (data-model "Tab"). Older entries are dropped.
pub const MAX_HISTORY: usize = 50;
/// Most tabs per server profile (data-model "TabSet").
pub const MAX_TABS: usize = 100;
/// Largest serialised view state per history entry (data-model "HistoryEntry").
pub const MAX_VIEW_STATE_BYTES: usize = 16 * 1024;
/// Most notifications kept (data-model "NotificationsFile").
pub const MAX_NOTIFICATIONS: usize = 200;
