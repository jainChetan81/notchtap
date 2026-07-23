use std::collections::VecDeque;
use std::time::{Duration, Instant};

use serde::Serialize;
use uuid::Uuid;

use crate::error::QueueError;
use crate::event::{Event, Priority, RotationSpec, SlotState, SourceKind};

pub struct QueueItem {
    pub event: Event,
    pub enqueued_at: Instant,
    pub promoted_at: Option<Instant>,
    pub extension_secs: u64,
}

/// Read-only wire summary of a single WAITING item — the settings
/// window's Queue section (plan 121). Deliberately a projection, not
/// `QueueItem` itself: `QueueItem` carries `Instant`s (not serializable)
/// and internal bookkeeping the settings window has no business seeing.
/// `priority`/`source` are plain lowercase strings rather than the
/// `Priority`/`SourceKind` enums directly — see `priority_tier_label`/
/// `source_kind_label` just below.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QueueItemSummary {
    pub title: String,
    pub priority: String,
    pub source: String,
}

/// `Priority` already derives `Serialize` (`rename_all = "snake_case"`),
/// but `QueueItemSummary.priority` is a plain `String` field (plan 121),
/// not the enum itself — an explicit, exhaustive match keeps this in
/// lockstep with `Priority`'s own wire spelling without round-tripping
/// through `serde_json` for a three-variant enum. Exhaustive on purpose:
/// a future `Priority` variant fails to compile here until labeled.
fn priority_tier_label(priority: Priority) -> String {
    match priority {
        Priority::Low => "low",
        Priority::Medium => "medium",
        Priority::High => "high",
    }
    .to_string()
}

/// Same rationale as `priority_tier_label`, for `SourceKind` (event.rs's
/// `origin` field is the "source-ish field" `QueueItemSummary.source`
/// derives from — plan 121 step 1).
fn source_kind_label(source: SourceKind) -> String {
    match source {
        SourceKind::Football => "football",
        SourceKind::News => "news",
        SourceKind::Manual => "manual",
        SourceKind::Cmux => "cmux",
        SourceKind::Weather => "weather",
    }
    .to_string()
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
    /// plan 093: TTL hover-pause. `Some(t)` while the visible item is
    /// currently under the cursor (`t` is when the CURRENT hover session
    /// started); `hover_paused_total` is the cumulative real wall-clock
    /// duration banked from every PAST hover session on this same visible
    /// item. Both reset to their empty state at every promotion
    /// (`set_expanded_for_promotion`), same as `expanded`/
    /// `window_expanded`/`auto_retract_armed` — a hover session can never
    /// leak from one visible item onto the next.
    ///
    /// The mechanism (see `hover_frozen_rotation_elapsed`): rather than
    /// mutating `promoted_at` directly on every hover-enter (which would
    /// need to know the FUTURE exit time up front), elapsed time for
    /// ROTATION purposes only is computed as `real_elapsed -
    /// hover_paused_total - (currently hovering ? real_elapsed_since_
    /// hover_started_at : 0)` — the in-flight subtraction exactly cancels
    /// the passage of time while a hover session is open (frozen), and
    /// gets permanently banked into `hover_paused_total` the moment the
    /// session ends (`hover_exit`). This guarantees the two design
    /// invariants pinned by this plan's property tests: (1) a card can
    /// never rotate out while `hover_started_at` is `Some` (rotation
    /// elapsed time cannot advance while frozen), and (2) no number of
    /// hover cycles can ever grant MORE total active (non-paused) time
    /// than `rotation_window(...) + extension_secs` already allowed — each
    /// cycle only pauses, it never adds.
    ///
    /// Deliberately scoped to the ROTATION deadline only — the auto-
    /// retract (expand→collapse) deadline is intentionally NOT frozen by
    /// this mechanism (`retract_if_elapsed` is untouched); the plan's own
    /// design constraint names only "the rotation deadline holds."
    hover_started_at: Option<Instant>,
    hover_paused_total: Duration,
    /// plan 107 Step C: the ONE queue-owned sample the TTL-restart
    /// detector compares each new wire-emission attempt against — see
    /// `TtlEmissionSample`'s own doc and `observe_emission_for_ttl_restart`
    /// for the full mechanism. Deliberately adjacent to `last_emitted`
    /// (the same "one piece of queue-owned state tracking the last thing
    /// that went out over the wire" shape), not a second/engine-side
    /// tracking structure.
    ttl_sample: Option<TtlEmissionSample>,
}

