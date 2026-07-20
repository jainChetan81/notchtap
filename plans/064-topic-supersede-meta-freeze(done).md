# Plan 064: Topic-supersede stops dropping `meta`, so live-match Clock/Cards update after the first event

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat f6c2f46..HEAD -- src-tauri/src/queue.rs src-tauri/src/poller.rs`
> If either file changed since this plan was written, compare the
> "Current state" excerpts below against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: S–M
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `f6c2f46`, 2026-07-20

## Why this matters

`docs/ARCHITECTURE.md` §18 and plans 039/041/042 built the opt-in ESPN
live-match scorecard: one collapsed card per match that updates in place
via Topic supersession instead of spawning a new card per event. Plan 042
specifically added a Clock line and per-side Cards counts to that card's
`meta.details`, computed fresh on every poller event
(`src-tauri/src/poller.rs`'s `meta` block, lines ~486-526, attached via
`event.meta = meta.clone()` at all five `make_event` call sites).

But the queue-side merge function every Topic-supersede path calls,
`apply_fresh_content` (`src-tauri/src/queue.rs:524-529`), never copies
`meta` onto the existing (displayed) event — only `payload`, `priority`,
`rotation`, and `signal`. The practical effect: a live match's card shows
whatever Clock/Cards values were true on the **first** event of that
match's Topic (e.g. kickoff), and never updates again for the rest of the
match — even though the body text (via `payload`) keeps updating on every
goal/card/state-change. This is a real, currently-reachable bug in an
opt-in but shipped feature (`espn_live_card`, default `false`, but this is
exactly the feature plans 039-042 exist to deliver). It also silently
contradicts a factual claim in `docs/design/manual-cmux-topic-supersession.md`
§4 that ESPN's meta is "per-match-constant" and the gap is "harmless" — it
is neither constant (Cards changes every card event, Clock every poll) nor
harmless once you look at what actually reaches the frontend.

No existing test catches this: poller tests never touch the queue,
`queue.rs`'s own supersede tests never vary `meta` between the two events
being merged (see `visible_supersede_updates_content_priority_rotation`,
which asserts `payload`/`priority`/`rotation`/`signal` update but never
touches `meta`), and the property-test fuzzer (`§9.1`) hardcodes
`EventMeta::default()` for every generated event.

## Current state

- `src-tauri/src/queue.rs:524-529` — the merge function every
  Topic-supersede path routes through:

  ```rust
  fn apply_fresh_content(existing: &mut Event, fresh: &Event) {
      existing.payload = fresh.payload.clone();
      existing.priority = fresh.priority;
      existing.rotation = fresh.rotation;
      existing.signal = fresh.signal;
  }
  ```

- `src-tauri/src/queue.rs:185-213` — `supersede_if_topic_matches`, the
  sole caller of `apply_fresh_content` (3 call sites: visible-item update
  at line 188, same-tier waiting update at line 202, cross-tier waiting
  update at line 207 — all three need the fix, since they all go through
  the one function above):

  ```rust
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
  ```

- `src-tauri/src/poller.rs:486-526` — confirms the poller side is
  correct: it computes a fresh `EventMeta` (Clock line always; a Cards
  line only when `home_y + home_r + away_y + away_r > 0`) on every poll
  and attaches it via `event.meta = meta.clone()` at all five
  `make_event(...)` call sites (score update, kickoff, half-time,
  full-time, card). The bug is entirely on the queue-merge side — the
  poller is already doing the right thing and the fresh `meta` is simply
  discarded on arrival.

- `src-tauri/src/event.rs:152-162` — `EventMeta` shape (for reference,
  don't need to change it):

  ```rust
  pub struct EventMeta {
      pub source: Option<String>,
      pub category: Option<String>,
      pub published_at_ms: Option<i64>,
      pub link: Option<String>,
      pub subtitle: Option<String>,
      pub details: Vec<DetailItem>,
  }
  ```

- Repo convention: `apply_fresh_content` is the one and only place that
  decides what "fresh content" means for a superseding event — match its
  existing style (flat field copies, no helper abstraction) rather than
  introducing a new merge strategy.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `cd src-tauri && cargo test --locked` | all pass (baseline at planning time: 332 + 3 doc-tests — see plan 071 for the up-to-date count if that's landed first) |
| Rust tests, scoped | `cd src-tauri && cargo test --locked queue::` | all pass, including the new test from Step 2 |
| Clippy | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Format | `cd src-tauri && cargo fmt --check` | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/queue.rs` (the `apply_fresh_content` fix + new test)

**Out of scope** (do NOT touch, even though they look related):
- `src-tauri/src/poller.rs` — already correct, computes and attaches
  fresh `meta` on every event; no change needed there.
- `docs/design/manual-cmux-topic-supersession.md` — its "meta is
  per-match-constant / harmless" claim (§4) is now factually wrong, but
  correcting a docs-only design spike is not this plan's job; leave a
  one-line note in your final report so the operator can amend it if they
  pick up that spike later, but do not edit the file yourself.
