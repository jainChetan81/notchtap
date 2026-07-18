# Plan 036: Close the heartbeat's lost-wakeup race (register the waiter under the queue lock)

> Numbering note: this plan was drafted as 024 and renumbered to 036 —
> a concurrent session's in-flight `plans/024-proptest-rotation-order-coverage.md`
> holds that number. Despite the high number it is **P1, execute first**
> among the third-audit-session plans.

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat a58f115..HEAD -- src-tauri/src/lib.rs docs/TESTING_STRATEGY.md`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `a58f115`, 2026-07-18. **Review-plan pass
  2026-07-18 (advisor, against `d926977` + filing)**: every excerpt
  and the seven-site `notify_waiters()` list re-verified exact against
  live code (the list is complete — no eighth caller exists);
  `Cargo.lock` has tokio `1.52.3`, so the `Notified::enable` ≥1.16
  STOP condition is expected to pass. Known benign drift: plan 022
  landed after this plan was written — the drift check WILL show
  `docs/TESTING_STRATEGY.md` movement and §0 now reads **257 + 3
  doc-tests** (queue 53, http 32), not the 251 quoted below; that is
  the expected 022 drift, NOT a STOP — `src-tauri/src/lib.rs` is
  unchanged and Step 3's recount language governs the counts.

## Why this matters

Plan 015 replaced the queue's fixed 250 ms polling heartbeat with
deadline-based wakeups: the heartbeat sleeps until the visible item's
rotation deadline (or forever when the queue is idle) and every queue
mutation wakes it via `tokio::sync::Notify::notify_waiters()`. That
rewrite introduced a classic lost-wakeup race: the heartbeat computes
its deadline **under** the queue lock, releases the lock, and only
*then* creates its `notified()` future — but `notify_waiters()` only
wakes waiters that are already registered; it never stores a permit for
a future waiter. A mutation that lands in the gap between the
heartbeat's lock release and its `.notified().await` is silently lost.

The damaging case is the idle branch: the heartbeat computes
`next_deadline() == None` (queue just rotated to empty) and is about to
park **with no fallback timer**. If an enqueue races into that gap, the
new card is displayed (the enqueue path emits directly) but the
heartbeat never learns about it — the card sticks on the overlay past
its Rotation window until some *future* mutation happens to wake the
heartbeat. With both pollers disabled (`espn_enabled = false`,
`rss_enabled = false` — the cmux/manual-push-only workflow, a
first-class configuration), nothing else ever wakes it, so a single
push can wedge the overlay indefinitely. The old 250 ms tick self-healed
within one tick; the deadline rewrite regressed that resilience. The
race window recurs every time the rail rotates to empty, because the
enqueue that races may already be parked on the queue lock while the
heartbeat computes `None` — the window is not merely microseconds.

The `Some(deadline)` branch has the same race but a bounded symptom
(the `sleep_until` still fires at the previously computed deadline, so
the heartbeat over-sleeps by at most the old window); fixing the
registration order fixes both branches at once.

## Current state

- `src-tauri/src/lib.rs` — app wiring; contains `spawn_heartbeat`
  (lines 684–714) and its test
  `heartbeat_rotates_out_via_deadline_sleep_not_polling` (line ~1049).
- `src-tauri/src/http.rs:69-95` — `enqueue_and_emit`, the shared
  enqueue path; note its doc comment (lines 60–68) promising "every
  caller of this function wakes the heartbeat by construction". The
  wake-site side is correct; the bug is purely on the heartbeat's
  registration side.
- `src-tauri/src/queue.rs:364-369` — `next_deadline()`: returns
  `Some(promoted_at + rotation_window + extension)` for a visible item,
  `None` when nothing is visible.

The buggy loop as it exists today (`src-tauri/src/lib.rs:693-713`):

```rust
    tauri::async_runtime::spawn(async move {
        loop {
            let deadline = {
                let mut q = queue.lock().await;
                q.tick(Instant::now());
                if let Some(state) = q.slot_state_if_changed() {
                    emit_slot_state(&app, state);
                }
                q.next_deadline()
            };
            match deadline {
                Some(at) => {
                    tokio::select! {
                        _ = tokio::time::sleep_until(tokio::time::Instant::from_std(at + Duration::from_millis(10))) => {}
                        _ = wake.notified() => {}
                    }
                }
                None => wake.notified().await,
            }
        }
    });
