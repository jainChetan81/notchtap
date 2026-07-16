use std::collections::VecDeque;
use std::time::Instant;

use crate::error::QueueError;
use crate::event::Event;

pub struct QueueItem {
    pub event: Event,
    // fifo tie-break ordering only, never used for ttl (spec §2). ordering
    // is carried structurally by the VecDeque, so this field exists as the
    // recorded fact rather than being read on any current code path.
    #[allow(dead_code)]
    pub enqueued_at: Instant,
    pub promoted_at: Option<Instant>,
}

pub struct NotificationQueue {
    visible: VecDeque<QueueItem>,
    waiting: VecDeque<QueueItem>,
    max_concurrent: usize,
    max_queued: usize,
    paused: bool,
    promoted: Vec<Event>,
}

impl NotificationQueue {
    pub fn new(max_concurrent: usize, max_queued: usize) -> Self {
        Self {
            visible: VecDeque::new(),
            waiting: VecDeque::new(),
            max_concurrent,
            max_queued,
            paused: false,
            promoted: Vec::new(),
        }
    }

    pub fn enqueue(&mut self, event: Event) -> Result<(), QueueError> {
        // fifo: the fast-path may only promote when nothing is already
        // waiting, otherwise a new push would jump items queued before it
        // (a visible slot can be free while items wait, in the window
        // between a ttl expiry and the next heartbeat tick)
        let can_promote_now =
            !self.paused && self.visible.len() < self.max_concurrent && self.waiting.is_empty();
        if !can_promote_now && self.waiting.len() >= self.max_queued {
            return Err(QueueError::QueueFull);
        }

        let item = QueueItem {
            event,
            enqueued_at: Instant::now(),
            promoted_at: None,
        };

        if can_promote_now {
            self.promote_item(item, Instant::now());
        } else {
            self.waiting.push_back(item);
        }

        Ok(())
    }

    pub fn expire_and_promote(&mut self, now: Instant) {
        self.expire_visible(now);
        if !self.paused {
            while self.visible.len() < self.max_concurrent && !self.waiting.is_empty() {
                let item = self.waiting.pop_front().expect("waiting is not empty");
                self.promote_item(item, now);
            }
        }
    }

    fn promote_item(&mut self, mut item: QueueItem, now: Instant) {
        item.promoted_at = Some(now);
        self.promoted.push(item.event.clone());
        self.visible.push_back(item);
    }

    fn expire_visible(&mut self, now: Instant) {
        self.visible.retain(|item| {
            let promoted_at = item
                .promoted_at
                .expect("visible items must have promoted_at");
            let elapsed_secs = now.duration_since(promoted_at).as_secs();
            elapsed_secs < item.event.ttl_secs
        });
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    // spec §4 public api; no runtime caller yet (the frontend learns about
    // visibility via events, not queries) — exercised by the test suite
    #[allow(dead_code)]
    pub fn visible(&self) -> &VecDeque<QueueItem> {
        &self.visible
    }

    pub fn waiting(&self) -> &VecDeque<QueueItem> {
        &self.waiting
    }

    pub fn take_promoted(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.promoted)
    }

