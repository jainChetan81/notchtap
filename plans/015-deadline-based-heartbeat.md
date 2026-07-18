# Plan 015: Replace the 250 ms heartbeat with deadline-based wakeups

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report. When done,
> update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat b43a7ca..HEAD -- src-tauri/src/lib.rs src-tauri/src/queue.rs src-tauri/src/http.rs src-tauri/src/poller.rs src-tauri/src/rss_poller.rs`
> On any change, compare excerpts below; mismatch = STOP. Plan 008
> (expanded semantics) HAS landed (`8ca01e3`, in the `b43a7ca` baseline):
> `promote_next` now calls `set_expanded_for_promotion(item.event.priority)`
> via a shared helper — that difference from the excerpt below is expected,
> not drift.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED — rotation/promotion timing is the core behavior; a wake-signal bug stalls promotion
- **Depends on**: plans/009 (seam pin — land first); 008 is DONE and already
  in this plan's baseline
- **Category**: perf
- **Planned at**: commit `d40445e`, 2026-07-17; drift baseline refreshed to `b43a7ca` 2026-07-18 (excerpts re-verified unchanged)

## Why this matters

`spawn_heartbeat` ticks every 250 ms forever — ~345,000 wakeups/day on a
24/7 background app whose queue is empty the overwhelming majority of the
time. Sub-second repeating timers are exactly what defeats macOS timer
coalescing/App Nap and shows up as steady background energy use. Each tick
also calls `slot_state_if_changed()`, which clones every String on the
visible card (title/body/source/category/link) 4×/second just to compare
and discard. The queue already knows precisely when the next
time-driven transition is due (visible item's rotation deadline); the loop
should sleep until that deadline — or forever when idle — and be woken
early by mutations.

Design: keep ONE emitter task (the heartbeat) exactly as today —
mutation sites still emit their own immediate state changes as they do
now; the reworked heartbeat only needs waking so its *next rotation
deadline* is recomputed after any mutation.

## Current state

`src-tauri/src/lib.rs:597-614`:

```rust
fn spawn_heartbeat(app: tauri::AppHandle, queue: Arc<Mutex<SingleSlotQueue>>) {
    // 250ms rotation heartbeat (v3.6 spec §4.3): rotation and promotion
    // never depend on a new push arriving
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(250));
        loop {
            interval.tick().await;
            let slot_change = {
                let mut q = queue.lock().await;
                q.tick(Instant::now());
                q.slot_state_if_changed()
            };
            if let Some(state) = slot_change {
                emit_slot_state(&app, state);
            }
        }
    });
}
```

`src-tauri/src/queue.rs` internals the new method needs (all existing):
- `visible: Option<QueueItem>`; `QueueItem.promoted_at: Option<Instant>`,
  `extension_secs: u64`
- rotation window math at `rotate_out_if_elapsed` (queue.rs:165-177):
  `let window = item.event.rotation_window(self.expanded) + item.extension_secs;`
  — deadline = `promoted_at + Duration::from_secs(window)`.
- `paused: bool`; `total_waiting()`.

Mutation entry points that must wake the heartbeat (each already holds/
takes the queue lock and already emits its own immediate change):
- `src-tauri/src/http.rs` `/notify` handler (enqueue)
- `src-tauri/src/poller.rs` `enqueue_and_fan_out` call-site in the espn
  loop; `src-tauri/src/rss_poller.rs` enqueue loop
- `src-tauri/src/lib.rs`: `toggle_pause` (resume must recompute),
  `dismiss_current`, `toggle_manual_expand` (expanded changes the
  window), and the settings/tray paths that call these.

Wiring: `spawn_heartbeat(app.handle().clone(), setup_queue.clone())` is
called once in `run()`'s setup (lib.rs:262). The queue is shared as
`Arc<Mutex<SingleSlotQueue>>` everywhere; a `tokio::sync::Notify` can ride
alongside it (new `Arc<Notify>` created in setup and threaded to the same
places the queue Arc already goes — every wake site already receives the
queue, so add a parameter or bundle the pair in a small struct; prefer the
smallest-diff option: pass `Arc<Notify>` as an extra parameter to the
spawn fns and store one in the axum `AppState`).

Repo conventions: queue logic tests are simulated-`Instant`, no sleeps;
async integration is allowed to use short real waits only in
`#[tokio::test]` (see notifier's 10ms retry_delay pattern). Counts live
ONLY in `docs/TESTING_STRATEGY.md` §0. The v3.6 spec's §4.3 names the
250 ms heartbeat — update that spec line (it is a working draft;
adjusting it to "deadline-based, woken by mutations" is the designed
mechanism).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Queue tests | `cargo test queue::` (from `src-tauri/`) | all pass |
| Full suite | `cargo test` (from `src-tauri/`) | all pass |
| Gates | `cargo clippy --all-targets -- -D warnings && cargo fmt --check` (from `src-tauri/`) | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/queue.rs` — add `next_deadline(&self) -> Option<Instant>`
  (+ tests)
- `src-tauri/src/lib.rs` — rework `spawn_heartbeat`; thread the `Notify`
- `src-tauri/src/http.rs`, `src-tauri/src/poller.rs`,
  `src-tauri/src/rss_poller.rs` — one `notify_waiters()`/`notify_one()`
  call after each mutation
- `docs/V3_6_TECHNICAL_SPEC.md` §4.3 (one-line update)
- `docs/TESTING_STRATEGY.md` §0
- `plans/README.md` (status row)

**Out of scope**:
- Queue rotation/promotion semantics — `tick()` behavior unchanged.
- The pollers' own intervals; the emit mechanism; the frontend.
- Removing mutation-site emissions (they stay; see Design above).

## Git workflow

- Current branch; commit style: `queue+lib: deadline-based heartbeat — sleep to next rotation, wake on mutation`.
- Do NOT push.

## Steps

### Step 1 (tests first): `next_deadline`

Add to `SingleSlotQueue`:

```rust
/// The next Instant at which time alone changes state: the visible
/// item's rotation deadline. None when nothing is visible (promotion
/// of waiting items is driven by mutations, which wake the heartbeat)
/// — and None while paused with a visible item still counting down is
/// WRONG: paused items still age out (Paused disables promotion only),
/// so the deadline must be returned regardless of `paused`.
pub fn next_deadline(&self) -> Option<Instant> {
    let item = self.visible.as_ref()?;
    let promoted_at = item.promoted_at?;
    let window = item.event.rotation_window(self.expanded) + item.extension_secs;
    Some(promoted_at + Duration::from_secs(window))
}
```

Tests (queue.rs test module, simulated Instants):
1. empty queue → `None`
2. visible item → `Some(promoted_at + window)` exact value
3. expanded visible item → deadline uses the expanded window
4. paused with a visible item → still `Some(..)` (aging continues while
   paused — matches the existing paused semantics tests)
5. after a supersession extension (`extension_secs > 0`) → deadline
   includes it (drive via the existing supersede test helpers).

**Verify**: `cargo test queue::` → all pass (write tests first, watch them fail on the missing method, then implement).

### Step 2: Rework the heartbeat loop

```rust
fn spawn_heartbeat(
    app: tauri::AppHandle,
    queue: Arc<Mutex<SingleSlotQueue>>,
    wake: Arc<tokio::sync::Notify>,
) {
    // Deadline-based (plan 015): sleeps until the visible item's rotation
    // deadline (or forever when idle) and is woken by any queue mutation.
    // Replaces the fixed 250ms tick — same observable behavior, ~0 idle
    // wakeups. A small grace addition avoids sub-ms re-loops at the edge.
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
}
```

Note the emit moved INSIDE the lock scope — with a single looping emitter
this also removes the emit-after-unlock reordering window the audit
flagged separately for this task (a strict improvement; the mutation-site
emitters are unchanged and out of scope).

Create the `Notify` in `run()`'s setup next to `setup_queue`, pass it to
`spawn_heartbeat`.

**Verify**: `cargo build` → exit 0. Existing lib tests still pass (`cargo test`).

### Step 3: Wake on every mutation

Add exactly one wake call after each mutation site listed in Current
state (after the lock is released or just before — `Notify::notify_waiters()`
is cheap and never blocks):
- http.rs handler: after enqueue handling → `wake.notify_waiters();`
  (thread the `Arc<Notify>` through `AppState` — add a field alongside
  the queue).
- poller.rs espn loop + rss_poller.rs loop: after their enqueue/emit
  block (add an `Arc<Notify>` parameter to both spawn fns, passed from
  `run()`).
- lib.rs `toggle_pause`, `dismiss_current`, `toggle_manual_expand`:
  after their queue mutation.

A missed wake site = a stalled state until the next legitimate wake, so
after wiring, grep-audit: every call of `q.enqueue`, `toggle_pause`,
`dismiss_visible`, `toggle_expanded`, `apply_fresh_content`-via-enqueue
outside queue.rs must have a wake nearby. List each site + its wake in
your report.

**Verify**: `cargo test` → all pass. `cargo clippy --all-targets -- -D warnings && cargo fmt --check` → exit 0.

### Step 4: Integration test + manual smoke

Add one `#[tokio::test]` (in lib.rs's test module or http.rs's,
whichever already constructs the fuller harness) with tokio's paused
clock if feasible: spawn the heartbeat against a mock-app queue, enqueue
an item with a 1-second window via the normal path, `notify` the waker,
advance time, and assert the item rotates out (queue state empty) without
any 250 ms polling — i.e., the deadline sleep fired. If tokio's
`start_paused` test clock fights `tauri::async_runtime` (they are
different runtimes — tauri wraps tokio; check how existing async tests in
http.rs spawn), fall back to a short real-time test: 1s window, assert
rotated within 2s. Keep it under ~3s wall time.

