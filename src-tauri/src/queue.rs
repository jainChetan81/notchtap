//! The single-slot priority queue: one Visible item, three Waiting lines keyed by Priority, and the
//! rotation clock that moves items between them.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use serde::Serialize;
use uuid::Uuid;

use crate::error::QueueError;
use crate::event::{Event, Priority, RotationSpec, SlotState, SourceKind, EXPANDED_MULTIPLIER};

pub struct QueueItem {
    pub event: Event,
    pub enqueued_at: Instant,
    pub promoted_at: Option<Instant>,
    pub extension_secs: u64,
    /// `None` for every item that has never been preempted — the overwhelmingly common case.
    pub preempted_remaining_secs: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QueueItemSummary {
    pub title: String,
    pub priority: String,
    pub source: String,
}

fn priority_tier_label(priority: Priority) -> String {
    match priority {
        Priority::Low => "low",
        Priority::Medium => "medium",
        Priority::High => "high",
    }
    .to_string()
}

fn source_kind_label(source: SourceKind) -> String {
    match source {
        SourceKind::Football => "football",
        SourceKind::News => "news",
        SourceKind::Manual => "manual",
        SourceKind::Agent => "agent",
    }
    .to_string()
}

/// A single-slot, priority-ordered notification queue with bounded per-tier waiting, ttl/recur
/// rotation, pause/resume gating, and supersession by topic.
pub struct SingleSlotQueue {
    visible: Option<QueueItem>,
    waiting: [VecDeque<QueueItem>; 3],
    max_queued_per_tier: usize,
    paused: bool,
    silenced: bool,
    /// Never consulted by rotation arithmetic.
    expanded: bool,
    window_expanded: bool,
    auto_retract_armed: bool,
    batch_total: usize,
    batch_done: usize,
    last_emitted: Option<SlotState>,
    rotation_order: Vec<SourceKind>,
    /// Both reset to their empty state at every promotion (`set_expanded_for_promotion`), same as
    /// `expanded`/ `window_expanded`/`auto_retract_armed`.
    hover_started_at: Option<Instant>,
    hover_paused_total: Duration,
    ttl_sample: Option<TtlEmissionSample>,
}

struct TtlEmissionSample {
    item_id: Uuid,
    ttl_ms: u64,
    remaining_ms: u64,
    emitted_at: Instant,
    hover_held_at_sample: Duration,
}

impl SingleSlotQueue {
    pub fn new(max_queued_per_tier: usize) -> Self {
        Self {
            visible: None,
            waiting: [VecDeque::new(), VecDeque::new(), VecDeque::new()],
            max_queued_per_tier,
            paused: false,
            silenced: false,
            expanded: false,
            window_expanded: false,
            auto_retract_armed: false,
            batch_total: 0,
            batch_done: 0,
            last_emitted: None,
            rotation_order: Vec::new(),
            hover_started_at: None,
            hover_paused_total: Duration::ZERO,
            ttl_sample: None,
        }
    }

    /// Builder: sets the same-tier tie-break order.
    pub fn with_rotation_order(mut self, rotation_order: Vec<SourceKind>) -> Self {
        self.rotation_order = rotation_order;
        self
    }

    pub fn enqueue(&mut self, event: Event, now: Instant) -> Result<(), QueueError> {
        self.enqueue_with_options(event, now, false)
    }

    /// Allows a test notification into an empty slot despite pause or silence.
    pub fn enqueue_test(&mut self, event: Event, now: Instant) -> Result<(), QueueError> {
        self.enqueue_with_options(event, now, true)
    }

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
        // a strictly-higher-priority arrival cuts the current Visible item's turn short and takes
        // the Slot immediately.
        if let Some(preempted) = self.try_preempt_visible(event.priority, now) {
            let priority = event.priority;
            self.batch_total += 1;
            let preempted_tier = preempted.event.priority as usize;
            self.waiting[preempted_tier].push_front(preempted);
            let item = QueueItem {
                event,
                enqueued_at: now,
                promoted_at: Some(now),
                extension_secs: 0,
                preempted_remaining_secs: None,
            };
            self.set_expanded_for_promotion(priority);
            self.visible = Some(item);
            return Ok(());
        }

        let tier = event.priority as usize;
        let can_promote_now = self.visible.is_none()
            && (bypass_pause_when_slot_empty || !self.paused)
            && (bypass_pause_when_slot_empty || !self.silenced || event.priority == Priority::High)
            && self.all_tiers_empty();
        if !can_promote_now && self.waiting[tier].len() >= self.max_queued_per_tier {
            return Err(QueueError::QueueFull);
        }
        // (Draining back to idle re-zeroes them too, so this is belt-and-braces for the exact
        // start semantics.)
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
            preempted_remaining_secs: None,
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

