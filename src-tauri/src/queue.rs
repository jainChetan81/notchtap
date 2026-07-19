use std::collections::VecDeque;
use std::time::{Duration, Instant};

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
/// queue.enqueue(event("a", Priority::Medium, 1), Instant::now()).unwrap();
/// queue.enqueue(event("b", Priority::Medium, 8), Instant::now()).unwrap();
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
    /// Render state — what the visible card looks like. Set `true` at every
    /// promotion (plan 033 expand-all), flipped by the auto-retract and by
    /// manual toggles. Never consulted by rotation arithmetic.
    expanded: bool,
    /// Rotation arithmetic — how long the turn is. `false` at every
    /// promotion (auto-expansion is display-only and free); set `true`
    /// only by a manual expand, sticky for the rest of the turn. This is
    /// the flag every `rotation_window(...)` call reads.
    window_expanded: bool,
    /// Set at every promotion alongside `expanded`; the auto-retract fires
    /// at half the base rotation window while armed. Any manual toggle
    /// press disarms it.
    auto_retract_armed: bool,
    /// Queue-slider counters (plan 033 decision 4): a batch starts when an
    /// event is accepted while the engine is fully idle; every accepted
    /// enqueue increments `batch_total`, every completion (rotated out,
    /// dismissed, skipped) increments `batch_done` — except a `Recurring`
    /// rotation-out or skip, which requeues rather than leaves and so does
    /// not count — and draining back to fully idle resets both.
    /// Supersession is neither.
    batch_total: usize,
    batch_done: usize,
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
            window_expanded: false,
            auto_retract_armed: false,
            batch_total: 0,
            batch_done: 0,
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

    /// Clock-agnostic (plan 037): `now` comes from the caller — the Engine
    /// reads `Instant::now()` once per operation; tests pass a simulated
    /// clock. No wall-clock read happens inside the queue.
    pub fn enqueue(&mut self, event: Event, now: Instant) -> Result<(), QueueError> {
        self.enqueue_with_options(event, now, false)
    }

    /// Test-enqueue variant: promotes into the visible slot even when the
    /// engine is paused, provided the slot is empty and no one is waiting.
    /// Real `/notify` pushes must never bypass pause, so the public `enqueue`
    /// path stays unchanged. Used by `send_test_notification`.
    pub fn enqueue_test(&mut self, event: Event, now: Instant) -> Result<(), QueueError> {
        self.enqueue_with_options(event, now, true)
    }

    // `now`-parameterized core so tests can drive the supersede/top-up path
    // deterministically without real sleeps — `top_up_visible_remaining_time`
    // needs a consistent notion of "now" alongside `promoted_at`, the same
    // way `tick()` already does. Both public entry points above take `now`
    // at the interface (plan 037 — clock-agnostic queue); this is the shared
    // internal path.
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
        // plan 033 decision 4: a batch starts when an event is accepted
        // while the engine is fully idle — counters (re)zero at that
        // moment. (Draining back to idle re-zeroes them too, so this is
        // belt-and-braces for the exact start semantics.)
        if self.visible.is_none() && self.all_tiers_empty() {
            self.batch_total = 0;
            self.batch_done = 0;
        }
        self.batch_total += 1;
        let mut item = QueueItem {
            event,
            enqueued_at: now,
            promoted_at: None,
            extension_secs: 0,
        };
        if can_promote_now {
            item.promoted_at = Some(now);
            self.set_expanded_for_promotion();
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
        self.retract_if_elapsed(now);
        self.rotate_out_if_elapsed(now);
        self.promote_next(now);
        self.reset_batch_if_idle();
    }

    /// plan 033: every promotion starts expanded (render-only — the turn
    /// length is untouched) and auto-collapses at half the *base* window.
    /// Runs before rotate/promote so a retract and a rotation due at the
    /// same instant collapse-then-rotate in one tick, and so a freshly
    /// promoted item's retract can never fire in its own promotion tick.
    fn retract_if_elapsed(&mut self, now: Instant) {
        if !(self.auto_retract_armed && self.expanded) {
            return;
        }
        let Some(item) = &self.visible else { return };
        let Some(promoted_at) = item.promoted_at else {
            return;
        };
        // Duration math, not seconds truncation: a 1s base window retracts
        // at 500ms, which `as_secs()` halving would round down to 0.
        let retract_after = Duration::from_secs(item.event.rotation_window(false)) / 2;
        if now.saturating_duration_since(promoted_at) < retract_after {
            return;
        }
        self.expanded = false;
        self.auto_retract_armed = false;
    }

    fn rotate_out_if_elapsed(&mut self, now: Instant) {
        let Some(item) = &self.visible else { return };
        let promoted_at = item.promoted_at.expect("visible items have promoted_at");
        let window = item.event.rotation_window(self.window_expanded) + item.extension_secs;
        if now.duration_since(promoted_at).as_secs() < window {
            return;
        }
        let item = self.visible.take().expect("checked Some above");
        if let RotationSpec::Recurring { .. } = item.event.rotation {
            let tier = item.event.priority as usize;
            self.waiting[tier].push_back(item);
        } else {
            self.batch_done += 1;
        }
    }

    fn promote_next(&mut self, now: Instant) {
        if self.visible.is_some() || self.paused {
            return;
        }
        if let Some(mut item) = self.pop_highest_priority_waiting() {
            item.promoted_at = Some(now);
            item.extension_secs = 0;
            self.set_expanded_for_promotion();
            self.visible = Some(item);
        }
    }

    // plan 033: every promotion starts expanded (render state), with the
    // base rotation window (auto-expansion never extends the turn — the
    // 3× window is manual-expand-only) and the auto-retract armed. The
    // per-turn reset also means a leftover manual expand/window can never
    // leak onto the next item. Called from both promotion sites
    // (promote_next and enqueue_new's immediate-promote fast path) so
    // neither can drift from the other.
    fn set_expanded_for_promotion(&mut self) {
        self.expanded = true;
        self.window_expanded = false;
        self.auto_retract_armed = true;
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
        let base_window = item.event.rotation_window(self.window_expanded);
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
        if self.visible.take().is_some() {
            self.batch_done += 1;
        }
        self.promote_next(now);
        self.reset_batch_if_idle();
    }

    /// Skip the Visible item: end its turn now, exactly as if its Rotation
    /// window had elapsed naturally — a Recurring item requeues to the back
    /// of its own Priority tier's Waiting line, a OneShot drops — then
    /// promote the next Waiting item immediately. Contrast with
    /// [`Self::dismiss_visible`], which drops a Recurring item outright:
    /// skip means "not now, come back later", dismiss means "gone".
    /// The requeue arm deliberately mirrors (not shares — see the plan that
    /// added this) `rotate_out_if_elapsed`'s: stale `promoted_at` /
    /// `extension_secs` on the requeued item are reset at its next
    /// Promotion, so neither needs touching here.
    pub fn skip_visible(&mut self, now: Instant) {
        if let Some(item) = self.visible.take() {
            if let RotationSpec::Recurring { .. } = item.event.rotation {
                let tier = item.event.priority as usize;
                self.waiting[tier].push_back(item);
            } else {
                self.batch_done += 1;
            }
        }
        self.promote_next(now);
        self.reset_batch_if_idle();
    }

    pub fn current_priority(&self) -> Option<Priority> {
        self.visible.as_ref().map(|i| i.event.priority)
    }

    pub fn current_link(&self) -> Option<&str> {
        self.visible
            .as_ref()
            .and_then(|item| item.event.meta.link.as_deref())
    }

    /// plan 033: with expand-all the hotkey always flips. Any press disarms
    /// the auto-retract; collapse is render-only (the turn length never
    /// changes on collapse); expand sets `window_expanded` — the manual 3×
    /// extension, sticky for the rest of the turn.
    pub fn toggle_expanded(&mut self) {
        if self.visible.is_none() {
            return;
        }
        self.auto_retract_armed = false;
        self.expanded = !self.expanded;
        if self.expanded {
            self.window_expanded = true;
        }
    }

    pub fn total_waiting(&self) -> usize {
        self.waiting.iter().map(|t| t.len()).sum()
    }

    /// plan 033 decision 4: fully idle (nothing visible, every tier empty)
    /// resets the batch counters for the next batch. Checked after every
    /// mutation that can drain the engine (tick, dismiss, skip); an
    /// accepted enqueue can never *reach* idle, so its batch-start zeroing
    /// lives in `enqueue_new` instead.
    fn reset_batch_if_idle(&mut self) {
        if self.visible.is_none() && self.all_tiers_empty() {
            self.batch_total = 0;
            self.batch_done = 0;
        }
    }

    /// The next Instant at which time alone changes state: the earlier of
    /// the visible item's auto-retract deadline (half the base window,
    /// while armed — plan 033) and its rotation deadline (plan 015). `None`
    /// when nothing is visible — promotion of waiting items is driven by
    /// mutations, which wake the heartbeat directly, not by a deadline this
    /// method could return. The deadline is returned regardless of
    /// `paused`: paused items still age out (`rotate_out_if_elapsed`
    /// doesn't check `paused`), Paused only disables `promote_next`.
    pub fn next_deadline(&self) -> Option<Instant> {
        let item = self.visible.as_ref()?;
        let promoted_at = item.promoted_at?;
        let window = item.event.rotation_window(self.window_expanded) + item.extension_secs;
        let rotation_deadline = promoted_at + Duration::from_secs(window);
        if self.auto_retract_armed && self.expanded {
            let retract_deadline =
                promoted_at + Duration::from_secs(item.event.rotation_window(false)) / 2;
            Some(retract_deadline.min(rotation_deadline))
        } else {
            Some(rotation_deadline)
        }
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
            Some(item) => {
                // Queue-slider position (plan 033): `total` never dips below
                // 1 (a visible item is always at least its own segment) and
                // `done` never reaches `total` while an item is visible
                // (the current segment stays bright) — defensive: no known
                // path pushes `batch_done` past `batch_total`; the cap stays
                // as cheap insurance against a future double-count.
                let queue_total = u32::try_from(self.batch_total.max(1)).unwrap_or(u32::MAX);
                let queue_done = u32::try_from(self.batch_done)
                    .unwrap_or(u32::MAX)
                    .min(queue_total - 1);
                SlotState::Showing {
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
                    subtitle: item.event.meta.subtitle.clone(),
                    details: item.event.meta.details.clone(),
                    queue_total,
                    queue_done,
                }
            }
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
    use crate::event::{DetailItem, EventMeta, EventPayload, EventSignal, EventType};
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
        q.enqueue(event("a", Priority::Medium, 8), Instant::now())
            .unwrap();
        assert_eq!(visible_title(&q), Some("a"));
        assert_eq!(q.total_waiting(), 0);
    }

    #[test]
    fn current_link_returns_visible_event_link() {
        let mut q = SingleSlotQueue::new(50);
        let mut story = event("story", Priority::Low, 8);
        story.meta.link = Some("https://example.com/story".to_string());

        q.enqueue(story, Instant::now()).unwrap();

        assert_eq!(q.current_link(), Some("https://example.com/story"));
    }

    #[test]
    fn current_link_returns_none_without_link_or_visible_event() {
        let mut q = SingleSlotQueue::new(50);
        assert_eq!(q.current_link(), None);

        q.enqueue(event("status", Priority::Medium, 8), Instant::now())
            .unwrap();

        assert_eq!(q.current_link(), None);
    }

    #[test]
    fn second_item_waits_when_slot_is_occupied() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 8), Instant::now())
            .unwrap();
        q.enqueue(event("b", Priority::Medium, 8), Instant::now())
            .unwrap();
        assert_eq!(visible_title(&q), Some("a"));
        assert_eq!(waiting_titles(&q, Priority::Medium as usize), vec!["b"]);
    }

    #[test]
    fn expired_item_is_removed_and_next_waiting_promoted() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 1), Instant::now())
            .unwrap();
        q.enqueue(event("b", Priority::Medium, 8), Instant::now())
            .unwrap();
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
        q.enqueue(event("low", Priority::Low, 8), Instant::now())
            .unwrap();
        q.enqueue(event("medium", Priority::Medium, 8), Instant::now())
            .unwrap();
        q.enqueue(event("high", Priority::High, 8), Instant::now())
            .unwrap();
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
        q.enqueue(event("first", Priority::Medium, 1), Instant::now())
            .unwrap();
        q.enqueue(event("second", Priority::Medium, 8), Instant::now())
            .unwrap();
        q.enqueue(event("third", Priority::Medium, 8), Instant::now())
            .unwrap();
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
        q.enqueue(
            event_from("news", Priority::Medium, 8, SourceKind::News),
            Instant::now(),
        )
        .unwrap();
        q.enqueue(
            event_from("manual", Priority::Medium, 8, SourceKind::Manual),
            Instant::now(),
        )
        .unwrap();
        q.enqueue(
            event_from("football", Priority::Medium, 8, SourceKind::Football),
            Instant::now(),
        )
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
        q.enqueue(
            event_from("news", Priority::Medium, 8, SourceKind::News),
            Instant::now(),
        )
        .unwrap();
        q.enqueue(
            event_from("football", Priority::Medium, 8, SourceKind::Football),
            Instant::now(),
        )
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
        q.enqueue(
            event_from("news-high", Priority::High, 8, SourceKind::News),
            Instant::now(),
        )
        .unwrap();
        q.enqueue(
            event_from("football-medium", Priority::Medium, 8, SourceKind::Football),
            Instant::now(),
        )
        .unwrap();
        q.resume();
        q.tick(Instant::now());
        assert_eq!(visible_title(&q), Some("news-high"));
    }

    #[test]
    fn high_enqueue_does_not_interrupt_currently_visible() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 4), Instant::now())
            .unwrap();
        q.enqueue(event("b", Priority::High, 2), Instant::now())
            .unwrap();
        assert_eq!(visible_title(&q), Some("a"));
        // tick before a's window elapses keeps a visible
        q.tick(Instant::now() + Duration::from_secs(2));
        assert_eq!(visible_title(&q), Some("a"));
    }

    #[test]
    fn oneshot_drops_forever() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 1), Instant::now())
            .unwrap();
        q.tick(Instant::now() + Duration::from_secs(2));
        assert!(q.visible.is_none());
        assert_eq!(q.total_waiting(), 0);
    }

    #[test]
    fn recurring_requeues_to_back_of_own_tier() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(recurring_event("a", Priority::Medium, 1), Instant::now())
            .unwrap();
        q.enqueue(event("b", Priority::Medium, 8), Instant::now())
            .unwrap();
        q.tick(Instant::now() + Duration::from_secs(2));
        assert_eq!(visible_title(&q), Some("b"));
        assert_eq!(waiting_titles(&q, Priority::Medium as usize), vec!["a"]);
    }

    #[test]
    fn recurring_requeues_not_to_front_or_different_tier() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(recurring_event("recur", Priority::Low, 1), Instant::now())
            .unwrap();
        q.enqueue(event("low2", Priority::Low, 8), Instant::now())
            .unwrap();
        q.enqueue(event("high", Priority::High, 8), Instant::now())
            .unwrap();
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
        q.enqueue(event("a", Priority::Medium, 1), Instant::now())
            .unwrap();
        q.enqueue(event("b", Priority::Low, 8), Instant::now())
            .unwrap();
        // a expired but tick hasn't run; slot is free and Low tier has b waiting
        let later = Instant::now() + Duration::from_secs(2);
        // manually age out a without tick's promotion half
        q.rotate_out_if_elapsed(later);
        assert!(q.visible.is_none());
        // a new high push must not jump b
        q.enqueue(event("c", Priority::High, 8), Instant::now())
            .unwrap();
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
        // fully simulated clock — enqueue takes `now` at the interface
        // (plan 037), so no real sleeps and no hidden wall-clock read
        // inside the top-up calculation.
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
        q.enqueue(base, t0).unwrap();
        let promoted_at = q.visible.as_ref().unwrap().promoted_at;
        q.enqueue(fresh, t0 + Duration::from_millis(10)).unwrap();
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
        q.enqueue(topic_event("a", Priority::Medium, 10, "topic"), t0)
            .unwrap();
        q.enqueue(topic_event("a2", Priority::Medium, 10, "topic"), t0)
            .unwrap();
        assert_eq!(q.visible.as_ref().unwrap().extension_secs, 0);

        // base window 1s: an immediate supersede already has only ~1s
        // remaining, below the 2s floor — extension granted to close the gap.
        let mut q2 = SingleSlotQueue::new(50);
        q2.enqueue(topic_event("b", Priority::Medium, 1, "topic"), t0)
            .unwrap();
        q2.enqueue(topic_event("b2", Priority::Medium, 1, "topic"), t0)
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
        q.enqueue(topic_event("a0", Priority::Medium, 1, "topic"), base)
            .unwrap();

        for i in 1..=25 {
            let t = base + Duration::from_millis(i * 100);
            q.tick(t);
            q.enqueue(
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
        q.enqueue(topic_event("a0", Priority::Medium, 1, "topic"), base)
            .unwrap();
        q.enqueue(topic_event("a1", Priority::Medium, 1, "topic"), base)
            .unwrap();
        assert!(q.visible.as_ref().unwrap().extension_secs > 0);

        // advance well past base_window + max extension: item rotates out
        q.tick(base + Duration::from_secs(1 + MAX_EXTENSION_ON_SUPERSEDE_SECS + 1));
        assert!(q.visible.is_none());

        // a fresh, unrelated promotion starts with extension_secs back at 0
        q.enqueue(event("b", Priority::Medium, 8), Instant::now())
            .unwrap();
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
        q.enqueue(
            topic_event("first", Priority::Medium, 8, "topic"),
            Instant::now(),
        )
        .unwrap();
        q.enqueue(
            topic_event("second", Priority::Medium, 8, "topic2"),
            Instant::now(),
        )
        .unwrap();
        assert_eq!(q.waiting[Priority::Medium as usize].len(), 2);

        // supersede "topic" (position 0) with fresh content, same priority
        q.enqueue(
            topic_event("first-updated", Priority::Medium, 8, "topic"),
            Instant::now(),
        )
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
        q.enqueue(
            topic_event("topic", Priority::Low, 8, "topic"),
            Instant::now(),
        )
        .unwrap();
        q.enqueue(event("low", Priority::Low, 8), Instant::now())
            .unwrap();
        q.enqueue(event("high", Priority::High, 8), Instant::now())
            .unwrap();

        q.enqueue(
            topic_event("topic-upgraded", Priority::High, 8, "topic"),
            Instant::now(),
        )
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
        q.enqueue(event("low1", Priority::Low, 8), Instant::now())
            .unwrap();
        let low_err = q
            .enqueue(event("low2", Priority::Low, 8), Instant::now())
            .unwrap_err();
        assert!(matches!(low_err, QueueError::QueueFull));
        q.enqueue(event("high1", Priority::High, 8), Instant::now())
            .unwrap();
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
        q.enqueue(event("a", Priority::Medium, 8), Instant::now())
            .unwrap();
        assert!(q.visible.is_none());
        assert_eq!(waiting_titles(&q, Priority::Medium as usize), vec!["a"]);
    }

    #[test]
    fn pause_gates_promotion_but_not_rotation() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 1), Instant::now())
            .unwrap();
        q.enqueue(event("b", Priority::Medium, 8), Instant::now())
            .unwrap();
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
        q.enqueue_test(event("test", Priority::Medium, 8), Instant::now())
            .unwrap();
        assert_eq!(visible_title(&q), Some("test"));
        assert!(
            q.is_paused(),
            "engine must remain paused after a test promotion"
        );
    }

    #[test]
    fn test_enqueue_waits_behind_a_visible_item() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("real", Priority::Medium, 8), Instant::now())
            .unwrap();
        q.enqueue_test(event("test", Priority::Medium, 8), Instant::now())
            .unwrap();
        assert_eq!(visible_title(&q), Some("real"));
        assert_eq!(waiting_titles(&q, Priority::Medium as usize), vec!["test"]);
    }

    #[test]
    fn resume_then_tick_promotes_immediately() {
        let mut q = SingleSlotQueue::new(50);
        q.pause();
        q.enqueue(event("a", Priority::Medium, 8), Instant::now())
            .unwrap();
        q.resume();
        q.tick(Instant::now());
        assert_eq!(visible_title(&q), Some("a"));
        assert_eq!(q.total_waiting(), 0);
    }

    #[test]
    fn queue_full_is_enforced_identically_while_paused() {
        let mut q = SingleSlotQueue::new(2);
        q.pause();
        q.enqueue(event("a", Priority::Medium, 8), Instant::now())
            .unwrap();
        q.enqueue(event("b", Priority::Medium, 8), Instant::now())
            .unwrap();
        let err = q
            .enqueue(event("c", Priority::Medium, 8), Instant::now())
            .unwrap_err();
        assert!(matches!(err, QueueError::QueueFull));
    }

    #[test]
    fn dismiss_visible_clears_and_promotes_next_waiting() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 8), Instant::now())
            .unwrap();
        q.enqueue(event("b", Priority::Medium, 8), Instant::now())
            .unwrap();

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
        q.enqueue(
            recurring_event("recur", Priority::Medium, 8),
            Instant::now(),
        )
        .unwrap();

        q.dismiss_visible(Instant::now());

        assert!(q.visible.is_none());
        assert!(q.waiting.iter().all(|t| t.is_empty()));
    }

    #[test]
    fn dismiss_visible_respects_paused() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 8), Instant::now())
            .unwrap();
        q.enqueue(event("b", Priority::Medium, 8), Instant::now())
            .unwrap();
        q.pause();

        q.dismiss_visible(Instant::now());

        assert!(q.visible.is_none());
        assert_eq!(waiting_titles(&q, Priority::Medium as usize), vec!["b"]);
    }

    #[test]
    fn skip_visible_requeues_recurring_to_back_of_own_tier_and_promotes_next() {
        // the exact case that distinguishes skip from dismiss: a Recurring
        // item survives a skip (dismiss_visible_drops_recurring_item_rather_
        // than_requeue proves dismiss destroys it)
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(
            recurring_event("recur", Priority::Medium, 8),
            Instant::now(),
        )
        .unwrap();
        q.enqueue(event("next", Priority::Medium, 8), Instant::now())
            .unwrap();

        q.skip_visible(Instant::now());

        assert_eq!(visible_title(&q), Some("next"));
        assert_eq!(waiting_titles(&q, Priority::Medium as usize), vec!["recur"]);
    }

    #[test]
    fn skip_visible_drops_oneshot_and_promotes_next() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 8), Instant::now())
            .unwrap();
        q.enqueue(event("b", Priority::Medium, 8), Instant::now())
            .unwrap();

        q.skip_visible(Instant::now());

        assert_eq!(visible_title(&q), Some("b"));
        assert_eq!(q.total_waiting(), 0);
    }

    #[test]
    fn skip_visible_is_noop_when_nothing_visible() {
        let mut q = SingleSlotQueue::new(50);

        q.skip_visible(Instant::now());

        assert!(q.visible.is_none());
        assert_eq!(q.total_waiting(), 0);
    }

    #[test]
    fn skip_visible_respects_paused() {
        // paused: the recurring item still requeues, but nothing promotes
        // (Promotion is frozen — CONTEXT.md's Paused contract)
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(
            recurring_event("recur", Priority::Medium, 8),
            Instant::now(),
        )
        .unwrap();
        q.pause();

        q.skip_visible(Instant::now());

        assert!(q.visible.is_none());
        assert_eq!(waiting_titles(&q, Priority::Medium as usize), vec!["recur"]);
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

        q.enqueue(event("a", Priority::Medium, 1), Instant::now())
            .unwrap(); // fast-path promotes
        if let Some(item) = q.visible.as_ref() {
            all_promoted.push(item.event.id);
        }

        q.pause();
        let t1 = Instant::now() + Duration::from_secs(2);
        q.tick(t1); // a ages out even while paused; nothing promotes (paused)
        assert!(q.visible.is_none());

        q.enqueue(event("d", Priority::Medium, 1), Instant::now())
            .unwrap();
        q.enqueue(event("e", Priority::Medium, 1), Instant::now())
            .unwrap();
        q.enqueue(event("f", Priority::Medium, 1), Instant::now())
            .unwrap();
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
        q.enqueue(event("a", Priority::Medium, 8), Instant::now())
            .unwrap();
        let first = q.slot_state_if_changed();
        assert!(first.is_some());
        let second = q.slot_state_if_changed();
        assert!(second.is_none());
    }

    #[test]
    fn slot_state_emits_on_promotion_and_rotation_to_empty() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 1), Instant::now())
            .unwrap();
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
        q.enqueue(event("a", Priority::Medium, 8), Instant::now())
            .unwrap();
        q.slot_state_if_changed();

        // plan 033: every promotion starts expanded, so the first press
        // collapses — the point here is that a toggle emits, whichever
        // way it flips.
        q.toggle_expanded();
        let change = q.slot_state_if_changed();
        assert!(change.is_some());
        match change.unwrap() {
            SlotState::Showing { expanded, .. } => assert!(!expanded),
            SlotState::Empty => panic!("expected Showing"),
        }
    }

    #[test]
    fn expanded_increases_rotation_window() {
        // plan 033: the 3× window is manual-expand-only. The promotion
        // starts auto-expanded with the *base* window; the auto-retract
        // collapses it at half that window, and a manual expand from
        // there extends the turn 3×.
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("a", Priority::Medium, 3), t0).unwrap();

        // retract fires at 1.5s (half the 3s base); the item is still
        // inside its base window at +2s.
        q.tick(t0 + Duration::from_secs(2));
        assert!(q.visible.is_some());
        match q.current_slot_state() {
            SlotState::Showing { expanded, .. } => {
                assert!(!expanded, "auto-retract must have collapsed the card")
            }
            SlotState::Empty => panic!("expected Showing"),
        }

        q.toggle_expanded(); // manual expand: window becomes 3 × 3s = 9s

        let just_before = t0 + Duration::from_secs(8);
        q.tick(just_before);
        assert!(q.visible.is_some(), "expanded window is 9s");

        let at_deadline = t0 + Duration::from_secs(10);
        q.tick(at_deadline);
        assert!(q.visible.is_none());
    }

    // ------------------------------------------------------------------
    // plan 008's expanded-semantics cases, rewritten for plan 033:
    // auto-expand for every priority, half-window auto-retract, manual-only
    // window extension, per-item reset, idle no-op
    // ------------------------------------------------------------------

    #[test]
    fn every_priority_auto_expands_on_immediate_enqueue() {
        // Exercises enqueue_new's can_promote_now fast path directly (no
        // tick()/promote_next involved) — the more common of the two
        // promotion call sites in production. plan 033: expand-all, not
        // plan 008's High-only.
        for priority in [Priority::Low, Priority::Medium, Priority::High] {
            let mut q = SingleSlotQueue::new(50);
            q.enqueue(event("x", priority, 8), Instant::now()).unwrap();
            match q.current_slot_state() {
                SlotState::Showing { expanded, .. } => {
                    assert!(
                        expanded,
                        "{priority:?} must auto-expand on immediate promotion"
                    )
                }
                SlotState::Empty => panic!("expected Showing"),
            }
        }
    }

    #[test]
    fn low_priority_promoted_from_waiting_auto_expands() {
        // A Medium item occupies the slot, so the Low item queues behind
        // it. Ticking past the Medium item's window drives promote_next,
        // not the enqueue fast path — and the promotion must still start
        // expanded (Low is the case plan 008 never auto-expanded).
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("medium", Priority::Medium, 1), Instant::now())
            .unwrap();
        q.enqueue(event("l", Priority::Low, 8), Instant::now())
            .unwrap();
        assert_eq!(waiting_titles(&q, Priority::Low as usize), vec!["l"]);

        let later = Instant::now() + Duration::from_secs(2);
        q.tick(later);

        assert_eq!(visible_title(&q), Some("l"));
        match q.current_slot_state() {
            SlotState::Showing { expanded, .. } => {
                assert!(expanded, "every priority must auto-expand on promote_next")
            }
            SlotState::Empty => panic!("expected Showing"),
        }
    }

    #[test]
    fn expanded_resets_when_next_item_promotes() {
        // plan 033 keeps plan 008's per-item reset, but the reset target
        // flipped: the next item now starts *expanded* (with the base
        // window and a freshly armed retract), never inheriting the
        // previous item's manual expand/window.
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("a", Priority::Medium, 1), t0).unwrap();
        // let the auto-retract collapse "a" (at 0.5s, half its 1s base),
        // then manually expand it — the only path to the 3× window.
        q.tick(t0 + Duration::from_millis(600));
        q.toggle_expanded();
        match q.current_slot_state() {
            SlotState::Showing { expanded, .. } => assert!(expanded),
            SlotState::Empty => panic!("expected Showing"),
        }

        q.enqueue(event("b", Priority::Medium, 8), Instant::now())
            .unwrap();

        // "a" is manually expanded (3s window); ticking past its base 1s
        // but before its expanded 3s window must not promote "b" yet.
        let later = t0 + Duration::from_secs(4);
        q.tick(later);

        assert_eq!(visible_title(&q), Some("b"));
        match q.current_slot_state() {
            SlotState::Showing { expanded, .. } => {
                assert!(expanded, "next item must start expanded (plan 033)")
            }
            SlotState::Empty => panic!("expected Showing"),
        }
        // ...and with the base window, not "a"'s leftover 3× extension:
        // "b" (ttl 8) must rotate out at 8s, not 24s.
        q.tick(later + Duration::from_secs(7));
        assert!(q.visible.is_some(), "base window is 8s");
        q.tick(later + Duration::from_secs(9));
        assert!(q.visible.is_none(), "no inherited 3× window");
    }

    #[test]
    fn auto_expanded_item_keeps_base_rotation_window() {
        // plan 008's auto_expanded_high_uses_expanded_rotation_window,
        // rewritten: auto-expansion is display-only and free — the turn
        // length stays the configured ttl even though the card promoted
        // expanded.
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("h", Priority::High, 3), t0).unwrap();
        match q.current_slot_state() {
            SlotState::Showing { expanded, .. } => assert!(expanded),
            SlotState::Empty => panic!("expected Showing"),
        }

        let just_before = t0 + Duration::from_secs(2);
        q.tick(just_before);
        assert!(q.visible.is_some(), "base window is 3s");

        let at_deadline = t0 + Duration::from_secs(4);
        q.tick(at_deadline);
        assert!(
            q.visible.is_none(),
            "auto-expand must not extend the rotation window"
        );
    }

    #[test]
    fn toggle_expanded_is_noop_while_slot_empty() {
        let mut q = SingleSlotQueue::new(50);
        assert!(q.visible.is_none());

        q.toggle_expanded(); // idle press must arm nothing

        q.enqueue(event("a", Priority::Medium, 8), Instant::now())
            .unwrap();
        // the promotion auto-expands (plan 033) — that's the default, not
        // a leak. What the idle press must not leak is the manual 3×
        // window, and the retract must still come armed from the
        // promotion itself.
        assert!(q.expanded, "promotion auto-expands (plan 033)");
        assert!(
            !q.window_expanded,
            "idle toggle must not leak the 3× window into the next promotion"
        );
        assert!(
            q.auto_retract_armed,
            "the retract is armed fresh at promotion"
        );
    }

    // ------------------------------------------------------------------
    // plan 033: auto-retract at half the base window
    // ------------------------------------------------------------------

    #[test]
    fn auto_retract_fires_at_half_the_base_window_and_emits() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("a", Priority::Medium, 4), t0).unwrap();
        q.slot_state_if_changed(); // consume the promotion emission

        // just before half the 4s base window: still expanded, no emission
        q.tick(t0 + Duration::from_millis(1900));
        assert!(q.slot_state_if_changed().is_none());
        match q.current_slot_state() {
            SlotState::Showing { expanded, .. } => assert!(expanded),
            SlotState::Empty => panic!("expected Showing"),
        }

        // past half: the retract fires, emits expanded:false, and the item
        // stays visible to finish its turn compact
        q.tick(t0 + Duration::from_millis(2100));
        match q.slot_state_if_changed() {
            Some(SlotState::Showing { expanded, .. }) => {
                assert!(!expanded, "retract must collapse the render state")
            }
            other => panic!("expected a Showing collapse emission, got {other:?}"),
        }
        assert!(
            q.visible.is_some(),
            "retract is display-only — the item finishes its turn"
        );
        // the retract fires once: a later tick emits nothing more
        q.tick(t0 + Duration::from_secs(3));
        assert!(q.slot_state_if_changed().is_none());
    }

    #[test]
    fn auto_retract_uses_subsecond_duration_math() {
        // a 1s base window retracts at 500ms — seconds truncation
        // (`as_secs()` halving) would round that to 0 and retract
        // immediately.
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("a", Priority::Medium, 1), t0).unwrap();

        q.tick(t0 + Duration::from_millis(400));
        match q.current_slot_state() {
            SlotState::Showing { expanded, .. } => {
                assert!(expanded, "400ms < 500ms: too early to retract")
            }
            SlotState::Empty => panic!("expected Showing"),
        }

        q.tick(t0 + Duration::from_millis(600));
        match q.current_slot_state() {
            SlotState::Showing { expanded, .. } => {
                assert!(!expanded, "600ms >= 500ms: retract must have fired")
            }
            SlotState::Empty => panic!("expected Showing"),
        }
    }

    #[test]
    fn manual_toggle_disarms_the_auto_retract() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("a", Priority::Medium, 4), t0).unwrap();
        q.slot_state_if_changed(); // consume the promotion emission

        q.toggle_expanded(); // manual collapse, well before the retract moment
        q.slot_state_if_changed(); // consume the collapse emission

        // past half the base window: no auto-retract fires — the press
        // disarmed it — and the turn length is untouched (collapse is
        // render-only, so the item still rotates at 4s, not sooner).
        q.tick(t0 + Duration::from_secs(3));
        assert!(q.slot_state_if_changed().is_none());
        match q.current_slot_state() {
            SlotState::Showing { expanded, .. } => assert!(!expanded),
            SlotState::Empty => panic!("expected Showing"),
        }
    }

    // ------------------------------------------------------------------
    // plan 015: next_deadline (plan 033: min of armed retract + rotation)
    // ------------------------------------------------------------------

    #[test]
    fn next_deadline_is_none_on_empty_queue() {
        let q = SingleSlotQueue::new(50);
        assert!(q.next_deadline().is_none());
    }

    #[test]
    fn next_deadline_prefers_an_armed_retract_over_rotation() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("a", Priority::Medium, 8), t0).unwrap();
        let promoted_at = q.visible.as_ref().unwrap().promoted_at.unwrap();

        // the armed auto-retract fires at half the 8s base window —
        // earlier than the 8s rotation deadline, so it's what the
        // heartbeat must wake for.
        assert_eq!(
            q.next_deadline(),
            Some(promoted_at + Duration::from_secs(4))
        );
    }

    #[test]
    fn next_deadline_is_the_rotation_deadline_once_the_retract_has_fired() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("a", Priority::Medium, 8), t0).unwrap();
        // past half the base window: the retract fires and disarms, so the
        // only deadline left is rotation.
        q.tick(t0 + Duration::from_secs(5));
        let promoted_at = q.visible.as_ref().unwrap().promoted_at.unwrap();

        assert_eq!(
            q.next_deadline(),
            Some(promoted_at + Duration::from_secs(8))
        );
    }

    #[test]
    fn next_deadline_uses_expanded_window() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("a", Priority::Medium, 8), t0).unwrap();
        // plan 033: a promotion starts auto-expanded with the *base*
        // window — a manual expand (the only 3× path) first needs the
        // auto-retract to have collapsed the card.
        q.tick(t0 + Duration::from_secs(5));
        q.toggle_expanded();
        let promoted_at = q.visible.as_ref().unwrap().promoted_at.unwrap();

        // Medium's 8s base window becomes 24s expanded (EXPANDED_MULTIPLIER = 3)
        assert_eq!(
            q.next_deadline(),
            Some(promoted_at + Duration::from_secs(24))
        );
    }

    #[test]
    fn next_deadline_is_some_while_paused() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("a", Priority::Medium, 8), t0).unwrap();
        q.pause();

        assert!(
            q.next_deadline().is_some(),
            "a paused visible item must keep aging toward rotation"
        );
    }

    #[test]
    fn next_deadline_includes_supersede_extension() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(topic_event("a", Priority::Medium, 1, "topic"), t0)
            .unwrap();
        // below the floor: this supersede grants a top-up (see
        // visible_supersede_grants_extension_only_when_below_floor)
        q.enqueue(topic_event("a2", Priority::Medium, 1, "topic"), t0)
            .unwrap();
        // past the retract moment (half of the 1s base): the retract
        // deadline is spent, so the remaining deadline is rotation —
        // which is where the extension shows up.
        q.tick(t0 + Duration::from_millis(600));
        let visible = q.visible.as_ref().unwrap();
        let promoted_at = visible.promoted_at.unwrap();
        let extension_secs = visible.extension_secs;
        assert!(extension_secs > 0, "expected a top-up to have been granted");

        assert_eq!(
            q.next_deadline(),
            Some(promoted_at + Duration::from_secs(1 + extension_secs))
        );
    }

    // ------------------------------------------------------------------
    // plan 033: batch counters behind the queue slider (decision 4)
    // ------------------------------------------------------------------

    fn queue_progress(q: &SingleSlotQueue) -> (u32, u32) {
        match q.current_slot_state() {
            SlotState::Showing {
                queue_total,
                queue_done,
                ..
            } => (queue_total, queue_done),
            SlotState::Empty => panic!("expected Showing"),
        }
    }

    #[test]
    fn batch_starts_at_first_accepted_enqueue_from_idle() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 8), Instant::now())
            .unwrap();
        assert_eq!(queue_progress(&q), (1, 0));
    }

    // plan 035: the rich-relay fields ride on EventMeta, so a visible
    // item's subtitle/details must surface in current_slot_state — the one
    // passthrough the frontend renders as manifest cells.
    #[test]
    fn current_slot_state_carries_subtitle_and_details_from_event_meta() {
        let mut q = SingleSlotQueue::new(50);
        let mut ev = event("a", Priority::High, 8);
        ev.meta.subtitle = Some("Permission request".to_string());
        ev.meta.details = vec![
            DetailItem {
                label: "Tool".to_string(),
                value: "Bash".to_string(),
            },
            DetailItem {
                label: "Command".to_string(),
                value: "git push".to_string(),
            },
        ];
        q.enqueue(ev, Instant::now()).unwrap();
        match q.current_slot_state() {
            SlotState::Showing {
                subtitle, details, ..
            } => {
                assert_eq!(subtitle.as_deref(), Some("Permission request"));
                assert_eq!(details.len(), 2);
                assert_eq!(details[0].label, "Tool");
                assert_eq!(details[0].value, "Bash");
                assert_eq!(details[1].label, "Command");
                assert_eq!(details[1].value, "git push");
            }
            SlotState::Empty => panic!("expected Showing"),
        }
    }

    #[test]
    fn every_accepted_enqueue_increments_batch_total() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 8), Instant::now())
            .unwrap();
        q.enqueue(event("b", Priority::Medium, 8), Instant::now())
            .unwrap();
        q.enqueue(event("c", Priority::High, 8), Instant::now())
            .unwrap();
        assert_eq!(queue_progress(&q), (3, 0));
    }

    #[test]
    fn every_completion_increments_batch_done() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("a", Priority::Medium, 1), t0).unwrap();
        q.enqueue(event("b", Priority::Medium, 8), t0).unwrap();
        q.enqueue(event("c", Priority::Medium, 8), t0).unwrap();

        // rotation-out: a's 1s ttl elapses, b promotes
        q.tick(t0 + Duration::from_secs(2));
        assert_eq!(visible_title(&q), Some("b"));
        assert_eq!(queue_progress(&q), (3, 1));

        // dismiss: b drops, c promotes
        q.dismiss_visible(t0 + Duration::from_secs(3));
        assert_eq!(visible_title(&q), Some("c"));
        assert_eq!(queue_progress(&q), (3, 2));

        // skip: c drops, nothing waiting — the batch is drained
        q.skip_visible(t0 + Duration::from_secs(4));
        assert_eq!(q.current_slot_state(), SlotState::Empty);
    }

    #[test]
    fn fully_idle_resets_the_batch_for_the_next_one() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("a", Priority::Medium, 1), t0).unwrap();
        q.enqueue(event("b", Priority::Medium, 1), t0).unwrap();

        // drain the batch completely
        q.tick(t0 + Duration::from_secs(2)); // a out, b promotes
        q.tick(t0 + Duration::from_secs(4)); // b out, engine fully idle
        assert_eq!(q.current_slot_state(), SlotState::Empty);
        assert_eq!((q.batch_total, q.batch_done), (0, 0));

        // the next batch counts from zero, not from the drained one
        q.enqueue(event("c", Priority::Medium, 8), t0 + Duration::from_secs(5))
            .unwrap();
        assert_eq!(queue_progress(&q), (1, 0));
    }

    #[test]
    fn supersession_is_neither_a_new_item_nor_a_completion() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(topic_event("a", Priority::Medium, 8, "topic"), t0)
            .unwrap();
        q.enqueue(event("b", Priority::Medium, 8), t0).unwrap();
        assert_eq!(queue_progress(&q), (2, 0));

        // superseding the visible item merges content in place — the
        // slider must not move in either direction
        q.enqueue(topic_event("a-fresh", Priority::Medium, 8, "topic"), t0)
            .unwrap();
        assert_eq!(visible_title(&q), Some("a-fresh"));
        assert_eq!(queue_progress(&q), (2, 0));
    }

    #[test]
    fn batch_done_caps_at_total_minus_one_while_an_item_is_visible() {
        // A Recurring rotation-out requeues rather than leaves, so it does
        // not advance `batch_done`; only a real completion (a OneShot
        // leaving) does. `done` still never reaches `total` while an item
        // is visible — the current segment stays lit.
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(recurring_event("r", Priority::Medium, 1), t0)
            .unwrap();
        q.enqueue(event("b", Priority::Medium, 1), t0).unwrap();

        q.tick(t0 + Duration::from_secs(2)); // r rotates out, requeues; b promotes
        assert_eq!(visible_title(&q), Some("b"));
        assert_eq!(queue_progress(&q), (2, 0));

        q.tick(t0 + Duration::from_secs(4)); // b completes; r promotes again
        assert_eq!(visible_title(&q), Some("r"));
        assert_eq!(queue_progress(&q), (2, 1));
    }

    #[test]
    fn recurring_requeue_via_tick_or_skip_does_not_advance_batch_done() {
        // A visible Recurring item that rotates out (tick) or is skipped
        // requeues to the back of its own tier without advancing
        // `batch_done`; a sibling OneShot completion still does.
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(recurring_event("r", Priority::Medium, 1), t0)
            .unwrap();
        q.enqueue(event("b", Priority::Medium, 8), t0).unwrap();
        q.enqueue(event("c", Priority::Medium, 8), t0).unwrap();

        // natural rotation-out: r requeues to the back of its tier, not done
        q.tick(t0 + Duration::from_secs(2));
        assert_eq!(visible_title(&q), Some("b"));
        assert_eq!(waiting_titles(&q, Priority::Medium as usize), ["c", "r"]);
        assert_eq!(queue_progress(&q), (3, 0));

        // skip: b is OneShot and leaves for good — a real completion
        q.skip_visible(t0 + Duration::from_secs(3));
        assert_eq!(visible_title(&q), Some("c"));
        assert_eq!(queue_progress(&q), (3, 1));

        // skip: c is OneShot — done; r (still waiting) promotes
        q.skip_visible(t0 + Duration::from_secs(4));
        assert_eq!(visible_title(&q), Some("r"));
        assert_eq!(queue_progress(&q), (3, 2));

        // skip the Recurring r: it requeues again, still not done
        q.skip_visible(t0 + Duration::from_secs(5));
        assert_eq!(visible_title(&q), Some("r"));
        assert_eq!(queue_progress(&q), (3, 2));
    }
}

