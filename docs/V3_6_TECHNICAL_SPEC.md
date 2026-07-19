# notchtap — v3.6 technical spec: permanent rotating overlay (v0 draft)

operationalizes `IMPLEMENTATION_PLAN.md` §3.6 into code-level specifics.
like the v1/v2/v3 specs, this is a working draft — adjust freely as
implementation surfaces friction; if a change is a *decision* change
(a default, a scope boundary), it goes to `ARCHITECTURE.md` /
`IMPLEMENTATION_PLAN.md` §3.6 instead. §3.6 itself calls this out as
still-needed: *"a code-level technical spec (mirroring
`archive/V3_TECHNICAL_SPEC.md`'s precedent) for the wire schema change, the
`NSWindowCollectionBehavior` call, and the global-hotkey registration
mechanism."* This is that spec.

this is the **biggest architectural change since v1**. it does not
replace `archive/V3_TECHNICAL_SPEC.md` (telegram/notifier — shipped, tested,
unrelated seam) — it replaces the *display* half of v1/v2/v3.5: the
3-item TTL stack, the pill/grow/mini shape table, and the frontend's
self-timed dismissal.

read `IMPLEMENTATION_PLAN.md` §3.6 first — this doc assumes its
decisions as locked and does not re-argue them. three implementation
calls below fill gaps §3.6 deliberately left open; each is marked
**(this spec's call)** and is *not* re-litigating §3.6, just resolving
a detail §3.6 didn't pin.

---

## 0. scope

- replace `NotificationQueue`'s 3-item FIFO/TTL stack with a
  single-slot, priority-ordered, some-items-recur queue.
- replace the frontend's self-timed enter/hold/exit model with a
  rust-authoritative "what's on screen right now" push.
- replace `.pill`/`.grow`/`.mini` + `getMorphShape` with one slot
  renderer keyed on `priority` + `expanded`.
- add a macOS global hotkey (rust-side only) that toggles manual
  expand.
- add `NSWindowCollectionBehavior` so the window survives Spaces
  switches and fullscreen apps.
- **not in this pass**: any actual evergreen/idle content source
  (world clock, calendar, order tracking — §3.6 defers these
  explicitly; this spec only builds the *mechanism* that would host
  them), the exact hotkey combo (placeholder chosen below, changeable
  in one constant), `CONTEXT.md` rewrite (listed as a prerequisite by
  §3.6 — see §9 below), multi-monitor support.

---

## 1. this spec's implementation calls

§3.6 left these open; each was reasoned through, then adversarially
reviewed once before freezing the interfaces below (parallel agents
build against these, so they're pinned here rather than left to
whoever gets to that file first). Two of the four were revised after
that review surfaced real problems with the first draft — both
revisions are called out explicitly where they land (§4.4), not
silently folded in, since other agents may have already read the
first-draft reasoning:

1. **rust becomes the sole dismissal authority.** today,
   `useVisibleNotifications.ts` self-times its own exit via
   `setTimeout(ttlSecs * 1000)`. §3.6 requires a currently-visible
   item's effective duration to change *while it's showing* (manual
   hotkey expand → "longer while expanded" — plan §3.6). A client-side
   timer armed at mount time cannot react to that after the fact.
   **fix**: rotation is decided in the rust core by `tick()`, full
   stop — driven since plan 015 by deadline-based wakeups
   (`next_deadline` + a `tokio::sync::Notify`, see §4.3) rather than a
   fixed interval; the frontend holds no duration logic at all, just
   plays a fixed-length CSS transition whenever the pushed slot state
   changes. §5 covers this in full — it is the largest single behavior
   change in this spec. (unchanged by review — no issue found.)
2. **supersede-while-visible grants a capped extension when remaining
   time is low, rather than resetting the clock, leaving it fully
   untouched, or granting unbounded top-ups.** a `Recurring` item's
   topic can receive fresh data while it is the currently-visible item
   (e.g. a live score every 10s against an 8s rotation). **revised
   twice after review**: draft 1 left `promoted_at` completely
   untouched on every update, reasoning only about monopolization risk
   (a fast-updating topic never rotating out under a full-reset rule).
   That missed the opposite failure: fresh content landing with, say,
   0.1s of rotation left flashes and vanishes almost invisibly. Draft 2
   fixed that by topping up remaining time to a floor on each
   supersede — but a hand-traced simulation of rapid supersedes showed
   this was *itself* unbounded: topping up on every below-floor
   supersede, with no cap on how many times that can happen, let the
   item never rotate out at all under sustained fast updates — exactly
   the failure draft 1's reasoning was trying to prevent, just via a
   different mechanism. The final fix tracks accumulated extension
   separately from `promoted_at` and caps it: an item can be topped up
   toward the floor, but the *total* extension across every supersede
   during one visible turn is hard-capped, so the effective window is
   always `base_window + min(needed, cap)`, provably bounded. See
   §4.4.
3. **the enqueue fast-path requires all three priority tiers empty,
   not just the item's own tier.** today's queue only lets a new push
   skip straight to Visible when the slot is free *and nothing at all
   is waiting* — deliberately, to never let a new arrival jump a
   line that already has other items in it. Generalizing that
   correctly to three tiers means the fast-path checks all three, not
   just its own — otherwise a same-tier item already waiting could
   get jumped by same-priority latecomer via the fast path while an
   opposite-tier waiter looks on, and cross-tier promotion order would
   depend on *when* things arrived relative to tick(), not just
   priority. One promotion code path (`tick()`'s pop-highest) is
   simpler to reason about than two. See §4.2. Review confirmed this
   is purely a latency optimization (worst case: one heartbeat wake of
   added delay under the deadline-based wakeup model, plan 015 — see
   §4.3) and never changes final promotion ordering, since `tick()`
   still runs on every heartbeat wake regardless of whether the fast
   path ever fires — so its absence in any given case is harmless,
   just slower by one wake. (unchanged by review.)
4. **a priority-changing supersede moves the item to the back of its
   new tier; a same-priority supersede never moves position.** the
   first draft let a topic's `priority` field change in place while
   the item physically stayed in its original tier's `VecDeque` —
   described at the time as "a deliberate, narrow inconsistency, not
   a bug." **review found that framing wrong**: because promotion pops
   by physical tier (array index), not by the `priority` field on the
   item it holds, that draft meant a topic upgraded to `High` would
   keep sitting in the `Low` tier indefinitely — starved behind every
   `Medium`/`High` item and every earlier `Low` item, directly
   breaking the queue's own tier-strict promotion invariant for
   exactly the kind of item most likely to want a priority change (an
   escalating situation). This was a correctness bug, not a stylistic
   judgment call. See §4.4.

---

## 2. glossary deltas (for whoever updates `CONTEXT.md` — §9)

not edited here (§3.6 marks the glossary rewrite as a prerequisite,
separate task) but the new/changed terms this spec introduces, for
that pass:

- **Slot** — the single Visible position (replaces the 3-item
  "Visible... ordered as a stack" definition).
- **Priority** — `Low | Medium | High` on every Event, independent of
  `EventType`. Governs promotion order only, never interrupts a
  currently-visible item.
- **Rotation** — replaces TTL as the display-duration concept; measured
  from promotion, extended while Expanded.
- **Recurring** — a Rotation kind that requeues to the back of its own
  priority tier after its turn, instead of being dropped. Bounded by
  supersession or external end, not a clock.
- **Topic** — the supersession identity carried by a Recurring event;
  a fresh Event sharing a Topic updates the existing item in place
  (Waiting or Visible) rather than adding a new one.
- **Expanded** — a Slot's optional grown state: automatic for `High`
  priority, manual (global hotkey) for everything else.
- **Paused** — unchanged definition, now gates the single Slot instead
  of a 3-item cap.

---

## 3. wire & type changes

### 3.1 `Priority` — from single-variant to real

```rust
// event.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low,
    Medium,
    High,
}
// derive order matters: Low < Medium < High (declaration order == Ord).
// this file's variant order is a *load-bearing* code fact — a rustfmt
// or refactor pass must never reorder these variants. add a unit test
// (`priority_ord_is_low_lt_medium_lt_high`) pinning it, not just a
// comment.
```

removes today's single-variant `Priority::Normal`. every call site
constructing an `Event` gains a real priority to pick — see §3.4 for
each source's default.

### 3.2 `RotationSpec` — replaces bare `ttl_secs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RotationSpec {
    OneShot { ttl_secs: u64 },
    Recurring { display_secs: u64 },
}
```

- `OneShot` is today's ttl semantics exactly: shown once for
  `ttl_secs`, then dropped forever.
- `Recurring` shows for `display_secs`, then requeues to the **back
  of its own priority tier** in Waiting (§4.3) rather than being
  dropped — unless a same-topic supersede already replaced it in
  place (§4.4), in which case there is nothing left to requeue.
- the wire (`/notify`, cli) can only ever produce `OneShot` — see
  §3.3. `Recurring` is constructed by internal Rust sources only
  (poller today; any future evergreen source later — §3.6's explicit
  "generic enough" requirement, satisfied by this type existing, not
  by anything consuming it yet).

### 3.3 `Event` — gains `rotation` and `topic`, drops `ttl_secs`

```rust
pub struct Event {
    pub id: Uuid,
    pub event_type: EventType,
    pub priority: Priority,
    pub rotation: RotationSpec,   // was: ttl_secs: u64
    pub topic: Option<String>,   // NEW — supersession identity
    pub payload: EventPayload,
}
```

**`topic` shape — this spec's call, resolving §3.6's "must stay
generic" requirement**: free-form `Option<String>`, not a structured
`(EventType, id)` pair. Reasoning: `EventType` is deliberately
decoupled from `Priority` already (§3.6's own words — "not every
high-priority thing is a score"), and tying supersession identity to
`EventType` would smuggle that coupling back in through a side door.
A future evergreen source (a calendar item, a world clock) may not
even want a distinct `EventType` variant. Convention, not enforced by
the type system: sources namespace their own topics
(`"espn:{match_id}"`, `"clock:local"`) to avoid accidental collision
across sources — this is a documentation convention like `EventType`
snake-case naming, not a new validation layer.

`topic: None` means "never superseded, never deduped" — the correct
default for one-shot pushes (cli, cmux relay), which have no notion of
"the same thing happening again."

wire (`/notify`) request shape, `http.rs`'s `NotifyRequest`
(`src-tauri/src/http.rs`, current as of plan 035):

```rust
#[derive(Deserialize)]
struct NotifyRequest {
    title: Option<String>,
    body: Option<String>,
    priority: Option<Priority>,
    #[serde(default)]
    signal: EventSignal,
    source: Option<RequestSource>,
    // plan 035: display-only rich-relay fields, both optional. A missing
    // field deserializes to None (serde special-cases Option), so an old
    // {title, body} payload is byte-identical.
    subtitle: Option<String>,
    details: Option<Vec<DetailItem>>, // DetailItem { label, value }
}
```

`priority` absent → falls back to per-source config rather than a
single hardcoded default: `state.manual_default_priority` when
`source` is absent/`None`, `state.cmux_priority` when `source` is
`Cmux` (both `Config` fields, independently editable from the settings
window — see §3.4 below). `rotation` is never accepted from the wire —
always constructed server-side from the resolved source's ttl config.
`topic` is never accepted from the wire — always `None`. This keeps the
http schema narrow while satisfying "every source feeds this one
queue" (§3.6) without opening `Recurring`/`topic` to untrusted/external
input in this pass.

`subtitle` and `details` (plan 035) are the only *display* metadata a
`/notify` caller may set — `source`/`category`/`published`/`link` stay
poller-only. both are sanitized/capped server-side before they reach
`EventMeta` (the trust boundary, since `details` originates in untrusted
hook input): `subtitle` ≤ 120 chars (empty string → `None`); `details`
≤ 8 pairs with empty-label pairs dropped first, each label ≤ 40 and
value ≤ 200 chars, all truncated with a `…`. absent → `None`/empty, so
old payloads are unchanged. a `DetailItem` is `{label, value}` — its own
field names on the wire, not camelCased. these caps exist because the
card lives in a fixed 500×300 window; if that ever grows, revisit the
numbers, not the mechanism.

the outbound `SlotState::Showing` wire (`src-tauri/src/event.rs`) also
carries the v5.1 `link: Option<String>` field — the target for the ⌃⇧O
open-story hotkey on news items; absent for non-news sources — and,
since plan 035, `subtitle` + `details` mirrored from `EventMeta`: the
manifest renders `subtitle` as its own cell and one cell per `details`
pair (Layout A), plain text (details are untrusted), generic branch only.

### 3.4 default priority per source

**superseded 2026-07-17 (v6/v6.1):** the table below described the
original, hardcoded-in-rust defaults. Each source's default is now a
`Config` field, independently configurable from the settings window
(`docs/V5_TECHNICAL_SPEC.md`) rather than a compile-time constant —
see `src-tauri/src/config.rs` (`espn_priority`, `rss_priority`,
`manual_default_priority`, `cmux_priority`) and `src-tauri/src/http.rs`
(`notify_handler`'s `req.priority.unwrap_or(default_priority)`, where
`default_priority` is looked up by `req.source`). The *values* below
are still the shipped defaults (`Config::default`) — only "hardcoded"
became "configurable, defaulting to this."

| source | default priority | rationale |
|---|---|---|
| `/notify` http, no `source` field (manual/cli) | request's `priority` field, else `manual_default_priority` (`Medium`) | `Config::default_manual_default_priority` |
| `/notify` http, `source: "cmux"` | request's `priority` field, else `cmux_priority` (`High`) | `Config::default_cmux_priority`. **Caveat:** cmux's own built-in notification-command setting (`docs/IMPLEMENTATION_PLAN.md` §2.2, external to this repo) always passes `--priority high` explicitly today, so this config default is currently a no-op for the real relay unless that external command is edited to drop the flag. |
| cli `--priority low\|medium\|high` | folds into the same http field; when the flag is omitted the cli sends no `priority` field at all, so the source-appropriate default above applies (see §6) |
| espn poller: `ScoreUpdate` (goal) | `espn_priority` (`High`) | `Config::default_espn_priority` — was unconditional `High`, now configurable |
| espn poller: `MatchState` (kickoff/half/full-time, cards) | `espn_priority` (`High`) | same field, both event kinds share one per-source default |
| rss poller | `rss_priority` (`Low`) | `Config::default_rss_priority` — added with the news-poller phase, not present in the original v3.6 pass |

~~no source in this pass constructs `Recurring`~~ — **amended
2026-07-19 (plan 039):** the espn poller is now the first `Recurring`/
Topic producer, behind the opt-in config flag `espn_live_card`
(default `false`, `Config::default_espn_live_card`; no
`settings::validate` rule — a plain bool). when on, each live match's
events carry `topic: Some("espn:{league}:{match_id}")` and ride
`Recurring { display_secs: espn_ttl_secs }` while in play, so
kickoff/goal/card/half-time supersede into one updating card; the
full-time event is emitted `OneShot` on the same Topic, retiring the
card via the ordinary one-shot path. when off (the default), the
poller emits `topic: None` one-shots exactly as before. `Low` is
now live (the rss poller's default), no longer test-only.

### 3.5 wire-out payload (`notification-promoted` → replaced by `slot-state`)

see §5.2 — the payload shape changes substantially because the event
itself changes (§5.1), not just because of the new fields. Not a
drop-in field addition to `NotificationPayload`.

---

## 4. the queue (`queue.rs`) — LLD

### 4.1 types

```rust
pub struct QueueItem {
    pub event: Event,
    pub enqueued_at: Instant,     // fifo tie-break within a tier; unchanged role
    pub promoted_at: Option<Instant>,
    pub extension_secs: u64,      // accumulated supersede-while-visible top-up
                                   // (§4.4); 0 until first promoted, reset to 0
                                   // on each new promotion — see promote_next
}

pub struct SingleSlotQueue {
    visible: Option<QueueItem>,               // was: VecDeque<QueueItem> capped at max_concurrent
    waiting: [VecDeque<QueueItem>; 3],         // indexed by Priority as usize: Low=0, Medium=1, High=2
    max_queued_per_tier: usize,                // §4.6 — cap changes shape, not just size
    paused: bool,
    expanded: bool,                            // current slot's expand state (§5)
    last_emitted: Option<SlotState>,           // §5.1's change-guard — see slot_state_if_changed
}
```

**today's `promoted: Vec<Event>` field is dropped, not ported
(caught in review)**: the original draft carried it over unchanged
from `NotificationQueue`, reasoning it was "unchanged: exactly-once
promotion report" — but the report-what-changed job in this redesign
belongs entirely to `slot_state_if_changed` (§5.1), which compares
against `last_emitted`, not an accumulating log. `self.promoted.push`
in `promote_next` (§4.3) would have pushed onto a `Vec` **nothing ever
drains** in the new design — the old `take_promoted()` call that
consumed it is gone, since emission no longer happens through a
promotion-report drain at all. Left in, this is a silent unbounded
memory leak in a long-running background app: every promotion, ever,
accumulating forever. There is no replacement drain to add — the field
and the `self.promoted.push(...)` line in `promote_next` (§4.3) are
both deleted outright.

renamed from `NotificationQueue` to `SingleSlotQueue` — the old name
now lies about the shape (there is no longer a fifo queue of visible
items). rename the module-level type; keep the module filename
`queue.rs` (no reason to churn the file path).

`waiting` as a fixed `[VecDeque<QueueItem>; 3]` rather than three
named fields: promotion needs to iterate tiers high-to-low uniformly
(§4.2), and a fixed array indexed by `priority as usize` keeps that a
one-line loop instead of three copy-pasted branches. `Priority`'s
`#[repr(usize)]`-equivalent ordering (declaration order, pinned in
§3.1) *is* the array index — this is the second reason variant order
is load-bearing, not just `Ord`.

