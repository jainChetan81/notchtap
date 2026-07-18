# Plan 024: Extend the queue property suite to exercise invariant 4's rotation_order rank tie-break

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> This is a TESTS-ONLY change — zero production-code edits. If anything in
> "STOP conditions" occurs, stop and report. When done, update this plan's
> status row in `plans/README.md` — unless a reviewer dispatched you and
> said they maintain the index.
>
> **Drift check (run first)**: `git diff --stat d926977..HEAD -- src-tauri/src/queue.rs docs/TESTING_STRATEGY.md`
> `queue.rs`'s `mod proptest_queue` and the production ranking fns quoted
> below were captured at `d926977`. Locate every target with the `rg`/`grep`
> command given, not the line number (this repo churns from concurrent
> sessions). Run `git status` first — if `queue.rs` or
> `docs/TESTING_STRATEGY.md` is already dirty in the working tree, STOP and
> report rather than layering onto someone's in-flight edit.

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW (tests only; the risk is the new predictor being wrong and
  either false-failing or masking a real ordering bug — Step 3 guards both)
- **Depends on**: none — plan 022 is DONE and merged (`4cc3a4b`); this
  extends the `mod proptest_queue` it created
- **Category**: tests
- **Planned at**: commit `d926977`, 2026-07-18

## Why this matters

Plan 022's queue property suite (`#[cfg(test)] mod proptest_queue` in
`src-tauri/src/queue.rs`) exercises invariant 4 (promotion order) but only
its **FIFO** tie-break: the harness builds every queue with an *empty*
`rotation_order`, so the per-source rank dimension the queue actually
implements (`best_index_in_tier`) is never generated in the property model.
The v6 rotation-order behavior stays covered by the hand-written example
test `rotation_order_breaks_same_tier_ties_ahead_of_arrival_order`, but the
generated adversary never explores the interaction of ranked ordering with
supersession, pause, and rotation. This plan closes that gap by generating a
per-case `rotation_order` and teaching the invariant-4 predictor to mirror
the production rank-then-FIFO rule.

## Current state

All targets are inside `mod proptest_queue` at the bottom of
`src-tauri/src/queue.rs` (find it: `rg -n "mod proptest_queue" src-tauri/src/queue.rs`).

**The production ranking logic the predictor must mirror EXACTLY**
(`fn best_index_in_tier`, find with
`rg -n "fn best_index_in_tier" src-tauri/src/queue.rs`):

```rust
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
        if r < best_rank {      // STRICT <: ties keep the earliest (FIFO) index
            best = i;
            best_rank = r;
        }
    }
    best
}
```

Semantics to reproduce: within a tier, pick the item of **minimum rank**;
rank = index of the item's `origin` in `rotation_order`, or
`rotation_order.len()` if the origin is not listed (or the order is empty);
**ties are broken by lowest index = earliest arrival (FIFO)**. `promote_next`
→ `pop_highest_priority_waiting` scans tiers High→Low and applies
`best_index_in_tier` to the first non-empty tier.

**The five current test helpers/structures to change** (all in
`mod proptest_queue`):

1. `fn snapshot_ids(q: &SingleSlotQueue) -> [Vec<Uuid>; 3]` — captures only
   ids. It must also capture each waiting item's `origin` so the predictor
   can rank.
2. `fn highest_nonempty_front(snap: &[Vec<Uuid>; 3]) -> Option<Uuid>` — the
   FIFO-only predictor. Must become rank-aware.
3. `struct Harness { q, now, max_queued_per_tier, ... }` and its
   `fn new(max_queued_per_tier: usize)` — builds
   `SingleSlotQueue::new(max_queued_per_tier)` with NO rotation order. Must
   accept and apply a generated `rotation_order`, and store it for the
   predictor.
4. The proptest body `fn queue_invariants_hold_under_any_op_script(...)` —
   must generate a `rotation_order` and pass it to `Harness::new`.