// ----------------------------------------------------------------------
// §9.1 (docs/TESTING_STRATEGY.md) — generated-adversary property suite.
// Supplements `mod tests` above; never replaces it. A sibling module (not
// nested inside `mod tests`) so it can stay organized around its own
// harness, while still reusing the same private, `#[cfg(test)]`-gated
// surface (direct `visible`/`waiting`/`expanded`/
// `window_expanded`/`auto_retract_armed` field access) that `mod tests`
// already relies on.
// ----------------------------------------------------------------------
#[cfg(test)]
mod proptest_queue {
    use super::*;
    use crate::event::{EventMeta, EventPayload, EventSignal, EventType};
    use proptest::prelude::*;
    use uuid::Uuid;

    // ------------------------------------------------------------------
    // Op model (docs/TESTING_STRATEGY.md §9.1) — one variant per
    // state-mutating `pub fn` on `SingleSlotQueue`, minus the two
    // pre-cleared exceptions: `enqueue_test` (test-only /notify bypass,
    // not part of the production op model) and `slot_state_if_changed`
    // (the invariant-7 probe, called every step, never generated).
    // `with_rotation_order` is a per-case queue parameter, not a scripted
    // op — generated once per case by `arb_rotation_order()` below
    // (empty, partial, or a full permutation), not scripted mid-run.
    // Invariant 4 below checks the resulting minimum-rank/FIFO-tie
    // promotion order directly.
    // ------------------------------------------------------------------

