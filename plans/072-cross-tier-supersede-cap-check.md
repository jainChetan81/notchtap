# Plan 072: Cross-tier Topic supersede skips the `max_queued_per_tier` cap check

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat f6c2f46..HEAD -- src-tauri/src/queue.rs`
> If the file changed since this plan was written (especially plan 064,
> which also touches `apply_fresh_content`/`supersede_if_topic_matches` in
> this same file), compare the "Current state" excerpt against the live
> code before proceeding; on a mismatch, treat it as a STOP condition.
> **This plan should land after plan 064** if both are selected, since
> they touch the same function region and 064 is the higher-priority fix.

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW
- **Depends on**: 064 (soft — same file region, land 064 first to avoid rebasing)
- **Category**: bug (latent/defensive — not reachable by any producer today)
- **Planned at**: commit `f6c2f46`, 2026-07-20

## Why this matters

Every other path that adds an item to `SingleSlotQueue.waiting[tier]`
enforces `max_queued_per_tier` before pushing — `enqueue_new`
(`src-tauri/src/queue.rs:147-183`) explicitly checks the cap at lines
157-159 and rejects with `QueueError` if the destination tier is full.
The cross-tier branch of `supersede_if_topic_matches`
(`queue.rs:203-209`) does not: when a Topic-matching waiting item's fresh
priority differs from its current tier, it removes the item from its old
tier and pushes it onto the new tier's back with no cap check at all.

Today this is **not reachable in production**: the only Topic producer,
the ESPN live-match card (`poller.rs`'s `make_event`), passes a single
`priority: Priority` sourced from the constant `Config.espn_priority` for
the whole match — so a given match's Topic never crosses tiers across
polls, and this code path's `if new_tier_idx == tier_idx` branch is
always taken instead of the `else` branch this finding is about. But the
gap is structural, not incidental: `queue.rs`'s own test suite already
covers `cross_tier_supersede_moves_to_back_of_new_tier`
(`queue.rs:1019-1050`) without ever setting the destination tier near
its cap — meaning the *existing* test coverage doesn't protect this
path either. Any future Topic-carrying producer whose priority can vary
per push (plausible given `EventMeta`/Topic groundwork already exists for
other sources per the design docs in `docs/design/`) would silently be
able to push a destination tier's waiting queue past
`max_queued_per_tier`, defeating the cap
`full_low_tier_rejects_low_but_accepts_high` (`queue.rs:1059-1077`)
exists to test for every other insert path.

This is deliberately filed as a small, standalone defensive-hardening
plan rather than bundled with plan 064 — it's a different kind of bug
(latent capacity-limit gap vs. active data-loss bug) with a different,
non-trivial question attached (what should happen on overflow?) that
deserves its own STOP-and-decide gate rather than a mechanical fix.

## Current state

- `src-tauri/src/queue.rs:185-213` — `supersede_if_topic_matches`, full
  function (the cross-tier branch is lines 203-209):

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

  **Note**: if plan 064 has landed first, `apply_fresh_content` will
  also copy `meta` — that's an unrelated, already-landed change; don't
  let it confuse this plan's own diff, which is purely about the
  `self.waiting[new_tier_idx].push_back(existing);` line at 208 needing a
  cap check before it.

- `src-tauri/src/queue.rs:147-183` — `enqueue_new`, the cap-check pattern
  to mirror (read the full function; the relevant excerpt is the check
  itself around lines 157-159 — re-locate exactly, since this plan's
  Step 1 needs to match its error type/message style precisely):

  ```rust
  if self.waiting[tier].len() >= self.max_queued_per_tier {
      return Err(QueueError::TierFull /* or whatever the actual variant/message is — read it */);
  }
  ```

- `src-tauri/src/queue.rs:1019-1050` — the existing
  `cross_tier_supersede_moves_to_back_of_new_tier` test, to extend (not
  replace) with a capacity-boundary case.

- `src-tauri/src/queue.rs:1059-1077` — `full_low_tier_rejects_low_but_accepts_high`,
  the exemplar cap-enforcement test pattern for `enqueue_new` to mirror
  for this new cross-tier-supersede case.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests, scoped | `cd src-tauri && cargo test --locked queue::` | all pass, including the new test |
| Full suite | `cd src-tauri && cargo test --locked` | all pass |
| Clippy | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Format | `cd src-tauri && cargo fmt --check` | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/queue.rs` (the cap-check fix in `supersede_if_topic_matches`'s cross-tier branch + new test)

**Out of scope**:
- `poller.rs` or any producer — no producer varies priority per Topic
  today; this plan doesn't need to (and shouldn't) add one just to
  exercise the new code path in production. The regression test exercises
  it directly at the queue level, which is sufficient (matches how
  `enqueue_new`'s own cap test works — no poller involved).
- `supersede_if_topic_matches`'s same-tier branch (line 202) or the
  visible-item branch (lines 187-191) — neither inserts into a
  `waiting[tier]` Vec, so neither has a cap-check gap.

## Steps

### Step 0: Decide the overflow behavior — STOP if you can't decide confidently

Before writing code, this plan needs one product decision:  when a
cross-tier supersede's destination tier is already at
`max_queued_per_tier`, what happens to the fresh content?

Two reasonable options, matching this codebase's existing error-handling
conventions:
- **(a) Reject the supersede, leave the item in its old tier
  unchanged** — safest, matches `enqueue_new`'s "reject when full"
  precedent exactly, but means the fresh content is silently dropped
  rather than ever reaching the item (unusual for Topic supersession,
  whose whole point is "the item always reflects the latest content").
- **(b) Evict something from the destination tier to make room** — no
  existing precedent for this in the codebase; higher complexity, and
  picks a victim-selection question this plan doesn't want to open.

Recommended default: **(a)**, because it's the smallest change, matches
existing precedent exactly, and the practical impact is currently zero
(no producer reaches this path) — so erring toward the conservative,
already-precedented behavior is safe. If you have high confidence (a)
is right, proceed with Step 1 using it. If anything makes you uncertain
this is the right call (e.g. you find evidence a future producer's design
already assumes option (b)), STOP and report back with what you found
rather than guessing.

### Step 1: Add the cap check

In the cross-tier branch (`queue.rs:203-209`), add a capacity check
before the `push_back`, mirroring `enqueue_new`'s check exactly (same
comparison, same error type — re-read `enqueue_new`'s exact check before
writing this, don't approximate it):

```rust
let new_tier_idx = fresh.priority as usize;
if new_tier_idx == tier_idx {
    apply_fresh_content(&mut self.waiting[tier_idx][pos].event, fresh);
} else if self.waiting[new_tier_idx].len() >= self.max_queued_per_tier {
    // destination tier is full — per Step 0's decision, drop the fresh
    // content and leave the item in its current tier rather than
    // evicting something to make room. This function returns `bool`
    // (whether a Topic match was found and handled), not a `Result`, so
    // there's no error channel to report the drop through today — the
    // caller only cares "did I find and handle this Topic." Leave a
    // comment explaining this rather than silently changing the return
    // contract.
    return true;
} else {
    let mut existing = self.waiting[tier_idx]
        .remove(pos)
        .expect("position just found");
    apply_fresh_content(&mut existing.event, fresh);
    self.waiting[new_tier_idx].push_back(existing);
}
return true;
```

Note both the new early-return branch and the existing final `return
true;` return the same value (`true` — a Topic match was found) since
`supersede_if_topic_matches`'s contract is "found and handled," not "made
a change." Confirm this against every caller of this function before
assuming it's fine — re-check `queue.rs` for all call sites of
`supersede_if_topic_matches`.

**Verify**: `cd src-tauri && cargo build` → exit 0.

### Step 2: Add a regression test

Extend near `cross_tier_supersede_moves_to_back_of_new_tier`
(`queue.rs:1019`) with a new test modeled on
`full_low_tier_rejects_low_but_accepts_high`'s cap-filling setup
(`queue.rs:1059`):

```rust
#[test]
fn cross_tier_supersede_drops_fresh_content_when_destination_tier_full() {
    let mut q = SingleSlotQueue::new(1); // max_queued_per_tier = 1
    let t0 = Instant::now();
    // fill the visible slot so nothing promotes out from under us
    q.enqueue(event("visible", Priority::Medium, 60), t0).unwrap();
    // put the Topic item in the Medium tier
    q.enqueue(topic_event("match-a", Priority::Medium, 60, "espn:match"), t0)
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
```

Adjust field/method names (`q.waiting`'s visibility, `event`/`topic_event`
helper signatures, `Priority as usize` cast validity) to match what you
actually find in the file — this sketch follows the patterns visible in
the two exemplar tests but you must verify against the live code, not
copy this verbatim if anything doesn't compile.

**Verify**: `cd src-tauri && cargo test --locked queue:: -- cross_tier_supersede_drops` (adjust filter) → passes.

### Step 3: Full suite + lint

**Verify**:
- `cd src-tauri && cargo test --locked` → all pass, rust total baseline + 1
- `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` → exit 0
- `cd src-tauri && cargo fmt --check` → exit 0

## Test plan

- 1 new test: `cross_tier_supersede_drops_fresh_content_when_destination_tier_full`
  (or your chosen name), modeled on `full_low_tier_rejects_low_but_accepts_high`'s
  cap-filling setup and `cross_tier_supersede_moves_to_back_of_new_tier`'s
  Topic-event setup.
- Verification: `cargo test --locked queue::` → all pass, including the
  new case; re-confirm `cross_tier_supersede_moves_to_back_of_new_tier`
  still passes unmodified (this fix must not change behavior when the
  destination tier has room).

## Done criteria

- [ ] `cargo test --locked` exits 0; rust total is baseline + 1
- [ ] `cargo clippy --locked --all-targets -- -D warnings` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] The pre-existing `cross_tier_supersede_moves_to_back_of_new_tier` test still passes unmodified
- [ ] No files outside `src-tauri/src/queue.rs` modified (`git status`)
- [ ] `plans/README.md` status row for 072 updated

## STOP conditions

- Step 0's overflow-behavior decision — if you're not confident option
  (a) is right, stop and report rather than guessing.
- The code at `queue.rs:185-213` doesn't match the excerpt above (drift
  since planning, especially if plan 064 landed and changed
  `apply_fresh_content`'s signature or this function's shape beyond what
  this plan's note already accounts for).
- `supersede_if_topic_matches` has callers whose contract would break if
  the new early-return branch's `true` return value is wrong for their
  use — check every call site before assuming it's fine.

## Maintenance notes

- If a future Topic producer's priority ever does vary per push (the
  scenario this plan is defensive hardening against), re-examine whether
  option (a)'s "silently drop the fresh content" is still the right
  behavior for that specific producer's use case, or whether it needs a
  visible signal (a log line at minimum, via plan 070's logging work if
  that's landed) rather than silence.