    /// test-only: run expiry without the promotion half of the tick, to
    /// reproduce the freed-slot-between-ticks window
    #[cfg(test)]
    pub(crate) fn expire_visible_for_test(&mut self, now: Instant) {
        self.expire_visible(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::QueueError;
    use crate::event::{EventPayload, EventType, Priority};
    use std::time::Duration;
    use uuid::Uuid;

    fn event(title: &str, ttl_secs: u64) -> Event {
        Event {
            id: Uuid::new_v4(),
            event_type: EventType::Generic,
            priority: Priority::Normal,
            ttl_secs,
            payload: EventPayload {
                title: title.to_string(),
                body: "body".to_string(),
            },
        }
    }

    fn titles(items: &VecDeque<QueueItem>) -> Vec<&str> {
        items
            .iter()
            .map(|i| i.event.payload.title.as_str())
            .collect()
    }

    #[test]
    fn enqueue_one_is_visible_immediately() {
        let mut q = NotificationQueue::new(3, 50);
        q.enqueue(event("a", 8)).unwrap();
        assert_eq!(titles(q.visible()), vec!["a"]);
        assert!(q.waiting().is_empty());
    }

    #[test]
    fn fourth_item_waits_when_cap_is_three() {
        let mut q = NotificationQueue::new(3, 50);
        for t in ["a", "b", "c", "d"] {
            q.enqueue(event(t, 8)).unwrap();
        }
        assert_eq!(titles(q.visible()), vec!["a", "b", "c"]);
        assert_eq!(titles(q.waiting()), vec!["d"]);
    }

    #[test]
    fn expired_item_is_removed_and_next_waiting_promoted() {
        let mut q = NotificationQueue::new(1, 50);
        q.enqueue(event("a", 1)).unwrap();
        q.enqueue(event("b", 8)).unwrap();
        assert_eq!(titles(q.visible()), vec!["a"]);

        let later = Instant::now() + Duration::from_secs(2);
        q.expire_and_promote(later);
        assert_eq!(titles(q.visible()), vec!["b"]);
        assert!(q.waiting().is_empty());
    }

    #[test]
    fn empty_queue_tick_is_a_noop() {
        let mut q = NotificationQueue::new(3, 50);
        q.expire_and_promote(Instant::now());
        assert!(q.visible().is_empty());
        assert!(q.waiting().is_empty());
    }

    #[test]
    fn identical_ttls_expire_in_enqueue_order() {
        let mut q = NotificationQueue::new(2, 50);
        q.enqueue(event("a", 1)).unwrap();
        q.enqueue(event("b", 1)).unwrap();
        q.enqueue(event("c", 8)).unwrap();
        q.enqueue(event("d", 8)).unwrap();

        let later = Instant::now() + Duration::from_secs(2);
        q.expire_and_promote(later);
        // a and b (same ttl) both expire; c then d promote in fifo order
        assert_eq!(titles(q.visible()), vec!["c", "d"]);
    }

    #[test]
    fn fifty_first_waiting_item_is_rejected_and_state_unchanged() {
        let mut q = NotificationQueue::new(3, 50);
        for i in 0..53 {
            q.enqueue(event(&format!("n{i}"), 8)).unwrap();
        }
        assert_eq!(q.visible().len(), 3);
        assert_eq!(q.waiting().len(), 50);

        let err = q.enqueue(event("overflow", 8)).unwrap_err();
        assert!(matches!(err, QueueError::QueueFull));
        assert_eq!(q.visible().len(), 3);
        assert_eq!(q.waiting().len(), 50);
    }

    #[test]
    fn fast_path_never_jumps_waiting_items() {
        // a slot can free up between heartbeat ticks; a new enqueue in that
        // window must go behind already-waiting items, not ahead of them
        let mut q = NotificationQueue::new(1, 50);
        q.enqueue(event("a", 1)).unwrap();
        q.enqueue(event("b", 8)).unwrap(); // waits behind a
        let later = Instant::now() + Duration::from_secs(2);
        q.expire_visible_for_test(later); // slot free, b still waiting
        q.enqueue(event("c", 8)).unwrap();
        assert!(q.visible().is_empty());
        assert_eq!(titles(q.waiting()), vec!["b", "c"]);

        q.expire_and_promote(later);
        assert_eq!(titles(q.visible()), vec!["b"]);
    }

    #[test]
    fn pause_sends_enqueues_to_waiting_even_with_free_slots() {
        let mut q = NotificationQueue::new(3, 50);
        q.pause();
        q.enqueue(event("a", 8)).unwrap();
        assert!(q.visible().is_empty());
        assert_eq!(titles(q.waiting()), vec!["a"]);
    }

    #[test]
    fn pause_gates_promotion_but_not_expiry() {
        let mut q = NotificationQueue::new(1, 50);
        q.enqueue(event("a", 1)).unwrap();
        q.enqueue(event("b", 8)).unwrap();
        q.pause();

        let later = Instant::now() + Duration::from_secs(2);
        q.expire_and_promote(later);
        // a aged out even while paused; b was NOT promoted
        assert!(q.visible().is_empty());
        assert_eq!(titles(q.waiting()), vec!["b"]);
    }

    #[test]
    fn resume_then_tick_promotes_immediately() {
        let mut q = NotificationQueue::new(3, 50);
        q.pause();
        q.enqueue(event("a", 8)).unwrap();
        q.resume();
        q.expire_and_promote(Instant::now());
        assert_eq!(titles(q.visible()), vec!["a"]);
        assert!(q.waiting().is_empty());
    }

    #[test]
    fn queue_full_is_enforced_identically_while_paused() {
        let mut q = NotificationQueue::new(3, 2);
        q.pause();
        q.enqueue(event("a", 8)).unwrap();
        q.enqueue(event("b", 8)).unwrap();
        let err = q.enqueue(event("c", 8)).unwrap_err();
        assert!(matches!(err, QueueError::QueueFull));
    }

    #[test]
    fn exactly_one_promotion_event_per_item() {
        let mut q = NotificationQueue::new(1, 50);
        q.enqueue(event("a", 1)).unwrap(); // promoted via fast path
        q.enqueue(event("b", 8)).unwrap(); // waits
        assert_eq!(q.take_promoted().len(), 1);
        assert!(q.take_promoted().is_empty()); // drained, not re-reported

        let later = Instant::now() + Duration::from_secs(2);
        q.expire_and_promote(later); // b promoted via heartbeat path
        let promoted = q.take_promoted();
        assert_eq!(promoted.len(), 1);
        assert_eq!(promoted[0].payload.title, "b");
        assert!(q.take_promoted().is_empty());
    }
}