    #[derive(Debug, Clone, Copy)]
    enum RotKind {
        OneShot(u64),
        Recurring(u64),
    }

    #[derive(Debug, Clone)]
    struct EnqueueSpec {
        priority: Priority,
        rotation: RotKind,
        topic: Option<u8>,
        origin: SourceKind,
    }

    #[derive(Debug, Clone)]
    enum Op {
        Enqueue(EnqueueSpec),
        Tick(u64),
        Dismiss,
        Skip,
        ToggleExpanded,
        Pause,
        Resume,
    }

    fn arb_priority() -> impl Strategy<Value = Priority> {
        prop_oneof![
            Just(Priority::Low),
            Just(Priority::Medium),
            Just(Priority::High),
        ]
    }

    fn arb_origin() -> impl Strategy<Value = SourceKind> {
        prop_oneof![
            Just(SourceKind::Football),
            Just(SourceKind::News),
            Just(SourceKind::Manual),
            Just(SourceKind::Cmux),
        ]
    }

    fn arb_rotation() -> impl Strategy<Value = RotKind> {
        prop_oneof![
            (1u64..=10).prop_map(RotKind::OneShot),
            (1u64..=10).prop_map(RotKind::Recurring),
        ]
    }

    // Per-case rotation_order (docs/TESTING_STRATEGY.md §9.1 invariant 4):
    // shuffle the four SourceKind variants, then truncate to a random
    // 0..=4 length, so empty (pure FIFO), partial (some origins unlisted —
    // rank falls back to rotation_order.len()), and full permutation
    // orders are all reachable across cases.
    fn arb_rotation_order() -> impl Strategy<Value = Vec<SourceKind>> {
        let all = vec![
            SourceKind::Football,
            SourceKind::News,
            SourceKind::Manual,
            SourceKind::Cmux,
        ];
        (Just(all).prop_shuffle(), 0usize..=4).prop_map(|(mut v, len)| {
            v.truncate(len);
            v
        })
    }