```

The race: between the closing `}` of the `let deadline = { ... }` block
(which drops the `MutexGuard`) and the creation/first-poll of
`wake.notified()`, another task can lock the queue, mutate it, unlock,
and call `wake.notify_waiters()` — which finds no registered waiter and
does nothing.

Why registering under the lock is sufficient: every mutation site in
the codebase acquires the queue lock *before* mutating and calls
`notify_waiters()` *after* releasing it (`http.rs:87`,
`poller.rs:623`, `rss_poller.rs:518`, `lib.rs:614`, `lib.rs:739`,
`lib.rs:755`, `lib.rs:774`). So any mutation the heartbeat's deadline
computation did not see must acquire the lock after the heartbeat
released it — and by then the waiter is already registered, so its
`notify_waiters()` is guaranteed to be observed. This is exactly the
pattern `tokio::sync::Notify::notified()`'s `enable()` method exists
for (see the `Notified::enable` docs in tokio ≥ 1.16).

Repo conventions that apply: comments state constraints the code can't
show (see the existing plan-015 comment style at `lib.rs:689-692`);
rust tests live in `#[cfg(test)] mod tests` at the bottom of the same
file; test counts are recorded in `docs/TESTING_STRATEGY.md` §0 and
only there.

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Rust tests | `cd src-tauri && cargo test --locked` | 257 + 3 doc-tests pass at current HEAD (022 landed since planning; recount against §0, don't trust totals); more after this plan if tests are added |
| Lint | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Format | `cd src-tauri && cargo fmt --check` | exit 0 |
| Full local gate | `just test-all` (if `just` installed; else run the recipes from `justfile` manually) | all green |

## Scope

**In scope** (the only files you should modify):
- `src-tauri/src/lib.rs`
- `docs/TESTING_STRATEGY.md` (§0 counts, only if the test count changes)
- `plans/README.md` (status row)

**Out of scope** (do NOT touch, even though they look related):
- `src-tauri/src/http.rs`, `poller.rs`, `rss_poller.rs`, `queue.rs` —
  the wake sites and `next_deadline()` are correct; do not "harden"
  them, and do not switch any site from `notify_waiters()` to
  `notify_one()` (that would change semantics for the
  `enqueue_and_emit_wakes_the_heartbeat` test in http.rs, which
  registers its own waiter).
- The known, deliberately deferred slot-state emit-after-unlock
  reordering (recorded in `plans/README.md`) — not this plan's problem.

## Git workflow

- Branch: `advisor/036-heartbeat-lost-wakeup` (or work on master if the
  operator dispatched you that way — match how previous plans were run).
- Commit style (from `git log`): lowercase `area: imperative summary`,
  e.g. `heartbeat: register wake waiter under the queue lock`.
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Restructure `spawn_heartbeat` to arm the waiter under the lock

In `src-tauri/src/lib.rs`, rewrite the loop body of `spawn_heartbeat`
to this shape (comment included — it states the invariant):

```rust
    tauri::async_runtime::spawn(async move {
        loop {
            // Arm the wake waiter *while holding the queue lock* (plan 036):
            // every mutation site locks the queue before mutating and calls
            // `notify_waiters()` after unlocking, so a waiter registered
            // under the lock can never miss a mutation this iteration's
            // `next_deadline()` didn't already see. Registering after the
            // unlock (the original plan-015 shape) lost any wake that landed
            // in the gap — fatal in the `None` branch, which parks with no
            // fallback timer.
            let notified = wake.notified();
            tokio::pin!(notified);
            let deadline = {
                let mut q = queue.lock().await;
                q.tick(Instant::now());
                if let Some(state) = q.slot_state_if_changed() {
                    emit_slot_state(&app, state);
                }
                notified.as_mut().enable();
                q.next_deadline()
            };
            match deadline {
                Some(at) => {
                    tokio::select! {
                        _ = tokio::time::sleep_until(tokio::time::Instant::from_std(at + Duration::from_millis(10))) => {}
                        _ = notified.as_mut() => {}
                    }
                }
                None => notified.await,
            }
        }
    });
```

Notes:
- `notified.as_mut().enable()` registers the waiter; after this call,
  any `notify_waiters()` marks the future ready even though it has not
  been polled yet.
- The `select!` must poll `notified.as_mut()` (the pinned future), not
  a fresh `wake.notified()`.
- Keep the existing 10 ms grace addition and the plan-015 header
  comment above the spawn; amend the header comment's "Replaces the
  fixed 250ms tick" paragraph only if it now contradicts the code.

**Verify**: `cd src-tauri && cargo build 2>&1 | tail -3` → compiles
with no errors (warnings about unused imports would indicate a
mis-edit).

### Step 2: Add a regression test for the idle-park → wake path

In `lib.rs`'s `#[cfg(test)] mod tests`, next to
`heartbeat_rotates_out_via_deadline_sleep_not_polling` (~line 1049),
add one test modeled on it structurally (same `tauri::test::mock_app()`
+ `wake()` helper + timeout-poll loop):

`heartbeat_parked_idle_wakes_on_enqueue_and_rotates_out`:
1. Create an empty queue, spawn the heartbeat, and give it time to
   reach the idle park (`tokio::time::sleep(Duration::from_millis(100)).await`).
2. Enqueue a `RotationSpec::OneShot { ttl_secs: 1 }` event through
   `crate::http::enqueue_and_emit` (the real shared path — it performs
   the wake itself; visibility permitting, otherwise lock + `enqueue` +
   `notify_waiters()` exactly as the existing test does).
3. Assert the slot becomes `Showing`, then poll (50 ms interval, 3 s
   timeout, same as the existing test) until `SlotState::Empty`.
4. Assert the timeout did not elapse — i.e. the parked heartbeat woke
   and rotated the item out.

Be honest about what this pins: it covers the idle-park/wake/rotate
path end-to-end; the *race itself* (a wake landing in the old
unlock-to-park gap) is not deterministically reproducible from a test
without disproportionate machinery — the structural guarantee is the
`enable()`-under-lock shape, which Step 3's grep pins.

**Verify**: `cd src-tauri && cargo test --locked heartbeat` → both
heartbeat tests pass.

### Step 3: Reconcile counts and docs

- If Step 2 added one test, update `docs/TESTING_STRATEGY.md` §0: the
  `lib` module count and the rust total each +1 (at planning time: lib
  12 → 13, total 251 → 252 — recount against the actual §0 file, do
  not trust these numbers blindly if other plans landed first).

**Verify**: `cd src-tauri && cargo test --locked 2>&1 | tail -5` → full
suite green; the pass count matches what you wrote into §0.

## Test plan

- New test: `heartbeat_parked_idle_wakes_on_enqueue_and_rotates_out`
  in `src-tauri/src/lib.rs` (Step 2) — pattern:
  `heartbeat_rotates_out_via_deadline_sleep_not_polling` at
  `lib.rs:1049`.
- Existing tests that must stay green untouched:
  `heartbeat_rotates_out_via_deadline_sleep_not_polling` (lib.rs),
  `enqueue_and_emit_wakes_the_heartbeat` (http.rs:737).
- Verification: `cd src-tauri && cargo test --locked` → all pass.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `cd src-tauri && cargo test --locked` exits 0
- [ ] `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` exits 0
- [ ] `cd src-tauri && cargo fmt --check` exits 0
- [ ] `grep -n "enable()" src-tauri/src/lib.rs` shows the call inside
      `spawn_heartbeat`'s lock block (between `queue.lock().await` and
      `next_deadline()`)
