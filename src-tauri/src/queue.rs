use std::collections::VecDeque;
use std::time::Instant;

use crate::error::QueueError;
use crate::event::{Event, Priority, RotationSpec, SlotState, SourceKind};

pub struct QueueItem {
    pub event: Event,
    pub enqueued_at: Instant,
    pub promoted_at: Option<Instant>,
    pub extension_secs: u64,
}

/// A single-slot, priority-ordered notification queue with bounded
/// per-tier waiting, ttl/recur rotation, pause/resume gating, and
/// supersession by topic. Replaces the v1/v2/v3.5 three-item stack.
///
/// The full lifecycle:
///
/// ```
/// use std::time::{Duration, Instant};
/// use notchtap_lib::event::{Event, EventMeta, EventPayload, EventSignal, EventType, Priority, RotationSpec, SourceKind};
/// use notchtap_lib::queue::SingleSlotQueue;
///
/// fn event(title: &str, priority: Priority, ttl_secs: u64) -> Event {
///     Event {
///         id: uuid::Uuid::new_v4(),
///         event_type: EventType::Generic,
///         priority,
///         rotation: RotationSpec::OneShot { ttl_secs },
///         topic: None,
///         payload: EventPayload { title: title.into(), body: "body".into() },
///         meta: EventMeta::default(),
///         signal: EventSignal::Generic,
///         origin: SourceKind::Manual,
///     }
/// }
///
/// let mut queue = SingleSlotQueue::new(50);
/// queue.enqueue(event("a", Priority::Medium, 1)).unwrap();
/// queue.enqueue(event("b", Priority::Medium, 8)).unwrap();
/// assert!(queue.current_slot_state() != notchtap_lib::event::SlotState::Empty);
///
/// // once the ttl elapses, the next tick rotates and promotes the waiting item
/// queue.tick(Instant::now() + Duration::from_secs(2));
/// assert!(queue.slot_state_if_changed().is_some());
/// ```
pub struct SingleSlotQueue {
    visible: Option<QueueItem>,
    waiting: [VecDeque<QueueItem>; 3],
    max_queued_per_tier: usize,
    paused: bool,
    expanded: bool,
    last_emitted: Option<SlotState>,
    /// Same-tier promotion tie-break, checked before arrival order — see
    /// `pop_highest_priority_waiting`. Empty by default: every origin ties,
    /// so promotion degenerates to plain arrival-order FIFO (today's
    /// behavior). Set via `with_rotation_order`.
    rotation_order: Vec<SourceKind>,
}

impl SingleSlotQueue {
    pub fn new(max_queued_per_tier: usize) -> Self {
        Self {
            visible: None,
            waiting: [VecDeque::new(), VecDeque::new(), VecDeque::new()],
            max_queued_per_tier,
            paused: false,
            expanded: false,
            last_emitted: None,
            rotation_order: Vec::new(),
        }
    }

    /// Builder: sets the same-tier tie-break order. `Config.rotation_order`
    /// (validated as a permutation of all four `SourceKind` variants) is
    /// the only production caller; tests are free to leave this unset.
    pub fn with_rotation_order(mut self, rotation_order: Vec<SourceKind>) -> Self {
        self.rotation_order = rotation_order;
        self
    }

    // ------------------------------------------------------------------
    // enqueue / supersession
    // ------------------------------------------------------------------

    pub fn enqueue(&mut self, event: Event) -> Result<(), QueueError> {
        self.enqueue_with_options(event, Instant::now(), false)
    }

    /// Test-enqueue variant: promotes into the visible slot even when the
    /// engine is paused, provided the slot is empty and no one is waiting.
    /// Real `/notify` pushes must never bypass pause, so the public `enqueue`
    /// path stays unchanged. Used by `send_test_notification`.
    pub fn enqueue_test(&mut self, event: Event) -> Result<(), QueueError> {
        self.enqueue_with_options(event, Instant::now(), true)
    }

    /// Deterministic, simulated-clock entry point used only by tests.
    #[cfg(test)]
    fn enqueue_at(&mut self, event: Event, now: Instant) -> Result<(), QueueError> {
        self.enqueue_with_options(event, now, false)
    }

    // `now`-parameterized core so tests can drive the supersede/top-up path
    // deterministically without real sleeps — `top_up_visible_remaining_time`
    // needs a consistent notion of "now" alongside `promoted_at`, the same
    // way `tick()` already does. `enqueue` is the real entry point (always
    // real wall-clock time in production); this is the shared internal path.
    fn enqueue_with_options(
        &mut self,
        event: Event,
        now: Instant,
        bypass_pause_when_slot_empty: bool,
    ) -> Result<(), QueueError> {
        if let Some(topic) = event.topic.clone() {
            if self.supersede_if_topic_matches(&topic, &event, now) {
                return Ok(());
            }
        }
        self.enqueue_new(event, now, bypass_pause_when_slot_empty)
    }

    fn enqueue_new(
        &mut self,
        event: Event,
        now: Instant,
        bypass_pause_when_slot_empty: bool,
    ) -> Result<(), QueueError> {
        let tier = event.priority as usize;
        let can_promote_now = self.visible.is_none()
            && (bypass_pause_when_slot_empty || !self.paused)
            && self.all_tiers_empty();
        if !can_promote_now && self.waiting[tier].len() >= self.max_queued_per_tier {
            return Err(QueueError::QueueFull);
        }
        let mut item = QueueItem {
            event,
            enqueued_at: now,
            promoted_at: None,
            extension_secs: 0,
        };
        if can_promote_now {
            item.promoted_at = Some(now);
            self.set_expanded_for_promotion(item.event.priority);
            self.visible = Some(item);
        } else {
            self.waiting[tier].push_back(item);
        }
        Ok(())
    }