    // small closed set of topic tags (as Some(0..3)) plus None, so
    // supersession (same-topic collisions) actually happens often enough
    // in a 0..50-op script to exercise invariants 2(c) and 6.
    fn arb_topic() -> impl Strategy<Value = Option<u8>> {
        prop_oneof![Just(None), (0u8..3).prop_map(Some)]
    }

    fn arb_enqueue() -> impl Strategy<Value = EnqueueSpec> {
        (arb_priority(), arb_rotation(), arb_topic(), arb_origin()).prop_map(
            |(priority, rotation, topic, origin)| EnqueueSpec {
                priority,
                rotation,
                topic,
                origin,
            },
        )
    }

    fn arb_op() -> impl Strategy<Value = Op> {
        prop_oneof![
            3 => arb_enqueue().prop_map(Op::Enqueue),
            2 => (0u64..=12).prop_map(Op::Tick),
            1 => Just(Op::Dismiss),
            1 => Just(Op::Skip),
            1 => Just(Op::ToggleExpanded),
            1 => Just(Op::Pause),
            1 => Just(Op::Resume),
        ]
    }

    fn build_event(spec: &EnqueueSpec) -> Event {
        Event {
            id: Uuid::new_v4(),
            event_type: EventType::Generic,
            priority: spec.priority,
            rotation: match spec.rotation {
                RotKind::OneShot(secs) => RotationSpec::OneShot { ttl_secs: secs },
                RotKind::Recurring(secs) => RotationSpec::Recurring { display_secs: secs },
            },
            topic: spec.topic.map(|n| format!("topic-{n}")),
            payload: EventPayload {
                title: "t".to_string(),
                body: "b".to_string(),
            },
            meta: EventMeta::default(),
            signal: EventSignal::Generic,
            origin: spec.origin,
        }
    }

