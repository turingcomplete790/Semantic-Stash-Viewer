//! The request recorder: every request the adapter sends, by operation name (008 research R2).
//!
//! Tests count requests per user action with it, and the performance harness reports the live
//! `gql-*` rows from it (contracts/query-budget.md). Recording never changes what is sent.

use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What kind of request was sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
    GraphQl,
    Get,
}

/// One request: its operation name (for a GET, the kind of resource, never the URL), the bytes
/// sent and received, and how long the round trip took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestRecord {
    pub operation: String,
    pub kind: RequestKind,
    pub request_bytes: usize,
    pub response_bytes: usize,
    pub duration: Duration,
}

/// Receives a record for every request a `StashClient` sends.
pub trait Recorder: Send + Sync + std::fmt::Debug {
    fn record(&self, record: RequestRecord);
}

/// A recorder that keeps every record in memory, in order.
#[derive(Debug, Default)]
pub struct RequestLog {
    records: Mutex<Vec<RequestRecord>>,
}

impl RequestLog {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Every record so far.
    pub fn records(&self) -> Vec<RequestRecord> {
        self.records.lock().map(|r| r.clone()).unwrap_or_default()
    }

    /// Takes every record so far, leaving the log empty.
    pub fn take(&self) -> Vec<RequestRecord> {
        self.records
            .lock()
            .map(|mut r| std::mem::take(&mut *r))
            .unwrap_or_default()
    }

    /// `(operation, count)` in first-seen order.
    pub fn counts(&self) -> Vec<(String, usize)> {
        let mut counts: Vec<(String, usize)> = Vec::new();
        for record in self.records() {
            match counts
                .iter_mut()
                .find(|(name, _)| *name == record.operation)
            {
                Some((_, n)) => *n += 1,
                None => counts.push((record.operation, 1)),
            }
        }
        counts
    }
}

impl Recorder for RequestLog {
    fn record(&self, record: RequestRecord) {
        if let Ok(mut records) = self.records.lock() {
            records.push(record);
        }
    }
}

/// The operation name in a GraphQL document (`query Name(` / `mutation Name {`), for operations
/// built without an explicit name.
pub(crate) fn operation_name_in(query: &str) -> Option<&str> {
    let rest = query
        .trim_start()
        .strip_prefix("query")
        .or_else(|| query.trim_start().strip_prefix("mutation"))?;
    let name: &str = rest
        .trim_start()
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .next()?;
    (!name.is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_come_from_the_document() {
        assert_eq!(
            operation_name_in("query ScenesPage($filter: X) { a }"),
            Some("ScenesPage")
        );
        assert_eq!(
            operation_name_in("mutation SaveActivity { a }"),
            Some("SaveActivity")
        );
        assert_eq!(operation_name_in("{ a }"), None);
    }

    #[test]
    fn counts_keep_first_seen_order() {
        let log = RequestLog::new();
        for name in ["B", "A", "B"] {
            log.record(RequestRecord {
                operation: name.into(),
                kind: RequestKind::GraphQl,
                request_bytes: 1,
                response_bytes: 1,
                duration: Duration::ZERO,
            });
        }
        assert_eq!(log.counts(), vec![("B".into(), 2), ("A".into(), 1)]);
    }
}
