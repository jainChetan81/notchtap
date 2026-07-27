//! Outbound connectors (v3 spec). The `Notifier` seam is outbound-only:
//! the overlay is not an implementation, and a connector's outcome never
//! influences the http response. Fan-out happens at acceptance — after
//! `enqueue` returns Ok — via [`ConnectorHandle::offer`], which never
//! blocks (bounded channel, drop + warn when full).
//!
//! The telegram connector that originally motivated this module was
//! removed; this file now holds only the generic fan-out primitive that
//! future connectors (e.g. plan 128) build on.

use tokio::sync::mpsc;

use crate::event::Event;

/// One spawned connector: a name for logs and the sending half of its
/// bounded channel. Cheap to clone into the http state.
#[derive(Clone)]
pub struct ConnectorHandle {
    name: &'static str,
    tx: mpsc::Sender<Event>,
}

impl ConnectorHandle {
    // No connector is wired up in production right now (the telegram
    // connector that used to call this was removed; a future connector,
    // e.g. plan 128, is the next real caller) — only tests construct a
    // `ConnectorHandle` today, hence the explicit allow rather than
    // deleting a framework method mid-migration.
    #[allow(dead_code)]
    pub fn new(name: &'static str, tx: mpsc::Sender<Event>) -> Self {
        Self { name, tx }
    }

    /// Called at acceptance. Never blocks: on a full channel the event is
    /// dropped with a warning — bounded-and-non-blocking is the guarantee,
    /// not freshness (IMPLEMENTATION_PLAN.md §3).
    pub fn offer(&self, event: &Event) {
        match self.tx.try_send(event.clone()) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                tracing::warn!(
                    connector = self.name,
                    "channel full — outbound event dropped"
                );
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                tracing::warn!(
                    connector = self.name,
                    "worker gone — outbound event dropped"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::test_fixtures;
    use crate::event::EventType;

    fn event(event_type: EventType, title: &str, body: &str) -> Event {
        test_fixtures::with_body(
            test_fixtures::with_event_type(test_fixtures::event(title), event_type),
            body,
        )
    }

    // --- offer: drop-on-full, never blocks ---

    #[tokio::test]
    async fn offer_drops_when_channel_full_without_blocking() {
        let (tx, mut rx) = mpsc::channel(1);
        let handle = ConnectorHandle::new("test", tx);
        handle.offer(&event(EventType::Generic, "first", "kept"));
        handle.offer(&event(EventType::Generic, "second", "dropped"));

        let received = rx.try_recv().unwrap();
        assert_eq!(received.payload.title, "first");
        assert!(rx.try_recv().is_err()); // second was dropped, not queued
    }
}
