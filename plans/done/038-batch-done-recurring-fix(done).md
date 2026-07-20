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
>    the working tree, an in-flight session owns it; STOP and coordinate
>    rather than layering on top.
>
> **037 (the Engine) has already landed** — merged to master at
> `6b53c32`. Its rotation loop lives in `src-tauri/src/engine.rs` but
> only *drives* the queue (`q.tick(Instant::now())`, `engine.rs:241`);
> the rotation arms and all three `batch_done += 1` sites stayed in
> `queue.rs`, exactly where this plan's steps cite them (verified at the
> 2026-07-19 review-plan pass: `engine.rs` contains zero `batch_done`
> references). Execute the steps as written — there is nothing to
> re-locate and no 037 owner left to coordinate with.
>
> **Drift check (run second)**:
> `rg -n "batch_done \+= 1" src-tauri/src/queue.rs`
> Expected: three increment sites — the `tick()` natural-rotation arm,
> the skip-requeue arm, and the dismiss arm (three as of `2b8dcfa`).
> Line numbers in this plan are indicative (they shift as plans land);
> locate every target with `rg`, never by number.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW — a localized counter-semantics correction with a cap
  already masking the symptom in production; the 033 batch-counter tests
  are the safety net.
- **Depends on**: **035 (ordering only, DONE)** — land on settled 035.
  **037 (none)** — 037 landed first (`6b53c32`) without touching the
  arms (see gate); no coordination left.
- **Category**: bug
- **Planned at**: commit `2b8dcfa`, 2026-07-19 (re-anchored at the
  review-plan pass the same day)
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
(`u32::try_from(self.batch_done).unwrap_or(u32::MAX).min(queue_total - 1)`
at `current_slot_state()`, `queue.rs:487-490`, guarded by the "Recurring
requeues can push `batch_done` past `batch_total`" comment)
hides it. The moment plan 039's live-match card ships a `Recurring`
Topic, the uncapped drift becomes visible: the slider pins near
"complete" while the match is still cycling. The dismiss arm is correct
as-is — dismiss destroys the item (it truly leaves), so it must keep
incrementing.

## Done criteria

| Check | Command (from `src-tauri/`) | Expected |
|---|---|---|
| Rust tests | `cargo test --locked` | all pass; §0 count updated for any added test |
| Lint/format | `cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings` | exit 0 (matches `just check-rust` / CI exactly) |
| Increments guarded | `rg -n "batch_done \+= 1" src/queue.rs` | still 3 matches; the tick + skip sites now sit in the `else` of the Recurring check, dismiss stays unconditional |
| Comments swept | `rg -n "Recurring requeues can push" src/queue.rs` | no matches (cap comment rewritten as defensive — see Step 1) |

## Scope

- `src-tauri/src/queue.rs` — the `tick()` natural-rotation arm and the
  skip-requeue arm only; the dismiss arm and the `queue_done` cap
  expression are unchanged (the cap stays as defensive
  belt-and-suspenders).
- `src-tauri/src/queue.rs` — two comments that describe the pre-fix
  mechanism and are rewritten in Step 1: the counter doc-comment above
  `batch_total`/`batch_done` (`queue.rs:66-70`) and the cap comment
  (`queue.rs:485-486`).
- `src-tauri/src/queue.rs` `#[cfg(test)] mod tests` — reconcile the 033
  batch-counter tests (`every_completion_increments_batch_done`,
  `batch_done_caps_at_total_minus_one_while_an_item_is_visible`) and add
  one new test pinning the fix.
- `docs/TESTING_STRATEGY.md` §0 — bump the `queue` count if a test is
  added.

**Out of scope** (do NOT touch, even though they look related):

- `src-tauri/src/engine.rs` — it drives `q.tick()` but holds no
  `batch_done` logic; nothing in this plan touches it.
- The dismiss arm (`dismiss_visible`), the `queue_done` cap expression,
  and the batch-reset sites (`self.batch_done = 0`, `queue.rs:164` and
  `queue.rs:438`).
- `src-tauri/src/lib.rs` `skip_current_requeues_recurring_and_promotes_next`
  — asserts ids/promotion only; stays green unchanged.
- Any frontend slider code (`queueDone` rendering) — display-only.

## Git workflow

- Branch: `advisor/038-batch-done-recurring-fix` (or work directly if
  the operator dispatched you that way — match how sibling plans were
  run, e.g. `exec/036-heartbeat-lost-wakeup`).
- Commit style (from `git log`): lowercase `area: imperative summary`,
  e.g. `queue: don't count a Recurring requeue as batch_done`.
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Guard the two requeue increments

In `tick()`'s natural-rotation arm (`rotate_out_if_elapsed`,
`queue.rs:247-260` — locate with
`rg -n "fn rotate_out_if_elapsed"`) and the skip-requeue arm
(`skip_visible`, `queue.rs:389-399` — locate with
`rg -n "fn skip_visible"`), only increment `batch_done` when the
outgoing item is **not** `Recurring` (i.e. it genuinely leaves the
batch). Both arms have the identical `if let RotationSpec::Recurring
{ .. } = ... { requeue }` shape already — move `batch_done += 1` into
an `else` on that same `if let`, rather than adding a separate guard
above it (smaller diff, and the two branches — "requeues" vs. "really
left" — become visibly mutually exclusive at the point they're
decided). Concretely, `rotate_out_if_elapsed` becomes:

```rust
let item = self.visible.take().expect("checked Some above");
if let RotationSpec::Recurring { .. } = item.event.rotation {
    let tier = item.event.priority as usize;
    self.waiting[tier].push_back(item);
} else {
    self.batch_done += 1;
}
```

and `skip_visible`'s inner block becomes:

```rust
if let Some(item) = self.visible.take() {
    if let RotationSpec::Recurring { .. } = item.event.rotation {
        let tier = item.event.priority as usize;
        self.waiting[tier].push_back(item);
    } else {
        self.batch_done += 1;
    }
}
```

Leave the dismiss arm (`dismiss_visible`, `queue.rs:371-377`) untouched
— its `batch_done += 1` stays unconditional.

Two comments in the same file describe the pre-fix mechanism — rewrite
both as part of this step so they don't outlive it:

- The counter doc-comment (`queue.rs:66-70`) currently says "every
  completion (rotated out, dismissed, skipped) increments `batch_done`".
  Post-fix that is false for `Recurring` rotation-out and skip — qualify
  it, e.g. "every completion (rotated out, dismissed, skipped)
  increments `batch_done` — except a `Recurring` rotation-out or skip,
  which requeues rather than leaves and so does not count".
- The cap comment (`queue.rs:485-486`) currently says "Recurring
  requeues can push `batch_done` past `batch_total`, hence the cap".
  Post-fix no known path does that — recast it as defensive, e.g.
  "defensive: no known path pushes `batch_done` past `batch_total`;
  the cap stays as cheap insurance against a future double-count".

**Verify**: `cargo build --locked` (from `src-tauri/`) → exit 0, no
warnings. Do NOT run the full test suite yet:
`batch_done_caps_at_total_minus_one_while_an_item_is_visible` will now
fail (it asserts the pre-fix numbers) — that failure is expected and
is what Step 2 fixes next, not a sign this step went wrong.

### Step 2: Reconcile + add tests

- `every_completion_increments_batch_done` (locate with
  `rg -n "fn every_completion_increments_batch_done"` — `queue.rs:1842`
  as of `2b8dcfa`) only exercises `OneShot` items (rotate-out, dismiss,
  skip all use the plain `event()` helper) — it stays green unchanged.
- `batch_done_caps_at_total_minus_one_while_an_item_is_visible` (locate
  with `rg -n "fn batch_done_caps_at_total_minus_one"` — `queue.rs:1901`
  as of `2b8dcfa`) **does** encode the bug and needs both a value fix
  and a premise fix, not just a number tweak:
  - Its first assertion, `assert_eq!(queue_progress(&q), (2, 1))` right
    after the first `tick()` (the `Recurring` item `r` rotating out),
    must become `(2, 0)` — under the fix, `r` requeuing no longer
    advances `batch_done`.
  - Its second assertion, after the second `tick()` (the `OneShot`
    item `b` rotating out and `r` re-promoting), stays `(2, 1)` —
    but only because `b`'s single real completion happens to equal
    the pre-fix number, not because the cap clamped anything (`raw
    batch_done == 1 == total - 1`; `min()` is a no-op here). The
    `// ... (2nd completion)` comment and the test's doc-comment ("a
    Recurring item completes a turn every time it rotates out ... over
    a long batch its completions can outnumber the batch size") both
    describe the **pre-fix** mechanism and are now false: under the
    fix, a `Recurring` item's own rotations can never advance
    `batch_done`, so this scenario no longer demonstrates the cap
    engaging. Rewrite the test's doc-comment to say what it actually
    proves post-fix (that a `Recurring` rotation doesn't count, while
    an interleaved `OneShot` completion does) rather than leaving
    stale "caps at total-1" framing on a test that no longer clamps
    anything. If you want a test that still exercises the `min()`
    clamp itself, that requires enough real (non-Recurring) leavers to
    reach `batch_total`, which needs a different setup — add it only
    if you think the clamp path is worth a dedicated case; it's not
    required for this fix.