5. `struct VisSnap` and `fn vis_snapshot` — the visible-item snapshot used
   by the Recurring-requeue push in `apply_tick`/`apply_skip`. It captures
   `id`/`tier`/`recurring`/`promoted_at`/`window` but **not** `origin`; it
   must gain an `origin: SourceKind` field (from `item.event.origin`) so
   the requeue can push the `(id, origin)` pair (Step 3).

`SourceKind` has exactly four variants (find:
`rg -n "enum SourceKind" -A6 src-tauri/src/event.rs`): `Football`, `News`,
`Manual`, `Cmux`. The harness already imports `SourceKind` (it uses it in
`arb_origin`).

The queue is built via the builder `SingleSlotQueue::new(cap)
.with_rotation_order(order)` (find: `rg -n "pub fn with_rotation_order"`).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Build tests | `cargo build --tests --locked` (from `src-tauri/`) | exit 0 |
| Property suite | `cargo test --locked queue_invariants_hold_under_any_op_script` | 1 passed, 0 failed |
| Full suite | `cargo test --locked` (from `src-tauri/`) | all pass (count unchanged: 257 + 3 doc-tests) |
| Gates | `cargo clippy --locked --all-targets -- -D warnings && cargo fmt --check` | exit 0 |

(If `cargo: command not found`, prefix `PATH="$HOME/.cargo/bin:$PATH"`.)

## Scope

**In scope** (edit ONLY inside `mod proptest_queue`, plus one docs note):
- `src-tauri/src/queue.rs` — the five helpers/structs above, all within
  `#[cfg(test)] mod proptest_queue`. TESTS ONLY.
- `docs/TESTING_STRATEGY.md` — §9.1 invariant-4 line: note that
  `rotation_order` is now generated per case and the predictor checks
  rank-then-FIFO (find §9.1 with `rg -n "^### 9\.1" docs/TESTING_STRATEGY.md`).
- `plans/README.md` (status row).

**Out of scope**:
- ANY production-code change (no edits to `best_index_in_tier`,
  `with_rotation_order`, `pop_highest_priority_waiting`, or any non-test fn).
  This plan only teaches the TEST to generate + predict the existing
  behavior. If you believe the production ranking is wrong, that is a
  separate finding — STOP and report, do not "fix" it.
- The other invariants (1–3, 5–9) and their assertions — leave them exactly
  as they are. Only invariant 4's predictor and the queue construction change.
- The `arb_op` / `Op` enum — `with_rotation_order` is a per-CASE queue
  parameter, NOT a scripted op. Do not add a `SetRotationOrder` op.
- Adding new test count — the property suite stays ONE `#[test]` fn (proptest
  runs 256 cases inside it). The §0 count must remain rust 257 + 3 doc-tests.

## Git workflow

- You are on the current branch (or an isolated worktree if a reviewer
  dispatched you). One commit: `tests: generate rotation_order in queue
  proptest, rank-aware invariant 4`.
- Stage in-scope files by name (`git add src-tauri/src/queue.rs
  docs/TESTING_STRATEGY.md`). Do NOT push.

## Steps

### Step 1: Generate a per-case rotation_order

Add a proptest strategy near `arb_origin` producing a `Vec<SourceKind>` that
covers the full behavior space: empty (→ pure FIFO), a partial subset (some
origins unlisted → rank == len), and a full permutation. A simple, correct
approach: generate a shuffled prefix of the four variants — shuffle with
proptest's own `prop_shuffle` combinator (in `proptest::prelude::*`, already
imported at the top of `mod proptest_queue`), then truncate to a random
length `0..=4`:

```rust
fn arb_rotation_order() -> impl Strategy<Value = Vec<SourceKind>> {
    let all = vec![
        SourceKind::Football,
        SourceKind::News,
        SourceKind::Manual,
        SourceKind::Cmux,
    ];
    // Shuffled, then truncated to a random 0..=4 length so empty, partial
    // (unlisted origins), and full orders are all reachable.
    (Just(all).prop_shuffle(), 0usize..=4).prop_map(|(mut v, len)| {
        v.truncate(len);
        v
    })
}
```