### 4.2 promotion — pop order and the fast-path

```rust
impl SingleSlotQueue {
    fn pop_highest_priority_waiting(&mut self) -> Option<QueueItem> {
        for tier in (0..3).rev() {           // High=2 first, then Medium=1, then Low=0
            if let Some(item) = self.waiting[tier].pop_front() {
                return Some(item);
            }
        }
        None
    }

    fn all_tiers_empty(&self) -> bool {
        self.waiting.iter().all(|t| t.is_empty())
    }
}
```

**fast-path rule (this spec's call #3, §1)**: `enqueue()`'s
immediate-promote fast path fires only when `visible.is_none() &&
!paused && self.all_tiers_empty()` — *all three* tiers, not just the
incoming event's own tier. This is the direct generalization of
today's tested invariant (`fast_path_never_jumps_waiting_items`):
a slot freeing up between heartbeat ticks must never let a new push
jump *anything* already waiting, regardless of tier. The alternative
(bypass fast-path allowed when only lower-priority tiers are
non-empty) was rejected: it creates a second promotion code path
alongside `tick()`'s `pop_highest_priority_waiting`, and that second
path would need its own "did I just jump someone" reasoning instead of
inheriting it for free from "there was nothing to jump."

practical effect: once *anything* is waiting in any tier, every
subsequent enqueue — even a fresh `High` push — joins its own tier's
back and waits for the next `tick()` to be promoted via
`pop_highest_priority_waiting`. A `High` arrival still promotes next
(ahead of older `Low`/`Medium` waiters) because of tier ordering in
`tick()`, just not synchronously at enqueue time. This matches §3.6's
own wording precisely: *"higher-priority Waiting items are promoted
next... jumps the Waiting line for the next promotion"* — "next
promotion" is `tick()`'s job, not `enqueue()`'s.

### 4.3 tick — rotate then promote, one call

```rust
pub fn tick(&mut self, now: Instant) {
    self.rotate_out_if_elapsed(now);
    self.promote_next(now);
}

fn rotate_out_if_elapsed(&mut self, now: Instant) {
    let Some(item) = &self.visible else { return };
    let promoted_at = item.promoted_at.expect("visible items have promoted_at");
    // §4.4's supersede-while-visible top-up accumulates into
    // extension_secs rather than mutating promoted_at directly — the
    // effective window is base + accumulated extension, capped there.
    let window = item.event.rotation_window(self.expanded) + item.extension_secs; // §5.3, §4.4
    if now.duration_since(promoted_at).as_secs() < window {
        return; // not yet due
    }
    let item = self.visible.take().expect("checked Some above");
    if let RotationSpec::Recurring { .. } = item.event.rotation {
        let tier = item.event.priority as usize;
        self.waiting[tier].push_back(item); // requeue to the BACK of its own tier
    }
    // OneShot: item is dropped here, forever — no requeue.
}

fn promote_next(&mut self, now: Instant) {
    if self.visible.is_some() || self.paused {
        return;
    }
    if let Some(mut item) = self.pop_highest_priority_waiting() {
        item.promoted_at = Some(now);
        item.extension_secs = 0; // fresh promotion starts with no top-up
        self.visible = Some(item);
    }
}
```

single-call rotate-then-promote (rather than two heartbeat-driven
phases) is deliberate and matches today's `expire_and_promote` shape
exactly — no correctness gap versus a two-phase split, because
`promote_next` already checks `visible.is_some()` first: if rotation
didn't free the slot this tick, promotion is a no-op this tick, same
as today. The only behavior *change* from today's `expire_and_promote`
is that a `Recurring` item's departure conditionally requeues instead
of unconditionally vanishing — everything else is a 1:1 port with
`VecDeque`→`Option` and single-tier→3-tier substitutions.

`paused` gates `promote_next` only, exactly like today — `Paused` still
means "promotion frozen, already-visible item still finishes its
natural rotation and exits" (§3.6: "paused semantics carry over
unchanged").

**driving `tick()` (plan 015 — supersedes the original 250ms heartbeat
description below and in §5.1):** `lib.rs`'s heartbeat no longer polls
`tick()` on a fixed interval. It calls `queue.next_deadline()` — the
visible item's `promoted_at + rotation_window(expanded) +
extension_secs`, or `None` when the slot is empty — and sleeps until
that instant (plus a small grace), or forever if `None`. A
`tokio::sync::Notify` shared with every mutation site (the `/notify`
handler, both pollers, and the pause/dismiss/expand-toggle/skip
handlers) wakes the sleep early whenever any of them runs, so the next
loop iteration re-ticks and recomputes the deadline immediately rather
than waiting out a stale sleep. Net effect: the same rotate-then-promote
behavior as before, but with ~0 wakeups while the queue is idle instead
of ~4/second.

### 4.4 supersession — topic-based merge at enqueue

```rust
pub fn enqueue(&mut self, event: Event) -> Result<(), QueueError> {
    if let Some(topic) = event.topic.clone() {
        if self.supersede_if_topic_matches(&topic, &event) {
            return Ok(()); // merged in place; not a new item, no cap check needed
        }
    }
    self.enqueue_new(event) // today's cap-checked path, generalized to 3 tiers (§4.6)
}

fn supersede_if_topic_matches(&mut self, topic: &str, fresh: &Event) -> bool {
    if let Some(visible) = &mut self.visible {
        if visible.event.topic.as_deref() == Some(topic) {
            // update content in place; a capped extension is granted if
            // remaining time is low (promoted_at itself is never
            // touched — see "minimum remaining time" below)
            visible.event.payload = fresh.payload.clone();
            visible.event.priority = fresh.priority;
            visible.event.rotation = fresh.rotation;
            self.top_up_visible_remaining_time();
            return true;
        }
    }
    // indexed by plain tier number throughout (not `self.waiting.iter_mut()`)
    // deliberately: an `iter_mut()`-based loop holds a mutable borrow of
    // `self.waiting` for its whole body, which conflicts with the
    // cross-tier branch's separate `self.waiting[new_tier_idx]` borrow —
    // indexing keeps each borrow scoped to a single statement instead.
    for tier_idx in 0..3 {
        let Some(pos) = self.waiting[tier_idx]
            .iter()
            .position(|i| i.event.topic.as_deref() == Some(topic))
        else {
            continue;
        };
        let new_tier_idx = fresh.priority as usize;
        if new_tier_idx == tier_idx {
            // same tier: update in place, keep position (§ below)
            let existing = &mut self.waiting[tier_idx][pos];
            existing.event.payload = fresh.payload.clone();
            existing.event.priority = fresh.priority;
            existing.event.rotation = fresh.rotation;
        } else {
            // priority changed: this is a structural move, not a
            // content update — remove from the old tier and push to
            // the BACK of the new tier (revised — see "cross-tier
            // moves" below, replaces this spec's original call)
            let mut existing = self.waiting[tier_idx]
                .remove(pos)
                .expect("position just found");
            existing.event.payload = fresh.payload.clone();
            existing.event.priority = fresh.priority;
            existing.event.rotation = fresh.rotation;
            self.waiting[new_tier_idx].push_back(existing);
        }
        return true;
    }
    false
}
```

**minimum remaining time on a visible-item supersede (revised twice —
see both notes below)**: the first draft of this spec left
`promoted_at` completely untouched on a visible supersede, reasoning
only about the monopolization risk (a fast-updating topic never
rotating out). Adversarial review surfaced the opposite failure mode
that reasoning missed: if fresh content lands with, say, 0.1s left in
the rotation window, the user sees new content flash and vanish almost
immediately — effectively invisible, not "successfully rotating." The
first fix attempted to close that gap by pushing `promoted_at` forward
by a per-supersede deficit whenever remaining time fell below a floor.

**that first fix was itself broken in a way a sign-check alone didn't
catch (caught on a second, hand-traced review pass)**: pushing
`promoted_at` forward by *exactly* the deficit needed to reach the
floor, repeated on every supersede that lands below the floor, means a
topic updating faster than the floor decays can push `promoted_at`
forward indefinitely — hand-tracing 100 seconds of supersedes arriving
every 0.5s against an 8s window with a 2s floor showed the item never
rotates out at all. The claim "bounded, not unbounded" in the first
revision was false; it was exactly the unbounded monopolization the
whole mechanism exists to prevent, just via floor-topping instead of a
naive full reset. The floor logic needs a **hard cap on total
accumulated extension**, tracked separately from `promoted_at` itself,
not re-derived from "how much is currently missing" on every call:

```rust
const MIN_REMAINING_ON_SUPERSEDE_SECS: u64 = 2;
const MAX_EXTENSION_ON_SUPERSEDE_SECS: u64 = 6; // hard cap: an item can
    // never show for more than base_window + this, no matter how many
    // supersedes land, closing the actual monopolization risk this
    // whole mechanism exists to bound.

fn top_up_visible_remaining_time(&mut self) {
    let Some(item) = &mut self.visible else { return };
    let Some(promoted_at) = item.promoted_at else { return };
    let base_window = item.event.rotation_window(self.expanded);
    let effective_window = base_window + item.extension_secs;
    let elapsed = Instant::now().saturating_duration_since(promoted_at).as_secs();
    let remaining = effective_window.saturating_sub(elapsed);
    if remaining < MIN_REMAINING_ON_SUPERSEDE_SECS {
        let deficit = MIN_REMAINING_ON_SUPERSEDE_SECS - remaining;
        let room = MAX_EXTENSION_ON_SUPERSEDE_SECS.saturating_sub(item.extension_secs);
        // grant at most what's left in the extension budget — once the
        // cap is spent, further supersedes still update content (the
        // caller does that unconditionally before calling this function)
        // but get no more time; the item rotates out on the original
        // budgeted schedule regardless of how many more updates land.
        item.extension_secs += deficit.min(room);
    }
    // promoted_at itself is never mutated — rotation math (§4.3) reads
    // `base_window + extension_secs` as the effective window, so the
    // "current time since promotion" measurement stays a single,
    // unambiguous quantity instead of one that's been repeatedly
    // nudged by an unbounded number of prior supersedes.
}
```

`QueueItem` gains one field to carry this: `extension_secs: u64`,
defaulted to `0` at promotion (§4.3's `promote_next` sets it alongside
`promoted_at`), read by `rotate_out_if_elapsed`'s elapsed check as
`item.event.rotation_window(self.expanded) + item.extension_secs`
rather than the bare `rotation_window(...)` call shown earlier in §4.3
— that call site is updated by this revision too, not just this
function.

a fast-updating topic can now monopolize the slot for **at most**
`MAX_EXTENSION_ON_SUPERSEDE_SECS` beyond its own base window, provably
— traced by hand against the same 100-second rapid-supersede scenario
that broke the first revision: with the cap in place, the item rotates
out at `base_window + MAX_EXTENSION_ON_SUPERSEDE_SECS` exactly, every
time, regardless of how many supersedes arrive in between. This is the
actual "bounded, not unbounded" property the first revision claimed
but did not deliver.

**cross-tier moves on a priority-changing supersede (revised — this
spec's original draft called this "a deliberate, narrow
inconsistency, not a bug"; that was wrong and is corrected here)**:
the original draft let a topic's `priority` field change in place
while the item physically stayed in its original tier's `VecDeque`.
Because `pop_highest_priority_waiting` (§4.2) pops by **array
index** (physical tier), not by the `priority` field on the item it
holds, that draft meant a topic upgraded from `Low` to `High` via
supersede would keep sitting in the `Low` tier — starved behind every
`Medium` and `High` item, and every other `Low` item ahead of it,
indefinitely. That directly breaks the queue's own core invariant
(§4.2: tier-strict promotion order) for any topic that ever gets
reprioritized, which is exactly the kind of item most likely to *want*
a priority change (an escalating situation). This was a correctness
bug, not a stylistic judgment call, and the fix above removes the item
from its old tier and pushes it to the back of the new tier on any
priority change — a structural move, not a content update, so it does
not fall under "keep position to stop the topic dodging its own turn"
(that rule is specifically about a topic re-supersede at the *same*
priority trying to jump the queue by looking like a fresh arrival;
an actual priority change is a different item-identity concern and
gets FIFO placement at the back of its new, correct tier, same as any
other arrival there).

### 4.5 pause/resume

unchanged in spirit from today: `pause()` sets `paused = true`,
`resume()` sets it `false`. Resume's "promote immediately, not on the
next heartbeat tick" behavior (today's tray handler calls
`expire_and_promote(Instant::now())` right after `resume()`) ports
directly: the tray handler calls `queue.tick(Instant::now())` right
after `resume()`.

### 4.6 queue-full behavior — per-tier cap

today's single `max_queued` cap (50) becomes `max_queued_per_tier`
applied independently to each of the three `VecDeque`s — **not** one
shared cap of 50 split three ways. Rationale: a burst of `Low`
pushes must not be able to starve `High`'s own waiting room by filling
a shared cap first. `429` (`QueueError::QueueFull`) is returned only
when *that event's own tier* is at cap — an incoming `High` push can
still be accepted while `Low`'s tier is full. Config gains
`max_queued_per_tier` (replacing `max_queued`); default carries the
old value (50) unchanged in spirit — three tiers now each get their
own 50-item room rather than sharing one.

---

## 5. rust-authoritative display (`event.rs`, `lib.rs`, frontend)

### 5.1 the wire-out event: `slot-state`, replacing `notification-promoted`

today's model: rust emits `notification-promoted` **once per item, at
promotion**, and the frontend runs its own enter/hold/exit clock from
there, blind to anything rust does afterward. §3.6 breaks this
model directly (`expanded` can change a currently-visible item's
remaining duration; recurring items conceptually don't have a fixed
duration the frontend could precompute anyway). This spec's call #1
(§1.1) replaces it:

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "state")]
pub enum SlotState {
    Empty,
    Showing {
        id: Uuid,
        title: String,
        body: String,
        event_type: EventType,
        priority: Priority,
        expanded: bool,
    },
}
```

emitted as the `slot-state` tauri event, **whenever it changes**
(promotion, rotation-out with nothing to replace it, or an expand-state
flip on the current item) — not once-per-lifecycle like
`notification-promoted`. This is a state push, not an event stream:
the frontend's job shrinks to "render whatever `slot-state` last
said," full stop.

emission sites, replacing every current `emit_promoted` call:
- the heartbeat (`lib.rs`'s `spawn_heartbeat`): after `queue.tick(now)`,
  emit only if the slot state actually changed. Plan 015 replaced the
  original fixed-250ms-interval version of this loop with a
  deadline-sleep-plus-wake design (§4.3); the emission behavior here is
  unchanged by that.
- the tray pause/resume handler (`lib.rs`'s `build_tray`): same, after
  calling `tick()`.
- the http handler (`http.rs`): after `enqueue()`, same — an accepted
  push that fast-path-promotes immediately (empty queue) must show up
  immediately, not wait for the heartbeat's next scheduled wakeup,
  exactly like today's promotion-on-enqueue case.
- the new hotkey handler (§7.1's `toggle_manual_expand`): emits
  immediately on every toggle — this is the primary reason emission
  moved off "once per promotion."

**change-guard ownership (revised — the original draft left this
underspecified)**: the first draft described "a `last_emitted:
Option<...>` comparison" guarding a free function, `emit_slot_state`,
without saying where that comparison's state actually lives. A bare
function cannot safely own mutable comparison state on its own
(a module-level `static` would be a hidden-global-mutable-state smell
even in an otherwise single-instance app, and there's no reason to
reach for one here). The comparison state belongs on the type that
already owns everything it needs to compute it — `last_emitted` is
already declared on `SingleSlotQueue` itself in §4.1's struct, for
exactly this reason:

```rust
impl SingleSlotQueue {
    /// Returns the current slot state only if it differs from the last
    /// value returned by this method — every call site (heartbeat,
    /// tray, http, hotkey) calls this right after mutating the queue
    /// and emits only on `Some`. This is the single comparison point;
    /// no call site keeps its own copy of "what did I last send."
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
                expanded: self.expanded,
            },
        }
    }
}
```

`SlotState` derives `PartialEq` directly (added above) so the
comparison in `slot_state_if_changed` type-checks. This has one
transitive requirement worth calling out explicitly since it touches a
file this spec doesn't otherwise modify: **`EventType` (`event.rs`)
must also derive `PartialEq`** — it's a field inside `SlotState::
Showing`, and today's `EventType` derives only `Debug, Clone,
Serialize, Deserialize` (checked against the current source; no
`PartialEq`). Adding it is a trivial, purely-additive derive with no
behavior change to anything that exists today — but it's still a
one-line edit to a struct workstream A doesn't otherwise touch the
definition of, so it's called out here rather than left for whoever
hits the compile error first.

This also simplifies the workstream boundary from the
original draft: A owns `slot_state_if_changed` entirely inside the
type it already owns, and every other call site (owned by B or D) is
just `if let Some(state) = queue.slot_state_if_changed() { emit... }`
— one method signature to agree on across workstreams, not a shared
free-function contract plus separately-tracked comparison state.

`emit_promoted` (`event.rs`) is deleted outright — there is no
replacement free function; `emit_slot_state(app, state)` is a thin
one-line `app.emit("slot-state", &state)` wrapper callers use after
getting `Some` back from `slot_state_if_changed`, not a function that
itself decides whether to emit.

### 5.2 frontend: `useSlotState` replaces `useVisibleNotifications`

```ts
// useSlotState.ts
export type SlotState =
  | { state: "empty" }
  | {
      state: "showing";
      id: string;
      title: string;
      body: string;
      eventType: "generic" | "score_update" | "match_state";
      priority: "low" | "medium" | "high";
      expanded: boolean;
    };

export function useSlotState(): SlotState {
  const [slot, setSlot] = useState<SlotState>({ state: "empty" });
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    listen<SlotState>("slot-state", ({ payload }) => setSlot(payload))
      .then((fn) => (unlisten = fn));
    return () => unlisten?.();
  }, []);
  return slot;
}
```

no timers, no `deadline`, no wall-clock sweep — that entire class of
bug (§2.0's hardening fix, the sleep/throttling sweep) disappears
because there is nothing client-scheduled left to go stale. rust's
`tick()` (driven by the deadline-sleeping heartbeat, §4.3 — no longer a
fixed 250ms interval as of plan 015) is the only clock; the frontend
purely reacts.

`App.tsx` shrinks to:

```tsx
function App() {
  const slot = useSlotState();
  if (slot.state === "empty") return null;
  const cls = `slot ${slot.priority} ${slot.expanded ? "expanded" : ""}`.trim();
  return (
    <div className={cls}>
      <div className="title">{slot.title}</div>
      <div className="body">{slot.body}</div>
    </div>
  );
}
```

`.stack` (the flex column of up to 3 items) is deleted along with
`morphShape.ts` and `getMorphShape` (§3.6: "replaces `.notification
.pill`/`.grow`/`.mini` and `getMorphShape` entirely"). One `.slot`
element, always at most one in the DOM.

### 5.3 rotation window and CSS

```rust
impl Event {
    pub fn rotation_window(&self, expanded: bool) -> u64 {
        let base = match self.rotation {
            RotationSpec::OneShot { ttl_secs } => ttl_secs,
            RotationSpec::Recurring { display_secs } => display_secs,
        };
        if expanded { base * EXPANDED_MULTIPLIER } else { base }
    }
}
pub const EXPANDED_MULTIPLIER: u64 = 3; // placeholder — §3.6 pins "longer" but not a number
```

`EXPANDED_MULTIPLIER` is a named constant specifically because §3.6
says "longer while expanded" without a number — this is the one
free variable in the rotation model, isolated to one line so tuning
it later is a one-line change, not a design change.

CSS enter/exit transitions become **fixed-duration and content-blind**
— they no longer encode a "hold" phase keyed to ttl at all, because
rust's `slot-state` push already only fires at actual state changes.
`.slot` gets `enter`/`exit` treated as before (one fixed-length
animation each), but there is no `hold` phase class any more — the
frontend has no notion of "how long until this needs to end," only
"what does rust say right now." A `.expanded` class modifier drives a
`max-height`/width transition (reusing exactly the technique already
built for §3.5's `.grow` shape — same CSS mechanism, different
trigger: keyed off `slot.expanded` now, not `slot.eventType`).

---

## 6. cli (`notchtap` shell script)

adds `--priority low|medium|high` (§3.6 explicit ask):

```sh
priority=""
# ...
    --priority)
      [ $# -ge 2 ] || usage
      case "$2" in
        low|medium|high) priority="$2" ;;
        *) usage ;;
      esac
      shift 2 ;;