- Add a test: a visible `Recurring` item that rotates out via `tick()`
  (and one via skip) requeues to the back of its tier **without**
  advancing `batch_done`; a sibling `OneShot` completion still does.
  Model it on `batch_done_caps_at_total_minus_one_while_an_item_is_visible`
  — same helpers (`recurring_event`, `event`, `queue_progress`,
  `visible_title`).
- Run the plan-022 `mod proptest_queue` suite — the batch-counter
  invariant must still hold with the corrected increment (note: no
  existing proptest invariant currently asserts on `batch_done`
  directly — this run is a regression guard, not expected to change
  behavior).

**Verify**: `cargo test --locked` (from `src-tauri/`) → all pass,
including the new test; the reconciled caps test passes with its new
`(2, 0)` first assertion. To run just the property suite:
`cargo test --locked proptest_queue` → all pass.

### Step 3: Docs + status

Update `docs/TESTING_STRATEGY.md` §0's rust row — as of this plan it
reads `| rust unit/integration | 295 tests — settings 45, queue 64,
http 36, ...`: bump both the total and the `queue` sub-count by the
number of tests you added (one, if you follow Step 2). Recount from the
`cargo test --locked` summary line instead of trusting these numbers —
they drift with every merged plan. Flip this plan's `plans/README.md`
row to DONE and rename the file to `038-batch-done-recurring-fix(done).md`.

## STOP conditions

- `queue.rs` dirty in the working tree (another plan in flight) → STOP.
- The drift check does not return exactly three `batch_done += 1` sites
  in `queue.rs` → STOP; the code has moved since `2b8dcfa`.
- The corrected semantics would break a plan-022 proptest invariant in
  a way that isn't a stale-expectation update → STOP and report; the
  invariant may encode intended behavior this plan misread.

## Maintenance notes

- The `queue_done` cap at `current_slot_state()`
  (`u32::try_from(self.batch_done).unwrap_or(u32::MAX).min(queue_total - 1)`,
  `queue.rs:487-490`) was the only thing masking this bug. After this
  fix, `batch_done` should never actually reach `batch_total` while an
  item is visible through any known path — the cap becomes purely
  defensive. Do not delete it on that basis; it stays cheap insurance
  against the next producer that finds a new way to double-count. A
  reviewer should not read "the cap never clamps in the new tests" as
  evidence the cap is dead code to remove.
- **Plan 037 (the Engine)** landed first (merged `6b53c32`) and did NOT
  relocate the arms — its loop in `engine.rs` only calls `q.tick()`.
  The residual risk is any *future* restructure that moves
  `rotate_out_if_elapsed`/`skip_visible` logic out of `queue.rs`: the
  non-Recurring guard must travel with the increment, not revert to
  unconditional.
- **Plan 039** (opt-in ESPN live-match card) is the first real producer
  that will ever emit a `RotationSpec::Recurring` event in production.
  It's the scenario this fix exists for — once it lands, the
  `batch_done`/`queue_done` behavior on a live cycling card is worth a
  manual eyeball alongside its own manual checks, since nothing before
  it ever exercised this path outside `#[cfg(test)]`.
- Reviewer should scrutinize: that the guard was added to *both*
  requeue arms (`rotate_out_if_elapsed` and `skip_visible`), that the
  dismiss arm (`dismiss_visible`) was left unconditional, that
  `batch_done_caps_at_total_minus_one_while_an_item_is_visible`'s
  doc-comment was rewritten to match what it proves post-fix rather
  than left describing the pre-fix mechanism (see Step 2), and that the
  two source comments (the counter doc-comment and the cap comment)
  were rewritten rather than left claiming the pre-fix mechanism (see
  Step 1).