/// plan 107 Step C: a snapshot of the last observed slot-state wire
/// emission, kept purely so the NEXT emission for the same item can be
/// checked for a TTL restart (`is_ttl_restart`, below `SingleSlotQueue`'s
/// impl block) instead of a raw "did it get bigger" comparison, which
/// misses the canonical restart pattern (see that function's doc).
/// `hover_held_at_sample` is the CUMULATIVE hover-held total as of this
/// sample (`SingleSlotQueue::cumulative_hover_held`), not a delta — the
/// next observation subtracts this from ITS OWN cumulative total to
/// recover just the hover-held time that elapsed between the two samples.
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
            } else if self.waiting[new_tier_idx].len() >= self.max_queued_per_tier {
                // destination tier is full — per plan 072's decision, drop
                // the fresh content and leave the item in its current tier
                // rather than evicting something to make room. This
                // function returns `bool` (whether a Topic match was found
                // and handled), not a `Result`, so there's no error
                // channel to report the drop through today — the caller
                // only cares "did I find and handle this Topic." Leave a
                // comment explaining this rather than silently changing
                // the return contract.
                return true;
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
        // plan 093 design constraint: a card must NEVER rotate out while
        // under the cursor. `hover_started_at.is_some()` means a hover
        // session is currently open on the visible item — elapsed time is
        // frozen for the whole tick, so there is nothing to check.
        if self.hover_started_at.is_some() {
            return;
        }
        let Some(item) = &self.visible else { return };
        let promoted_at = item.promoted_at.expect("visible items have promoted_at");
        let window = item.event.rotation_window(self.window_expanded) + item.extension_secs;
        let elapsed = self.hover_frozen_rotation_elapsed(promoted_at, now);
        if elapsed.as_secs() < window {
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

    // ------------------------------------------------------------------
    // hover hold (plan 093) — see the `hover_started_at`/
    // `hover_paused_total` field doc comments for the full mechanism.
    // ------------------------------------------------------------------

    /// Rotation-purposes-only elapsed time since `promoted_at`, with every
    /// past AND (if currently open) in-flight hover session subtracted
    /// out. `saturating_sub` because a session that hasn't banked yet can
    /// make the subtrahend momentarily exceed the raw elapsed time by a
    /// few nanoseconds' rounding at the instant hover starts — never
    /// meaningfully, but `Duration` cannot go negative, so this is the
    /// honest guard rather than a `debug_assert` that could panic on a
    /// clock artifact.
    fn hover_frozen_rotation_elapsed(&self, promoted_at: Instant, now: Instant) -> Duration {
        let raw_elapsed = now.saturating_duration_since(promoted_at);
        let in_flight = match self.hover_started_at {
            Some(started) => now.saturating_duration_since(started),
            None => Duration::ZERO,
        };
        raw_elapsed.saturating_sub(self.hover_paused_total + in_flight)
    }

    /// Rotation deadline anchor with every banked (already-ended) hover
    /// session's duration added back — the "re-anchors with the remaining
    /// time it had at entry" half of the design constraint. Deliberately
    /// does NOT account for an in-flight session (unlike
    /// `hover_frozen_rotation_elapsed`) — callers that care about "is a
    /// session open right now" (`next_deadline`) check
    /// `hover_started_at` separately.
    fn hover_adjusted_promoted_at(&self, promoted_at: Instant) -> Instant {
        promoted_at + self.hover_paused_total
    }

    /// Called from every tracking-area transition into "hovering the
    /// visible card" (`lib.rs`'s `emit_hover_changed_if_transitioned`,
    /// gated there to fire only on the boolean's actual flip). A no-op if
    /// nothing is visible (nothing to hold) or a session is already open
    /// (idempotent — the transitions-only gate at the call site should
    /// already prevent a double-enter, but this stays defensive rather
    /// than trusting that from a distance).
    pub fn hover_enter(&mut self, now: Instant) {
        if self.visible.is_none() {
            return;
        }
        if self.hover_started_at.is_none() {
            self.hover_started_at = Some(now);
        }
    }

    /// The mirror-image transition. Banks the just-ended session's real
    /// duration into `hover_paused_total` — permanently, so it keeps
    /// shifting the rotation deadline forward even after this call
    /// returns, without needing to remember individual past sessions.
    pub fn hover_exit(&mut self, now: Instant) {
        if let Some(started) = self.hover_started_at.take() {
            self.hover_paused_total += now.saturating_duration_since(started);
        }
    }

    /// plan 107 Step C: cumulative hover-held time as of `now` — every
    /// banked past session (`hover_paused_total`) plus the in-flight one
    /// if a session is currently open. Same in-flight computation as
    /// `hover_frozen_rotation_elapsed` above, but returns the total
    /// itself rather than subtracting it from a rotation elapsed value —
    /// the TTL-restart detector (`observe_emission_for_ttl_restart`)
    /// needs the raw cumulative total so it can diff two samples and
    /// recover just the hover-held DELTA between them.
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
    //
    // plan 093: the hover-hold fields reset here too, for the same
    // reason — a hover session (or banked pause total) from the PREVIOUS
    // visible item must never leak onto the new one's rotation deadline.
    fn set_expanded_for_promotion(&mut self) {
        self.expanded = true;
        self.window_expanded = false;
        self.auto_retract_armed = true;
        self.hover_started_at = None;
        self.hover_paused_total = Duration::ZERO;
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
        let Some(promoted_at) = self.visible.as_ref().and_then(|i| i.promoted_at) else {
            return;
        };
        // plan 097: the top-up's notion of "remaining" must match the real
        // rotation deadline, which discounts banked and in-flight hover
        // time (`hover_adjusted_promoted_at` / plan 093) — every OTHER
        // deadline consumer (`next_deadline`, `remaining_ms`) already
        // anchors there. Using raw `now - promoted_at` here over-granted
        // extensions to previously-hovered cards: a card that had banked
        // hover time looked closer to expiry than it actually was.
        // `hover_frozen_rotation_elapsed` takes `&self`, so it must be
        // called before the `&mut self.visible` borrow below.
        let elapsed = self
            .hover_frozen_rotation_elapsed(promoted_at, now)
            .as_secs();
        let Some(item) = &mut self.visible else {
            return;
        };
        let base_window = item.event.rotation_window(self.window_expanded);
        let effective_window = base_window + item.extension_secs;
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

    /// Read-only summary of every WAITING item (never `visible`) for the
    /// settings window's Queue section (plan 121). Ordered tiers
    /// high -> normal -> low — the same order `pop_highest_priority_waiting`
    /// promotes from — then FIFO (arrival order) within each tier: display
    /// order, not `rotation_order`-aware pick order, since every waiting
    /// item is shown at once rather than picked one at a time.
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

    /// Drops every WAITING item across all three tiers — the settings
    /// window's "Clear queue" action (plan 121). The visible card is
    /// untouched; it finishes its normal ttl/rotation. Returns the count
    /// dropped.
    ///
    /// `batch_total` is recomputed rather than left stale: the invariant
    /// `current_slot_state` depends on ("done never reaches total while an
    /// item is visible") is preserved by pinning `batch_total` to
    /// `batch_done` plus one more if something is still visible — mirrors
    /// `reset_batch_if_idle`'s reasoning (drained -> counters reflect
    /// "nothing left to do") one step further: drained-of-WAITING, with a
    /// visible item still mid-turn, reads as "on the last segment" rather
    /// than stalling at whatever `batch_total` happened to be before the
    /// clear. `reset_batch_if_idle` still runs after, for the fully-idle
    /// case (nothing visible either) — it zeroes both counters, superseding
    /// the pin below.
    pub fn clear_waiting(&mut self) -> usize {
        let dropped = self.total_waiting();
        for tier in self.waiting.iter_mut() {
            tier.clear();
        }
        self.batch_total = self.batch_done + usize::from(self.visible.is_some());
        self.reset_batch_if_idle();
        dropped
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
    ///
    /// plan 093: the ROTATION half is also `None` while a hover session is
    /// open (`hover_started_at.is_some()`) — nothing about rotation
    /// elapsed time changes purely from time passing while frozen, so
    /// there is genuinely nothing to schedule a wake for; the eventual
    /// un-freeze is `hover_exit`'s own `apply_blocking` call waking the
    /// loop directly (the existing mutate→wake→emit protocol), not a
    /// timer this method could predict in advance. The auto-retract half
    /// is untouched by hovering (see the `hover_started_at` field doc for
    /// why only rotation freezes), so it can still be the earlier/only
    /// deadline even while a hover session holds rotation open.
    pub fn next_deadline(&self) -> Option<Instant> {
        let item = self.visible.as_ref()?;
        let promoted_at = item.promoted_at?;
        let retract_deadline = if self.auto_retract_armed && self.expanded {
            Some(promoted_at + Duration::from_secs(item.event.rotation_window(false)) / 2)
        } else {
            None
        };
        let rotation_deadline = if self.hover_started_at.is_some() {
            None
        } else {
            let window = item.event.rotation_window(self.window_expanded) + item.extension_secs;
            let anchor = self.hover_adjusted_promoted_at(promoted_at);
            Some(anchor + Duration::from_secs(window))
        };
        match (retract_deadline, rotation_deadline) {
            (Some(r), Some(t)) => Some(r.min(t)),
            (Some(d), None) | (None, Some(d)) => Some(d),
            (None, None) => None,
        }
    }

    // ------------------------------------------------------------------
    // slot-state emission helpers
    // ------------------------------------------------------------------

    /// Emits only when the state has meaningfully changed, per
    /// `SlotState::dedup_eq` (plan 081) — NOT the derived `PartialEq`. The
    /// derived equality would compare `remaining_ms` too, which is a pure
    /// function of `Instant::now()` and so is never stable between two
    /// calls even milliseconds apart; using it here reintroduces plan 081
    /// attempt 1's double-emission bug (the rotation loop's post-wake
    /// recheck always seeing "changed"). See `SlotState::dedup_eq`'s doc
    /// for the full mechanism.
    pub fn slot_state_if_changed(&mut self) -> Option<SlotState> {
        let current = self.current_slot_state();
        let changed = match self.last_emitted.as_ref() {
            Some(last) => !last.dedup_eq(&current),
            None => true,
        };
        if changed {
            self.last_emitted = Some(current.clone());
            // plan 107 Step C: every gated emission feeds the TTL-restart
            // sampler too — see `observe_emission_for_ttl_restart`'s doc
            // for why this needs to cover BOTH this path and the
            // unconditional one (`current_slot_state_for_emission`).
            self.observe_emission_for_ttl_restart(&current, Instant::now());
            Some(current)
        } else {
            None
        }
    }

    /// plan 107 Step C: the webview-reload re-emit's own door into
    /// `current_slot_state` — identical output, but ALSO feeds the
    /// TTL-restart sampler, so the one wire-emission route that bypasses
    /// `slot_state_if_changed`'s dedup gate entirely
    /// (`Engine::emit_current_blocking`, the on-page-load unconditional
    /// re-emit) still participates in detection instead of silently
    /// falling outside it. `now` is an explicit parameter (unlike
    /// `current_slot_state` itself, which is unaffected — see that
    /// method's own real-time `remaining_ms` computation) purely so tests
    /// can drive the sampler deterministically.
    pub fn current_slot_state_for_emission(&mut self, now: Instant) -> SlotState {
        let current = self.current_slot_state();
        self.observe_emission_for_ttl_restart(&current, now);
        current
    }

    /// plan 107 Step C: the boundary half of the TTL-restart detector —
    /// the decision itself is the pure `is_ttl_restart` function below
    /// (module scope, after this `impl` block); this half gathers the
    /// queue-owned state (the previous sample, hover accounting) the
    /// decision needs and performs the one real side effect
    /// (`tracing::warn!`). Same pure-decision/boundary split this
    /// codebase already uses for `presentation::presentation_mode` vs its
    /// subprocess-calling wrapper: one half is a plain function over
    /// plain values, unit-testable with zero timing dependencies; this
    /// half reads `Instant`-derived queue state and has a real side
    /// effect, so it isn't.
    ///
    /// Called from every attempted wire-emission site (`slot_state_if_
    /// changed`'s change-gated route AND `current_slot_state_for_
    /// emission`'s unconditional one), so a restart shows up regardless
    /// of which path pushed it.
    ///
    /// Boundary: this only ever detects a BACKEND WIRE jump —
    /// `remaining_ms` landing higher than elapsed real time (minus
    /// hover-held time) can explain. A visible restart with NO warning
    /// from this function implicates FRONTEND remount/re-anchor behavior
    /// instead of anything back here.
    ///
    /// Returns whether a restart was detected (and warned) — mainly so
    /// tests can assert on it directly without needing a `tracing`
    /// subscriber; both production call sites above ignore the return
    /// value.
    fn observe_emission_for_ttl_restart(&mut self, state: &SlotState, now: Instant) -> bool {
        let SlotState::Showing {
            id,
            ttl_ms,
            remaining_ms,
            ..
        } = state
        else {
            // Idle/empty: nothing meaningful to compare the NEXT emission
            // against either, so the sample resets without a warning —
            // matches the "state becomes Empty" reset case in the plan.
            self.ttl_sample = None;
            return false;
        };
        let hover_held_now = self.cumulative_hover_held(now);
        let restarted = match self.ttl_sample.as_ref() {
            // A different item id is a legitimate promotion, not a
            // restart — resets without warning below, same as Empty.
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

                // Timing (plan 081): the same window math `next_deadline`
                // uses, expressed as wire-friendly milliseconds. `ttl_ms` is
                // time-free (a pure function of the rotation spec,
                // `window_expanded`, and `extension_secs`); `remaining_ms`
                // is a pure function of `Instant::now()` taken right here at
                // emission time — the frontend anchors its own countdown
                // from it on receipt.
                //
                // plan 093: `remaining_ms` freezes while a hover session is
                // open — `reference_now` pins to `hover_started_at` instead
                // of the real `Instant::now()`, so this reads the same
                // value on every call for the whole session's duration
                // (note: `SlotState::dedup_eq` already excludes
                // `remaining_ms` from change detection, so freezing this
                // value causes no new emission either way — this is purely
                // what a webview reload / settings-preview snapshot would
                // see, not a wire push). Once hovering ends,
                // `hover_adjusted_promoted_at` folds the now-banked session
                // into the deadline permanently, matching `next_deadline`'s
                // own adjustment exactly.
                let window_secs =
                    item.event.rotation_window(self.window_expanded) + item.extension_secs;
                let ttl_ms = window_secs.saturating_mul(1000);
                let remaining_ms = match item.promoted_at {
                    Some(promoted_at) => {
                        let anchor = self.hover_adjusted_promoted_at(promoted_at);
                        let deadline = anchor + Duration::from_secs(window_secs);
                        let reference_now = self.hover_started_at.unwrap_or_else(Instant::now);
                        let remaining = deadline.saturating_duration_since(reference_now);
                        u64::try_from(remaining.as_millis()).unwrap_or(u64::MAX)
                    }
                    // Defensive: every real promotion path sets
                    // `promoted_at`. If it's somehow absent, render a full
                    // bar rather than panicking.
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
    existing.meta = fresh.meta.clone();
}

const MIN_REMAINING_ON_SUPERSEDE_SECS: u64 = 2;
const MAX_EXTENSION_ON_SUPERSEDE_SECS: u64 = 6;

/// plan 107 Step C: scheduling-jitter slack for the TTL-restart detector
/// (`is_ttl_restart`) — real wake-ups never land on the exact millisecond,
/// so a few hundred ms of "remaining_ms is a bit higher than the elapsed-
/// adjusted expectation" is normal noise, not a restart.
const TTL_RESTART_SLACK_MS: u64 = 500;

/// plan 107 Step C: pure decision half of the TTL-restart detector — see
/// `SingleSlotQueue::observe_emission_for_ttl_restart` for the boundary
/// half (queue-owned state, `tracing::warn!`) that calls this. Same
/// pure-decision/boundary split this codebase already uses for
/// `presentation::presentation_mode` vs its subprocess-calling wrapper.
///
/// The naive predicate (`new_remaining_ms > prev_remaining_ms + slack_ms`)
/// misses the canonical restart pattern: wire emissions are SPARSE
/// (`remaining_ms` is excluded from `SlotState::dedup_eq`, so most ticks
/// never emit at all), and a buggy reset back to full TTL is NOT
/// numerically greater than the LAST emission — e.g. emit 8000, 5s pass
/// silently, a bug re-emits 8000 again: `8000 > 8000 + slack` is false,
/// yet the frontend visibly jumps ~3000 -> 8000 (what the true elapsed-
/// adjusted expectation would have been). So the real test compares
/// against the ELAPSED-ADJUSTED expectation instead: how much
/// `remaining_ms` SHOULD have dropped by, given how much real (non-hover-
/// held) time passed since the last emission.
///
/// A changed total window (`new_ttl_ms != prev_ttl_ms`) is never a
/// restart on its own — both a manual expand and a legitimate
/// Topic-supersede top-up legitimately change `ttl_ms`, and neither is a
/// bug.
///
/// All subtraction is saturating: a hover-held delta that (via rounding)
/// slightly exceeds the raw elapsed time, or an elapsed-adjusted
/// expectation exceeding the previous remaining value, must clamp to
/// zero rather than wrap/panic — the same discipline
/// `hover_frozen_rotation_elapsed` already uses for the same reason.
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
mod tests {
    use super::*;
    use crate::error::QueueError;
    use crate::event::{test_fixtures, DetailItem, EventMeta, EventPayload, EventSignal};
    use std::time::Duration;
    use uuid::Uuid;

    fn event(title: &str, priority: Priority, ttl_secs: u64) -> Event {
        test_fixtures::with_rotation(
            test_fixtures::with_priority(test_fixtures::event(title), priority),
            RotationSpec::OneShot { ttl_secs },
        )
    }

    fn recurring_event(title: &str, priority: Priority, display_secs: u64) -> Event {
        test_fixtures::with_rotation(
            test_fixtures::with_priority(test_fixtures::event(title), priority),
            RotationSpec::Recurring { display_secs },
        )
    }

    fn topic_event(title: &str, priority: Priority, ttl_secs: u64, topic: &str) -> Event {
        test_fixtures::with_topic(event(title, priority, ttl_secs), topic)
    }

    /// Same as `event()` but from a specific origin — for tie-break tests
    /// only; every other test relies on every helper sharing one origin so
    /// rank-based selection degenerates to pre-v6 arrival-order FIFO.
    fn event_from(title: &str, priority: Priority, ttl_secs: u64, origin: SourceKind) -> Event {
        test_fixtures::with_origin(event(title, priority, ttl_secs), origin)
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
    fn visible_supersede_updates_meta() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        let base = topic_event("old", Priority::Medium, 8, "topic");
        let fresh = Event {
            meta: EventMeta {
                details: vec![DetailItem {
                    label: "Clock".to_string(),
                    value: "45'".to_string(),
                }],
                ..EventMeta::default()
            },
            ..base.clone()
        };
        q.enqueue(base, t0).unwrap();
        q.enqueue(fresh, t0 + Duration::from_millis(10)).unwrap();
        let visible = q.visible.as_ref().unwrap();
        assert_eq!(visible.event.meta.details.len(), 1);
        assert_eq!(visible.event.meta.details[0].label, "Clock");
        assert_eq!(visible.event.meta.details[0].value, "45'");
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

    // plan 097: the top-up's "remaining" must be computed against the
    // hover-adjusted elapsed time, not raw `now - promoted_at` — otherwise
    // a card that banked hover-pause time looks closer to expiry than it
    // really is and gets an extension it doesn't need.
    #[test]
    fn visible_supersede_top_up_ignores_banked_hover_time() {
        let t0 = Instant::now();
        let mut q = SingleSlotQueue::new(50);
        // base window 10s.
        q.enqueue(topic_event("a", Priority::Medium, 10, "topic"), t0)
            .unwrap();

        // hover for 3s, then exit — 3s banked into hover_paused_total.
        q.hover_enter(t0 + Duration::from_secs(1));
        q.hover_exit(t0 + Duration::from_secs(4));

        // raw elapsed at t0+9s is 9s, so RAW remaining (10 - 9 = 1s) is
        // below the 2s floor and the pre-fix code would grant an
        // extension. Hover-adjusted elapsed discounts the banked 3s
        // (9 - 3 = 6s), so real remaining is 10 - 6 = 4s — comfortably
        // above the floor. No extension should be granted.
        q.enqueue(
            topic_event("a2", Priority::Medium, 10, "topic"),
            t0 + Duration::from_secs(9),
        )
        .unwrap();

        assert_eq!(
            q.visible.as_ref().unwrap().extension_secs,
            0,
            "banked hover time must not make the card look closer to expiry than it is"
        );
    }

    // plan 097: the unhovered path (no hover session ever opened) must
    // behave exactly as before the hover-adjusted-elapsed change — this
    // replicates the below-the-floor half of
    // `visible_supersede_grants_extension_only_when_below_floor` as an
    // explicit regression guard for the Step 3 borrow reorder.
    #[test]
    fn visible_supersede_top_up_grants_extension_unchanged_without_hover() {
        let t0 = Instant::now();
        let mut q = SingleSlotQueue::new(50);
        // base window 1s: an immediate supersede already has only ~1s
        // remaining, below the 2s floor — extension granted to close the
        // gap, exactly as before Step 3's reorder (no hover involved).
        q.enqueue(topic_event("b", Priority::Medium, 1, "topic"), t0)
            .unwrap();
        q.enqueue(topic_event("b2", Priority::Medium, 1, "topic"), t0)
            .unwrap();
        let extension = q.visible.as_ref().unwrap().extension_secs;
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
    fn same_tier_waiting_supersede_updates_meta() {
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

        let fresh = Event {
            meta: EventMeta {
                details: vec![DetailItem {
                    label: "Clock".to_string(),
                    value: "45'".to_string(),
                }],
                ..EventMeta::default()
            },
            ..topic_event("first-updated", Priority::Medium, 8, "topic")
        };
        q.enqueue(fresh, Instant::now()).unwrap();

        assert_eq!(q.waiting[Priority::Medium as usize].len(), 2);
        assert_eq!(
            q.waiting[Priority::Medium as usize][0]
                .event
                .meta
                .details
                .len(),
            1
        );
        assert_eq!(
            q.waiting[Priority::Medium as usize][0].event.meta.details[0].label,
            "Clock"
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

    #[test]
    fn cross_tier_supersede_updates_meta() {
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

        let fresh = Event {
            meta: EventMeta {
                details: vec![DetailItem {
                    label: "Clock".to_string(),
                    value: "45'".to_string(),
                }],
                ..EventMeta::default()
            },
            ..topic_event("topic-upgraded", Priority::High, 8, "topic")
        };
        q.enqueue(fresh, Instant::now()).unwrap();

        assert_eq!(q.waiting[Priority::High as usize].len(), 2);
        let moved = &q.waiting[Priority::High as usize][1];
        assert_eq!(moved.event.meta.details.len(), 1);
        assert_eq!(moved.event.meta.details[0].label, "Clock");
    }

    #[test]
    fn cross_tier_supersede_drops_fresh_content_when_destination_tier_full() {
        let mut q = SingleSlotQueue::new(1); // max_queued_per_tier = 1
        let t0 = Instant::now();
        // fill the visible slot so nothing promotes out from under us
        q.enqueue(event("visible", Priority::Medium, 60), t0)
            .unwrap();
        // put the Topic item in the Medium tier
        q.enqueue(
            topic_event("match-a", Priority::Medium, 60, "espn:match"),
            t0,
        )
        .unwrap();
        // fill the High tier to its cap of 1
        q.enqueue(event("filler", Priority::High, 60), t0).unwrap();
        // a fresh event for the same Topic, now High priority — destination
        // tier (High) is already full
        let fresh = topic_event("match-a-updated", Priority::High, 60, "espn:match");
        q.enqueue(fresh, t0 + Duration::from_millis(10)).unwrap();
        // the original Medium-tier item must still be there, UNCHANGED
        // (fresh content dropped, not applied) — confirm both facts
        assert_eq!(q.waiting[Priority::Medium as usize].len(), 1);
        assert_eq!(
            q.waiting[Priority::Medium as usize][0].event.payload.title,
            "match-a" // NOT "match-a-updated" — the supersede was dropped
        );
        // the High tier still has only its original filler, not a second item
        assert_eq!(q.waiting[Priority::High as usize].len(), 1);
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

    // plan 081: `slot_state_if_changed` must dedupe across a real
    // wall-clock gap in which only `remaining_ms` moves. This is a direct,
    // deterministic complement to the engine-level regression test
    // (`engine.rs`'s
    // `one_accept_emits_exactly_one_slot_state_despite_live_remaining_ms`):
    // that test's two `current_slot_state` calls are separated only by an
    // async wake-and-relock, which on fast hardware can complete within
    // the same millisecond `remaining_ms` is truncated to — so it can
    // pass even under too-strict equality, purely by luck of timing. A
    // real sleep here forces the gap deterministically, proving
    // `SlotState::dedup_eq`'s `remaining_ms` exclusion (not scheduling
    // luck) is what keeps this deduped: reverting `slot_state_if_changed`
    // to derived `PartialEq` makes this test fail reliably.
    #[test]
    fn slot_state_if_changed_dedupes_across_a_real_time_gap_that_only_moves_remaining_ms() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 30), Instant::now())
            .unwrap();
        assert!(q.slot_state_if_changed().is_some(), "promotion must emit");

        // real sleep: guarantees remaining_ms strictly decreases while
        // ttl_ms (time-free) stays exactly the same.
        std::thread::sleep(Duration::from_millis(5));

        assert!(
            q.slot_state_if_changed().is_none(),
            "remaining_ms alone moving must not trigger a re-emission"
        );
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

    // plan 081: current_slot_state's timing fields. No clock-injection
    // harness exists for this method (it deliberately consults real
    // `Instant::now()` at emission time, per the doc comment on
    // `remaining_ms`) — so unlike the `next_deadline` tests above (which
    // use a synthetic `t0` clock passed through `tick`), this test moves
    // `promoted_at` directly using real `Instant::now()` arithmetic.
    #[test]
    fn current_slot_state_emits_ttl_and_remaining_from_real_promoted_at() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 8), Instant::now())
            .unwrap();
        // move the visible item's promotion 2s into its 8s window.
        q.visible.as_mut().unwrap().promoted_at = Some(Instant::now() - Duration::from_secs(2));

        match q.current_slot_state() {
            SlotState::Showing {
                ttl_ms,
                remaining_ms,
                ..
            } => {
                assert_eq!(ttl_ms, 8000);
                // ~6000ms remaining; allow slack for test execution time.
                assert!(
                    (5500..=6000).contains(&remaining_ms),
                    "expected remaining_ms near 6000, got {remaining_ms}"
                );
            }
            SlotState::Empty => panic!("expected Showing"),
        }
    }

    // plan 081: a supersede top-up (queue.rs:338-348) grows extension_secs,
    // which must show up in ttl_ms — the whole point of keeping ttl_ms in
    // the emission is that the bar re-anchors to the real, possibly-
    // extended deadline. Mirrors
    // next_deadline_includes_supersede_extension's setup.
    #[test]
    fn current_slot_state_ttl_includes_supersede_extension() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(topic_event("a", Priority::Medium, 1, "topic"), t0)
            .unwrap();
        // below the floor: this supersede grants a top-up.
        q.enqueue(topic_event("a2", Priority::Medium, 1, "topic"), t0)
            .unwrap();

        let extension_secs = q.visible.as_ref().unwrap().extension_secs;
        assert!(extension_secs > 0, "expected a top-up to have been granted");

        match q.current_slot_state() {
            SlotState::Showing { ttl_ms, .. } => {
                assert_eq!(ttl_ms, (1 + extension_secs) * 1000);
            }
            SlotState::Empty => panic!("expected Showing"),
        }
    }

    // ------------------------------------------------------------------
    // plan 093: TTL hover-pause — the two design constraints named in the
    // plan (never rotates while held; repeated hover cycles never grant
    // more total time than ttl+extension already allows), plus focused
    // mechanics tests for hover_enter/hover_exit themselves.
    // ------------------------------------------------------------------

    #[test]
    fn hover_enter_is_a_noop_when_nothing_visible() {
        let mut q = SingleSlotQueue::new(50);
        let now = Instant::now();
        q.hover_enter(now);
        assert!(q.hover_started_at.is_none());
    }

    #[test]
    fn hover_enter_is_idempotent_double_enter_keeps_the_first_start_time() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("a", Priority::Medium, 8), t0).unwrap();
        q.hover_enter(t0);
        let first_start = q.hover_started_at;
        q.hover_enter(t0 + Duration::from_secs(1));
        assert_eq!(
            q.hover_started_at, first_start,
            "a second hover_enter before an exit must not reset the session start"
        );
    }

    #[test]
    fn hover_exit_without_a_prior_enter_is_a_noop() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("a", Priority::Medium, 8), t0).unwrap();
        q.hover_exit(t0 + Duration::from_secs(1));
        assert_eq!(q.hover_paused_total, Duration::ZERO);
    }

    #[test]
    fn hover_exit_banks_the_session_duration() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("a", Priority::Medium, 8), t0).unwrap();
        q.hover_enter(t0);
        q.hover_exit(t0 + Duration::from_secs(3));
        assert_eq!(q.hover_paused_total, Duration::from_secs(3));
        assert!(q.hover_started_at.is_none());
    }

    // plan 093 design constraint 1, deterministic case: a card whose
    // rotation window has already fully elapsed in wall-clock time must
    // still be visible if a hover session opened before the deadline and
    // is still open.
    #[test]
    fn visible_item_does_not_rotate_out_while_hover_held_past_its_window() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        let ev = event("held", Priority::Medium, 2);
        let id = ev.id;
        q.enqueue(ev, t0).unwrap();
        q.hover_enter(t0 + Duration::from_secs(1));
        // well past the 2s window, but still hover-held throughout.
        q.tick(t0 + Duration::from_secs(50));
        match q.current_slot_state() {
            SlotState::Showing { id: cur, .. } => assert_eq!(cur, id),
            SlotState::Empty => panic!("must never rotate out while hover-held"),
        }
    }

    // plan 093 design constraint 1: re-anchors with the remaining time it
    // had at entry — after hover exits, the item rotates out only once
    // its full (unpaused) window has actually elapsed, not immediately.
    #[test]
    fn visible_item_rotates_out_the_correct_amount_of_active_time_after_hover_exit() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        let ev = event("held", Priority::Medium, 2);
        let id = ev.id;
        q.enqueue(ev, t0).unwrap();
        // 1s active, then a 10s hover pause.
        q.hover_enter(t0 + Duration::from_secs(1));
        q.tick(t0 + Duration::from_secs(11));
        q.hover_exit(t0 + Duration::from_secs(11));
        // only 1s of the 2s window has actually elapsed (active time) —
        // ticking 1 more active second must not yet rotate it out.
        q.tick(t0 + Duration::from_secs(11) + Duration::from_millis(900));
        match q.current_slot_state() {
            SlotState::Showing { id: cur, .. } => assert_eq!(cur, id),
            SlotState::Empty => panic!("only 1.9s of active time has elapsed against a 2s window"),
        }
        // the remaining 0.2s of active time passes — now it rotates out.
        q.tick(t0 + Duration::from_secs(12) + Duration::from_millis(200));
        assert_eq!(q.current_slot_state(), SlotState::Empty);
    }

    // plan 097: `dismiss_visible` (and `skip_visible`) promote the next
    // waiting item via `promote_next`, which resets the new item's hover
    // fields (`set_expanded_for_promotion`) — but that's a queue-internal
    // fact. The bug this guards against lived one layer up, in lib.rs's
    // AppKit glue: the `was_hovered` latch there didn't reset on a hotkey
    // dismiss (no mouse event fires), so the queue never saw a matching
    // `hover_exit` and the newly-promoted item could be left with no
    // active hover session yet still get treated as hover-frozen by a
    // stale `hover_started_at` from... except promotion already clears
    // that field, which is exactly what this test pins down: the item
    // promoted BY `dismiss_visible` must rotate out strictly on its own
    // schedule, with nothing inherited from the previous item's hover
    // session.
    #[test]
    fn dismiss_while_hover_held_leaves_next_items_rotation_unfrozen() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        let first = event("first", Priority::Medium, 2);
        let first_id = first.id;
        q.enqueue(first, t0).unwrap();
        let second = event("second", Priority::Medium, 2);
        let second_id = second.id;
        q.enqueue(second, t0).unwrap();

        q.hover_enter(t0 + Duration::from_secs(1));
        // well past the first item's 2s window, but hover-held throughout —
        // must not rotate out (same invariant as the test above).
        q.tick(t0 + Duration::from_secs(50));
        match q.current_slot_state() {
            SlotState::Showing { id: cur, .. } => assert_eq!(cur, first_id),
            SlotState::Empty => panic!("must never rotate out while hover-held"),
        }

        // hotkey dismiss, still "mid-hover" from the queue's point of view
        // (no hover_exit call — mirrors the lib.rs latch bug: the AppKit
        // side never told the queue the session ended either).
        q.dismiss_visible(t0 + Duration::from_secs(50));
        match q.current_slot_state() {
            SlotState::Showing { id: cur, .. } => assert_eq!(cur, second_id),
            SlotState::Empty => panic!("dismiss must promote the second item"),
        }

        // advance past the second item's full 2s window with NO further
        // hover_enter — its deadline must not have inherited the first
        // item's frozen/banked state, so it rotates out strictly on
        // schedule.
        q.tick(t0 + Duration::from_secs(50) + Duration::from_secs(3));
        assert_eq!(
            q.current_slot_state(),
            SlotState::Empty,
            "the second item's rotation must not be frozen by the first item's hover session"
        );
    }

    // plan 093: `remaining_ms` freezes for the whole hover session — the
    // TTL bar's rust-side data source must never appear to keep
    // decrementing while the cursor holds it.
    //
    // `current_slot_state`'s non-hovering branch reads REAL
    // `Instant::now()` internally (it's the emission-time snapshot every
    // other `current_slot_state_*` test already relies on) — so, same
    // technique as `current_slot_state_emits_ttl_and_remaining_from_real_
    // promoted_at`, this backdates `promoted_at` directly off a real
    // `Instant::now()` anchor rather than driving an independent
    // simulated clock, and drives `hover_enter`/`hover_exit` with real
    // `Instant::now()` calls too — mixing a simulated clock with the one
    // internal call site that always reads the real one would desync the
    // two and produce a nonsensical reading (this test's first draft
    // caught exactly that mistake).
    #[test]
    fn remaining_ms_freezes_during_a_hover_session_and_resumes_after() {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("a", Priority::Medium, 10), Instant::now())
            .unwrap();

        fn remaining_ms(q: &SingleSlotQueue) -> u64 {
            match q.current_slot_state() {
                SlotState::Showing { remaining_ms, .. } => remaining_ms,
                SlotState::Empty => panic!("expected Showing"),
            }
        }

        // 2s of the 10s window has already "elapsed" by the time hover
        // starts.
        let promoted_at = Instant::now() - Duration::from_secs(2);
        q.visible.as_mut().unwrap().promoted_at = Some(promoted_at);

        q.hover_enter(Instant::now());
        let frozen_at_entry = remaining_ms(&q);
        assert!(
            (7800..=8000).contains(&frozen_at_entry),
            "expected ~8000ms remaining at hover-enter (2s of a 10s window elapsed), got {frozen_at_entry}"
        );

        // real time passes while still hovering — remaining_ms must not
        // move (`reference_now` pins to `hover_started_at`, not the real
        // `Instant::now()` this sleep advances, while a session is open).
        std::thread::sleep(Duration::from_millis(20));
        let still_frozen = remaining_ms(&q);
        assert_eq!(
            still_frozen, frozen_at_entry,
            "remaining_ms must not move while a hover session is open"
        );

        q.hover_exit(Instant::now());
        // the ~20ms just spent hovering is banked (paused, not counted) —
        // remaining_ms picks back up from essentially the same ~8000ms it
        // was frozen at, not from wherever a non-paused countdown would
        // have reached by now.
        let at_exit = remaining_ms(&q);
        assert!(
            (7800..=8000).contains(&at_exit),
            "expected remaining_ms to pick back up from ~8000ms right at hover-exit, got {at_exit}"
        );
    }

    // ------------------------------------------------------------------
    // plan 107 Step C: TTL-restart instrumentation. Pure predicate cases
    // (a)-(e) below are the plan's own lettered list; case (f) (a new
    // item id resets the sample without warning) is queue-driven — the
    // pure `is_ttl_restart` function has no id concept, that reset is
    // `observe_emission_for_ttl_restart`'s own responsibility.
    // ------------------------------------------------------------------

    // (a) normal decay after 5s (8000 -> ~3000) must not warn.
    #[test]
    fn is_ttl_restart_case_a_normal_decay_does_not_warn() {
        assert!(!is_ttl_restart(
            8000,
            8000,
            5000,
            0,
            8000,
            3000,
            TTL_RESTART_SLACK_MS
        ));
    }

    // (b) hover-held for the whole gap, then re-emit ~8000, must not warn
    // — the elapsed-adjusted expectation is still ~8000ms once the full
    // gap is subtracted out as hover-held.
    #[test]
    fn is_ttl_restart_case_b_hover_held_the_whole_gap_does_not_warn() {
        assert!(!is_ttl_restart(
            8000,
            8000,
            5000,
            5000,
            8000,
            8000,
            TTL_RESTART_SLACK_MS
        ));
    }

    // (c) the canonical restart pattern: emit 8000, 5s pass silently and
    // UNHOVERED (expected ~3000 by now), a bug re-emits 8000 again. Must
    // warn — and, per the plan's own red/green requirement, must be RED
    // (fail) if the elapsed adjustment is removed: `8000 > 8000 + slack`
    // is false, so the naive predicate this replaces would have missed
    // it (pinned explicitly by the second test below).
    #[test]
    fn is_ttl_restart_case_c_canonical_restart_pattern_warns() {
        assert!(is_ttl_restart(
            8000,
            8000,
            5000,
            0,
            8000,
            8000,
            TTL_RESTART_SLACK_MS
        ));
    }

    #[test]
    fn is_ttl_restart_case_c_the_naive_elapsed_unaware_predicate_would_have_missed_it() {
        // the exact predicate this function replaces (see its own doc):
        // comparing only against the PREVIOUS remaining_ms, never the
        // elapsed-adjusted expectation.
        let naive_would_warn = 8000_u64 > 8000_u64.saturating_add(TTL_RESTART_SLACK_MS);
        assert!(
            !naive_would_warn,
            "case (c) must be undetectable by the naive predicate — that's the whole reason \
             is_ttl_restart compares against the elapsed-adjusted expectation instead"
        );
    }

    // (d) small jitter within slack must not warn — 300ms over the
    // elapsed-adjusted expectation, safely under the 500ms slack.
    #[test]
    fn is_ttl_restart_case_d_small_jitter_within_slack_does_not_warn() {
        assert!(!is_ttl_restart(
            8000,
            8000,
            1000,
            0,
            8000,
            7300,
            TTL_RESTART_SLACK_MS
        ));
    }

    // (e) a grown ttl_ms (manual expand, or a legitimate Topic-supersede
    // top-up) is never a restart, short-circuited before the
    // elapsed-adjusted math even runs — however implausible the raw
    // remaining_ms jump looks.
    #[test]
    fn is_ttl_restart_case_e_a_changed_ttl_window_is_never_a_restart() {
        assert!(!is_ttl_restart(
            8000,
            3000,
            5000,
            0,
            14000,
            14000,
            TTL_RESTART_SLACK_MS
        ));
    }

    // (f), queue-driven: a new item id resets the sample without warning
    // — a promotion, not a restart, even though the new item's
    // remaining_ms would look like an enormous unexplained jump against
    // the PREVIOUS item's sample.
    #[test]
    fn a_new_item_id_resets_the_ttl_sample_without_warning() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("a", Priority::Medium, 8), t0).unwrap();
        let first = q.current_slot_state();
        assert!(!q.observe_emission_for_ttl_restart(&first, t0));

        q.dismiss_visible(t0);
        q.enqueue(event("b", Priority::High, 8), t0 + Duration::from_secs(1))
            .unwrap();
        let second = q.current_slot_state();
        assert!(!q.observe_emission_for_ttl_restart(&second, t0 + Duration::from_secs(1)));
    }

    // Queue-driven: a real hover-pause sequence (genuine `hover_enter`/
    // `hover_paused_total` accounting, not hand-fed numbers) must never
    // warn. Stays inside one open hover session throughout — `current_
    // slot_state`'s `reference_now` pins to `hover_started_at` while a
    // session is open (see that method's own doc), so every sample here
    // is a pure function of the injected Instants, never the real wall
    // clock (post-hover-exit resumption is already covered by
    // `remaining_ms_freezes_during_a_hover_session_and_resumes_after`
    // above; this test is scoped to the restart detector specifically).
    #[test]
    fn a_hover_pause_sequence_never_warns_the_ttl_restart_detector() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("a", Priority::Medium, 8), t0).unwrap();
        q.hover_enter(t0);

        let first = q.current_slot_state();
        assert!(!q.observe_emission_for_ttl_restart(&first, t0));

        // A naive (elapsed-only) predicate would see 5s pass and expect
        // remaining_ms to have dropped by ~5000ms — it hasn't (the whole
        // 5s was hover-held, frozen), so the hover-aware predicate must
        // not warn.
        let second = q.current_slot_state();
        assert!(!q.observe_emission_for_ttl_restart(&second, t0 + Duration::from_secs(5)));
    }

    // Queue-driven: the unconditional page-load re-emit route
    // (`current_slot_state_for_emission`, what `Engine::emit_current_
    // blocking` calls — bypasses `slot_state_if_changed`'s dedup gate
    // entirely) must actually feed the shared sampler, not silently fall
    // outside it. Proven directly against the queue's own recorded
    // sample (private-field inspection, same test module) — first-ever
    // sample, so a warning is impossible by construction either way
    // (`observe_emission_for_ttl_restart`'s `None` branch), which is
    // exactly the "without warning" half of this test's name.
    #[test]
    fn unconditional_page_load_reemit_participates_in_ttl_restart_sampling() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("a", Priority::Medium, 8), t0).unwrap();
        assert!(
            q.ttl_sample.is_none(),
            "nothing observed by the sampler yet"
        );

        let emitted = q.current_slot_state_for_emission(t0);
        let (id, ttl_ms, remaining_ms) = match emitted {
            SlotState::Showing {
                id,
                ttl_ms,
                remaining_ms,
                ..
            } => (id, ttl_ms, remaining_ms),
            SlotState::Empty => panic!("expected a showing slot"),
        };
        let sample = q
            .ttl_sample
            .as_ref()
            .expect("the unconditional route must record a sample, not bypass the detector");
        assert_eq!(sample.item_id, id);
        assert_eq!(sample.ttl_ms, ttl_ms);
        assert_eq!(sample.remaining_ms, remaining_ms);

        // a second call shortly after keeps updating it, consistently —
        // proving this is the live sampling route, not a one-shot fluke.
        let emitted2 = q.current_slot_state_for_emission(t0 + Duration::from_millis(50));
        let (id2, ttl_ms2, remaining_ms2) = match emitted2 {
            SlotState::Showing {
                id,
                ttl_ms,
                remaining_ms,
                ..
            } => (id, ttl_ms, remaining_ms),
            SlotState::Empty => panic!("expected a showing slot"),
        };
        let sample2 = q.ttl_sample.as_ref().unwrap();
        assert_eq!(sample2.item_id, id2);
        assert_eq!(sample2.ttl_ms, ttl_ms2);
        assert_eq!(sample2.remaining_ms, remaining_ms2);
    }

    mod hover_hold_properties {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            // Design constraint 1 (property form): however far real
            // wall-clock time advances while a hover session stays open,
            // the visible item is never rotated out.
            #[test]
            fn never_rotates_while_hover_held(
                ttl_secs in 1u64..20,
                hold_secs in 0u64..200,
                steps in 1usize..8,
            ) {
                let mut q = SingleSlotQueue::new(50);
                let start = Instant::now();
                let ev = event("held", Priority::Medium, ttl_secs);
                let id = ev.id;
                q.enqueue(ev, start).unwrap();
                q.hover_enter(start);

                let step = Duration::from_secs(hold_secs) / u32::try_from(steps).unwrap_or(1).max(1);
                let mut now = start;
                for _ in 0..steps {
                    now += step;
                    q.tick(now);
                    match q.current_slot_state() {
                        SlotState::Showing { id: cur, .. } => prop_assert_eq!(cur, id),
                        SlotState::Empty => prop_assert!(false, "rotated out while hover-held"),
                    }
                }
            }

            // Design constraint 2 (property form): across any sequence of
            // hover-pause / active-time cycles, the item rotates out
            // exactly when cumulative ACTIVE (non-paused) time reaches the
            // rotation window — never sooner (constraint 1, restated) and
            // never later than that same window would allow on its own
            // (no cycle count can inject bonus life).
            #[test]
            fn hover_cycles_never_grant_more_than_the_rotation_window(
                ttl_secs in 1u64..10,
                cycles in proptest::collection::vec((0u64..5, 0u64..4), 1..8),
            ) {
                let mut q = SingleSlotQueue::new(50);
                let start = Instant::now();
                let ev = event("held", Priority::Medium, ttl_secs);
                let id = ev.id;
                q.enqueue(ev, start).unwrap();

                let mut now = start;
                let mut total_active = 0u64;
                let mut already_rotated = false;
                for (pause_secs, active_secs) in cycles {
                    if already_rotated {
                        break;
                    }

                    q.hover_enter(now);
                    now += Duration::from_secs(pause_secs);
                    q.tick(now);
                    // constraint 1, exercised inline: never rotates while
                    // the pause phase (still hover-held) is in progress.
                    let visible_during_pause = matches!(
                        q.current_slot_state(),
                        SlotState::Showing { id: cur, .. } if cur == id
                    );
                    prop_assert!(visible_during_pause, "rotated out during a hover pause");
                    q.hover_exit(now);

                    now += Duration::from_secs(active_secs);
                    total_active += active_secs;
                    q.tick(now);
                    let still_visible = matches!(
                        q.current_slot_state(),
                        SlotState::Showing { id: cur, .. } if cur == id
                    );
                    if total_active < ttl_secs {
                        prop_assert!(
                            still_visible,
                            "total_active={total_active} < ttl={ttl_secs} but rotated out early"
                        );
                    } else {
                        prop_assert!(
                            !still_visible,
                            "total_active={total_active} >= ttl={ttl_secs} but did not rotate out — hover cycles granted extra life"
                        );
                        already_rotated = true;
                    }
                }
            }
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

    // ------------------------------------------------------------------
    // plan 121: waiting_summaries / clear_waiting (settings window Queue
    // section)
    // ------------------------------------------------------------------

    #[test]
    fn waiting_summaries_orders_tiers_high_to_low_then_fifo_within_tier() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        // visible slot takes the first enqueue regardless of tier — fill
        // it with something distinct so every summary below is genuinely
        // a WAITING item, never `visible`.
        q.enqueue(event("visible", Priority::High, 8), t0).unwrap();
        q.enqueue(event("low-1", Priority::Low, 8), t0).unwrap();
        q.enqueue(event("low-2", Priority::Low, 8), t0).unwrap();
        q.enqueue(event("high-1", Priority::High, 8), t0).unwrap();
        q.enqueue(event("medium-1", Priority::Medium, 8), t0)
            .unwrap();
        q.enqueue(event("high-2", Priority::High, 8), t0).unwrap();

        let summaries = q.waiting_summaries();
        let titles: Vec<&str> = summaries.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, ["high-1", "high-2", "medium-1", "low-1", "low-2"]);
        assert_eq!(summaries[0].priority, "high");
        assert_eq!(summaries[2].priority, "medium");
        assert_eq!(summaries[3].priority, "low");
        // every summary in this test shares one origin (`event()`'s
        // fixture default) — pins that `source` round-trips at all.
        assert!(summaries.iter().all(|s| s.source == "manual"));
    }

    #[test]
    fn waiting_summaries_is_empty_when_nothing_is_waiting() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("only-visible", Priority::Medium, 8), t0)
            .unwrap();
        assert!(q.waiting_summaries().is_empty());
    }

    #[test]
    fn clear_waiting_empties_every_tier_and_returns_the_dropped_count() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("visible", Priority::Medium, 8), t0)
            .unwrap();
        q.enqueue(event("low", Priority::Low, 8), t0).unwrap();
        q.enqueue(event("medium", Priority::Medium, 8), t0).unwrap();
        q.enqueue(event("high", Priority::High, 8), t0).unwrap();

        assert_eq!(q.clear_waiting(), 3);
        assert!(q.waiting_summaries().is_empty());
        // the visible card is untouched by a clear
        assert_eq!(visible_title(&q), Some("visible"));
    }

    #[test]
    fn clear_waiting_is_a_noop_returning_zero_when_nothing_is_waiting() {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("visible", Priority::Medium, 8), t0)
            .unwrap();
        assert_eq!(q.clear_waiting(), 0);
        assert_eq!(visible_title(&q), Some("visible"));
    }

    #[test]
    fn clear_waiting_preserves_the_done_never_reaches_total_invariant() {
        // Three enqueued (batch_total 3), one already completed
        // (batch_done 1), one visible, one still waiting — clearing the
        // one waiting item must not leave `batch_total` stale at 3 (which
        // would make the dots look like there's still a second item to
        // go): with the visible item still up, `total` becomes
        // `done + 1` so the current segment reads as the last one.
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("a", Priority::Medium, 1), t0).unwrap();
        q.enqueue(event("b", Priority::Medium, 8), t0).unwrap();
        q.enqueue(event("c", Priority::Medium, 8), t0).unwrap();
        q.tick(t0 + Duration::from_secs(2)); // a completes, b promotes
        assert_eq!(visible_title(&q), Some("b"));
        assert_eq!(queue_progress(&q), (3, 1));

        let dropped = q.clear_waiting();
        assert_eq!(dropped, 1); // only "c" was waiting
        assert_eq!(visible_title(&q), Some("b")); // untouched
        assert_eq!(queue_progress(&q), (2, 1)); // done never reaches total
    }

    #[test]
    fn clear_waiting_resets_batch_counters_to_zero_when_nothing_is_visible_either() {
        // Paused with nothing promoted: visible is None, waiting nonempty.
        // Clearing drains to fully idle, so `reset_batch_if_idle` zeroes
        // both counters rather than pinning `batch_total` to `batch_done
        // + 0` (which would read the same here, but this pins the actual
        // mechanism, not just the resulting numbers).
        let mut q = SingleSlotQueue::new(50);
        q.pause();
        let t0 = Instant::now();
        q.enqueue(event("a", Priority::Medium, 8), t0).unwrap();
        q.enqueue(event("b", Priority::Medium, 8), t0).unwrap();
        assert_eq!(visible_title(&q), None);
        // Nothing is visible (paused), so `current_slot_state()` is
        // `Empty` — `queue_progress`'s Showing-only match would panic
        // here, hence the direct field reads (private fields are
        // reachable from this same-file `tests` submodule).
        assert_eq!((q.batch_total, q.batch_done), (2, 0));

        assert_eq!(q.clear_waiting(), 2);
        assert_eq!((q.batch_total, q.batch_done), (0, 0));
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
            Just(SourceKind::Weather),
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
            SourceKind::Weather,
        ];
        (Just(all).prop_shuffle(), 0usize..=5).prop_map(|(mut v, len)| {
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