```

when set, folds into the posted json as `"priority": "$priority"`;
when unset, **the field is omitted from the payload entirely** (not
defaulted client-side to `"medium"` in the script) — the http layer's
`Option<Priority>` default (§3.3) is the single source of truth for
"what does unspecified mean," so there is exactly one place that
default lives, not two that must be kept in sync. cmux's relay path
(which posts through the same script) inherits the same
unspecified-→-medium default; §3.6 explicitly leaves "whether cmux
needs its own default" as an open, non-blocking detail — this spec
resolves it by *not* giving cmux a separate default, since nothing
about the relay's use case (heads-up alerts) argues for a different
default than a bare cli push.

---

## 7. macOS native additions

researched against current Tauri v2 docs (2026-07-17) — see inline
source notes; both items are new dependencies, not yet in the repo.

### 7.1 global hotkey — `tauri-plugin-global-shortcut`

```toml
# src-tauri/Cargo.toml
[target.'cfg(target_os = "macos")'.dependencies]
tauri-plugin-global-shortcut = "2.3.2"
```

registered and handled entirely rust-side in `lib.rs`'s `.setup()`
(main thread — `.setup()` already runs there):

```rust
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

// placeholder combo — §3.6 explicitly defers "exact global hotkey
// combination" as an open detail; isolated to one constant.
const EXPAND_TOGGLE_SHORTCUT: (Option<Modifiers>, Code) =
    (Some(Modifiers::CONTROL.union(Modifiers::SHIFT)), Code::KeyN);