    /// The Silenced check cannot be skipped on only-High-can-be-Visible grounds: entering silence
    /// does not evict the current card (it finishes its natural turn, same as Paused).
    fn try_preempt_visible(
        &mut self,
        incoming_priority: Priority,
        now: Instant,
    ) -> Option<QueueItem> {
        if self.paused {
            return None;
        }
        if self.silenced && incoming_priority != Priority::High {
            return None;
        }
        let visible_priority = self.visible.as_ref()?.event.priority;
        if incoming_priority <= visible_priority {
            return None;
        }
        let mut item = self.visible.take().expect("checked Some above");
        let elapsed = item
            .promoted_at
            .map(|promoted_at| {
                self.hover_frozen_rotation_elapsed(promoted_at, now)
                    .as_secs()
            })
            .unwrap_or(0);
        let window = self.full_window_secs(&item);
        item.preempted_remaining_secs = Some(window.saturating_sub(elapsed));
        item.promoted_at = None;
        item.extension_secs = 0;
        self.hover_started_at = None;
        self.hover_paused_total = Duration::ZERO;
        Some(item)
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
            } else if self.waiting[new_tier_idx].len() >= self.max_queued_per_tier {
                return true;
            } else if let Some(mut existing) = self.waiting[tier_idx].remove(pos) {
                apply_fresh_content(&mut existing.event, fresh);
                self.waiting[new_tier_idx].push_back(existing);
            }
            return true;
        }
        false
    }

    pub fn tick(&mut self, now: Instant) {
        self.retract_if_elapsed(now);
        self.rotate_out_if_elapsed(now);
        self.promote_next(now);
        self.reset_batch_if_idle();
    }

    /// Runs before rotate/promote so a retract and a rotation due at the same instant
    /// collapse-then-rotate in one tick.
    fn retract_if_elapsed(&mut self, now: Instant) {
        if !(self.auto_retract_armed && self.expanded) {
            return;
        }
        let Some(item) = &self.visible else { return };
        let Some(promoted_at) = item.promoted_at else {
            return;
        };
        let retract_after = Duration::from_secs(self.base_window_secs(item)) / 2;
        if now.saturating_duration_since(promoted_at) < retract_after {
            return;
        }
        self.expanded = false;
        self.auto_retract_armed = false;
    }

    fn rotate_out_if_elapsed(&mut self, now: Instant) {
        // a card must NEVER rotate out while under the cursor.
        if self.hover_started_at.is_some() {
            return;
        }
        let Some(item) = &self.visible else { return };
        // Defensive, mirroring `current_slot_state`'s posture for the same invariant: every real
        // promotion path sets `promoted_at`.
        let Some(promoted_at) = item.promoted_at else {
            tracing::warn!("visible item missing promoted_at — rotation check skipped");
            return;
        };
        let window = self.full_window_secs(item);
        let elapsed = self.hover_frozen_rotation_elapsed(promoted_at, now);
        if elapsed.as_secs() < window {
            return;
        }
        let mut item = self.visible.take().expect("checked Some above");
        if let RotationSpec::Recurring { .. } = item.event.rotation {
            item.preempted_remaining_secs = None;
            let tier = item.event.priority as usize;
            self.waiting[tier].push_back(item);
        } else {
            self.batch_done += 1;
        }
    }

    fn hover_frozen_rotation_elapsed(&self, promoted_at: Instant, now: Instant) -> Duration {
        let raw_elapsed = now.saturating_duration_since(promoted_at);
        let in_flight = match self.hover_started_at {
            Some(started) => now.saturating_duration_since(started),
            None => Duration::ZERO,
        };
        raw_elapsed.saturating_sub(self.hover_paused_total + in_flight)
    }

    fn hover_adjusted_promoted_at(&self, promoted_at: Instant) -> Instant {
        promoted_at + self.hover_paused_total
    }

    pub fn hover_enter(&mut self, now: Instant) {
        if self.visible.is_none() {
            return;
        }
        if self.hover_started_at.is_none() {
            self.hover_started_at = Some(now);
        }
    }

    /// The mirror-image transition.
    pub fn hover_exit(&mut self, now: Instant) {
        if let Some(started) = self.hover_started_at.take() {
            self.hover_paused_total += now.saturating_duration_since(started);
        }
    }

    fn cumulative_hover_held(&self, now: Instant) -> Duration {
        let in_flight = match self.hover_started_at {
            Some(started) => now.saturating_duration_since(started),
            None => Duration::ZERO,
        };
        self.hover_paused_total + in_flight
    }

    fn promote_next(&mut self, now: Instant) {
        if self.visible.is_some() || self.paused {
            return;
        }
        if let Some(mut item) = self.pop_highest_priority_waiting() {
            item.promoted_at = Some(now);
            item.extension_secs = 0;
            let priority = item.event.priority;
            self.set_expanded_for_promotion(priority);
            self.visible = Some(item);
        }
    }

    // A leftover manual expand/window can never leak onto the next item, and the hover-hold fields
    // reset here for the same reason.
    fn set_expanded_for_promotion(&mut self, priority: Priority) {
        let compact = priority == Priority::Low || self.silenced;
        self.expanded = !compact;
        self.window_expanded = false;
        self.auto_retract_armed = !compact;
        self.hover_started_at = None;
        self.hover_paused_total = Duration::ZERO;
    }

    fn pop_highest_priority_waiting(&mut self) -> Option<QueueItem> {
        let min_tier = if self.silenced {
            Priority::High as usize
        } else {
            0
        };
        for tier in (min_tier..3).rev() {
            if self.waiting[tier].is_empty() {
                continue;
            }
            let index = self.best_index_in_tier(tier);
            return self.waiting[tier].remove(index);
        }
        None
    }

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

    fn base_window_secs(&self, item: &QueueItem) -> u64 {
        item.preempted_remaining_secs
            .unwrap_or_else(|| item.event.rotation_window(false))
    }

    /// For an item that was never preempted this equals
    /// `item.event.rotation_window(self.window_expanded) + item.extension_secs`.
    fn full_window_secs(&self, item: &QueueItem) -> u64 {
        let base = self.base_window_secs(item);
        let window = if self.window_expanded {
            base.saturating_mul(EXPANDED_MULTIPLIER)
        } else {
            base
        };
        window + item.extension_secs
    }

    fn top_up_visible_remaining_time(&mut self, now: Instant) {
        let Some(promoted_at) = self.visible.as_ref().and_then(|i| i.promoted_at) else {
            return;
        };
        // the top-up's notion of "remaining" must match the real rotation deadline, which
        // discounts banked and in-flight hover time (`hover_adjusted_promoted_at`).
        let elapsed = self
            .hover_frozen_rotation_elapsed(promoted_at, now)
            .as_secs();
        let effective_window = match self.visible.as_ref() {
            Some(item) => self.full_window_secs(item),
            None => return,
        };
        let Some(item) = &mut self.visible else {
            return;
        };
        let remaining = effective_window.saturating_sub(elapsed);
        if remaining < MIN_REMAINING_ON_SUPERSEDE_SECS {
            let deficit = MIN_REMAINING_ON_SUPERSEDE_SECS - remaining;
            let room = MAX_EXTENSION_ON_SUPERSEDE_SECS.saturating_sub(item.extension_secs);
            item.extension_secs += deficit.min(room);
        }
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

    pub fn silence(&mut self) {
        self.silenced = true;
    }

    /// Leaves Silenced — normal (every-tier) promotion resumes immediately, same relationship
    /// `resume()` has to `pause()`.
    pub fn unsilence(&mut self) {
        self.silenced = false;
    }

    pub fn is_silenced(&self) -> bool {
        self.silenced
    }

    /// Manually dismiss the current visible item, if any, and promote the next waiting item
    /// immediately — mirrors what `tick` does on natural rotation-out.
    pub fn dismiss_visible(&mut self, now: Instant) {
        if self.visible.take().is_some() {
            self.batch_done += 1;
        }
        self.promote_next(now);
        self.reset_batch_if_idle();
    }

    /// Skip the Visible item: end its turn now, exactly as if its Rotation window had elapsed
    /// naturally — a Recurring item requeues to the back of its own Priority tier's Waiting line.
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
        self.reanchor_wire_if_skip_repromoted_the_last_emitted_item();
    }

    fn reanchor_wire_if_skip_repromoted_the_last_emitted_item(&mut self) {
        let Some(SlotState::Showing { id: last_id, .. }) = self.last_emitted.as_ref() else {
            return;
        };
        let Some(visible) = self.visible.as_ref() else {
            return;
        };
        if visible.event.id == *last_id {
            self.last_emitted = None;
            self.ttl_sample = None;
        }
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
        self.auto_retract_armed = false;
        self.expanded = !self.expanded;
        if self.expanded {
            self.window_expanded = true;
        }
    }

    pub fn total_waiting(&self) -> usize {
        self.waiting.iter().map(|t| t.len()).sum()
    }

    /// Read-only summary of every WAITING item (never `visible`) for the settings window's Queue
    /// section.
    pub fn waiting_summaries(&self) -> Vec<QueueItemSummary> {
        let mut out = Vec::with_capacity(self.total_waiting());
        for tier in (0..3).rev() {
            for item in &self.waiting[tier] {
                out.push(QueueItemSummary {
                    title: item.event.payload.title.clone(),
                    priority: priority_tier_label(item.event.priority),
                    source: source_kind_label(item.event.origin),
                });
            }
        }
        out
    }

    /// Clears waiting tiers while preserving batch progress for any visible item.
    pub fn clear_waiting(&mut self) -> usize {
        let dropped = self.total_waiting();
        for tier in self.waiting.iter_mut() {
            tier.clear();
        }
        self.batch_total = self.batch_done + usize::from(self.visible.is_some());
        self.reset_batch_if_idle();
        dropped
    }

    /// Checked after every mutation that can drain the engine (tick, dismiss, skip); an accepted
    /// enqueue can never *reach* idle, so its batch-start zeroing lives in `enqueue_new` instead.
    fn reset_batch_if_idle(&mut self) {
        if self.visible.is_none() && self.all_tiers_empty() {
            self.batch_total = 0;
            self.batch_done = 0;
        }
    }

    /// The next Instant at which time alone changes state: the earlier of the visible item's
    /// auto-retract deadline (half the base window, while armed) and its rotation deadline.
    pub fn next_deadline(&self) -> Option<Instant> {
        let item = self.visible.as_ref()?;
        let promoted_at = item.promoted_at?;
        let retract_deadline = if self.auto_retract_armed && self.expanded {
            Some(promoted_at + Duration::from_secs(self.base_window_secs(item)) / 2)
        } else {
            None
        };
        let rotation_deadline = if self.hover_started_at.is_some() {
            None
        } else {
            let window = self.full_window_secs(item);
            let anchor = self.hover_adjusted_promoted_at(promoted_at);
            Some(anchor + Duration::from_secs(window))
        };
        match (retract_deadline, rotation_deadline) {
            (Some(r), Some(t)) => Some(r.min(t)),
            (Some(d), None) | (None, Some(d)) => Some(d),
            (None, None) => None,
        }
    }

    // The derived equality would compare `remaining_ms` too, which is a pure function of
    // `Instant::now()` and so is never stable between two calls even milliseconds apart.
    pub fn slot_state_if_changed(&mut self) -> Option<SlotState> {
        let current = self.current_slot_state();
        let changed = match self.last_emitted.as_ref() {
            Some(last) => !last.dedup_eq(&current),
            None => true,
        };
        if changed {
            self.last_emitted = Some(current.clone());
            self.observe_emission_for_ttl_restart(&current, Instant::now());
            Some(current)
        } else {
            None
        }
    }

    /// the webview-reload re-emit's own door into `current_slot_state` — identical output, but ALSO
    /// feeds the TTL-restart sampler.
    pub fn current_slot_state_for_emission(&mut self, now: Instant) -> SlotState {
        let current = self.current_slot_state();
        self.observe_emission_for_ttl_restart(&current, now);
        current
    }

    fn observe_emission_for_ttl_restart(&mut self, state: &SlotState, now: Instant) -> bool {
        let SlotState::Showing {
            id,
            ttl_ms,
            remaining_ms,
            ..
        } = state
        else {
            self.ttl_sample = None;
            return false;
        };
        let hover_held_now = self.cumulative_hover_held(now);
        let restarted = match self.ttl_sample.as_ref() {
            Some(prev) if prev.item_id == *id => {
                let elapsed_ms =
                    u64::try_from(now.saturating_duration_since(prev.emitted_at).as_millis())
                        .unwrap_or(u64::MAX);
                let hover_held_delta_ms = u64::try_from(
                    hover_held_now
                        .saturating_sub(prev.hover_held_at_sample)
                        .as_millis(),
                )
                .unwrap_or(u64::MAX);
                let is_restart = is_ttl_restart(
                    prev.ttl_ms,
                    prev.remaining_ms,
                    elapsed_ms,
                    hover_held_delta_ms,
                    *ttl_ms,
                    *remaining_ms,
                    TTL_RESTART_SLACK_MS,
                );
                if is_restart {
                    tracing::warn!(
                        item_id = %id,
                        prev_remaining_ms = prev.remaining_ms,
                        new_remaining_ms = remaining_ms,
                        elapsed_ms,
                        hover_held_delta_ms,
                        "ttl-restart: remaining_ms jumped further than elapsed real time \
                         (minus hover-held time) can explain on the backend wire — see \
                         queue.rs::is_ttl_restart. A visible restart with no matching warning \
                         here points at the frontend instead."
                    );
                }
                is_restart
            }
            _ => false,
        };
        self.ttl_sample = Some(TtlEmissionSample {
            item_id: *id,
            ttl_ms: *ttl_ms,
            remaining_ms: *remaining_ms,
            emitted_at: now,
            hover_held_at_sample: hover_held_now,
        });
        restarted
    }

    pub fn current_slot_state(&self) -> SlotState {
        match &self.visible {
            None => SlotState::Empty,
            Some(item) => {
                let queue_total = u32::try_from(self.batch_total.max(1)).unwrap_or(u32::MAX);
                let queue_done = u32::try_from(self.batch_done)
                    .unwrap_or(u32::MAX)
                    .min(queue_total - 1);

                let window_secs = self.full_window_secs(item);
                let ttl_ms = window_secs.saturating_mul(1000);
                let remaining_ms = match item.promoted_at {
                    Some(promoted_at) => {
                        let anchor = self.hover_adjusted_promoted_at(promoted_at);
                        let deadline = anchor + Duration::from_secs(window_secs);
                        let reference_now = self.hover_started_at.unwrap_or_else(Instant::now);
                        let remaining = deadline.saturating_duration_since(reference_now);
                        u64::try_from(remaining.as_millis()).unwrap_or(u64::MAX)
                    }
                    None => ttl_ms,
                };

                SlotState::Showing {
                    id: item.event.id,
                    title: item.event.payload.title.clone(),
                    body: item.event.payload.body.clone(),
                    event_type: item.event.event_type.clone(),
                    priority: item.event.priority,
                    signal: item.event.signal,
                    origin: item.event.origin,
                    expanded: self.expanded,
                    source: item.event.meta.source.clone(),
                    category: item.event.meta.category.clone(),
                    published_at_ms: item.event.meta.published_at_ms,
                    link: item.event.meta.link.clone(),
                    subtitle: item.event.meta.subtitle.clone(),
                    details: item.event.meta.details.clone(),
                    queue_total,
                    queue_done,
                    ttl_ms,
                    remaining_ms,
                    espn: item.event.meta.espn.clone(),
                    agent_runtime: item.event.meta.agent.as_ref().map(|a| a.runtime.clone()),
                }
            }
        }
    }
}