- Any other field on `Event` (`id`, `event_type`, `topic`, `origin`) —
  none of those should ever change on supersede; don't add them.

## Steps

### Step 1: Copy `meta` in `apply_fresh_content`

Add one line to `src-tauri/src/queue.rs:524-529` so the fresh event's
`meta` replaces the existing event's `meta`, matching how every other
mutable field is already handled (full replacement, not a merge):

```rust
fn apply_fresh_content(existing: &mut Event, fresh: &Event) {
    existing.payload = fresh.payload.clone();
    existing.priority = fresh.priority;
    existing.rotation = fresh.rotation;
    existing.signal = fresh.signal;
    existing.meta = fresh.meta.clone();
}
```

**Verify**: `cd src-tauri && cargo build` → exit 0 (this alone won't catch
the bug — it's a silent behavioral gap, not a type error — so Step 2's
test is the real gate).

### Step 2: Add a regression test pinning the fix

Model this after the existing exemplar `visible_supersede_updates_content_priority_rotation`
(`src-tauri/src/queue.rs:850-884`), which already asserts payload/priority/
rotation/signal update on supersede — extend the same pattern to cover
`meta`. Add a new test in the same `mod tests` block (near line 884, right
after the exemplar):

```rust
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
```

Also add a same-tier and cross-tier waiting-item variant, modeled after
`same_tier_waiting_supersede_keeps_position` (`queue.rs:981`) and
`cross_tier_supersede_moves_to_back_of_new_tier` (`queue.rs:1019`)
respectively — same shape, just add a `meta` assertion on the waiting
item after the second `enqueue`. Use `DetailItem` from
`crate::event::{DetailItem, EventMeta}` — both are already imported at
the top of `mod tests` (`queue.rs:538`).

**Verify**: `cd src-tauri && cargo test --locked queue::visible_supersede_updates_meta queue::same_tier_waiting_supersede_updates_meta queue::cross_tier_supersede_updates_meta -- --exact` → all 3 pass (adjust names to whatever you actually call the waiting-item variants).

### Step 3: Full suite + lint

**Verify**:
- `cd src-tauri && cargo test --locked` → all pass, count increased by 3 from baseline
- `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` → exit 0
- `cd src-tauri && cargo fmt --check` → exit 0

## Test plan

- 3 new tests in `src-tauri/src/queue.rs`'s `mod tests`: visible-item
  meta update, same-tier waiting-item meta update, cross-tier waiting-item
  meta update — one per `apply_fresh_content` call site, since all three
  currently share the same bug and should share the same regression
  coverage.
- Pattern: `visible_supersede_updates_content_priority_rotation`
  (`queue.rs:850`), `same_tier_waiting_supersede_keeps_position`
  (`queue.rs:981`), `cross_tier_supersede_moves_to_back_of_new_tier`
  (`queue.rs:1019`) — copy their `enqueue`/`enqueue`/assert shape, add a
  `meta` field with distinguishable content to the "fresh" event, assert
  it lands on the merged event.
- Verification: `cargo test --locked queue::` → all pass, including the 3
  new tests.

## Done criteria

- [ ] `cargo test --locked` exits 0; rust total is baseline + 3
- [ ] `cargo clippy --locked --all-targets -- -D warnings` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] `grep -n "existing.meta = fresh.meta.clone();" src-tauri/src/queue.rs` returns exactly one match, inside `apply_fresh_content`
- [ ] No files outside `src-tauri/src/queue.rs` modified (`git status`)
- [ ] `plans/README.md` status row for 064 updated
- [ ] Update `docs/TESTING_STRATEGY.md` §0's `queue` count (+3) if this plan lands before plan 071 (the docs truth pass) — otherwise note it in this plan's own completion note so 071's executor picks it up

## STOP conditions

- The code at `queue.rs:524-529` or `queue.rs:185-213` doesn't match the
  excerpts above (drift since planning).
- `apply_fresh_content` has additional callers beyond
  `supersede_if_topic_matches`'s three sites by the time you read this —
  if so, re-check each one individually rather than assuming the fix
  covers them.
- A test failure persists after a reasonable fix attempt — stop and
  report rather than loosening the assertion.

## Maintenance notes

- Any future field added to `Event` needs a conscious decision in
  `apply_fresh_content`: does a supersede refresh it (like `payload`) or
  preserve the original (nothing currently does — `id`/`event_type`/
  `topic`/`origin` are all implicitly preserved by never being listed)?
  Leave a one-line comment above the function noting this is a deliberate
  allowlist, not an omission, so a future reader doesn't wonder why it's
  not just `*existing = fresh.clone()` (that would also stomp `id`,
  which must stay stable across a supersede — that's `topic`'s whole
  point).
- Flag to the operator: `docs/design/manual-cmux-topic-supersession.md`
  §4's claim that ESPN meta is "per-match-constant, so dropping it on
  supersede is harmless" is now known-false and should be corrected if
  that spike is ever picked up for a build (plan candidate in
  `plans/README.md`, currently a design doc only).