app.handle().plugin(
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(move |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                toggle_manual_expand(app, &queue_handle); // handler behavior below
            }
        })
        .build(),
)?;
app.global_shortcut().register(Shortcut::new(
    EXPAND_TOGGLE_SHORTCUT.0,
    EXPAND_TOGGLE_SHORTCUT.1,
))?;
```

**capabilities**: because the frontend never calls the plugin's JS API
(receive-only boundary, unchanged — §3.6 explicitly preserves this),
`src-tauri/capabilities/default.json` needs **no new
`global-shortcut:*` permission entries**. Registration happens through
`app.global_shortcut()` in Rust directly, not through the IPC/capability
layer that gates frontend-invoked commands. **flagged as needing
build-time confirmation** (the plugin's own docs don't explicitly spell
out the zero-capability-entries case for pure-Rust-side registration —
verify with a clean `cargo build` + runtime keypress before relying on
it; if wrong, the fix is adding the three `allow-register*` entries
`AGENTS.md`'s security section already anticipates as a possible future
grant).

no entitlements or `Info.plist` changes are needed for ordinary
letter-key shortcuts (this uses the Carbon `RegisterEventHotKey` path
under the hood, not the media-key `CGEventTap` path that can prompt for
Accessibility/Input Monitoring access).

**7.1.1 hotkey handler behavior**:

```rust
fn toggle_manual_expand(app: &AppHandle, queue: &Arc<Mutex<SingleSlotQueue>>) {
    let mut q = queue.blocking_lock(); // called off the tokio runtime — mirrors the tray handler's guard
    // no-op while the current slot is already auto-expanded (High priority)
    // — the hotkey is a *manual* override for "everything else", not a
    // forced-collapse of an automatic expand (this spec's reading of
    // §3.6: "manual, ... for everything else ('news')" reads as scoped
    // to the non-high case, not a global override).
    if q.current_priority() == Some(Priority::High) {
        return;
    }
    q.toggle_expanded();
    // toggling always changes `expanded` on the current Showing state,
    // so this is always Some in practice — routed through the same
    // change-guarded accessor as every other call site (§5.1) rather
    // than a bespoke always-emit path, so there is exactly one emission
    // rule in the codebase, not two.
    if let Some(state) = q.slot_state_if_changed() {
        drop(q);
        emit_slot_state(app, state);
    }
}
```

this no-op-while-High behavior is **this spec's reading**, not a line
lifted verbatim from §3.6 — §3.6 pins automatic-for-high and
manual-for-else as two independent triggers but doesn't say what the
hotkey does if pressed while a High item happens to be showing. Flagged
here explicitly so it's easy to override in one function if that
reading is wrong.

**7.1.2 dismiss and pause-toggle shortcuts** — two more combos, added
later, same file, same registration pattern as §7.1's `EXPAND_TOGGLE_SHORTCUT`:

```rust
const DISMISS_SHORTCUT: (Option<Modifiers>, Code) =
    (Some(Modifiers::CONTROL.union(Modifiers::SHIFT)), Code::KeyX);