**Do NOT add a `rand` dev-dependency** to reach `gen_range`-style manual
shuffling — the dev-dependencies are `proptest = "1"` only, `Cargo.toml`/
`Cargo.lock` are out of scope, and the plan's `--locked` verification
commands would fail on a lockfile change. If `prop_shuffle` turns out to be
unavailable in the resolved proptest version, the acceptable fallback is
`prop_oneof!` over a handful of hand-listed orders **that must include: the
empty vec, at least one partial (e.g. `vec![Cmux]`), and at least one full
permutation with a non-identity order (e.g. `vec![Cmux, Manual, News,
Football]`)**. Whatever you choose, a comment must state that empty, partial,
and full orders are all reachable — that coverage is the point.

**Verify**: `cargo build --tests --locked` → exit 0.

### Step 2: Thread rotation_order into the harness

- Add a field `rotation_order: Vec<SourceKind>` to `struct Harness`.
- Change `Harness::new` to `fn new(max_queued_per_tier: usize, rotation_order:
  Vec<SourceKind>) -> Self`, building the queue as
  `SingleSlotQueue::new(max_queued_per_tier).with_rotation_order(rotation_order.clone())`
  and storing `rotation_order` on the struct.
- In the proptest body `queue_invariants_hold_under_any_op_script`, add a
  generated input `rotation_order in arb_rotation_order()` and pass it:
  `Harness::new(max_queued_per_tier, rotation_order)`.

**Verify**: `cargo build --tests --locked` → exit 0.

### Step 3: Make the invariant-4 predictor rank-aware

This is the load-bearing step. Two sub-changes:

(a) `snapshot_ids` must also capture origins. Change it to return, per tier,
a `Vec<(Uuid, SourceKind)>` (rename to `snapshot_waiting` if clearer). The
origin comes from `item.event.origin`.

(b) Replace `highest_nonempty_front` with a predictor that, given the
per-tier snapshot AND the harness's `rotation_order`, returns the id the
production code would promote: scan tiers `(0..3).rev()` (High→Low) for the
first non-empty tier, then within it pick the entry of **minimum rank**,
ties broken by **lowest index**, where
`rank(origin) = rotation_order.iter().position(|o| *o == origin)
.unwrap_or(rotation_order.len())`. This must reproduce `best_index_in_tier`
exactly — including the strict-`<` FIFO tie-break (iterate front→back, only
replace the current best on a *strictly smaller* rank).

The three call sites of `highest_nonempty_front` are the invariant-4
assertions in `apply_tick`, `apply_dismiss`, and `apply_skip` (find:
`rg -n "highest_nonempty_front" src-tauri/src/queue.rs`). All three build a
`waiting_before` snapshot, but only `apply_tick` and `apply_skip` push a
rotated/skipped Recurring item's id back into its tier's snapshot before
predicting — `apply_dismiss` never does (dismiss drops Recurring items
outright, and its snapshot is deliberately non-`mut`; leave that as is).
You MUST preserve the requeue step in tick and skip, and it must push the
`(id, origin)` pair now. The origin comes from the visible-item snapshot:
extend `struct VisSnap`/`fn vis_snapshot` with an `origin: SourceKind`
field from `item.event.origin` (Current state item 5), making the push
`waiting_before[v.tier].push((v.id, v.origin))`. Since a Recurring requeue
goes to the **back** of its tier (`push_back` in both the tick rotation
path and `skip_visible`), pushing to the end of the snapshot vec is correct.

While here, update the three invariant-4 assertion **messages** (currently
"… did not pick the highest-tier FIFO front") to describe the new rule,
e.g. "… did not pick the highest-tier, best-rotation_order-rank, FIFO-tie
front" — a failure message describing the old rule would mislead whoever
debugs it later.

