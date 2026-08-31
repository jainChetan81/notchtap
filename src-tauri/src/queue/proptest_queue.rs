use super::*;
use crate::event::{EventMeta, EventPayload, EventSignal, EventType};
use proptest::prelude::*;
use uuid::Uuid;

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
    enqueued_accepted: u64,
    rotated_out_dropped: u64,
    dismissed: u64,
    skipped_oneshot_dropped: u64,
    last_some_state: Option<SlotState>,
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
            return;
        };
        let after_total = self.total_in_queue();
        if after_total <= before_total {
            return;
        }
        self.enqueued_accepted += 1;
        let promoted = current_vis_id(&self.q) == Some(event_id);
        if promoted {
            self.assert_expanded_at_promotion(Some(event_id));
            if should_preempt {
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
                self.skipped_oneshot_dropped += 1;
            }
        }
        let after_id = current_vis_id(&self.q);
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

    fn check_blanket_invariants(&mut self) {
        if let Some(item) = &self.q.visible {
            assert!(
                item.extension_secs <= MAX_EXTENSION_ON_SUPERSEDE_SECS,
                "invariant 6(i): extension_secs {} exceeded the hard cap {}",
                item.extension_secs,
                MAX_EXTENSION_ON_SUPERSEDE_SECS
            );
        }

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

        let total = self.total_in_queue();
        assert_eq!(
            self.enqueued_accepted,
            total + self.rotated_out_dropped + self.dismissed + self.skipped_oneshot_dropped,
            "invariant 5/6: enqueued-accepted count conservation violated"
        );

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