    // ------------------------------------------------------------------
    // harness — direct field access (this module is a descendant of the
    // module `SingleSlotQueue` is defined in, same as `mod tests`).
    // ------------------------------------------------------------------

    fn snapshot_waiting(q: &SingleSlotQueue) -> [Vec<(Uuid, SourceKind)>; 3] {
        [
            q.waiting[0]
                .iter()
                .map(|i| (i.event.id, i.event.origin))
                .collect(),
            q.waiting[1]
                .iter()
                .map(|i| (i.event.id, i.event.origin))
                .collect(),
            q.waiting[2]
                .iter()
                .map(|i| (i.event.id, i.event.origin))
                .collect(),
        ]
    }

    // Invariant 4: highest-index non-empty tier, then within that tier the
    // item of minimum rotation_order rank (ties broken by lowest index —
    // i.e. FIFO / earliest arrival). This is a self-contained mirror of
    // production `SingleSlotQueue::best_index_in_tier` — same strict-`<`
    // comparison, so a rank tie keeps the earliest (lowest-index) item,
    // and an origin absent from rotation_order (or an empty order) ranks
    // last via `unwrap_or(rotation_order.len())`.
    fn predict_promoted(
        snap: &[Vec<(Uuid, SourceKind)>; 3],
        rotation_order: &[SourceKind],
    ) -> Option<Uuid> {
        let rank = |origin: SourceKind| {
            rotation_order
                .iter()
                .position(|o| *o == origin)
                .unwrap_or(rotation_order.len())
        };
        for tier in (0..3).rev() {
            let items = &snap[tier];
            if items.is_empty() {
                continue;
            }
            let mut best = 0;
            let mut best_rank = rank(items[0].1);
            for (i, &(_, origin)) in items.iter().enumerate().skip(1) {
                let r = rank(origin);
                if r < best_rank {
                    best = i;
                    best_rank = r;
                }
            }
            return Some(items[best].0);
        }
        None
    }