fn apply_fresh_content(existing: &mut Event, fresh: &Event) {
    existing.payload = fresh.payload.clone();
    existing.priority = fresh.priority;
    existing.rotation = fresh.rotation;
    existing.signal = fresh.signal;
    existing.meta = fresh.meta.clone();
}

const MIN_REMAINING_ON_SUPERSEDE_SECS: u64 = 2;
const MAX_EXTENSION_ON_SUPERSEDE_SECS: u64 = 6;

/// scheduling-jitter slack for the TTL-restart detector (`is_ttl_restart`) — real wake-ups never
/// land on the exact millisecond.
const TTL_RESTART_SLACK_MS: u64 = 500;

/// Same pure-decision/boundary split this codebase already uses for
/// `presentation::presentation_mode` vs its subprocess-calling wrapper.
fn is_ttl_restart(
    prev_ttl_ms: u64,
    prev_remaining_ms: u64,
    elapsed_since_prev_emission_ms: u64,
    hover_held_delta_ms: u64,
    new_ttl_ms: u64,
    new_remaining_ms: u64,
    slack_ms: u64,
) -> bool {
    if new_ttl_ms != prev_ttl_ms {
        return false;
    }
    let active_elapsed_ms = elapsed_since_prev_emission_ms.saturating_sub(hover_held_delta_ms);
    let expected_remaining_ms = prev_remaining_ms.saturating_sub(active_elapsed_ms);
    new_remaining_ms > expected_remaining_ms.saturating_add(slack_ms)
}

#[cfg(test)]
mod tests;

// Supplements `mod tests` above; never replaces it.
#[cfg(test)]
mod proptest_queue;