    fn supersede_if_topic_matches(&mut self, topic: &str, fresh: &Event, now: Instant) -> bool {
        if let Some(visible) = &mut self.visible {
            if visible.event.topic.as_deref() == Some(topic) {
                apply_fresh_content(&mut visible.event, fresh);
                self.top_up_visible_remaining_time(now);
                return true;
            }
        }
        for tier_idx in 0..3 {
            let Some(pos) = self.waiting[tier_idx]
                .iter()
                .position(|i| i.event.topic.as_deref() == Some(topic))
            else {
                continue;
            };
            let new_tier_idx = fresh.priority as usize;
            if new_tier_idx == tier_idx {
                apply_fresh_content(&mut self.waiting[tier_idx][pos].event, fresh);
            } else {
                let mut existing = self.waiting[tier_idx]
                    .remove(pos)
                    .expect("position just found");
                apply_fresh_content(&mut existing.event, fresh);
                self.waiting[new_tier_idx].push_back(existing);
            }
            return true;
        }
        false
    }

    // ------------------------------------------------------------------
    // tick — rotation then promotion
    // ------------------------------------------------------------------

    pub fn tick(&mut self, now: Instant) {
        self.rotate_out_if_elapsed(now);
        self.promote_next(now);
    }

    fn rotate_out_if_elapsed(&mut self, now: Instant) {
        let Some(item) = &self.visible else { return };
        let promoted_at = item.promoted_at.expect("visible items have promoted_at");
        let window = item.event.rotation_window(self.expanded) + item.extension_secs;
        if now.duration_since(promoted_at).as_secs() < window {
            return;
        }
        let item = self.visible.take().expect("checked Some above");
        if let RotationSpec::Recurring { .. } = item.event.rotation {
            let tier = item.event.priority as usize;
            self.waiting[tier].push_back(item);
        }
    }

    fn promote_next(&mut self, now: Instant) {
        if self.visible.is_some() || self.paused {
            return;
        }
        if let Some(mut item) = self.pop_highest_priority_waiting() {
            item.promoted_at = Some(now);
            item.extension_secs = 0;
            self.set_expanded_for_promotion(item.event.priority);
            self.visible = Some(item);
        }
    }

    // Expanded is per-turn (CONTEXT.md "Expanded"): automatic for High,
    // cleared for everything else — a leftover manual expand must not leak
    // onto the next item. Called from both promotion sites (promote_next and
    // enqueue_new's immediate-promote fast path) so neither can drift from
    // the other.
    fn set_expanded_for_promotion(&mut self, priority: Priority) {
        self.expanded = priority == Priority::High;
    }

    fn pop_highest_priority_waiting(&mut self) -> Option<QueueItem> {
        for tier in (0..3).rev() {
            if self.waiting[tier].is_empty() {
                continue;
            }
            let index = self.best_index_in_tier(tier);
            return self.waiting[tier].remove(index);
        }
        None
    }

    /// Within one tier: the item whose `origin` sorts earliest in
    /// `rotation_order` wins; unlisted origins (or an empty/unset order)
    /// rank last and tie with each other, so ties fall back to position —
    /// i.e. arrival order, identical to the pre-v6 behavior. `position()`
    /// finds the first (lowest-index / earliest-arrived) match among equal
    /// ranks, which is what makes that fallback exact.
    fn best_index_in_tier(&self, tier: usize) -> usize {
        let rank = |item: &QueueItem| {
            self.rotation_order
                .iter()
                .position(|origin| *origin == item.event.origin)
                .unwrap_or(self.rotation_order.len())
        };
        let mut best = 0;
        let mut best_rank = rank(&self.waiting[tier][0]);
        for (i, item) in self.waiting[tier].iter().enumerate().skip(1) {
            let r = rank(item);
            if r < best_rank {
                best = i;
                best_rank = r;
            }
        }
        best
    }

    fn all_tiers_empty(&self) -> bool {
        self.waiting.iter().all(|t| t.is_empty())
    }

    // ------------------------------------------------------------------
    // supersede time top-up
    // ------------------------------------------------------------------

    fn top_up_visible_remaining_time(&mut self, now: Instant) {
        let Some(item) = &mut self.visible else {
            return;
        };
        let Some(promoted_at) = item.promoted_at else {
            return;
        };
        let base_window = item.event.rotation_window(self.expanded);
        let effective_window = base_window + item.extension_secs;
        let elapsed = now.saturating_duration_since(promoted_at).as_secs();
        let remaining = effective_window.saturating_sub(elapsed);
        if remaining < MIN_REMAINING_ON_SUPERSEDE_SECS {
            let deficit = MIN_REMAINING_ON_SUPERSEDE_SECS - remaining;
            let room = MAX_EXTENSION_ON_SUPERSEDE_SECS.saturating_sub(item.extension_secs);
            item.extension_secs += deficit.min(room);
        }
    }

    // ------------------------------------------------------------------
    // pause / resume / expand / inspection
    // ------------------------------------------------------------------

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Manually dismiss the current visible item, if any, and promote the
    /// next waiting item immediately — mirrors what `tick` does on natural
    /// rotation-out, but caller-triggered rather than TTL-triggered. Unlike
    /// a natural rotation-out, a dismissed Recurring item is dropped, not
    /// requeued: "get rid of this" means gone, not "back after a lap
    /// through the other tiers."
    pub fn dismiss_visible(&mut self, now: Instant) {
        self.visible = None;
        self.promote_next(now);
    }