    struct VisSnap {
        id: Uuid,
        tier: usize,
        recurring: bool,
        promoted_at: Instant,
        window: u64,
        origin: SourceKind,
    }

    fn vis_snapshot(q: &SingleSlotQueue) -> Option<VisSnap> {
        q.visible.as_ref().map(|item| VisSnap {
            id: item.event.id,
            tier: item.event.priority as usize,
            recurring: matches!(item.event.rotation, RotationSpec::Recurring { .. }),
            promoted_at: item.promoted_at.expect("visible items have promoted_at"),
            window: item.event.rotation_window(q.window_expanded) + item.extension_secs,
            origin: item.event.origin,
        })
    }

    fn current_vis_id(q: &SingleSlotQueue) -> Option<Uuid> {
        q.visible.as_ref().map(|i| i.event.id)
    }

    struct Harness {
        q: SingleSlotQueue,
        now: Instant,
        max_queued_per_tier: usize,
        rotation_order: Vec<SourceKind>,
        // invariant 5/6 conservation counters
        enqueued_accepted: u64,
        rotated_out_dropped: u64,
        dismissed: u64,
        skipped_oneshot_dropped: u64,
        // invariant 7 probe state
        last_some_state: Option<SlotState>,
    }

    impl Harness {
        fn new(max_queued_per_tier: usize, rotation_order: Vec<SourceKind>) -> Self {
            Self {
                q: SingleSlotQueue::new(max_queued_per_tier)
                    .with_rotation_order(rotation_order.clone()),
                now: Instant::now(),
                max_queued_per_tier,
                rotation_order,
                enqueued_accepted: 0,
                rotated_out_dropped: 0,
                dismissed: 0,
                skipped_oneshot_dropped: 0,
                last_some_state: None,
            }
        }