const PAUSE_TOGGLE_SHORTCUT: (Option<Modifiers>, Code) =
    (Some(Modifiers::CONTROL.union(Modifiers::SHIFT)), Code::KeyP);
```

`⌃⇧X`/`⌃⇧P` were picked to avoid the two combos already registered
(`N`, `O`) and to avoid common macOS `⌘`-based shortcuts — same `⌃⇧`
family, same low-collision reasoning already applied to the first two.

`⌃⇧X` calls `dismiss_current`, which clears the visible slot and calls
`SingleSlotQueue::promote_next` immediately (`queue.rs`) rather than
waiting for the item's TTL to elapse — the manual equivalent of what
`tick` does on natural rotation-out, except a dismissed `Recurring` item
is dropped, not requeued (deliberate: "get rid of this" means gone, not
"back after a lap through the other tiers").

`⌃⇧P` calls `toggle_pause`, extracted out of the tray's `"pause"`
menu-event handler so both the tray click and the hotkey drive the exact
same pause/resume + label-sync logic — the tray's "Pause"/"Resume" label
now needs to stay correct regardless of which path triggered the toggle,
so there's exactly one place that updates it, not two. Same
resume-promotes-immediately behavior as the tray (§4.5).

Neither addition needs a `capabilities/default.json` entry, for the same
reason §7.1 already gives: registration goes through
`app.global_shortcut()` directly in Rust, not the IPC/capability layer.

### 7.2 `NSWindowCollectionBehavior`

```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2-app-kit = { version = "0.3.2", features = ["NSWindow"] }
```

set once, in `lib.rs`'s `.setup()`, right after `window.set_always_on_top(true)?`:

```rust
#[cfg(target_os = "macos")]
{
    use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};
    let ns_window_ptr = window.ns_window()? as *mut NSWindow;
    let ns_window: &NSWindow = unsafe { &*ns_window_ptr };
    let behavior = NSWindowCollectionBehavior::CanJoinAllSpaces
        | NSWindowCollectionBehavior::FullScreenAuxiliary;
    ns_window.setCollectionBehavior(behavior);
}
```

`window.ns_window()` is Tauri's own documented accessor (returns
`Result<*mut c_void>`, macOS-only) — this is the same pattern Tauri's
official window-customization guide uses, called from `.setup()`
where Tauri already guarantees main-thread execution, so no
`run_on_main_thread` wrapper is needed here. `objc2-app-kit`
(not the older `cocoa` crate) matches what Tauri 2.x itself already
pins internally.

single-monitor scope only (§3.6) — no additional code needed for that
constraint, it's simply "don't build multi-monitor logic," not a
setting to configure.

---

## 8. testing crosswalk (extends `TESTING_STRATEGY.md` §4.1)

| area | approach |
|---|---|
| `Priority` ordering | unit: `Low < Medium < High`, declaration-order pinned (§3.1) |
| `SingleSlotQueue::tick` | unit, mirroring today's `queue.rs` suite: never-interrupt (a `High` enqueue while something is Visible does not promote until the Visible item's own rotation elapses), tier-strict promotion order, FIFO within tier, `OneShot` drops forever, `Recurring` requeues to the back of its **own** tier (not the front, not a different tier) |
| fast-path (§4.2, call #3) | unit: a push with any tier non-empty never fast-path-promotes, even a `High` push arriving while only `Low` is waiting — this is the direct 3-tier generalization of today's `fast_path_never_jumps_waiting_items` test and should reuse its exact framing |
| supersession (§4.4) | unit: visible-item supersede updates payload/priority/rotation and grants extension only when remaining time is already below the floor (assert `promoted_at` never changes — only `extension_secs` does — and `extension_secs` increases by exactly the deficit when below floor, by nothing when already ≥ floor); **the hard cap**: a sequence of rapid supersedes each landing below the floor must rotate the item out at exactly `base_window + MAX_EXTENSION_ON_SUPERSEDE_SECS`, never later, regardless of how many supersedes land in between (this is the exact scenario that broke the first two draft revisions — a dedicated test simulating ~20+ rapid supersedes against a short base window, asserting a hard deadline, not just "eventually rotates out"); `extension_secs` resets to `0` on the *next* promotion, not carried across items; same-tier waiting supersede updates in place at the **same VecDeque position** (assert index, not just presence); cross-tier supersede (priority actually changes) removes from the old tier and appends to the **back** of the new tier (assert both: gone from old tier's `VecDeque`, present at the new tier's back, not front) |
| per-tier cap (§4.6) | unit: a full `Low` tier returns `429` for a new `Low` push while a simultaneous `High` push at the same moment is still accepted |
| pause/resume | port today's suite 1:1 against the single-slot shape — pause gates promotion not rotation, resume promotes immediately |
| `slot-state` emission (§5.1) | unit (tauri `MockRuntime`, mirroring `http.rs`'s existing pattern): change-guard suppresses a re-emit when nothing changed between two `tick()` calls; an actual promotion, rotation-to-empty, or expand toggle always emits |
| cli `--priority` (§6) | manual + a `test-cli.sh` case if one gets written (today's cli stays manually verified per `IMPLEMENTATION_PLAN.md` §8 — no new automated coverage tier introduced by this field alone) |
| frontend `useSlotState` (§5.2) | vitest: renders `empty` as nothing, renders `showing` with the right classes, re-render on a new `slot-state` payload replaces content without an intermediate empty frame |
| global hotkey (§7.1) | manual only — `TESTING_STRATEGY.md` §5's existing rule (hardware-dependent OS interaction) extends naturally; the **pure** no-op-while-High branch of `toggle_manual_expand` should still be unit-tested against the queue directly, bypassing the actual OS hotkey |
| dismiss/pause-toggle hotkeys (§7.1.2) | manual only — same `TESTING_STRATEGY.md` §5 rule; `dismiss_current`/`toggle_pause` and `SingleSlotQueue::dismiss_visible` are unit-tested directly against the queue, bypassing the actual OS keypress |
| `NSWindowCollectionBehavior` (§7.2) | manual only (physical Spaces-switch + fullscreen-app check on the macbook) |

no live network calls, no live hotkey simulation, no live NSWindow
assertions in `cargo test` / `npx vitest run` — same standing rule as
every prior spec.

---

## 9. sequencing note for `CONTEXT.md`

§3.6 lists the glossary rewrite as needed **before implementation
starts**, not written speculatively in the architecture doc itself.
§2 above is the delta for whoever does that pass — it is deliberately
not applied to `CONTEXT.md` by this spec, so that edit stays a single
reviewable commit rather than bundled into a code-focused doc.

---

## 10. parallel-agent workstream breakdown

five workstreams, ordered by dependency. **A must land and be
merged before B or C start** — everything else can run concurrently
with A once its interfaces are read (not merged) from this spec
directly, since the types are fully pinned above.

### workstream A — queue + event core (rust, foundational)
**owns**: `event.rs` (`Priority`, `RotationSpec`, `Event`, `SlotState`,
the thin `emit_slot_state` wrapper), `queue.rs` (`SingleSlotQueue`,
rename from `NotificationQueue`, all of §4, including
`slot_state_if_changed`/`current_slot_state`/`current_priority`/
`toggle_expanded` — the full public surface every other workstream
calls into).
**blocks**: B (needs `Priority`/`RotationSpec` to exist),
C indirectly (needs the frozen `SlotState`/`slot-state` wire shape,
but see the parallel-start note below).
**does not touch**: `http.rs`, `poller.rs`, `notifier.rs`, frontend,
native macOS code.
**exit test**: `cargo test -p notchtap_lib queue::` and `event::`
green, covering every row in §8 tagged "unit," including the
cross-tier-move-on-priority-change and minimum-remaining-time cases
added in §4.4's revision, plus a `slot_state_if_changed` change-guard
test (`same state twice → second call returns None`).

### workstream B — http wire + cli + poller updates (rust)
**depends on**: A merged (needs real `Priority`/`RotationSpec`/`Event`
shape).
**owns**: `http.rs`'s `NotifyRequest`/`notify_handler` (§3.3), the
`notchtap` cli script (§6), `poller.rs`'s `Event` construction sites
(§3.4 — set `Priority::High` on score/state events, keep `EventType`
unchanged), `lib.rs`'s heartbeat/tray emission call sites switching
from `emit_promoted` to `emit_slot_state` (§5.1's four call sites —
three of the four live in `lib.rs`/`http.rs`, which is why B owns this
integration even though A owns the function itself).
**does not touch**: queue internals, frontend, native macOS code.
**exit test**: `http.rs`'s existing suite ported to the new request
shape + new fan-out-still-works assertions (connector fan-out in
`http.rs` is unrelated to this redesign and must keep passing
unmodified — do not touch `notifier.rs` or the fan-out call in
`notify_handler`).

### workstream C — frontend (react/ts)
**depends on**: the `SlotState`/`slot-state` **shape** being frozen
(§5.1/§5.2 above) — does **not** need A or B's rust code to actually
compile or run. Can start immediately by hand-writing a mock tauri
`emit("slot-state", ...)` harness (vitest already mocks `@tauri-apps/
api/event`, per the existing `useVisibleNotifications.test.tsx`
pattern) against the exact TS type in §5.2.
**owns**: `useSlotState.ts` (replaces `useVisibleNotifications.ts`),
`App.tsx` rewrite (§5.2), `styles.css` (delete `.stack`/`.pill`/
`.grow`/`.mini`, add `.slot`/`.expanded` per §5.3), delete
`morphShape.ts` + its test, delete `presentationMode`-adjacent stack
logic that assumed multiple items.
**does not touch**: any rust file, any native macOS code.
**exit test**: `npx vitest run` green on the new `useSlotState.test.ts`
+ updated `App.tsx` render tests; `npx tsc --noEmit` clean.
**integration risk**: if A/B's actual emitted JSON drifts from §5.2's
type at merge time, C's tests still pass (they're mocked) but the real
app breaks silently until manually run. **mitigation**: A's exit test
should include one `serde_json::to_value` snapshot assertion on
`SlotState::Showing`'s exact camelCase field names, so a drift breaks
A's own CI, not just C's manual smoke test.

### workstream D — macOS native (rust, mostly independent)
**depends on**: nothing from A/B functionally — touches `lib.rs`'s
`.setup()` and `Cargo.toml` only, plus a `queue_handle` reference it
needs to call `toggle_manual_expand` (§7.1's handler). **this is a
real, small coupling to A**: the hotkey handler must call into
`SingleSlotQueue`'s public API (`current_priority()`,
`toggle_expanded()`, `current_slot_state()` — three new methods A must
expose). **resolution**: A's exit criteria includes shipping these
three methods (even before B/C need them) specifically so D can start
against A's *public API* without waiting for B's http/poller
integration or C's frontend work. D does not need B or C at all.
**owns**: `Cargo.toml` additions (§7.1, §7.2), the hotkey registration
block and `NSWindowCollectionBehavior` block in `lib.rs`'s `.setup()`,
`toggle_manual_expand`.
**exit test**: `cargo build` clean with the new deps on macOS; the
pure no-op-while-High branch unit-tested directly against a
`SingleSlotQueue` (no real hotkey needed for that assertion); manual
checklist entries for the actual keypress + Spaces/fullscreen behavior
(§8's last two rows).

### workstream E — docs
**depends on**: nothing functionally; best done last so it reflects
what actually landed rather than what was planned. **owns**:
`CONTEXT.md` glossary pass (§2/§9), `TESTING_STRATEGY.md` crosswalk
entry mirroring §3's `archive/V3_TECHNICAL_SPEC.md` precedent (a `§4.10 single-
slot rotating overlay` section, same shape as `§4.9`'s), and
`IMPLEMENTATION_PLAN.md` §3.6's own exit-criteria checklist (currently
absent — §3.6 has no `### 3.6.1 exit criteria` subsection the way
§3.5.1 and §3.1 do; add one modeled on those, listing this spec's §8
rows).
**exit test**: none (docs only) — but should not merge before A–D's
actual exit criteria are known, so it isn't describing work that
didn't happen.

### dependency graph

```
A (queue+event core)
├──> B (http+cli+poller integration)      [needs A's types]
├──> D (needs 3 new public methods on A's queue, nothing else)
C (frontend)  — parallel-startable against this spec's frozen shape,
                no rust dependency to even begin
E (docs)      — last, depends on A–D's actual landed shape
```

B and D do not depend on each other. C can start on day one without
waiting for A to merge, as long as it treats §5.2's TS type as frozen
(it is, as of this doc) rather than waiting to observe A's real JSON.
