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
    /// set only by `try_preempt_visible` at the moment a
    /// higher-priority arrival cuts this item's turn short. Holds the
    /// item's WHOLE remaining turn length in seconds (base window,
    /// window-expanded multiplier, and any supersede extension already
    /// folded in — see `try_preempt_visible`'s doc), so `base_window_secs`
    /// can substitute it for `event.rotation_window(..)` at every
    /// subsequent deadline computation until this item is re-promoted.
    /// `None` for every item that has never been preempted — the
    /// overwhelmingly common case.
    pub preempted_remaining_secs: Option<u64>,
}

/// Read-only wire summary of a single WAITING item — the settings
/// window's Queue section. Deliberately a projection, not
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
/// but `QueueItemSummary.priority` is a plain `String` field,
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
/// derives from — step 1).
fn source_kind_label(source: SourceKind) -> String {
    match source {
        SourceKind::Football => "football",
        SourceKind::News => "news",
        SourceKind::Manual => "manual",
        SourceKind::Agent => "agent",
    }
    .to_string()
}

/// A single-slot, priority-ordered notification queue with bounded
/// per-tier waiting, ttl/recur rotation, pause/resume gating, and
/// supersession by topic.
///
/// The full lifecycle:
///
/// ```
/// use std::time::{Duration, Instant};
/// use notchtap_lib::event::{Event, EventMeta, EventPayload, EventSignal, EventType, Priority, RotationSpec, SourceKind};
/// use notchtap_lib::queue::SingleSlotQueue;
///
/// fn event(title: &str, priority: Priority, ttl_secs: u64) -> Event {
/// Event {
/// id: uuid::Uuid::new_v4(),
/// event_type: EventType::Generic,
/// priority,
/// rotation: RotationSpec::OneShot { ttl_secs },
/// topic: None,
/// payload: EventPayload { title: title.into(), body: "body".into() },
/// meta: EventMeta::default(),
/// signal: EventSignal::Generic,
/// origin: SourceKind::Manual,
/// }
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
    /// a second, independent gate beside `paused`. While
    /// Silenced, `enqueue_new`'s fast path and `pop_highest_priority_
    /// waiting` both refuse to promote Medium/Low — they buffer exactly
    /// as under Paused — but a High arrival still promotes (a
    /// Breakthrough, always compact — see `set_expanded_for_promotion`).
    /// `paused` is checked FIRST everywhere it matters (`try_preempt_
    /// visible`, `promote_next`'s guard, `enqueue_new`'s fast path) and
    /// wins unconditionally: a Paused engine promotes nothing, High
    /// included, silenced or not.
    silenced: bool,
    /// Render state — what the visible card looks like. Set `true` at every
    /// promotion, flipped by the auto-retract and by
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
    /// Queue-slider counters: a batch starts when an
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
    /// TTL hover-pause. `Some(t)` while the visible item is
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
    /// invariants pinned by the property tests below: (1) a card can
    /// never rotate out while `hover_started_at` is `Some` (rotation
    /// elapsed time cannot advance while frozen), and (2) no number of
    /// hover cycles can ever grant MORE total active (non-paused) time
    /// than `rotation_window(...) + extension_secs` already allowed — each
    /// cycle only pauses, it never adds.
    ///
    /// Deliberately scoped to the ROTATION deadline only — the auto-
    /// retract (expand→collapse) deadline is intentionally NOT frozen by
    /// this mechanism (`retract_if_elapsed` is untouched): only the
    /// rotation deadline holds.
    hover_started_at: Option<Instant>,
    hover_paused_total: Duration,
    /// the ONE queue-owned sample the TTL-restart
    /// detector compares each new wire-emission attempt against — see
    /// `TtlEmissionSample`'s own doc and `observe_emission_for_ttl_restart`
    /// for the full mechanism. Deliberately adjacent to `last_emitted`
    /// (the same "one piece of queue-owned state tracking the last thing
    /// that went out over the wire" shape), not a second/engine-side
    /// tracking structure.
    ttl_sample: Option<TtlEmissionSample>,
}