- [ ] `grep -c "wake.notified()" src-tauri/src/lib.rs` returns 0 inside
      `spawn_heartbeat` (the loop uses the single pinned `notified` —
      manual eyeball of the function body is acceptable for this one)
- [ ] `docs/TESTING_STRATEGY.md` §0 matches the actual test count
- [ ] No files outside the in-scope list are modified (`git status`)
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- `spawn_heartbeat` in the live code no longer matches the "Current
  state" excerpt (another session may have touched it).
- `tokio::pin!` or `Notified::enable` fails to compile — that would
  mean the vendored tokio is older than 1.16; report the version from
  `Cargo.lock` instead of working around it.
- The new test in Step 2 is flaky (fails on any of 3 consecutive
  `cargo test heartbeat` runs) — report rather than papering over with
  longer sleeps.
- Fixing this appears to require changing any wake site
  (`notify_waiters()` caller) — it must not.

## Maintenance notes

- Any future queue-mutation site must keep the contract: lock → mutate
  → unlock → `notify_waiters()`. The heartbeat's under-lock
  registration depends on mutation sites acquiring the same lock; a
  mutation that bypasses the queue lock would reopen the race.
- Reviewer should scrutinize: that `enable()` is inside the lock block,
  and that the `select!` polls the pinned future rather than creating a
  second `notified()`.
- **Plan 037 (engine propagation, filed 2026-07-18) depends on this
  plan's exact shape**: it will later relocate this loop into a new
  `engine.rs` preserving the `enable()`-under-lock structure, and its
  dependency gate greps for `enable()` inside `spawn_heartbeat` to
  prove 036 landed. Implement the loop as specified here (no renames,
  no restructuring beyond the plan) so that gate stays meaningful.
- Deferred out of this plan: the existing heartbeat test's real-clock
  polling loop (audit finding TEST-02) — a single wall-clock
  characterization test, accepted as-is; revisit only if it flakes in
  CI. Also deferred: the known slot-state emit-order gap (tracked in
  `plans/README.md` dependency notes).