Manual smoke (operator or dev machine): `npm run tauri dev`, push a
notification (`./notchtap --title t --body b`), confirm it appears and
rotates out on schedule; leave the app idle 5 minutes and confirm the
process shows ~0% CPU in Activity Monitor (vs the constant tick before).

**Verify**: `cargo test` all green; manual smoke reported or handed to operator.

### Step 5: Docs

- `docs/V3_6_TECHNICAL_SPEC.md` §4.3: replace the "250ms heartbeat"
  description with the deadline+wake design (one short paragraph).
- `docs/TESTING_STRATEGY.md` §0: queue +5, plus the integration test's
  home module +1.

**Verify**: `grep -rn "250ms\|250 ms" docs/V3_6_TECHNICAL_SPEC.md src-tauri/src/lib.rs` → no remaining claims that the heartbeat is 250 ms (historical mentions elsewhere in docs are fine — check context).

## Test plan

- 5 unit tests for `next_deadline` (Step 1).
- 1 async integration test for the sleep/wake loop (Step 4).
- Full existing suite green throughout — the queue's `tick()` semantics
  are untouched, so every existing rotation/promotion test doubles as a
  regression net.

## Done criteria

- [ ] `grep -c "interval(Duration::from_millis(250))" src-tauri/src/lib.rs` → 0
- [ ] `grep -c "next_deadline" src-tauri/src/queue.rs` → ≥2
- [ ] `grep -c "notify_waiters\|notified()" src-tauri/src` → wake sites for http, both pollers, and the three lib handlers (list them in the report)
- [ ] `cargo test` exits 0 with the new tests
- [ ] clippy/fmt gates exit 0
- [ ] Spec §4.3 + §0 updated
- [ ] Manual smoke reported
- [ ] `plans/README.md` status row updated

## STOP conditions

- `tauri::async_runtime` does not expose/cooperate with
  `tokio::time::sleep_until` + `Notify` (it should — it's tokio
  underneath) — report before inventing a channel-based design.
- Any existing queue/lib/http test fails in a way that implicates
  rotation *semantics* rather than wiring — the plan's premise (tick
  behavior unchanged) would be wrong; report.
- You cannot enumerate the mutation sites confidently (grep results
  ambiguous) — report the list you found and stop rather than shipping a
  possible missed-wake stall.

## Maintenance notes

- **Every future queue-mutation call-site must wake the heartbeat.** This
  is the new invariant reviewers must check on any PR touching enqueue/
  dismiss/pause/expand paths. Consider (deferred) bundling
  `(Arc<Mutex<SingleSlotQueue>>, Arc<Notify>)` into one handle type so a
  mutation without a wake is impossible to express.
- The `Instant`-based deadline freezes across system sleep (existing,
  documented-by-audit behavior — items resume their remaining time at
  wake). Unchanged by this plan; if wall-clock semantics are ever wanted,
  that's a separate decision.
- Plan 022's property tests, if executed, should include "after any Op
  sequence, if a deadline exists it equals promoted_at + window +
  extension" as an invariant.