/// a snapshot of the last observed slot-state wire
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

    /// Clock-agnostic: `now` comes from the caller — the Engine
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
    // at the interface; this is the shared
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
        // a strictly-higher-priority arrival cuts the current
        // Visible item's turn short and takes the Slot immediately — see
        // `try_preempt_visible`'s doc for the full contract (equal/lower
        // never preempts, Paused wins unconditionally, and why Silenced
        // needs no extra check here). This is a genuine promotion, not the
        // ordinary `can_promote_now` fast path below (which requires
        // `all_tiers_empty` and is deliberately NOT reused: an incoming
        // preempting item is entitled to the Slot right now regardless of
        // what else happens to be waiting in its own tier).
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
        // Silenced blocks the fast path for Medium/Low exactly
        // like Paused does — only a High arrival (a Breakthrough) can
        // promote straight into an empty, fully-idle slot while Silenced.
        // `bypass_pause_when_slot_empty` (the test-enqueue escape hatch)
        // bypasses both gates identically — a manual test push isn't
        // subject to either.
        let can_promote_now = self.visible.is_none()
            && (bypass_pause_when_slot_empty || !self.paused)
            && (bypass_pause_when_slot_empty || !self.silenced || event.priority == Priority::High)
            && self.all_tiers_empty();
        if !can_promote_now && self.waiting[tier].len() >= self.max_queued_per_tier {
            return Err(QueueError::QueueFull);
        }
        // a batch starts when an event is accepted
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

    /// the preemption check. A strictly-higher-priority
    /// `incoming_priority` than the currently-Visible item's cuts that
    /// item's turn short right now — returns it (NOT yet re-queued
    /// anywhere; the caller pushes it to the HEAD of its own tier) with
    /// `preempted_remaining_secs` set to exactly how much of its turn was
    /// left, so a later re-promotion picks up where it was cut off rather
    /// than starting a fresh full turn. Equal or lower priority returns
    /// `None` — the existing finish-your-turn contract holds for those.
    ///
    /// Gated on `!self.paused` (Paused wins unconditionally over
    /// everything — nothing promotes while Paused, preemption included)
    /// AND on the Silenced promotion rule: while Silenced only a High
    /// arrival may take the Slot, so only High may preempt. The Silenced
    /// check cannot be skipped on only-High-can-be-Visible grounds:
    /// entering silence does not evict the current card (it finishes its
    /// natural turn, same as Paused), so for that onset window a
    /// Medium/Low can be Visible while Silenced — and a Medium arrival
    /// over a leftover Low must buffer, not promote.
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
        // Remaining turn time at the instant of interruption — the same
        // hover-frozen elapsed/window math `top_up_visible_remaining_
        // time` uses, so this is exactly what the countdown bar would
        // have shown at this moment. Read before resetting the hover
        // fields below (they're what `hover_frozen_rotation_elapsed`
        // reads).
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
        // A hover session (if any) on the interrupted item ends the
        // instant it's cut short — nothing to carry onto whatever item is
        // Visible next (mirrors `set_expanded_for_promotion`'s own reset
        // for the same reason: hover state is strictly per-turn).
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
                // destination tier is full — drop the fresh content and
                // leave the item in its current tier
                // rather than evicting something to make room. This
                // function returns `bool` (whether a Topic match was found
                // and handled), not a `Result`, so there's no error
                // channel to report the drop through today — the caller
                // only cares "did I find and handle this Topic." Leave a
                // comment explaining this rather than silently changing
                // the return contract.
                return true;
            } else if let Some(mut existing) = self.waiting[tier_idx].remove(pos) {
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

    /// every promotion starts expanded (render-only — the turn
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
        let retract_after = Duration::from_secs(self.base_window_secs(item)) / 2;
        if now.saturating_duration_since(promoted_at) < retract_after {
            return;
        }
        self.expanded = false;
        self.auto_retract_armed = false;
    }

    fn rotate_out_if_elapsed(&mut self, now: Instant) {
        // a card must NEVER rotate out while
        // under the cursor. `hover_started_at.is_some()` means a hover
        // session is currently open on the visible item — elapsed time is
        // frozen for the whole tick, so there is nothing to check.
        if self.hover_started_at.is_some() {
            return;
        }
        let Some(item) = &self.visible else { return };
        // Defensive, mirroring `current_slot_state`'s posture for the same
        // invariant: every real promotion path sets `promoted_at`. If a
        // future bug ever leaves it unset, log and skip this tick's
        // rotation check rather than panicking the rotation task and
        // silently freezing the overlay.
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
            // a preemption override only ever covers ONE
            // interrupted turn. This item just finished a full turn
            // naturally (whether that was its original window or a
            // preemption's `preempted_remaining_secs`), so the override is
            // spent — clear it, or its NEXT promotion would wrongly reuse
            // the exhausted remaining-time instead of a fresh full window.
            item.preempted_remaining_secs = None;
            let tier = item.event.priority as usize;
            self.waiting[tier].push_back(item);
        } else {
            self.batch_done += 1;
        }
    }

    // ------------------------------------------------------------------
    // hover hold — see the `hover_started_at`/
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

    /// cumulative hover-held time as of `now` — every
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
            let priority = item.event.priority;
            self.set_expanded_for_promotion(priority);
            self.visible = Some(item);
        }
    }

    // The per-turn render/window reset, called from every promotion site
    // (promote_next, enqueue_new's immediate-promote fast path, and
    // enqueue_new's preemption path) so none of the three can drift from
    // the others. A leftover manual expand/window can never leak onto the
    // next item, and the hover-hold fields reset here for the same reason
    // — a hover session (or banked pause total) from the PREVIOUS visible
    // item must never shift the new one's rotation deadline.
    //
    // Medium/High promotions while not Silenced start expanded (render
    // state only — auto-expansion never extends the turn; the 3× window
    // is manual-expand-only) with the auto-retract armed. A Low-priority
    // promotion, or ANY promotion landing while the engine is Silenced (a
    // Breakthrough — only a High item can reach this while Silenced; see
    // the gates in `promote_next`'s own guard and `enqueue_new`'s fast
    // path), starts compact instead: `expanded` false and no auto-retract
    // armed — there's nothing expanded to retract FROM. The manual expand
    // hotkey (`toggle_expanded`) is unaffected either way.
    fn set_expanded_for_promotion(&mut self, priority: Priority) {
        let compact = priority == Priority::Low || self.silenced;
        self.expanded = !compact;
        self.window_expanded = false;
        self.auto_retract_armed = !compact;
        self.hover_started_at = None;
        self.hover_paused_total = Duration::ZERO;
    }

    /// while Silenced, only the High tier is eligible to
    /// promote — Medium/Low stay buffered in Waiting exactly as under
    /// Paused. `Priority::High as usize == 2`, the top of the `(0..3)`
    /// range this function otherwise walks, so restricting the range's
    /// start to it is sufficient; the loop still walks high-to-low, so
    /// non-Silenced behavior (the full `0..3` range) is untouched.
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

    /// Within one tier: the item whose `origin` sorts earliest in
    /// `rotation_order` wins; unlisted origins (or an empty/unset order)
    /// rank last and tie with each other, so ties fall back to position —
    /// i.e. plain arrival order. `position()`
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
    // window math — the one place
    // `item.event.rotation_window(...) + item.extension_secs` is computed,
    // so the preemption override (`preempted_remaining_secs`) is threaded
    // through exactly once.
    // ------------------------------------------------------------------

    /// The item's un-expanded base window in seconds: its `preempted_
    /// remaining_secs` override if a preemption cut it short (that number
    /// already IS its whole remaining turn — extensions included, see
    /// `try_preempt_visible`'s doc), otherwise the plain rotation-spec
    /// window (`Event::rotation_window(false)`).
    fn base_window_secs(&self, item: &QueueItem) -> u64 {
        item.preempted_remaining_secs
            .unwrap_or_else(|| item.event.rotation_window(false))
    }

    /// The full rotation window in seconds: `base_window_secs` above,
    /// tripled if `window_expanded` (the manual-expand multiplier) is
    /// armed, plus any supersede extension. For an item that was never
    /// preempted this equals
    /// `item.event.rotation_window(self.window_expanded) +
    /// item.extension_secs`.
    fn full_window_secs(&self, item: &QueueItem) -> u64 {
        let base = self.base_window_secs(item);
        let window = if self.window_expanded {
            base.saturating_mul(EXPANDED_MULTIPLIER)
        } else {
            base
        };
        window + item.extension_secs
    }

    // ------------------------------------------------------------------
    // supersede time top-up
    // ------------------------------------------------------------------

    fn top_up_visible_remaining_time(&mut self, now: Instant) {
        let Some(promoted_at) = self.visible.as_ref().and_then(|i| i.promoted_at) else {
            return;
        };
        // the top-up's notion of "remaining" must match the real
        // rotation deadline, which discounts banked and in-flight hover
        // time (`hover_adjusted_promoted_at`) — every OTHER
        // deadline consumer (`next_deadline`, `remaining_ms`) already
        // anchors there. Raw `now - promoted_at` would over-grant
        // extensions to hovered cards: a card with banked hover time
        // looks closer to expiry than it actually is.
        // `hover_frozen_rotation_elapsed` takes `&self`, so it must be
        // called before the `&mut self.visible` borrow below.
        let elapsed = self
            .hover_frozen_rotation_elapsed(promoted_at, now)
            .as_secs();
        // `full_window_secs` takes `&self`, so it must run before the
        // `&mut self.visible` borrow below (same ordering constraint the
        // comment above already calls out for `hover_frozen_rotation_
        // elapsed`).
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

    /// enters Silenced — Medium/Low buffer into Waiting exactly
    /// like Paused (same 202-style acceptance, per-tier cap, topic
    /// supersede still applies), but a High arrival still promotes
    /// (Breakthrough, always compact). Unlike `pause`, silencing an engine
    /// with an already-Visible item never interrupts it — Silenced only
    /// changes what's eligible to promote NEXT, once the slot frees up.
    /// `paused` is checked first everywhere it matters and wins
    /// unconditionally over this.
    pub fn silence(&mut self) {
        self.silenced = true;
    }

    /// Leaves Silenced — normal (every-tier) promotion resumes immediately,
    /// same relationship `resume()` has to `pause()`.
    pub fn unsilence(&mut self) {
        self.silenced = false;
    }

    pub fn is_silenced(&self) -> bool {
        self.silenced
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
    /// The requeue arm deliberately mirrors (not shares)
    /// `rotate_out_if_elapsed`'s: stale `promoted_at` /
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
        self.reanchor_wire_if_skip_repromoted_the_last_emitted_item();
    }

    /// a skip's re-promotion can land the exact same item id
    /// that was already the last thing sent over the wire — the only path
    /// is a Recurring item that's the sole occupant of its Priority tier,
    /// so `skip_visible`'s requeue-then-`promote_next` pulls it straight
    /// back out with a fresh `promoted_at` (a lone Recurring item skipped
    /// mid-turn, after it auto-expanded at the half-window mark). `SlotState::dedup_eq` deliberately excludes
    /// `remaining_ms` (the CLAUDE.md rule — continuously-varying wire
    /// fields must never fall into derived `PartialEq`) and every OTHER
    /// field of the re-promoted Showing state is identical to what's
    /// cached in `last_emitted` (same id, same ttl_ms once
    /// `set_expanded_for_promotion` resets `window_expanded`/
    /// `extension_secs`, same `expanded`/`queue_total`/`queue_done`) — so
    /// without this, `slot_state_if_changed` would read "unchanged" and
    /// never re-emit; the overlay's TTL bar would sit stale at wherever it
    /// was on skip, for up to half a window, until some unrelated real
    /// change forced the next emit.
    ///
    /// This lives here, not in `dedup_eq` itself: clearing
    /// `last_emitted` forces the NEXT `slot_state_if_changed` call to hit
    /// its `None` arm (its "first-ever emission" case), re-anchoring the
    /// wire with the restarted `remaining_ms` — `dedup_eq`'s own
    /// `remaining_ms` exclusion is untouched and still guards every other
    /// tick against double emission.
    ///
    /// `ttl_sample` is cleared in the same breath, for the ttl-restart
    /// detector's sake (`observe_emission_for_ttl_restart`, below): that
    /// function already treats "no previous sample for this id" as
    /// "nothing to compare, don't warn" — its `_ => false` arm, the same
    /// path a fresh promotion (different id) or an Empty state takes. A
    /// skip's restart is INTENTIONAL (skip means "start this item's turn
    /// over"), not the backend-wire bug the detector exists to catch, so
    /// resetting the sample here keeps it silent for exactly this case
    /// without touching `is_ttl_restart` or any other call site — a
    /// genuine restart (no skip in between) still finds its `ttl_sample`
    /// intact and still warns.
    ///
    /// Only fires when the freshly promoted item's id actually matches
    /// `last_emitted`'s: every other skip outcome (a different waiting
    /// item promoted, the OneShot-drop case, nothing left to promote)
    /// leaves both caches untouched.
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

    /// with expand-all the hotkey always flips. Any press disarms
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
    /// settings window's Queue section. Ordered tiers
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
    /// window's "Clear queue" action. The visible card is
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

    /// fully idle (nothing visible, every tier empty)
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
    /// while armed) and its rotation deadline. `None`
    /// when nothing is visible — promotion of waiting items is driven by
    /// mutations, which wake the heartbeat directly, not by a deadline this
    /// method could return. The deadline is returned regardless of
    /// `paused`: paused items still age out (`rotate_out_if_elapsed`
    /// doesn't check `paused`), Paused only disables `promote_next`.
    ///
    /// the ROTATION half is also `None` while a hover session is
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

    // ------------------------------------------------------------------
    // slot-state emission helpers
    // ------------------------------------------------------------------

    /// Emits only when the state has meaningfully changed, per
    /// `SlotState::dedup_eq` — NOT the derived `PartialEq`. The
    /// derived equality would compare `remaining_ms` too, which is a pure
    /// function of `Instant::now()` and so is never stable between two
    /// calls even milliseconds apart; using it here reintroduces the
    /// double-emission bug (the rotation loop's post-wake recheck always
    /// seeing "changed"). See `SlotState::dedup_eq`'s doc
    /// for the full mechanism.
    pub fn slot_state_if_changed(&mut self) -> Option<SlotState> {
        let current = self.current_slot_state();
        let changed = match self.last_emitted.as_ref() {
            Some(last) => !last.dedup_eq(&current),
            None => true,
        };
        if changed {
            self.last_emitted = Some(current.clone());
            // every gated emission feeds the TTL-restart
            // sampler too — see `observe_emission_for_ttl_restart`'s doc
            // for why this needs to cover BOTH this path and the
            // unconditional one (`current_slot_state_for_emission`).
            self.observe_emission_for_ttl_restart(&current, Instant::now());
            Some(current)
        } else {
            None
        }
    }

    /// the webview-reload re-emit's own door into
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

    /// the boundary half of the TTL-restart detector —
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
            // the "state becomes Empty" reset case.
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
                // Queue-slider position: `total` never dips below
                // 1 (a visible item is always at least its own segment) and
                // `done` never reaches `total` while an item is visible
                // (the current segment stays bright) — defensive: no known
                // path pushes `batch_done` past `batch_total`; the cap stays
                // as cheap insurance against a future double-count.
                let queue_total = u32::try_from(self.batch_total.max(1)).unwrap_or(u32::MAX);
                let queue_done = u32::try_from(self.batch_done)
                    .unwrap_or(u32::MAX)
                    .min(queue_total - 1);

                // Timing: the same window math `next_deadline`
                // uses, expressed as wire-friendly milliseconds. `ttl_ms` is
                // time-free (a pure function of the rotation spec,
                // `window_expanded`, and `extension_secs`); `remaining_ms`
                // is a pure function of `Instant::now()` taken right here at
                // emission time — the frontend anchors its own countdown
                // from it on receipt.
                //
                // `remaining_ms` freezes while a hover session is
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
                    agent_runtime: item.event.meta.agent.as_ref().map(|a| a.runtime.clone()),
                }
            }
        }
    }
}

// The one place a superseding event's content lands on an existing item —
// used at all three supersede sites (visible, same-tier waiting, cross-tier
// waiting) so a new `Event` field only needs to be added here once. A
// shared function makes a per-site field gap structurally impossible.
fn apply_fresh_content(existing: &mut Event, fresh: &Event) {
    existing.payload = fresh.payload.clone();
    existing.priority = fresh.priority;
    existing.rotation = fresh.rotation;
    existing.signal = fresh.signal;
    existing.meta = fresh.meta.clone();
}

const MIN_REMAINING_ON_SUPERSEDE_SECS: u64 = 2;
const MAX_EXTENSION_ON_SUPERSEDE_SECS: u64 = 6;

/// scheduling-jitter slack for the TTL-restart detector
/// (`is_ttl_restart`) — real wake-ups never land on the exact millisecond,
/// so a few hundred ms of "remaining_ms is a bit higher than the elapsed-
/// adjusted expectation" is normal noise, not a restart.
const TTL_RESTART_SLACK_MS: u64 = 500;

/// pure decision half of the TTL-restart detector — see
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
mod tests;

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
mod proptest_queue;
