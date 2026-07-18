# Plan 038: batch_done must not count a Recurring requeue as "done"

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report — do not
> improvise. When done, update this plan's status row in
> `plans/README.md`.
>
> **Dependency gate (run BEFORE the drift check)**:
> 1. `plans/README.md` status row for **035** reads DONE (035 merges into
>    `queue.rs`; execute this only on a `queue.rs` that already has 035).
> 2. `git status` clean for `src-tauri/src/queue.rs` — if it is dirty in
>    the working tree, an in-flight session (035 or **037**) owns it;
>    STOP and coordinate rather than layering on top.
> If **037 (the Engine)** has already landed, the `tick()`/skip rotation
> arms this plan edits now live inside the Engine's rotation loop —
> re-locate the two `batch_done += 1` sites there; the fix is identical
> in spirit (guard the increment on non-Recurring). Coordinate with the
> 037 owner either way: whoever lands second must carry this correction
> forward, not revert it.
>
> **Drift check (run second)**:
> `rg -n "batch_done \+= 1" src-tauri/src/queue.rs`
> Expected: three increment sites — the `tick()` natural-rotation arm,
> the skip-requeue arm, and the dismiss arm. Line numbers are indicative
> (they shift with 035); locate every target with `rg`, never by number.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW — a localized counter-semantics correction with a cap
  already masking the symptom in production; the 033 batch-counter tests
  are the safety net.
- **Depends on**: **035 (ordering only)** — 035 touches `queue.rs`;
  land this on settled 035. **037 (coordination)** — 037 rewrites the
  rotation arms; whoever lands second preserves this fix (see gate).
- **Origin**: surfaced (not caused) by the 031 scoreboard-card design
  spike, §4. Maintainer decision 2026-07-19 (031 §10 Q4): land it
  **first, as its own small plan**, before the live-match card build.

## Problem

`batch_done` is meant to count items that have **left** the current
batch (completed / dismissed / skipped away for good), feeding the 033
queue-slider's `queueDone`. But in `tick()` the increment is
unconditional and runs *before* the `Recurring` check:

```
self.batch_done += 1;                                    // increments…
if let RotationSpec::Recurring { .. } = item.event.rotation {
    // …then the item requeues to the back of its tier — it did NOT leave
}
```

The skip-requeue arm has the same shape (`batch_done += 1` then an
`if Recurring { requeue }`). So a `Recurring` card that rotates naturally
or is skipped counts itself "done" on every lap, even though it comes
back. In production this is currently **inert** — no producer emits
`Recurring` (every `topic: None`) — and a cap
(`queue_done = min(batch_done, batch_total.saturating_sub(1))`, see the
"Recurring requeues can push `batch_done` past `batch_total`" comment)
hides it. The moment plan 039's live-match card ships a `Recurring`
Topic, the uncapped drift becomes visible: the slider pins near
"complete" while the match is still cycling. The dismiss arm is correct
as-is — dismiss destroys the item (it truly leaves), so it must keep
incrementing.

## Done criteria

| Check | Command (from `src-tauri/`) | Expected |
|---|---|---|
| Rust tests | `cargo test --locked` | all pass; §0 count updated for any added test |
| Lint/format | `cargo clippy --locked --all-targets -D warnings && cargo fmt --check` | exit 0 |
| No stray increment | `rg -n "batch_done \+= 1" src/queue.rs` | dismiss arm unconditional; tick + skip arms guarded on non-Recurring |

## Scope

- `src-tauri/src/queue.rs` — the `tick()` natural-rotation arm and the
  skip-requeue arm only; the dismiss arm and the `queue_done` cap are
  unchanged (the cap stays as defensive belt-and-suspenders).
- `src-tauri/src/queue.rs` `#[cfg(test)] mod tests` — reconcile the 033
  batch-counter tests (`every_completion_increments_batch_done`,
  `batch_done_caps_at_total_minus_one_while_an_item_is_visible`) and add
  one new test pinning the fix.
- `docs/TESTING_STRATEGY.md` §0 — bump the `queue` count if a test is
  added.

## Steps

### Step 1: Guard the two requeue increments

In `tick()`'s natural-rotation arm and the skip-requeue arm, only
increment `batch_done` when the outgoing item is **not** `Recurring`
(i.e. it genuinely leaves the batch). Prefer moving `batch_done += 1`
into the non-Recurring branch, or guarding it with
`if !matches!(item.event.rotation, RotationSpec::Recurring { .. })`.
Leave the dismiss arm untouched.

### Step 2: Reconcile + add tests

- Re-read the two named 033 tests. If either asserts a `Recurring`
  item's requeue increments `batch_done`, it encoded the bug — update it
  to the corrected semantics; if they only exercise `OneShot`
  completions, they stay green unchanged.
- Add a test: a visible `Recurring` item that rotates out via `tick()`
  (and one via skip) requeues to the back of its tier **without**
  advancing `batch_done`; a sibling `OneShot` completion still does.
- Run the plan-022 `mod proptest_queue` suite — the batch-counter
  invariant must still hold with the corrected increment.

### Step 3: Docs + status

Update `docs/TESTING_STRATEGY.md` §0 queue count if a test was added.
Flip this plan's `plans/README.md` row to DONE and rename the file to
`038-batch-done-recurring-fix(done).md`.

## STOP conditions

- `queue.rs` dirty in the working tree (035/037 in flight) → STOP.
- The corrected semantics would break a plan-022 proptest invariant in
  a way that isn't a stale-expectation update → STOP and report; the
  invariant may encode intended behavior this plan misread.