    pub fn current_priority(&self) -> Option<Priority> {
        self.visible.as_ref().map(|i| i.event.priority)
    }

    pub fn current_link(&self) -> Option<&str> {
        self.visible
            .as_ref()
            .and_then(|item| item.event.meta.link.as_deref())
    }

    pub fn toggle_expanded(&mut self) {
        if self.visible.is_none() {
            return;
        }
        self.expanded = !self.expanded;
    }

    pub fn total_waiting(&self) -> usize {
        self.waiting.iter().map(|t| t.len()).sum()
    }

    // ------------------------------------------------------------------
    // slot-state emission helpers
    // ------------------------------------------------------------------

    pub fn slot_state_if_changed(&mut self) -> Option<SlotState> {
        let current = self.current_slot_state();
        if self.last_emitted.as_ref() == Some(&current) {
            None
        } else {
            self.last_emitted = Some(current.clone());
            Some(current)
        }
    }

    pub fn current_slot_state(&self) -> SlotState {
        match &self.visible {
            None => SlotState::Empty,
            Some(item) => SlotState::Showing {
                id: item.event.id,
                title: item.event.payload.title.clone(),
                body: item.event.payload.body.clone(),
                event_type: item.event.event_type.clone(),
                priority: item.event.priority,
                signal: item.event.signal,
                expanded: self.expanded,
                source: item.event.meta.source.clone(),
                category: item.event.meta.category.clone(),
                published_at_ms: item.event.meta.published_at_ms,
                link: item.event.meta.link.clone(),
            },
        }
    }
}

// The one place a superseding event's content lands on an existing item —
// used at all three supersede sites (visible, same-tier waiting, cross-tier
// waiting) so a new `Event` field only needs to be added here once. `signal`
// was previously missing from all three call sites until Stage A's
// EventSignal work added it back to each copy independently; a shared
// function makes that class of gap structurally impossible to reintroduce.
fn apply_fresh_content(existing: &mut Event, fresh: &Event) {
    existing.payload = fresh.payload.clone();
    existing.priority = fresh.priority;
    existing.rotation = fresh.rotation;
    existing.signal = fresh.signal;
}