**Verify**:
- `cargo test --locked queue_invariants_hold_under_any_op_script` → 1 passed;
  run it **5 times** (proptest reseeds) — all pass, no `proptest-regressions`
  file appears under `src-tauri/`. If a case fails, see STOP conditions.
- `cargo test --locked` → full suite still 257 + 3 doc-tests, 0 failed.
- `cargo clippy --locked --all-targets -- -D warnings && cargo fmt --check`
  → exit 0.

### Step 4: Sanity-check the predictor actually discriminates

A rank-aware predictor that silently degenerates to FIFO would pass
vacuously. Prove the new dimension is exercised: temporarily invert the
predictor's tie-break (e.g. pick MAXIMUM rank instead of minimum) and confirm
`cargo test --locked queue_invariants_hold_under_any_op_script` now FAILS
within a few seconds. Then revert to the correct minimum-rank version and
confirm it passes again. Report both outcomes (this is a scratch check — the
committed code must be the correct version). If inverting the predictor does
NOT cause a failure, the generated `rotation_order` isn't influencing
promotion in any case — STOP and report (the generator or wiring is wrong).

**Verify**: inverted predictor → suite fails; correct predictor → suite
passes. Both observed and reported.

### Step 5: Docs note

In `docs/TESTING_STRATEGY.md` §9.1, update the invariant-4 line to state that
`rotation_order` is now generated per case (empty/partial/full) and the
predictor checks highest-tier → minimum rotation_order rank → FIFO. Keep it
to one or two sentences; do not restructure §9.1.

**Verify**: `rg -n "rotation_order" docs/TESTING_STRATEGY.md` shows the new
note in §9.1.

## Test plan

No NEW test function — this deepens the existing property test's input space
and its invariant-4 oracle. The proof is: (1) the suite passes 5× with
rotation_order generated, and (2) Step 4's inversion check demonstrates the
new dimension genuinely gates the assertion (not a vacuous pass).

## Done criteria

- [ ] `rg -n "arb_rotation_order|with_rotation_order" src-tauri/src/queue.rs`
      shows the generator and its use in the proptest harness
- [ ] `cargo test --locked queue_invariants_hold_under_any_op_script` passes
      5× consecutively; no `proptest-regressions` file created
- [ ] Step 4 inversion check demonstrated (fails inverted, passes correct)
- [ ] `cargo test --locked` → 257 + 3 doc-tests, 0 failed (count UNCHANGED)
- [ ] `cargo clippy --locked --all-targets -- -D warnings && cargo fmt
      --check` → exit 0
- [ ] `git diff` shows ZERO changes outside `mod proptest_queue` in queue.rs
      and the one §9.1 docs line (no production-code edits)
- [ ] `docs/TESTING_STRATEGY.md` §9.1 rotation_order note present
- [ ] `plans/README.md` status row updated

## STOP conditions

- The property suite fails with a generated `rotation_order` and you cannot
  tell whether the PREDICTOR is wrong or the QUEUE is wrong. Minimize the
  failing case, commit it as an `#[ignore]`d `#[test]` with a comment, and
  report — do NOT edit production ranking code to make it pass. (A genuine
  production bug here is a real finding for a separate plan.)
- Step 4's inversion check does NOT produce a failure — the generated order
  isn't affecting promotion; report the wiring gap, don't ship a vacuous test.
- Making the predictor match requires reading queue state the snapshot can't
  reach (e.g. you feel the need to call a production fn to predict) — stop and
  report; the predictor must be a self-contained mirror, not a call into the
  code under test.

## Maintenance notes

- The predictor in the test now duplicates `best_index_in_tier`'s ranking.
  Any future change to the production tie-break rule must update BOTH — a
  reviewer should check the test predictor moves with the production fn.
- If per-source rank ever gains a secondary key (e.g. priority-within-source),
  the generated `rotation_order` model and the predictor both extend here.
- This does not touch §9.3/§9.4 (still parked) or the http burst tests.
