//! Property tests for the single-slot queue: the nine documented
//! invariants, checked against generated enqueue/rotate sequences.

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
    // one variant per new state-mutating pub fn, same rule
    // this enum's own doc states above.
    Silence,
    Unsilence,
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
        Just(SourceKind::Agent),
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
        SourceKind::Agent,
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
        1 => Just(Op::Silence),
        1 => Just(Op::Unsilence),
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
// `min_tier` restricts the scan to `Priority::High as usize`
// (2) while Silenced — mirrors production `pop_highest_priority_
// waiting`'s own `min_tier` gate. `0` (every tier) otherwise.
fn predict_promoted(
    snap: &[Vec<(Uuid, SourceKind)>; 3],
    rotation_order: &[SourceKind],
    min_tier: usize,
) -> Option<Uuid> {
    let rank = |origin: SourceKind| {
        rotation_order
            .iter()
            .position(|o| *o == origin)
            .unwrap_or(rotation_order.len())
    };
    for tier in (min_tier..3).rev() {
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

// mirrors production `SingleSlotQueue::full_window_secs`
// independently (this module mirrors `best_index_in_tier` the same
// way) — a preempted item's `preempted_remaining_secs` override
// substitutes for the plain rotation-spec window.
fn window_secs_mirror(q: &SingleSlotQueue, item: &QueueItem) -> u64 {
    let base = item
        .preempted_remaining_secs
        .unwrap_or_else(|| item.event.rotation_window(false));
    let window = if q.window_expanded {
        base.saturating_mul(EXPANDED_MULTIPLIER)
    } else {
        base
    };
    window + item.extension_secs
}

fn vis_snapshot(q: &SingleSlotQueue) -> Option<VisSnap> {
    q.visible.as_ref().map(|item| VisSnap {
        id: item.event.id,
        tier: item.event.priority as usize,
        recurring: matches!(item.event.rotation, RotationSpec::Recurring { .. }),
        promoted_at: item.promoted_at.expect("visible items have promoted_at"),
        window: window_secs_mirror(q, item),
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
    // set by `apply_skip` when Skip re-anchors the SAME
    // visible item id (the `reanchor_wire_if_skip_repromoted_the_last_
    // emitted_item` case) — see invariant 7's own comment for why this
    // narrowly exempts exactly that step from the "never repeats"
    // check. Reset to `false` at the top of every `apply` call so it
    // only ever describes the step just taken.
    last_op_was_skip_reanchor: bool,
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
            last_op_was_skip_reanchor: false,
        }
    }

    fn total_in_queue(&self) -> u64 {
        (self.q.visible.is_some() as u64) + self.q.total_waiting() as u64
    }

    // Invariant 8, called at every detected promotion site. A Low
    // priority (everywhere) or ANY priority while Silenced (a
    // Breakthrough) must start COMPACT; every other promotion starts
    // expanded. Reads the promoted item's own
    // priority via `current_priority()` rather than taking one as a
    // parameter, so every call site (Enqueue/Tick/Dismiss/Skip alike)
    // stays untouched.
    fn assert_expanded_at_promotion(&self, promoted_id: Option<Uuid>) {
        let Some(id) = promoted_id else { return };
        let Some(priority) = self.q.current_priority() else {
            return;
        };
        if let SlotState::Showing { expanded, .. } = self.q.current_slot_state() {
            let expect_compact = priority == Priority::Low || self.q.is_silenced();
            assert_eq!(
                expanded,
                !expect_compact,
                "invariant 8: promotion expanded-flag mismatch (id={id:?}, priority={priority:?}, silenced={})",
                self.q.is_silenced()
            );
        }
    }

    fn apply(&mut self, op: &Op) {
        self.last_op_was_skip_reanchor = false;
        match op {
            Op::Enqueue(spec) => self.apply_enqueue(spec),
            Op::Tick(secs) => self.apply_tick(*secs),
            Op::Dismiss => self.apply_dismiss(),
            Op::Skip => self.apply_skip(),
            Op::ToggleExpanded => self.q.toggle_expanded(),
            Op::Pause => self.q.pause(),
            Op::Resume => self.q.resume(),
            Op::Silence => self.q.silence(),
            Op::Unsilence => self.q.unsilence(),
        }
        self.check_blanket_invariants();
    }

    // `min_tier` for `predict_promoted` — restricted to
    // `Priority::High as usize` while Silenced, mirroring production
    // `pop_highest_priority_waiting`'s own gate.
    fn predict_min_tier(&self) -> usize {
        if self.q.is_silenced() {
            Priority::High as usize
        } else {
            0
        }
    }

    fn apply_enqueue(&mut self, spec: &EnqueueSpec) {
        let event = build_event(spec);
        let event_id = event.id;
        let tier = spec.priority as usize;
        let before_total = self.total_in_queue();
        let vis_before = self
            .q
            .visible
            .as_ref()
            .map(|v| (v.event.id, v.event.priority));
        let paused = self.q.is_paused();
        // a strictly-higher-priority arrival preempts the
        // Visible item — unless Paused (absolute), and while Silenced
        // only a High arrival may preempt (the silence-onset window
        // can leave a Medium/Low Visible; anything below High must
        // buffer then, not promote). Mirrors `try_preempt_visible`.
        let should_preempt = match vis_before {
            Some((_, vis_priority)) => {
                !paused
                    && spec.priority > vis_priority
                    && (!self.q.is_silenced() || spec.priority == Priority::High)
            }
            None => false,
        };

        let result = self.q.enqueue(event, self.now);

        let Ok(()) = result else {
            // Rejected (QueueFull): not part of the 9 documented
            // invariants, so there is deliberately no
            // rejection-untouched check here.
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
            if should_preempt {
                // invariant P1: the interrupted item must
                // land at the HEAD of its own tier, carrying its
                // remaining turn — checked precisely (remaining-time
                // restoration) by the dedicated preemption test suite;
                // this proptest invariant only pins the ordering.
                let (old_id, old_priority) =
                    vis_before.expect("should_preempt implies a Visible item existed");
                let old_tier = old_priority as usize;
                assert_eq!(
                    self.q.waiting[old_tier].front().map(|i| i.event.id),
                    Some(old_id),
                    "invariant P1: a preempted item must requeue at the head of its own tier"
                );
            }
        } else {
            assert!(
                !should_preempt,
                "invariant P1: an eligible preemption must always promote the arriving item"
            );
            if let Some((old_id, _)) = vis_before {
                assert_eq!(
                    current_vis_id(&self.q),
                    Some(old_id),
                    "invariant 3: a non-preempting Enqueue must never disturb the Visible item"
                );
            }
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

        let predicted = predict_promoted(
            &waiting_before,
            &self.rotation_order,
            self.predict_min_tier(),
        );
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
        let predicted = predict_promoted(
            &waiting_before,
            &self.rotation_order,
            self.predict_min_tier(),
        );
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
        // Skip re-anchoring the SAME id (a lone Recurring
        // item requeued then immediately re-promoted, nothing else
        // waiting to take its place) is the one case
        // `reanchor_wire_if_skip_repromoted_the_last_emitted_item`
        // forces a fresh wire emission for even when the new
        // SlotState is otherwise dedup_eq to the last one emitted —
        // see invariant 7 below for why that step is exempted from
        // the "never repeats" check.
        self.last_op_was_skip_reanchor =
            matches!((&vis_before, after_id), (Some(v), Some(a)) if v.id == a);
        if paused {
            assert!(
                after_id.is_none(),
                "invariant 5: Skip while paused must never promote"
            );
            return;
        }
        let predicted = predict_promoted(
            &waiting_before,
            &self.rotation_order,
            self.predict_min_tier(),
        );
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
                let rotation_deadline =
                    promoted_at + Duration::from_secs(window_secs_mirror(&self.q, item));
                let expected = if self.q.auto_retract_armed && self.q.expanded {
                    let base = item
                        .preempted_remaining_secs
                        .unwrap_or_else(|| item.event.rotation_window(false));
                    let retract_deadline = promoted_at + Duration::from_secs(base) / 2;
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

        // invariant 7: slot_state_if_changed never repeats a state —
        // EXCEPT the one skip-re-anchor case flagged above, which
        // intentionally clears `last_emitted` (rather than weakening
        // `dedup_eq`) so the wire re-anchors after a skip re-promotes
        // the same item id; the resulting SlotState is normally still
        // dedup_eq-DIFFERENT from the last one in production (real
        // wall-clock time separates the original promotion from the
        // skip, so `remaining_ms` — the one field dedup_eq excludes —
        // is never the only thing that moved). This harness can run
        // an Enqueue immediately followed by a Skip within the same
        // real-time millisecond, though, since `remaining_ms` is
        // always computed from the REAL clock (`current_slot_state`'s
        // non-hovering branch), not the harness's own injected `now`
        // — producing two byte-identical emissions purely by timing
        // coincidence, not the double-emission bug this invariant
        // otherwise exists to catch. The guard below only
        // exempts that one flagged step; every other op (including a
        // Skip that does NOT re-anchor the same id) is still held to
        // the full check.
        if let Some(state) = self.q.slot_state_if_changed() {
            if let Some(prev) = &self.last_some_state {
                if !self.last_op_was_skip_reanchor {
                    assert_ne!(
                        &state, prev,
                        "invariant 7: slot_state_if_changed returned the same state twice in a row"
                    );
                }
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