        fn total_in_queue(&self) -> u64 {
            (self.q.visible.is_some() as u64) + self.q.total_waiting() as u64
        }

        // Invariant 8, called at every detected promotion site.
        fn assert_expanded_at_promotion(&self, promoted_id: Option<Uuid>) {
            let Some(id) = promoted_id else { return };
            if let SlotState::Showing { expanded, .. } = self.q.current_slot_state() {
                assert!(
                    expanded,
                    "invariant 8: every promotion starts expanded (id={id:?})"
                );
            }
        }

        fn apply(&mut self, op: &Op) {
            match op {
                Op::Enqueue(spec) => self.apply_enqueue(spec),
                Op::Tick(secs) => self.apply_tick(*secs),
                Op::Dismiss => self.apply_dismiss(),
                Op::Skip => self.apply_skip(),
                Op::ToggleExpanded => self.q.toggle_expanded(),
                Op::Pause => self.q.pause(),
                Op::Resume => self.q.resume(),
            }
            self.check_blanket_invariants();
        }

        fn apply_enqueue(&mut self, spec: &EnqueueSpec) {
            let event = build_event(spec);
            let event_id = event.id;
            let tier = spec.priority as usize;
            let before_total = self.total_in_queue();

            let result = self.q.enqueue(event, self.now);

            let Ok(()) = result else {
                // Rejected (QueueFull): not part of the 9 documented
                // invariants (no I6-style rejection-untouched check here
                // by design — kept to the retargeted §9.1 list exactly).
                return;
            };
            let after_total = self.total_in_queue();
            if after_total <= before_total {
                // Merged into an existing item via topic supersede — not
                // a new item, invariant 2's cap never applies here.
                return;
            }
            // A genuinely new item was accepted.
            self.enqueued_accepted += 1;
            let promoted = current_vis_id(&self.q) == Some(event_id);
            if promoted {
                self.assert_expanded_at_promotion(Some(event_id));
            } else {
                assert_eq!(
                    self.q.waiting[tier].back().map(|i| i.event.id),
                    Some(event_id),
                    "a newly-accepted, non-promoted item must land at the back of its own tier"
                );
                assert!(
                    self.q.waiting[tier].len() <= self.max_queued_per_tier,
                    "invariant 2: per-tier waiting cap violated immediately after an Enqueue landing in waiting (tier={tier}, len={}, cap={})",
                    self.q.waiting[tier].len(),
                    self.max_queued_per_tier
                );
            }
        }

