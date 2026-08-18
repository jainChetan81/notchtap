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
/// rank-based selection degenerates to arrival-order FIFO.
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

// the old contract ("a Priority arrival never interrupts
// the currently-Visible item") is DELETED — a strictly-higher-priority
// arrival now preempts immediately. See the `preemption` test module
// below for the full suite (chained preemption, head-of-tier requeue,
// remaining-time restoration, equal/lower no-preempt, silenced
// interaction).
#[test]
fn high_enqueue_preempts_currently_visible_medium() {
    let mut q = SingleSlotQueue::new(50);
    q.enqueue(event("a", Priority::Medium, 4), Instant::now())
        .unwrap();
    q.enqueue(event("b", Priority::High, 2), Instant::now())
        .unwrap();
    assert_eq!(visible_title(&q), Some("b"));
    assert_eq!(waiting_titles(&q, Priority::Medium as usize), vec!["a"]);
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
    // a High enqueue now PREEMPTS a Visible Low item
    // immediately rather than waiting out its turn — see the
    // dedicated `preemption` test module for the full contract. The
    // preempted "recur" lands at the HEAD of its own tier (ahead of
    // "low2", which arrived earlier but was never Visible) — the
    // point this test still pins is that a Recurring item's requeue
    // never crosses tiers, whichever end of the tier it lands at.
    let mut q = SingleSlotQueue::new(50);
    q.enqueue(recurring_event("recur", Priority::Low, 1), Instant::now())
        .unwrap();
    q.enqueue(event("low2", Priority::Low, 8), Instant::now())
        .unwrap();
    q.enqueue(event("high", Priority::High, 8), Instant::now())
        .unwrap();
    // "high" preempted "recur" immediately, on enqueue — no tick needed.
    assert_eq!(visible_title(&q), Some("high"));
    assert_eq!(
        waiting_titles(&q, Priority::Low as usize),
        vec!["recur", "low2"]
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
    //, so no real sleeps and no hidden wall-clock read
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
    // signal must be carried through by all three supersede call
    // sites — this is the regression test for that gap.
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

// the top-up's "remaining" must be computed against the
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

// the unhovered path (no hover session ever opened) must
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
    // a High enqueue now preempts a Visible Medium item —
    // irrelevant to what THIS test is pinning (cross-tier supersede
    // dropping fresh content when the destination tier is full), so
    // pause first to keep "visible" pinned in the Slot and "match-a"
    // undisturbed at the front of the Medium tier.
    q.pause();
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

// skipping the sole Recurring item in its tier requeues
// it then immediately re-promotes it (`promote_next` has nothing else
// to pick) — the exact re-anchor scenario
// `reanchor_wire_if_skip_repromoted_the_last_emitted_item` exists for.
// `promoted_at` is backdated off a REAL `Instant::now()` anchor, same
// technique as `remaining_ms_freezes_during_a_hover_session_and_
// resumes_after` — `current_slot_state`'s non-hovering branch always
// reads the real clock for `remaining_ms`, so a simulated/offset clock
// here would desync from it.
#[test]
fn skip_of_a_lone_recurring_item_forces_a_fresh_emit_with_restarted_remaining_ms() {
    let mut q = SingleSlotQueue::new(50);
    q.enqueue(
        recurring_event("recur", Priority::Medium, 8),
        Instant::now(),
    )
    .unwrap();

    // seed last_emitted/ttl_sample exactly as the real wire path would.
    let first = q.slot_state_if_changed().expect("promotion must emit");
    let first_id = match first {
        SlotState::Showing { id, .. } => id,
        SlotState::Empty => panic!("expected a showing slot"),
    };

    // 4s of the 8s window has already "elapsed" — the auto-expanded
    // first half-window scenario.
    let backdated = Instant::now() - Duration::from_secs(4);
    q.visible.as_mut().unwrap().promoted_at = Some(backdated);

    q.skip_visible(Instant::now());

    // dedup_eq alone would suppress this (id/ttl_ms/expanded/queue
    // fields are all unchanged) — the `last_emitted` clear must force
    // it through.
    let second = q.slot_state_if_changed().expect(
        "skip's re-promotion of the same item must force a fresh emit, not be dedup-suppressed",
    );
    let (second_id, second_remaining) = match second {
        SlotState::Showing {
            id, remaining_ms, ..
        } => (id, remaining_ms),
        SlotState::Empty => panic!("expected a showing slot"),
    };
    assert_eq!(second_id, first_id, "same item, re-promoted");
    assert!(
        second_remaining >= 7_800,
        "expected remaining_ms to have restarted to ~8000ms after the skip, got {second_remaining}"
    );
}

// the ttl-restart detector must stay silent for exactly
// this intentional case — the restart it would otherwise see is skip's
// own doing, not a backend-wire bug.
#[test]
fn skip_of_a_lone_recurring_item_never_warns_the_ttl_restart_detector() {
    let mut q = SingleSlotQueue::new(50);
    q.enqueue(
        recurring_event("recur", Priority::Medium, 8),
        Instant::now(),
    )
    .unwrap();

    // seed BOTH `last_emitted` (what makes the reanchor guard fire at
    // all) and `ttl_sample` — the real wire path, same as
    // `skip_of_a_lone_recurring_item_forces_a_fresh_emit_with_
    // restarted_remaining_ms` above.
    assert!(q.slot_state_if_changed().is_some(), "promotion must emit");
    assert!(
        q.ttl_sample.is_some(),
        "the seeding emission must have populated the ttl sample"
    );

    let backdated = Instant::now() - Duration::from_secs(4);
    q.visible.as_mut().unwrap().promoted_at = Some(backdated);

    q.skip_visible(Instant::now());
    assert!(
        q.ttl_sample.is_none(),
        "skip's re-anchor must reset the sample so the next observation has nothing \
         (mismatched) to compare against"
    );

    let second = q.current_slot_state();
    assert!(
        !q.observe_emission_for_ttl_restart(&second, Instant::now()),
        "an intentional skip-triggered restart must never trip the ttl-restart warning"
    );
}

// proves the reset above is scoped to the skip site only —
// a genuine backend-wire restart of the SAME item id, with no skip in
// between, must still warn.
#[test]
fn a_genuine_restart_with_no_skip_in_between_still_warns() {
    let mut q = SingleSlotQueue::new(50);
    let t0 = Instant::now();
    q.enqueue(recurring_event("recur", Priority::Medium, 8), t0)
        .unwrap();

    let first = q.current_slot_state();
    assert!(!q.observe_emission_for_ttl_restart(&first, t0));

    // 5s pass, unhovered — a bug re-emits the full window again,
    // without a skip anywhere in the sequence.
    let t1 = t0 + Duration::from_secs(5);
    let mut restarted_state = first.clone();
    if let SlotState::Showing { remaining_ms, .. } = &mut restarted_state {
        *remaining_ms = 8000;
    }
    assert!(
        q.observe_emission_for_ttl_restart(&restarted_state, t1),
        "a genuine restart with no intervening skip must still warn"
    );
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

// `slot_state_if_changed` must dedupe across a real
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

    // every promotion starts expanded, so the first press
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
    // the 3× window is manual-expand-only. The promotion
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
// expanded semantics: Medium/High auto-expand, Low ALWAYS promotes
// compact, half-window auto-retract, manual-only window extension,
// per-item reset, idle no-op.
// ------------------------------------------------------------------

#[test]
fn medium_and_high_auto_expand_on_immediate_enqueue_low_stays_compact() {
    // Exercises enqueue_new's can_promote_now fast path directly (no
    // tick()/promote_next involved) — the more common of the two
    // promotion call sites in production.
    for (priority, expect_expanded) in [
        (Priority::Low, false),
        (Priority::Medium, true),
        (Priority::High, true),
    ] {
        let mut q = SingleSlotQueue::new(50);
        q.enqueue(event("x", priority, 8), Instant::now()).unwrap();
        match q.current_slot_state() {
            SlotState::Showing { expanded, .. } => {
                assert_eq!(
                    expanded, expect_expanded,
                    "{priority:?} expanded-on-promotion mismatch"
                )
            }
            SlotState::Empty => panic!("expected Showing"),
        }
    }
}

#[test]
fn low_priority_promoted_from_waiting_stays_compact() {
    // A Medium item occupies the slot, so the Low item queues behind
    // it. Ticking past the Medium item's window drives promote_next,
    // not the enqueue fast path — and a Low promotion must start
    // compact regardless of which promotion path it took.
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
            assert!(!expanded, "Low must promote compact even via promote_next")
        }
        SlotState::Empty => panic!("expected Showing"),
    }
}

#[test]
fn expanded_resets_when_next_item_promotes() {
    // keeps 's per-item reset, but the reset target
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
            assert!(expanded, "next item must start expanded")
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
    // 's auto_expanded_high_uses_expanded_rotation_window,
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
    // the promotion auto-expands — that's the default, not
    // a leak. What the idle press must not leak is the manual 3×
    // window, and the retract must still come armed from the
    // promotion itself.
    assert!(q.expanded, "promotion auto-expands");
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
// auto-retract at half the base window
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
// next_deadline
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
    // a promotion starts auto-expanded with the *base*
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
// batch counters behind the queue slider
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

// the rich-relay fields ride on EventMeta, so a visible
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

// agent_runtime mirrors item.event.meta.agent.runtime onto the
// wire — an agent-originated item's SlotState carries the runtime
// token, and every other item leaves it None.
#[test]
fn current_slot_state_carries_agent_runtime_from_event_meta_agent() {
    let mut q = SingleSlotQueue::new(50);
    let mut ev = event("a", Priority::High, 8);
    ev.meta.agent = Some(crate::event::AgentSignal {
        runtime: "claude-code".to_string(),
        kind: "permission_request".to_string(),
        session_hash: "deadbeef".to_string(),
        summary: None,
    });
    q.enqueue(ev, Instant::now()).unwrap();
    match q.current_slot_state() {
        SlotState::Showing { agent_runtime, .. } => {
            assert_eq!(agent_runtime.as_deref(), Some("claude-code"));
        }
        SlotState::Empty => panic!("expected Showing"),
    }
}

#[test]
fn current_slot_state_agent_runtime_is_none_for_non_agent_event() {
    let mut q = SingleSlotQueue::new(50);
    q.enqueue(event("a", Priority::High, 8), Instant::now())
        .unwrap();
    match q.current_slot_state() {
        SlotState::Showing { agent_runtime, .. } => {
            assert_eq!(agent_runtime, None);
        }
        SlotState::Empty => panic!("expected Showing"),
    }
}

// current_slot_state's timing fields. No clock-injection
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

// a supersede top-up (queue.rs:338-348) grows extension_secs,
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
// TTL hover-pause — the two design constraints (never rotates while
// held; repeated hover cycles never grant more total time than
// ttl+extension already allows), plus focused mechanics tests for
// hover_enter/hover_exit themselves.
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

// a card whose
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

// re-anchors with the remaining time it
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

// a visible item with a missing `promoted_at` must degrade
// to a skipped rotation check (logged), never a panic — mirrors
// `current_slot_state`'s existing graceful posture for the same
// invariant. Constructed directly (bypassing `enqueue`/`promote_next`,
// which always set `promoted_at`) since that's the only way to reach
// this defensive branch at all.
#[test]
fn rotate_out_skips_gracefully_when_promoted_at_is_missing() {
    let mut q = SingleSlotQueue::new(50);
    let t0 = Instant::now();
    let ev = event("no-promoted-at", Priority::Medium, 2);
    let id = ev.id;
    q.visible = Some(QueueItem {
        event: ev,
        enqueued_at: t0,
        promoted_at: None,
        extension_secs: 0,
        preempted_remaining_secs: None,
    });
    // way past any plausible window — must not panic, and (since
    // there's no promoted_at to measure elapsed time from) must not
    // rotate out either.
    q.tick(t0 + Duration::from_secs(50));
    match q.current_slot_state() {
        SlotState::Showing { id: cur, .. } => assert_eq!(cur, id),
        SlotState::Empty => panic!("missing promoted_at must skip rotation, not rotate out"),
    }
}

// `dismiss_visible` (and `skip_visible`) promote the next waiting
// item via `promote_next`, which resets the new item's hover fields
// (`set_expanded_for_promotion`). This pins the cross-layer coupling:
// a hotkey dismiss fires no mouse event, so lib.rs's AppKit glue
// sends no matching `hover_exit` — the item promoted BY
// `dismiss_visible` must still rotate out strictly on its own
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

// `remaining_ms` freezes for the whole hover session — the
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
// TTL-restart instrumentation. Pure predicate cases (a)-(e) below
// cover the detector itself; case (f) (a new
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
// warn, and must fail if the elapsed adjustment is removed:
// `8000 > 8000 + slack` is false, so an elapsed-unaware predicate
// misses it (pinned explicitly by the second test below).
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
    // the naive predicate (see this function's own doc): comparing
    // only against the PREVIOUS remaining_ms, never the
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
// waiting_summaries / clear_waiting (settings window Queue
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

// `source_kind_label` (used by `waiting_summaries`'
// `source` field) has four variants — until now only "manual" was
// pinned by name (the assertion above). `types.ts` claims this
// four-string union as a typed wire contract, so every spelling needs
// its own assert, not just coverage-by-coincidence of whichever
// origin a fixture happened to use.
#[test]
fn waiting_summaries_source_pins_all_four_source_kind_label_spellings() {
    let mut q = SingleSlotQueue::new(50);
    let t0 = Instant::now();
    // visible slot absorbs the first enqueue — everything below stays
    // WAITING, same technique as the test above.
    q.enqueue(
        event_from("visible", Priority::High, 8, SourceKind::Manual),
        t0,
    )
    .unwrap();
    q.enqueue(
        event_from("f", Priority::Medium, 8, SourceKind::Football),
        t0,
    )
    .unwrap();
    q.enqueue(event_from("n", Priority::Medium, 8, SourceKind::News), t0)
        .unwrap();
    q.enqueue(event_from("m", Priority::Medium, 8, SourceKind::Manual), t0)
        .unwrap();
    q.enqueue(event_from("a", Priority::Medium, 8, SourceKind::Agent), t0)
        .unwrap();

    let summaries = q.waiting_summaries();
    let by_title: std::collections::HashMap<&str, &str> = summaries
        .iter()
        .map(|s| (s.title.as_str(), s.source.as_str()))
        .collect();
    assert_eq!(by_title.get("f"), Some(&"football"));
    assert_eq!(by_title.get("n"), Some(&"news"));
    assert_eq!(by_title.get("m"), Some(&"manual"));
    assert_eq!(by_title.get("a"), Some(&"agent"));
}

// the settings window's Queue section shows WAITING
// items via `waiting_summaries` — a skipped Recurring item must stay
// visible there (requeued, not dropped), last in its own tier. Before
// this test the UI mock only encoded the OneShot intuition (skip =
// gone); this pins the Recurring contract at the queue layer, the
// settings window's actual data source.
#[test]
fn skip_of_a_recurring_visible_leaves_it_present_and_last_in_its_tier_in_waiting_summaries() {
    let mut q = SingleSlotQueue::new(50);
    let t0 = Instant::now();
    q.enqueue(recurring_event("recur", Priority::Medium, 8), t0)
        .unwrap(); // promotes immediately (fast path, empty queue)
    q.enqueue(event("w1", Priority::Medium, 8), t0).unwrap();
    q.enqueue(event("w2", Priority::Medium, 8), t0).unwrap();

    q.skip_visible(t0);

    assert_eq!(visible_title(&q), Some("w1"));
    let summaries = q.waiting_summaries();
    let titles: Vec<&str> = summaries.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(
        titles,
        ["w2", "recur"],
        "a skipped Recurring item must requeue to the BACK of its own tier, still present \
         in waiting_summaries — not dropped, the OneShot behavior"
    );
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
    // pause first — a High enqueue would otherwise preempt
    // "visible" immediately, which is irrelevant to what this test
    // pins (clear_waiting emptying every waiting tier without
    // touching the Visible card).
    q.pause();
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

    // a fresh enqueue AFTER the clear must not read the
    // pinned `batch_total` from `clear_waiting` as though it were
    // permanent bookkeeping — it's a one-shot pin for the
    // already-cleared state, and a genuinely new item queued behind
    // the still-visible "b" grows the batch normally from there:
    // `batch_total` 2 -> 3, `batch_done` stays 1, i.e. `(done + 2,
    // done)`.
    q.enqueue(event("d", Priority::Medium, 8), t0).unwrap();
    assert_eq!(visible_title(&q), Some("b")); // still untouched
    assert_eq!(queue_progress(&q), (3, 1));
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

// ------------------------------------------------------------------
// Silenced gate
// ------------------------------------------------------------------

#[test]
fn silenced_buffers_medium_and_low_but_high_breaks_through() {
    let mut q = SingleSlotQueue::new(50);
    q.silence();
    q.enqueue(event("m", Priority::Medium, 8), Instant::now())
        .unwrap();
    q.enqueue(event("l", Priority::Low, 8), Instant::now())
        .unwrap();
    assert_eq!(
        visible_title(&q),
        None,
        "Medium/Low must buffer while Silenced, exactly like Paused"
    );
    assert_eq!(waiting_titles(&q, Priority::Medium as usize), vec!["m"]);
    assert_eq!(waiting_titles(&q, Priority::Low as usize), vec!["l"]);

    // the fast path never jumps waiting items (pre-existing rule,
    // unrelated to Silenced) — with Medium/Low already waiting, "h"
    // lands in the High tier and needs a `tick` to promote via
    // `promote_next`, exactly like any other tier-non-empty arrival.
    let now = Instant::now();
    q.enqueue(event("h", Priority::High, 8), now).unwrap();
    q.tick(now);
    assert_eq!(
        visible_title(&q),
        Some("h"),
        "a High arrival must still promote (Breakthrough) while Silenced"
    );
}

#[test]
fn breakthrough_promotion_is_compact() {
    let mut q = SingleSlotQueue::new(50);
    q.silence();
    q.enqueue(event("h", Priority::High, 8), Instant::now())
        .unwrap();
    match q.current_slot_state() {
        SlotState::Showing { expanded, .. } => {
            assert!(!expanded, "a Breakthrough card must promote compact")
        }
        SlotState::Empty => panic!("expected Showing"),
    }
}

#[test]
fn silenced_promotion_only_pulls_high_from_waiting_medium_and_low_stay_buffered() {
    // A Breakthrough card's turn ending must not open the door for
    // Medium/Low to promote while still Silenced — only the next
    // waiting High (if any) is eligible; Medium/Low stay buffered even
    // once the Slot is empty.
    let mut q = SingleSlotQueue::new(50);
    let t0 = Instant::now();
    q.pause();
    q.enqueue(event("h1", Priority::High, 1), t0).unwrap();
    q.enqueue(event("h2", Priority::High, 8), t0).unwrap();
    q.enqueue(event("m", Priority::Medium, 8), t0).unwrap();
    q.enqueue(event("l", Priority::Low, 8), t0).unwrap();
    q.silence();
    q.resume();
    q.tick(t0);

    assert_eq!(visible_title(&q), Some("h1"));
    assert_eq!(waiting_titles(&q, Priority::High as usize), vec!["h2"]);
    assert_eq!(waiting_titles(&q, Priority::Medium as usize), vec!["m"]);
    assert_eq!(waiting_titles(&q, Priority::Low as usize), vec!["l"]);

    // h1's 1s turn elapses: the next High (h2) promotes, not m/l.
    q.tick(t0 + Duration::from_secs(2));
    assert_eq!(visible_title(&q), Some("h2"));
    assert_eq!(waiting_titles(&q, Priority::Medium as usize), vec!["m"]);
    assert_eq!(waiting_titles(&q, Priority::Low as usize), vec!["l"]);

    // h2 finishes too, and the High tier is now empty: the Slot goes
    // empty rather than pulling Medium/Low while still Silenced.
    q.tick(t0 + Duration::from_secs(20));
    assert_eq!(visible_title(&q), None);
    assert_eq!(waiting_titles(&q, Priority::Medium as usize), vec!["m"]);
    assert_eq!(waiting_titles(&q, Priority::Low as usize), vec!["l"]);
}

#[test]
fn paused_wins_unconditionally_over_silenced_high_included() {
    let mut q = SingleSlotQueue::new(50);
    q.pause();
    q.silence();
    q.enqueue(event("h", Priority::High, 8), Instant::now())
        .unwrap();
    assert_eq!(
        visible_title(&q),
        None,
        "Paused must block even a High Breakthrough"
    );
    assert_eq!(waiting_titles(&q, Priority::High as usize), vec!["h"]);
}

#[test]
fn unsilencing_resumes_normal_promotion_immediately() {
    let mut q = SingleSlotQueue::new(50);
    let t0 = Instant::now();
    q.pause();
    q.enqueue(event("m", Priority::Medium, 8), t0).unwrap();
    q.silence();
    q.resume();
    q.tick(t0);
    assert_eq!(
        visible_title(&q),
        None,
        "still Silenced — Medium must stay buffered"
    );

    q.unsilence();
    q.tick(t0);
    assert_eq!(
        visible_title(&q),
        Some("m"),
        "unsilencing must let normal (every-tier) promotion resume immediately"
    );
}

// ------------------------------------------------------------------
// Priority preemption
// ------------------------------------------------------------------

#[test]
fn high_preempts_visible_low() {
    let mut q = SingleSlotQueue::new(50);
    q.enqueue(event("low", Priority::Low, 8), Instant::now())
        .unwrap();
    q.enqueue(event("high", Priority::High, 8), Instant::now())
        .unwrap();
    assert_eq!(visible_title(&q), Some("high"));
    assert_eq!(waiting_titles(&q, Priority::Low as usize), vec!["low"]);
}

#[test]
fn medium_preempts_visible_low() {
    let mut q = SingleSlotQueue::new(50);
    q.enqueue(event("low", Priority::Low, 8), Instant::now())
        .unwrap();
    q.enqueue(event("medium", Priority::Medium, 8), Instant::now())
        .unwrap();
    assert_eq!(visible_title(&q), Some("medium"));
    assert_eq!(waiting_titles(&q, Priority::Low as usize), vec!["low"]);
}

#[test]
fn equal_priority_never_preempts() {
    for priority in [Priority::Low, Priority::Medium, Priority::High] {
        let mut q = SingleSlotQueue::new(50);
        let t0 = Instant::now();
        q.enqueue(event("first", priority, 10), t0).unwrap();
        q.enqueue(event("second", priority, 10), t0 + Duration::from_secs(1))
            .unwrap();
        assert_eq!(
            visible_title(&q),
            Some("first"),
            "{priority:?}: equal priority must never preempt"
        );
        assert_eq!(waiting_titles(&q, priority as usize), vec!["second"]);
    }
}

#[test]
fn lower_priority_never_preempts() {
    let mut q = SingleSlotQueue::new(50);
    let t0 = Instant::now();
    q.enqueue(event("high", Priority::High, 10), t0).unwrap();
    q.enqueue(
        event("medium", Priority::Medium, 10),
        t0 + Duration::from_secs(1),
    )
    .unwrap();
    q.enqueue(event("low", Priority::Low, 10), t0 + Duration::from_secs(2))
        .unwrap();
    assert_eq!(visible_title(&q), Some("high"));
    assert_eq!(
        waiting_titles(&q, Priority::Medium as usize),
        vec!["medium"]
    );
    assert_eq!(waiting_titles(&q, Priority::Low as usize), vec!["low"]);
}

#[test]
fn paused_blocks_preemption_of_an_already_visible_item() {
    let mut q = SingleSlotQueue::new(50);
    let t0 = Instant::now();
    q.enqueue(event("medium", Priority::Medium, 10), t0)
        .unwrap();
    q.pause();
    q.enqueue(
        event("high", Priority::High, 10),
        t0 + Duration::from_secs(1),
    )
    .unwrap();
    assert_eq!(
        visible_title(&q),
        Some("medium"),
        "Paused wins unconditionally — nothing preempts while Paused"
    );
    assert_eq!(waiting_titles(&q, Priority::High as usize), vec!["high"]);
}

#[test]
fn preempted_item_requeues_ahead_of_existing_waiting_items_in_its_tier() {
    let mut q = SingleSlotQueue::new(50);
    let t0 = Instant::now();
    q.enqueue(event("low-visible", Priority::Low, 10), t0)
        .unwrap();
    q.enqueue(event("low-waiting", Priority::Low, 10), t0)
        .unwrap();
    assert_eq!(
        waiting_titles(&q, Priority::Low as usize),
        vec!["low-waiting"]
    );

    q.enqueue(
        event("high", Priority::High, 10),
        t0 + Duration::from_secs(1),
    )
    .unwrap();
    assert_eq!(visible_title(&q), Some("high"));
    assert_eq!(
        waiting_titles(&q, Priority::Low as usize),
        vec!["low-visible", "low-waiting"],
        "a preempted item must jump ahead of items that were already waiting in its tier"
    );
}

#[test]
fn preempted_item_restores_remaining_time_on_repromotion() {
    let mut q = SingleSlotQueue::new(50);
    let t0 = Instant::now();
    // base window 10s.
    q.enqueue(event("low", Priority::Low, 10), t0).unwrap();
    // preempt after 4s elapsed -> 6s of the low item's turn remains.
    let preempt_at = t0 + Duration::from_secs(4);
    q.enqueue(event("high", Priority::High, 5), preempt_at)
        .unwrap();
    assert_eq!(visible_title(&q), Some("high"));
    assert_eq!(waiting_titles(&q, Priority::Low as usize), vec!["low"]);

    // high's own 5s window elapses -> low re-promotes, carrying its
    // remaining 6s (not a fresh 10s window).
    let repromoted_at = preempt_at + Duration::from_secs(5);
    q.tick(repromoted_at);
    assert_eq!(visible_title(&q), Some("low"));

    // 5s into the re-promoted turn: still inside the 6s remaining budget.
    q.tick(repromoted_at + Duration::from_secs(5));
    assert!(
        q.visible.is_some(),
        "6s remained at re-promotion; only 5s have since elapsed"
    );
    // past the 6s remaining budget: rotates out — a fresh 10s window
    // would still have time left here, so this pins the restored value.
    q.tick(repromoted_at + Duration::from_secs(7));
    assert!(
        q.visible.is_none(),
        "remaining time (6s) must have been exhausted, not a fresh 10s window"
    );
}

#[test]
fn repromoted_preempted_medium_item_still_auto_expands() {
    let mut q = SingleSlotQueue::new(50);
    let t0 = Instant::now();
    q.enqueue(event("medium", Priority::Medium, 10), t0)
        .unwrap();
    q.enqueue(
        event("high", Priority::High, 5),
        t0 + Duration::from_secs(2),
    )
    .unwrap();
    assert_eq!(visible_title(&q), Some("high"));

    // high finishes -> medium re-promotes with the normal expand rule
    // for its priority (Medium, not Silenced): auto-expanded.
    q.tick(t0 + Duration::from_secs(7));
    assert_eq!(visible_title(&q), Some("medium"));
    match q.current_slot_state() {
        SlotState::Showing { expanded, .. } => assert!(
            expanded,
            "a re-promoted Medium item keeps the normal expand-on-promotion rule"
        ),
        SlotState::Empty => panic!("expected Showing"),
    }
}

#[test]
fn repromoted_preempted_low_item_stays_compact() {
    let mut q = SingleSlotQueue::new(50);
    let t0 = Instant::now();
    q.enqueue(event("low", Priority::Low, 10), t0).unwrap();
    q.enqueue(
        event("high", Priority::High, 5),
        t0 + Duration::from_secs(2),
    )
    .unwrap();
    assert_eq!(visible_title(&q), Some("high"));

    q.tick(t0 + Duration::from_secs(7));
    assert_eq!(visible_title(&q), Some("low"));
    match q.current_slot_state() {
        SlotState::Showing { expanded, .. } => {
            assert!(
                !expanded,
                "Low must promote compact even after a re-promotion"
            )
        }
        SlotState::Empty => panic!("expected Showing"),
    }
}

#[test]
fn chained_preemption_returns_items_in_priority_order() {
    // Low visible -> Medium preempts -> High preempts the Medium; both
    // preempted items sit at the heads of their tiers and return in
    // priority order (Medium before Low) once the High card is done.
    let mut q = SingleSlotQueue::new(50);
    let t0 = Instant::now();
    q.enqueue(event("low", Priority::Low, 10), t0).unwrap();
    q.enqueue(
        event("medium", Priority::Medium, 10),
        t0 + Duration::from_secs(1),
    )
    .unwrap();
    assert_eq!(visible_title(&q), Some("medium"));
    assert_eq!(waiting_titles(&q, Priority::Low as usize), vec!["low"]);

    q.enqueue(
        event("high", Priority::High, 10),
        t0 + Duration::from_secs(2),
    )
    .unwrap();
    assert_eq!(visible_title(&q), Some("high"));
    assert_eq!(
        waiting_titles(&q, Priority::Medium as usize),
        vec!["medium"]
    );
    assert_eq!(waiting_titles(&q, Priority::Low as usize), vec!["low"]);

    // high's 10s window (from t0+2s) elapses well past t0+30s: medium
    // (the higher remaining tier) returns first, not low.
    q.tick(t0 + Duration::from_secs(30));
    assert_eq!(visible_title(&q), Some("medium"));

    // medium's restored remaining turn (9s, from its 1s-elapsed
    // preemption) elapses from its t0+30s re-promotion: low returns.
    q.tick(t0 + Duration::from_secs(40));
    assert_eq!(visible_title(&q), Some("low"));
}

#[test]
fn silenced_second_high_does_not_preempt_the_breakthrough_visible() {
    // While Silenced, only High can ever be Visible — a second High
    // arrival is EQUAL priority, so it must not preempt either,
    // exercising the interaction between Silenced and preemption
    // explicitly rather than resting on the general equal-never-
    // preempts case alone.
    let mut q = SingleSlotQueue::new(50);
    q.silence();
    let t0 = Instant::now();
    q.enqueue(event("h1", Priority::High, 10), t0).unwrap();
    assert_eq!(visible_title(&q), Some("h1"));

    q.enqueue(event("h2", Priority::High, 10), t0 + Duration::from_secs(1))
        .unwrap();
    assert_eq!(visible_title(&q), Some("h1"));
    assert_eq!(waiting_titles(&q, Priority::High as usize), vec!["h2"]);
}

#[test]
fn silence_onset_leaves_visible_low_and_a_medium_arrival_buffers_not_preempts() {
    // Silenced does NOT evict the card that was Visible when silence
    // began (it finishes its natural turn, same as Paused) — so for
    // that onset window a Medium/Low CAN be Visible while Silenced,
    // and a strictly-higher-but-not-High arrival must buffer, not
    // preempt: only High promotes while Silenced, preemption
    // included.
    let mut q = SingleSlotQueue::new(50);
    let t0 = Instant::now();
    q.enqueue(event("l1", Priority::Low, 10), t0).unwrap();
    assert_eq!(visible_title(&q), Some("l1"));

    q.silence();
    q.enqueue(
        event("m1", Priority::Medium, 10),
        t0 + Duration::from_secs(1),
    )
    .unwrap();
    assert_eq!(visible_title(&q), Some("l1"));
    assert_eq!(waiting_titles(&q, Priority::Medium as usize), vec!["m1"]);

    // A High arrival, by contrast, still preempts the leftover Low —
    // Breakthrough applies to preemption too.
    q.enqueue(event("h1", Priority::High, 10), t0 + Duration::from_secs(2))
        .unwrap();
    assert_eq!(visible_title(&q), Some("h1"));
    assert_eq!(waiting_titles(&q, Priority::Low as usize), vec!["l1"]);
}