const MIN_REMAINING_ON_SUPERSEDE_SECS: u64 = 2;
const MAX_EXTENSION_ON_SUPERSEDE_SECS: u64 = 6;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::QueueError;
    use crate::event::{EventMeta, EventPayload, EventSignal, EventType};
    use std::time::Duration;
    use uuid::Uuid;

    fn event(title: &str, priority: Priority, ttl_secs: u64) -> Event {
        Event {
            id: Uuid::new_v4(),
            event_type: EventType::Generic,
            priority,
            rotation: RotationSpec::OneShot { ttl_secs },
            topic: None,
            payload: EventPayload {
                title: title.to_string(),
                body: "body".to_string(),
            },
            meta: EventMeta::default(),
            signal: EventSignal::Generic,
            origin: SourceKind::Manual,
        }
    }

    fn recurring_event(title: &str, priority: Priority, display_secs: u64) -> Event {
        Event {
            id: Uuid::new_v4(),
            event_type: EventType::Generic,
            priority,
            rotation: RotationSpec::Recurring { display_secs },
            topic: None,
            payload: EventPayload {
                title: title.to_string(),
                body: "body".to_string(),
            },
            meta: EventMeta::default(),
            signal: EventSignal::Generic,
            origin: SourceKind::Manual,
        }
    }

    fn topic_event(title: &str, priority: Priority, ttl_secs: u64, topic: &str) -> Event {
        Event {
            id: Uuid::new_v4(),
            event_type: EventType::Generic,
            priority,
            rotation: RotationSpec::OneShot { ttl_secs },
            topic: Some(topic.to_string()),
            payload: EventPayload {
                title: title.to_string(),
                body: "body".to_string(),
            },
            meta: EventMeta::default(),
            signal: EventSignal::Generic,
            origin: SourceKind::Manual,
        }
    }

    /// Same as `event()` but from a specific origin — for tie-break tests
    /// only; every other test relies on every helper sharing one origin so
    /// rank-based selection degenerates to pre-v6 arrival-order FIFO.
    fn event_from(title: &str, priority: Priority, ttl_secs: u64, origin: SourceKind) -> Event {
        Event {
            origin,
            ..event(title, priority, ttl_secs)
        }
    }

    fn visible_title(q: &SingleSlotQueue) -> Option<&str> {
        q.visible.as_ref().map(|i| i.event.payload.title.as_str())
    }

    fn waiting_titles(q: &SingleSlotQueue, tier: usize) -> Vec<&str> {
        q.waiting[tier]
            .iter()
            .map(|i| i.event.payload.title.as_str())
            .collect()
    }

    // ------------------------------------------------------------------
    // basic tick behavior
    // ------------------------------------------------------------------

    #[test]
    fn enqueue_one_is_visible_immediately() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 8)).unwrap();
        assert_eq!(visible_title(&q), Some("a"));
        assert_eq!(q.total_waiting(), 0);
    }

    #[test]
    fn current_link_returns_visible_event_link() {
        let mut q = SingleSlotQueue::new(50);
        let mut story = event("story", Priority::Low, 8);
        story.meta.link = Some("https://example.com/story".to_string());

        q.enqueue(story).unwrap();

        assert_eq!(q.current_link(), Some("https://example.com/story"));
    }

    #[test]
    fn current_link_returns_none_without_link_or_visible_event() {
        let mut q = SingleSlotQueue::new(50);
        assert_eq!(q.current_link(), None);

        q.enqueue(event("status", Priority::Medium, 8)).unwrap();

        assert_eq!(q.current_link(), None);
    }

    #[test]
    fn second_item_waits_when_slot_is_occupied() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 8)).unwrap();
        q.enqueue(event("b", Priority::Medium, 8)).unwrap();
        assert_eq!(visible_title(&q), Some("a"));
        assert_eq!(waiting_titles(&q, Priority::Medium as usize), vec!["b"]);
    }

    #[test]
    fn expired_item_is_removed_and_next_waiting_promoted() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 1)).unwrap();
        q.enqueue(event("b", Priority::Medium, 8)).unwrap();
        assert_eq!(visible_title(&q), Some("a"));

        let later = Instant::now() + Duration::from_secs(2);
        q.tick(later);
        assert_eq!(visible_title(&q), Some("b"));
        assert!(q.waiting.iter().all(|t| t.is_empty()));
    }

    #[test]
    fn empty_queue_tick_is_a_noop() {
        let mut q = SingleSlotQueue::new(50);
        q.tick(Instant::now());
        assert!(q.visible.is_none());
        assert_eq!(q.total_waiting(), 0);
    }

    #[test]
    fn high_priority_waiting_promotes_before_medium_and_low() {
        // paused so all three land in their own waiting tier instead of the
        // first one fast-path-promoting regardless of priority (fast path
        // only checks "is anything waiting", not tier order).
        let mut q = SingleSlotQueue::new(50);
        q.pause();
        q.enqueue(event("low", Priority::Low, 8)).unwrap();
        q.enqueue(event("medium", Priority::Medium, 8)).unwrap();
        q.enqueue(event("high", Priority::High, 8)).unwrap();
        q.resume();
        q.tick(Instant::now());
        assert_eq!(visible_title(&q), Some("high"));
        assert_eq!(
            waiting_titles(&q, Priority::Medium as usize),
            vec!["medium"]
        );
        assert_eq!(waiting_titles(&q, Priority::Low as usize), vec!["low"]);
    }

    #[test]
    fn fifo_within_tier() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("first", Priority::Medium, 1)).unwrap();
        q.enqueue(event("second", Priority::Medium, 8)).unwrap();
        q.enqueue(event("third", Priority::Medium, 8)).unwrap();
        q.tick(Instant::now() + Duration::from_secs(2));
        assert_eq!(visible_title(&q), Some("second"));
        assert_eq!(waiting_titles(&q, Priority::Medium as usize), vec!["third"]);
    }

    #[test]
    fn rotation_order_breaks_same_tier_ties_ahead_of_arrival_order() {
        // arrival order is news, manual, football — rotation_order says
        // football wins regardless.
        let mut q = SingleSlotQueue::new(50).with_rotation_order(vec![
            SourceKind::Football,
            SourceKind::Manual,
            SourceKind::News,
        ]);
        q.pause();
        q.enqueue(event_from("news", Priority::Medium, 8, SourceKind::News))
            .unwrap();
        q.enqueue(event_from(
            "manual",
            Priority::Medium,
            8,
            SourceKind::Manual,
        ))
        .unwrap();
        q.enqueue(event_from(
            "football",
            Priority::Medium,
            8,
            SourceKind::Football,
        ))
        .unwrap();
        q.resume();
        q.tick(Instant::now());
        assert_eq!(visible_title(&q), Some("football"));
        assert_eq!(
            waiting_titles(&q, Priority::Medium as usize),
            vec!["news", "manual"]
        );
    }

    #[test]
    fn unset_rotation_order_falls_back_to_arrival_order_across_origins() {
        // default (empty) rotation_order: every origin ties, so mixed-origin
        // same-tier items still promote in plain arrival order.
        let mut q = SingleSlotQueue::new(50);
        q.pause();
        q.enqueue(event_from("news", Priority::Medium, 8, SourceKind::News))
            .unwrap();
        q.enqueue(event_from(
            "football",
            Priority::Medium,
            8,
            SourceKind::Football,
        ))
        .unwrap();
        q.resume();
        q.tick(Instant::now());
        assert_eq!(visible_title(&q), Some("news"));
    }

    #[test]
    fn rotation_order_only_breaks_ties_within_a_tier_not_across_tiers() {
        // football is favored by rotation_order, but a waiting High item
        // from a lower-ranked origin still promotes first — Priority beats
        // rotation_order, exactly as documented.
        let mut q = SingleSlotQueue::new(50).with_rotation_order(vec![
            SourceKind::Football,
            SourceKind::Manual,
            SourceKind::News,
        ]);
        q.pause();
        q.enqueue(event_from("news-high", Priority::High, 8, SourceKind::News))
            .unwrap();
        q.enqueue(event_from(
            "football-medium",
            Priority::Medium,
            8,
            SourceKind::Football,
        ))
        .unwrap();
        q.resume();
        q.tick(Instant::now());
        assert_eq!(visible_title(&q), Some("news-high"));
    }

    #[test]
    fn high_enqueue_does_not_interrupt_currently_visible() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 4)).unwrap();
        q.enqueue(event("b", Priority::High, 2)).unwrap();
        assert_eq!(visible_title(&q), Some("a"));
        // tick before a's window elapses keeps a visible
        q.tick(Instant::now() + Duration::from_secs(2));
        assert_eq!(visible_title(&q), Some("a"));
    }

    #[test]
    fn oneshot_drops_forever() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 1)).unwrap();
        q.tick(Instant::now() + Duration::from_secs(2));
        assert!(q.visible.is_none());
        assert_eq!(q.total_waiting(), 0);
    }

    #[test]
    fn recurring_requeues_to_back_of_own_tier() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(recurring_event("a", Priority::Medium, 1))
            .unwrap();
        q.enqueue(event("b", Priority::Medium, 8)).unwrap();
        q.tick(Instant::now() + Duration::from_secs(2));
        assert_eq!(visible_title(&q), Some("b"));
        assert_eq!(waiting_titles(&q, Priority::Medium as usize), vec!["a"]);
    }

    #[test]
    fn recurring_requeues_not_to_front_or_different_tier() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(recurring_event("recur", Priority::Low, 1))
            .unwrap();
        q.enqueue(event("low2", Priority::Low, 8)).unwrap();
        q.enqueue(event("high", Priority::High, 8)).unwrap();
        q.tick(Instant::now() + Duration::from_secs(2));
        // high promotes first; recur requeued behind low2 in the Low tier
        assert_eq!(visible_title(&q), Some("high"));
        assert_eq!(
            waiting_titles(&q, Priority::Low as usize),
            vec!["low2", "recur"]
        );
    }

    // ------------------------------------------------------------------
    // fast path
    // ------------------------------------------------------------------

    #[test]
    fn fast_path_never_jumps_waiting_items() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 1)).unwrap();
        q.enqueue(event("b", Priority::Low, 8)).unwrap();
        // a expired but tick hasn't run; slot is free and Low tier has b waiting
        let later = Instant::now() + Duration::from_secs(2);
        // manually age out a without tick's promotion half
        q.rotate_out_if_elapsed(later);
        assert!(q.visible.is_none());
        // a new high push must not jump b
        q.enqueue(event("c", Priority::High, 8)).unwrap();
        assert!(q.visible.is_none());
        assert_eq!(waiting_titles(&q, Priority::Low as usize), vec!["b"]);
        assert_eq!(waiting_titles(&q, Priority::High as usize), vec!["c"]);

        q.tick(later);
        assert_eq!(visible_title(&q), Some("c"));
    }

    // ------------------------------------------------------------------
    // supersession
    // ------------------------------------------------------------------

    #[test]
    fn visible_supersede_updates_content_priority_rotation() {
        // fully simulated clock via enqueue_at — no real sleeps, no
        // dependence on the real Instant::now() the pub enqueue() wrapper
        // would otherwise use internally for the top-up calculation.
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        let base = topic_event("old", Priority::Medium, 8, "topic");
        let fresh = Event {
            payload: EventPayload {
                title: "new".to_string(),
                body: "fresh body".to_string(),
            },
            meta: EventMeta::default(),
            priority: Priority::High,
            rotation: RotationSpec::Recurring { display_secs: 4 },
            signal: EventSignal::Goal,
            ..base.clone()
        };
        q.enqueue_at(base, t0).unwrap();
        let promoted_at = q.visible.as_ref().unwrap().promoted_at;
        q.enqueue_at(fresh, t0 + Duration::from_millis(10)).unwrap();
        let visible = q.visible.as_ref().unwrap();
        assert_eq!(visible.event.payload.title, "new");
        assert_eq!(visible.event.payload.body, "fresh body");
        assert_eq!(visible.event.priority, Priority::High);
        assert_eq!(
            visible.event.rotation,
            RotationSpec::Recurring { display_secs: 4 }
        );
        // signal was the field missing from all three supersede call sites
        // until Stage A added it — this is the regression test for that gap.
        assert_eq!(visible.event.signal, EventSignal::Goal);
        // promoted_at is never mutated by a supersede — only extension_secs is.
        assert_eq!(visible.promoted_at, promoted_at);
    }

    #[test]
    fn visible_supersede_grants_extension_only_when_below_floor() {
        let t0 = Instant::now();

        // base window 10s: an immediate supersede has ~10s remaining, well
        // above the 2s floor — no extension granted.
        let mut q = SingleSlotQueue::new(50);
        q.enqueue_at(topic_event("a", Priority::Medium, 10, "topic"), t0)
            .unwrap();
        q.enqueue_at(topic_event("a2", Priority::Medium, 10, "topic"), t0)
            .unwrap();
        assert_eq!(q.visible.as_ref().unwrap().extension_secs, 0);

        // base window 1s: an immediate supersede already has only ~1s
        // remaining, below the 2s floor — extension granted to close the gap.
        let mut q2 = SingleSlotQueue::new(50);
        q2.enqueue_at(topic_event("b", Priority::Medium, 1, "topic"), t0)
            .unwrap();
        q2.enqueue_at(topic_event("b2", Priority::Medium, 1, "topic"), t0)
            .unwrap();
        let extension = q2.visible.as_ref().unwrap().extension_secs;
        assert!(
            extension > 0,
            "remaining was below the floor, expected an extension"
        );
        assert!(extension <= MAX_EXTENSION_ON_SUPERSEDE_SECS);
    }

    #[test]
    fn rapid_supersedes_obey_hard_deadline() {
        // 1s base window, floor 2s: every supersede in this burst lands
        // below the floor (remaining shrinks as simulated time advances),
        // so each grants a top-up — but the accumulated total must never
        // exceed MAX_EXTENSION_ON_SUPERSEDE_SECS, however many land.
        let mut q = SingleSlotQueue::new(50);
        let base = Instant::now();
        q.enqueue_at(topic_event("a0", Priority::Medium, 1, "topic"), base)
            .unwrap();

        for i in 1..=25 {
            let t = base + Duration::from_millis(i * 100);
            q.tick(t);
            q.enqueue_at(
                topic_event(&format!("a{i}"), Priority::Medium, 1, "topic"),
                t,
            )
            .unwrap();
            if let Some(item) = &q.visible {
                assert!(
                    item.extension_secs <= MAX_EXTENSION_ON_SUPERSEDE_SECS,
                    "extension_secs must never exceed the hard cap, got {} at i={i}",
                    item.extension_secs
                );
            }
        }

        // by base_window(1) + the hard cap, the item must be gone — no
        // number of prior supersedes can push it later than this.
        let deadline = base + Duration::from_secs(1 + MAX_EXTENSION_ON_SUPERSEDE_SECS);
        q.tick(deadline);
        assert!(
            q.visible.is_none(),
            "item must rotate out by the hard deadline"
        );
    }

    #[test]
    fn extension_secs_resets_on_next_promotion() {
        let mut q = SingleSlotQueue::new(50);
        let base = Instant::now();
        // 1s base window: an immediate supersede already has ~1s remaining,
        // below the 2s floor, so it's guaranteed to grant an extension.
        q.enqueue_at(topic_event("a0", Priority::Medium, 1, "topic"), base)
            .unwrap();
        q.enqueue_at(topic_event("a1", Priority::Medium, 1, "topic"), base)
            .unwrap();
        assert!(q.visible.as_ref().unwrap().extension_secs > 0);

        // advance well past base_window + max extension: item rotates out
        q.tick(base + Duration::from_secs(1 + MAX_EXTENSION_ON_SUPERSEDE_SECS + 1));
        assert!(q.visible.is_none());

        // a fresh, unrelated promotion starts with extension_secs back at 0
        q.enqueue(event("b", Priority::Medium, 8)).unwrap();
        assert_eq!(q.visible.as_ref().unwrap().extension_secs, 0);
        assert_eq!(q.visible.as_ref().unwrap().event.payload.title, "b");
        // the old topic item is gone entirely (OneShot dropped, not requeued)
        assert!(q
            .waiting
            .iter()
            .all(|t| !t.iter().any(|i| i.event.topic.as_deref() == Some("topic"))));
    }

    #[test]
    fn same_tier_waiting_supersede_keeps_position() {
        // paused from the start so both land in the waiting tier (not the
        // visible slot) — the fast path would otherwise promote the first
        // one immediately, and a supersede against the *visible* item is a
        // different code path (covered by the visible_supersede_* tests).
        let mut q = SingleSlotQueue::new(50);
        q.pause();
        q.enqueue(topic_event("first", Priority::Medium, 8, "topic"))
            .unwrap();
        q.enqueue(topic_event("second", Priority::Medium, 8, "topic2"))
            .unwrap();
        assert_eq!(q.waiting[Priority::Medium as usize].len(), 2);

        // supersede "topic" (position 0) with fresh content, same priority
        q.enqueue(topic_event("first-updated", Priority::Medium, 8, "topic"))
            .unwrap();

        assert_eq!(q.waiting[Priority::Medium as usize].len(), 2);
        assert_eq!(
            q.waiting[Priority::Medium as usize][0].event.payload.title,
            "first-updated"
        );
        assert_eq!(
            q.waiting[Priority::Medium as usize][1].event.payload.title,
            "second"
        );
    }

    #[test]
    fn cross_tier_supersede_moves_to_back_of_new_tier() {
        let mut q = SingleSlotQueue::new(50);
        q.pause();
        q.enqueue(topic_event("topic", Priority::Low, 8, "topic"))
            .unwrap();
        q.enqueue(event("low", Priority::Low, 8)).unwrap();
        q.enqueue(event("high", Priority::High, 8)).unwrap();

        q.enqueue(topic_event("topic-upgraded", Priority::High, 8, "topic"))
            .unwrap();

        assert_eq!(q.waiting[Priority::Low as usize].len(), 1);
        assert_eq!(
            q.waiting[Priority::Low as usize][0].event.payload.title,
            "low"
        );
        assert_eq!(q.waiting[Priority::High as usize].len(), 2);
        assert_eq!(
            q.waiting[Priority::High as usize][0].event.payload.title,
            "high"
        );
        assert_eq!(
            q.waiting[Priority::High as usize][1].event.payload.title,
            "topic-upgraded"
        );
    }

    // ------------------------------------------------------------------
    // per-tier cap
    // ------------------------------------------------------------------

    #[test]
    fn full_low_tier_rejects_low_but_accepts_high() {
        let mut q = SingleSlotQueue::new(1);
        q.pause();
        q.enqueue(event("low1", Priority::Low, 8)).unwrap();
        let low_err = q.enqueue(event("low2", Priority::Low, 8)).unwrap_err();
        assert!(matches!(low_err, QueueError::QueueFull));
        q.enqueue(event("high1", Priority::High, 8)).unwrap();
        assert_eq!(q.waiting[Priority::Low as usize].len(), 1);
        assert_eq!(q.waiting[Priority::High as usize].len(), 1);
    }

    // ------------------------------------------------------------------
    // pause / resume
    // ------------------------------------------------------------------

    #[test]
    fn pause_sends_enqueues_to_waiting_even_with_free_slot() {
        let mut q = SingleSlotQueue::new(50);
        q.pause();
        q.enqueue(event("a", Priority::Medium, 8)).unwrap();
        assert!(q.visible.is_none());
        assert_eq!(waiting_titles(&q, Priority::Medium as usize), vec!["a"]);
    }

    #[test]
    fn pause_gates_promotion_but_not_rotation() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 1)).unwrap();
        q.enqueue(event("b", Priority::Medium, 8)).unwrap();
        q.pause();

        let later = Instant::now() + Duration::from_secs(2);
        q.tick(later);
        // a aged out even while paused; b was NOT promoted
        assert!(q.visible.is_none());
        assert_eq!(waiting_titles(&q, Priority::Medium as usize), vec!["b"]);
    }

    #[test]
    fn test_enqueue_promotes_when_slot_empty_even_while_paused() {
        let mut q = SingleSlotQueue::new(50);
        q.pause();
        q.enqueue_test(event("test", Priority::Medium, 8)).unwrap();
        assert_eq!(visible_title(&q), Some("test"));
        assert!(q.is_paused(), "engine must remain paused after a test promotion");
    }

    #[test]
    fn test_enqueue_waits_behind_a_visible_item() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("real", Priority::Medium, 8)).unwrap();
        q.enqueue_test(event("test", Priority::Medium, 8)).unwrap();
        assert_eq!(visible_title(&q), Some("real"));
        assert_eq!(waiting_titles(&q, Priority::Medium as usize), vec!["test"]);
    }

    #[test]
    fn resume_then_tick_promotes_immediately() {
        let mut q = SingleSlotQueue::new(50);
        q.pause();
        q.enqueue(event("a", Priority::Medium, 8)).unwrap();
        q.resume();
        q.tick(Instant::now());
        assert_eq!(visible_title(&q), Some("a"));
        assert_eq!(q.total_waiting(), 0);
    }

    #[test]
    fn queue_full_is_enforced_identically_while_paused() {
        let mut q = SingleSlotQueue::new(2);
        q.pause();
        q.enqueue(event("a", Priority::Medium, 8)).unwrap();
        q.enqueue(event("b", Priority::Medium, 8)).unwrap();
        let err = q.enqueue(event("c", Priority::Medium, 8)).unwrap_err();
        assert!(matches!(err, QueueError::QueueFull));
    }

    #[test]
    fn dismiss_visible_clears_and_promotes_next_waiting() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 8)).unwrap();
        q.enqueue(event("b", Priority::Medium, 8)).unwrap();

        q.dismiss_visible(Instant::now());

        assert_eq!(visible_title(&q), Some("b"));
        assert_eq!(q.total_waiting(), 0);
    }

    #[test]
    fn dismiss_visible_is_noop_when_nothing_visible() {
        let mut q = SingleSlotQueue::new(50);

        q.dismiss_visible(Instant::now());

        assert!(q.visible.is_none());
        assert_eq!(q.total_waiting(), 0);
    }

    #[test]
    fn dismiss_visible_drops_recurring_item_rather_than_requeue() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(recurring_event("recur", Priority::Medium, 8))
            .unwrap();

        q.dismiss_visible(Instant::now());

        assert!(q.visible.is_none());
        assert!(q.waiting.iter().all(|t| t.is_empty()));
    }

    #[test]
    fn dismiss_visible_respects_paused() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 8)).unwrap();
        q.enqueue(event("b", Priority::Medium, 8)).unwrap();
        q.pause();

        q.dismiss_visible(Instant::now());

        assert!(q.visible.is_none());
        assert_eq!(waiting_titles(&q, Priority::Medium as usize), vec!["b"]);
    }

    #[test]
    fn pause_resume_pause_interleaving_never_double_promotes() {
        // single-slot model: only one item is ever visible at a time, so
        // draining 4 enqueued items takes 4 promotion cycles, not one tick.
        // The invariant under test is "no id is ever promoted twice" across
        // pause/resume interleaving — verified via the id-uniqueness check
        // at the end, after fully draining the queue.
        let mut q = SingleSlotQueue::new(50);
        let mut all_promoted: Vec<Uuid> = Vec::new();

        q.enqueue(event("a", Priority::Medium, 1)).unwrap(); // fast-path promotes
        if let Some(item) = q.visible.as_ref() {
            all_promoted.push(item.event.id);
        }

        q.pause();
        let t1 = Instant::now() + Duration::from_secs(2);
        q.tick(t1); // a ages out even while paused; nothing promotes (paused)
        assert!(q.visible.is_none());

        q.enqueue(event("d", Priority::Medium, 1)).unwrap();
        q.enqueue(event("e", Priority::Medium, 1)).unwrap();
        q.enqueue(event("f", Priority::Medium, 1)).unwrap();
        assert!(q.visible.is_none(), "still paused: nothing promotes yet");

        q.resume();
        q.tick(t1);
        if let Some(item) = q.visible.as_ref() {
            all_promoted.push(item.event.id); // d
        }

        // pause again mid-drain: d keeps rotating out on schedule (pause
        // never gates rotation), but nothing new promotes behind it while
        // paused.
        q.pause();
        let t2 = t1 + Duration::from_secs(2);
        q.tick(t2); // d ages out; e does NOT promote (paused)
        assert!(q.visible.is_none());

        q.resume();
        q.tick(t2);
        if let Some(item) = q.visible.as_ref() {
            all_promoted.push(item.event.id); // e
        }

        let t3 = t2 + Duration::from_secs(2);
        q.tick(t3); // e ages out, f promotes (still resumed)
        if let Some(item) = q.visible.as_ref() {
            all_promoted.push(item.event.id); // f
        }

        let t4 = t3 + Duration::from_secs(2);
        q.tick(t4); // f ages out; nothing left waiting
        assert!(q.visible.is_none());
        assert!(q.waiting.iter().all(|t| t.is_empty()));

        let unique: std::collections::HashSet<_> = all_promoted.iter().copied().collect();
        assert_eq!(unique.len(), all_promoted.len(), "no id promoted twice");
        assert_eq!(all_promoted.len(), 4);
    }

    // ------------------------------------------------------------------
    // slot-state emission / change guard
    // ------------------------------------------------------------------

    #[test]
    fn slot_state_change_guard_suppresses_identical_second_tick() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 8)).unwrap();
        let first = q.slot_state_if_changed();
        assert!(first.is_some());
        let second = q.slot_state_if_changed();
        assert!(second.is_none());
    }

    #[test]
    fn slot_state_emits_on_promotion_and_rotation_to_empty() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 1)).unwrap();
        q.slot_state_if_changed();

        let later = Instant::now() + Duration::from_secs(2);
        q.tick(later);
        let change = q.slot_state_if_changed();
        assert!(change.is_some());
        assert_eq!(change.unwrap(), SlotState::Empty);
    }

    #[test]
    fn slot_state_emits_on_expand_toggle() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 8)).unwrap();
        q.slot_state_if_changed();

        q.toggle_expanded();
        let change = q.slot_state_if_changed();
        assert!(change.is_some());
        match change.unwrap() {
            SlotState::Showing { expanded, .. } => assert!(expanded),
            SlotState::Empty => panic!("expected Showing"),
        }
    }

    #[test]
    fn expanded_increases_rotation_window() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 3)).unwrap();
        q.toggle_expanded();
        q.tick(Instant::now());
        q.slot_state_if_changed();

        let just_before = Instant::now() + Duration::from_secs(8);
        q.tick(just_before);
        assert!(q.visible.is_some(), "expanded window is 9s");

        let at_deadline = Instant::now() + Duration::from_secs(10);
        q.tick(at_deadline);
        assert!(q.visible.is_none());
    }

    // ------------------------------------------------------------------
    // plan 008: expanded auto-expand for High, per-item reset, idle no-op
    // ------------------------------------------------------------------

    #[test]
    fn high_priority_immediate_enqueue_auto_expands() {
        // Exercises enqueue_new's can_promote_now fast path directly (no
        // tick()/promote_next involved) — the more common of the two
        // promotion call sites in production (see plan 008).
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("h", Priority::High, 8)).unwrap();
        match q.current_slot_state() {
            SlotState::Showing { expanded, .. } => {
                assert!(
                    expanded,
                    "High priority must auto-expand on immediate promotion"
                )
            }
            SlotState::Empty => panic!("expected Showing"),
        }
    }

    #[test]
    fn high_priority_promoted_from_waiting_auto_expands() {
        // A Medium item occupies the slot, so the High item queues behind
        // it (higher-or-equal doesn't preempt). Ticking past the Medium
        // item's window drives promote_next, not the enqueue fast path.
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("medium", Priority::Medium, 1)).unwrap();
        q.enqueue(event("h", Priority::High, 8)).unwrap();
        assert_eq!(waiting_titles(&q, Priority::High as usize), vec!["h"]);

        let later = Instant::now() + Duration::from_secs(2);
        q.tick(later);

        assert_eq!(visible_title(&q), Some("h"));
        match q.current_slot_state() {
            SlotState::Showing { expanded, .. } => {
                assert!(expanded, "High priority must auto-expand on promote_next")
            }
            SlotState::Empty => panic!("expected Showing"),
        }
    }

    #[test]
    fn expanded_resets_when_next_item_promotes() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 1)).unwrap();
        q.toggle_expanded();
        match q.current_slot_state() {
            SlotState::Showing { expanded, .. } => assert!(expanded),
            SlotState::Empty => panic!("expected Showing"),
        }

        q.enqueue(event("b", Priority::Medium, 8)).unwrap();

        // "a" is expanded (9s window); ticking past its base 1s but before
        // its expanded 3s window must not promote "b" yet.
        let later = Instant::now() + Duration::from_secs(4);
        q.tick(later);

        assert_eq!(visible_title(&q), Some("b"));
        match q.current_slot_state() {
            SlotState::Showing { expanded, .. } => {
                assert!(!expanded, "next item must start collapsed")
            }
            SlotState::Empty => panic!("expected Showing"),
        }
    }

    #[test]
    fn auto_expanded_high_uses_expanded_rotation_window() {
        // Mirrors expanded_increases_rotation_window's arithmetic, but the
        // expansion here is automatic (High priority), not a manual toggle.
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("h", Priority::High, 3)).unwrap();

        let just_before = Instant::now() + Duration::from_secs(8);
        q.tick(just_before);
        assert!(q.visible.is_some(), "expanded window is 9s");

        let at_deadline = Instant::now() + Duration::from_secs(10);
        q.tick(at_deadline);
        assert!(q.visible.is_none());
    }

    #[test]
    fn toggle_expanded_is_noop_while_slot_empty() {
        let mut q = SingleSlotQueue::new(50);
        assert!(q.visible.is_none());

        q.toggle_expanded(); // idle press must arm nothing

        q.enqueue(event("a", Priority::Medium, 8)).unwrap();
        match q.current_slot_state() {
            SlotState::Showing { expanded, .. } => {
                assert!(
                    !expanded,
                    "idle toggle must not leak into the next promotion"
                )
            }
            SlotState::Empty => panic!("expected Showing"),
        }
    }
}