        fn apply_tick(&mut self, advance_secs: u64) {
            self.now += Duration::from_secs(advance_secs);
            let vis_before = vis_snapshot(&self.q);
            let mut waiting_before = snapshot_waiting(&self.q);
            let paused = self.q.is_paused();

            self.q.tick(self.now);

            let rotated = match &vis_before {
                Some(v) => self.now.duration_since(v.promoted_at).as_secs() >= v.window,
                None => false,
            };
            let after_id = current_vis_id(&self.q);

            if let Some(v) = &vis_before {
                if !rotated {
                    // invariant 3: no premature rotation.
                    assert_eq!(
                        after_id,
                        Some(v.id),
                        "invariant 3: visible item removed before its rotation window elapsed"
                    );
                    return;
                }
                if v.recurring {
                    waiting_before[v.tier].push((v.id, v.origin));
                } else {
                    self.rotated_out_dropped += 1;
                }
            }

            if paused {
                // invariant 5: a Tick while paused never promotes, even
                // though the item above may have just aged out.
                assert!(
                    after_id.is_none(),
                    "invariant 5: a Tick while paused must never promote"
                );
                return;
            }

            let predicted = predict_promoted(&waiting_before, &self.rotation_order);
            assert_eq!(
                after_id, predicted,
                "invariant 4: tick promotion did not pick the highest-tier, best-rotation_order-rank, FIFO-tie front"
            );
            self.assert_expanded_at_promotion(after_id);
        }

        fn apply_dismiss(&mut self) {
            let vis_before = vis_snapshot(&self.q);
            let waiting_before = snapshot_waiting(&self.q);
            let paused = self.q.is_paused();

            self.q.dismiss_visible(self.now);

            if vis_before.is_some() {
                // dismiss_visible always drops the visible item outright,
                // Recurring or OneShot alike (unlike Skip).
                self.dismissed += 1;
            }
            let after_id = current_vis_id(&self.q);
            if paused {
                assert!(
                    after_id.is_none(),
                    "invariant 5: Dismiss while paused must never promote"
                );
                return;
            }
            let predicted = predict_promoted(&waiting_before, &self.rotation_order);
            assert_eq!(
                after_id, predicted,
                "invariant 4: dismiss promotion did not pick the highest-tier, best-rotation_order-rank, FIFO-tie front"
            );
            self.assert_expanded_at_promotion(after_id);
        }

        fn apply_skip(&mut self) {
            let vis_before = vis_snapshot(&self.q);
            let mut waiting_before = snapshot_waiting(&self.q);
            let paused = self.q.is_paused();

            self.q.skip_visible(self.now);

            if let Some(v) = &vis_before {
                if v.recurring {
                    waiting_before[v.tier].push((v.id, v.origin));
                } else {
                    // only the OneShot arm of Skip is a drop (invariant 5).
                    self.skipped_oneshot_dropped += 1;
                }
            }
            let after_id = current_vis_id(&self.q);
            if paused {
                assert!(
                    after_id.is_none(),
                    "invariant 5: Skip while paused must never promote"
                );
                return;
            }
            let predicted = predict_promoted(&waiting_before, &self.rotation_order);
            assert_eq!(
                after_id, predicted,
                "invariant 4: skip promotion did not pick the highest-tier, best-rotation_order-rank, FIFO-tie front"
            );
            self.assert_expanded_at_promotion(after_id);
        }

        // Invariants 6(i), 9, 5/6-conservation, and 7 — cheap, always-sound
        // checks that don't depend on which op just ran.
        fn check_blanket_invariants(&mut self) {
            // invariant 6(i): a visible topic-supersede top-up never
            // exceeds the hard extension cap.
            if let Some(item) = &self.q.visible {
                assert!(
                    item.extension_secs <= MAX_EXTENSION_ON_SUPERSEDE_SECS,
                    "invariant 6(i): extension_secs {} exceeded the hard cap {}",
                    item.extension_secs,
                    MAX_EXTENSION_ON_SUPERSEDE_SECS
                );
            }

            // invariant 9: next_deadline, when Some, is exactly the earlier
            // of the armed auto-retract deadline (half the base window) and
            // the rotation deadline (promoted_at + window + extension).
            match self.q.next_deadline() {
                Some(deadline) => {
                    let item = self
                        .q
                        .visible
                        .as_ref()
                        .expect("next_deadline Some implies a visible item");
                    let promoted_at = item.promoted_at.expect("visible has promoted_at");
                    let rotation_deadline = promoted_at
                        + Duration::from_secs(
                            item.event.rotation_window(self.q.window_expanded)
                                + item.extension_secs,
                        );
                    let expected = if self.q.auto_retract_armed && self.q.expanded {
                        let retract_deadline = promoted_at
                            + Duration::from_secs(item.event.rotation_window(false)) / 2;
                        retract_deadline.min(rotation_deadline)
                    } else {
                        rotation_deadline
                    };
                    assert_eq!(deadline, expected, "invariant 9: next_deadline mismatch");
                }
                None => assert!(
                    self.q.visible.is_none(),
                    "invariant 9: next_deadline is None but a visible item exists"
                ),
            }

            // invariants 5/6 conservation.
            let total = self.total_in_queue();
            assert_eq!(
                self.enqueued_accepted,
                total + self.rotated_out_dropped + self.dismissed + self.skipped_oneshot_dropped,
                "invariant 5/6: enqueued-accepted count conservation violated"
            );

            // invariant 7: slot_state_if_changed never repeats a state.
            if let Some(state) = self.q.slot_state_if_changed() {
                if let Some(prev) = &self.last_some_state {
                    assert_ne!(
                        &state, prev,
                        "invariant 7: slot_state_if_changed returned the same state twice in a row"
                    );
                }
                self.last_some_state = Some(state);
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn queue_invariants_hold_under_any_op_script(
            max_queued_per_tier in 1usize..=10,
            rotation_order in arb_rotation_order(),
            ops in proptest::collection::vec(arb_op(), 0..50),
        ) {
            let mut harness = Harness::new(max_queued_per_tier, rotation_order);
            for op in &ops {
                harness.apply(op);
            }
        }
    }
}
