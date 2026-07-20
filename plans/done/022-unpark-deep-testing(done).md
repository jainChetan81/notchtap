# Plan 022: Decide and execute the parked deep-testing work order — its un-park trigger has fired

> **Executor instructions**: This plan has a mandatory DECISION GATE at
> Step 0 — an operator/maintainer call, not an executor call. Do not
> proceed past Step 0 without it. Then follow steps in order, running
> every verification command. If anything in "STOP conditions" occurs,
> stop and report. When done, update this plan's status row in
> `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat a58f115..HEAD -- docs/TESTING_STRATEGY.md src-tauri/src/queue.rs src-tauri/src/http.rs src-tauri/Cargo.toml`
> Plans 008 (expanded semantics) and 015 (heartbeat/`next_deadline`) have
> BOTH landed (invariants 8 and 9 in Step 2 are now live, not
> conditional); their queue behaviors MUST be in the §9.1 retarget.
> **Plan 019 (dead-code removal) also landed at `a58f115`** — it deleted
> the `dispatch()` call + import from `http.rs` (irrelevant to your burst
> tests, which exercise the `/notify` enqueue path, not dispatch) and
> dropped the §0 counts to **rust 251 + 3 doc-tests / frontend 62**
> (poller 19→16, event 18→16, frontend lost presentation-mode 4). Step 5
> reconciles against THOSE numbers, not the older 256/66. `queue.rs` is
> UNCHANGED since the review pass — its excerpts are exact. The
> `file:line` numbers below are indicative; this repo churns from
> concurrent agent sessions, so locate every target with the `rg` command
> given, not the line number. Run `git status` first — if any in-scope
> file is already dirty in the working tree, STOP and report rather than
> layering onto someone's in-flight edit.

## Status

- **Priority**: P2
- **Effort**: L (Step 0 alone is S if the decision is "re-park")
- **Risk**: LOW (dev-dependency + additive tests; the "risk" is proptest
  finding real bugs, which is the point)
- **Depends on**: none blocking — 008 is DONE (`8ca01e3`; the expanded
  invariants in Step 2 already assume it); Step 0's operator decision gate
  remains
- **Category**: tests
- **Planned at**: commit `d40445e`, 2026-07-17; **review-plan pass
  2026-07-18 at `4af5e8e`** — the earlier `b43a7ca` baseline was stale
  (in-scope files moved since: queue.rs +88, http.rs +59). All excerpts
  and line refs re-verified against `4af5e8e`. New facts folded in:
  the queue grew a `skip_visible` public op (plan 001's wire-skip
  hotkey) that Step 2's Op model was missing — now added as `Skip`; both
  008 and 015 have landed (`next_deadline` present at queue.rs:364 with 5
  tests), so invariants 8/9 are live; the http test router now builds at
  http.rs:124 with the `oneshot` test module at ~280–380 (not ~220).
  **Step 0 DECIDED = EXECUTE by operator 2026-07-18** (un-park trigger
  confirmed fired: v3.6 tiers, v6 rotation tie-break, `8b216ee` gap).
  **Pre-dispatch reconcile at `a58f115`** (`/improve execute`): plan 019
  merged since `4af5e8e` (drift is benign — see Drift check); a
  kimi-authored §9.1 scoping pass, independently verified against
  `queue.rs` by the reviewer, was folded into Step 2 below — specifically
  the full cap-bypass set on invariant 2, the two-caps split on invariant
  6, and explicit `enqueue_test` handling so it does not trip the Step 2
  STOP condition.

## Why this matters

`docs/TESTING_STRATEGY.md` §9 is a fully-written, "implementation-ready"
deep-testing work order (proptest queue invariants, http burst cases,
poller parse fuzz, frontend timing fuzz), deliberately **parked
2026-07-16** with explicit un-park triggers in §8:

> **trigger to un-park**: first queue regression the example cases miss,
> the next queue-semantics change (e.g. a priority lane), or the user
> asking for it. when picked up, §9 is the work order — don't re-plan.

Since parking, the queue's semantics changed repeatedly: v3.6 landed
three priority tiers (literally "a priority lane"), v6 added the
rotation-order tie-break, dismiss-now and (if plan 008 landed)
per-item expanded semantics. Commit `8b216ee` ("queue: extract
apply_fresh_content, close signal-supersede test gap") is evidence the
example suite already let one gap through. The trigger has fired — twice.
The repo's own decision-hygiene demands the decision be *made*, not
drifted past: either execute §9 (this plan's Steps 1–5) or re-park with a
dated rationale (Step 0's alternative outcome).

The queue is now the most complex module in the repo (~345 impl lines:
three tiers × rotation kinds × topic supersession with extension caps ×
per-source rank tie-break × pause gating) — exactly the interleaving
state space §9.1's generated-adversary tests were designed for, and no
human enumerates it by hand.

## Current state

- `docs/TESTING_STRATEGY.md` §9 (lines ~623–861): the work order.
  §9.0 scope/philosophy; §9.1 queue property tests (proptest, an `Op`
  enum + invariants — header at ~677); §9.2 http burst/boundary; §9.3
  poller parse fuzz (~784); §9.4 frontend timing (fast-check, marked
  droppable); §9.5 exclusions (~834); §9.6 build order and review gates
  (~851). **§9's own stale-note** (lines ~649–659) says §9.1 was written
  against the pre-v3.6 queue and needs a retarget pass first — that note
  is part of the work order. (Locate all of these with
  `rg -n "^### 9\.|stale as of" docs/TESTING_STRATEGY.md`; line numbers
  drift.)
- `src-tauri/src/queue.rs`: `SingleSlotQueue` public surface —
  `enqueue`, `tick(Instant)`, `dismiss_visible(Instant)`,
  **`skip_visible(Instant)`** (queue.rs:326 — end the Visible item's turn
  now: a Recurring item requeues to the back of its own tier, a OneShot
  drops, then promote; distinct from dismiss, which drops Recurring
  outright), `toggle_expanded`, `pause`/`resume`, `current_slot_state`,
  `slot_state_if_changed`, `total_waiting`, `with_rotation_order`, plus
  `next_deadline` (015, present) — enumerate the FULL set yourself via
  `rg -n "pub fn" src-tauri/src/queue.rs` and treat every state-mutating
  fn as a candidate `Op` (the read-only accessors — `is_paused`,
  `current_priority`, `current_link`, `current_slot_state` — are
  invariant probes, not ops). 52 example tests in the same file show
  construction patterns (events with priorities/rotations/topics,
  simulated `Instant`s).
- `src-tauri/Cargo.toml` dev-deps: tower, tauri(test), wiremock — no
  proptest yet.
- The example suite is high quality (simulated clocks, no sleeps) — §9's
  tests SUPPLEMENT it, never replace it (§9.0's rule).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Suite | `cargo test` (from `src-tauri/`) | all pass |
| Just properties | `cargo test proptest_ -- --nocapture` (naming per Step 3) | all pass |
| Gates | `cargo clippy --all-targets -- -D warnings && cargo fmt --check` | exit 0 |

## Scope

**In scope**:
- `docs/TESTING_STRATEGY.md` (§8 trigger-status note; §9.1 retarget
  edits; §0 counts)
- `src-tauri/Cargo.toml` (`proptest` as dev-dependency)
- `src-tauri/src/queue.rs` (a `proptest`-based test module — tests only,
  zero production-code changes)
- `src-tauri/src/http.rs` (§9.2 burst cases — tests only)
- `plans/README.md` (status row)

**Out of scope**:
- §9.3 poller fuzz and §9.4 frontend timing fuzz — §9.6's build order
  puts them after the queue+http tranche; they get their own session
  once this one proves out (record as follow-up, don't start them).
- ANY production-code change. If a property test finds a real bug, that
  bug gets reported (and its fix planned separately) — the failing case
  gets committed as a `#[test]` regression with `#[ignore]` + a comment
  if the fix isn't immediate. Do not fix queue logic in this plan.

## Git workflow

- Current branch; commits: `tests: retarget §9.1 to SingleSlotQueue` (docs), `tests: queue property suite (proptest)`, `tests: http burst cases (§9.2)`.
- Do NOT push.

## Steps

### Step 0 (DECISION GATE — operator)

Present the maintainer this choice with the evidence above:
- **Execute §9** (continue to Step 1), or
- **Re-park**: add a dated entry to `docs/TESTING_STRATEGY.md` §8 —
  "trigger fired (v3.6 tiers, v6 rotation-order, `8b216ee` gap);
  re-parked 2026-MM-DD because <maintainer's reason>" — update this
  plan's status row to REJECTED with that one-liner, and stop. That
  outcome is a SUCCESS for this plan (the decision got made).

**Verify**: recorded answer.

### Step 1: Add proptest

`src-tauri/Cargo.toml` `[dev-dependencies]`: `proptest = "1"`.

**Verify**: `cargo build --tests` (from `src-tauri/`) → exit 0.

### Step 2: Retarget §9.1 (docs first, per §9's own stale-note)

Edit `docs/TESTING_STRATEGY.md` §9.1 in place: rewrite the `Op` model and
invariant list against today's queue. The op set (derive the exact list
from `pub fn`s — one variant per state-mutating fn):
`Enqueue(priority, rotation_kind, topic, source)`, `Tick(advance_secs)`,
`Dismiss`, `Skip`, `ToggleExpanded`, `Pause`, `Resume`. **`Skip`
(`skip_visible`) is easy to miss and must be in the model** — it ends
the Visible turn early like `Dismiss`, but requeues a Recurring item to
the back of its tier instead of dropping it, so it is a distinct
transition (and a documented cap-bypass, see invariant 2). If you find a
state-mutating `pub fn` not in this list, STOP and report before
building the strategy — **with two pre-cleared exceptions the reviewer
already verified, so do NOT stop on these**:
- `enqueue_test` (`rg -n "pub fn enqueue_test" src-tauri/src/queue.rs`)
  is a public, state-mutating, **test-only** variant of `enqueue` that
  promotes into the visible slot even while paused (it calls
  `enqueue_with_options(.., bypass_pause_when_slot_empty = true)`). Do
  NOT add it as a distinct `Op`; the production op model covers the real
  `/notify` path. (If you later want to cover the `send_test_notification`
  path, a separate `EnqueueTest` variant is the way — but that is
  out-of-scope for this pass; note it as a follow-up, don't build it.)
- `slot_state_if_changed` mutates `last_emitted` but is the **probe for
  invariant 7**, not an `Op`. Call it to *check* the invariant; never
  generate it as a transition.
Invariants to state (translate §9.1's originals + add the post-v3.6/v6
ones):

1. at most one Visible item ever;
2. per-tier waiting length ≤ `max_queued_per_tier`, **asserted only
   immediately after an `Enqueue` that lands in `waiting`** — NOT after
   every op. The reviewer verified (against `queue.rs`) that the cap
   check lives at `enqueue_new`'s `if !can_promote_now && waiting[tier]
   .len() >= max_queued_per_tier` guard, so there are THREE documented
   bypasses that can legally push a tier over the cap; an always-on
   assertion would false-fail on all three:
   (a) the **immediate-promote fast path** — when `can_promote_now`
   (slot empty, all tiers empty, not paused) the item goes straight to
   `visible` and the cap is never evaluated;
   (b) **Recurring requeue** — both natural rotation (`tick`) and `Skip`
   (`skip_visible`) `push_back` a Recurring item with no cap check;
   (c) **cross-tier Topic supersede** — moves an existing waiting item to
   the back of a different tier with no cap check.
   Encode the assertion accordingly (post-`Enqueue`-into-waiting only),
   and leave a note that tightening any of these bypasses is a known open
   question for the maintainer;
3. a Visible item is never replaced before its window elapses (no
   preemption) EXCEPT by an explicit `Dismiss` or `Skip` op — those two
   are the only early-removal transitions, and after either at most one
   Visible remains and the next promotion follows the tier/rotation rule
   in invariant 4;
4. promotion picks the highest non-empty tier, and within a tier respects
   rotation_order rank then FIFO;
5. while Paused, `tick` never promotes but Visible still ages out;
   Resume promotes on the next tick; nothing enqueued while paused is
   lost (count conservation: enqueued-accepted = visible + waiting +
   rotated-out-dropped + dismissed + skipped-oneshot-dropped, tracked by
   the model — note a skipped Recurring item is NOT a drop, it returns to
   waiting, so only the OneShot arm of `Skip` decrements);
6. supersession by Topic never creates a second item (count conservation
   again). Two DISTINCT caps are in play here — the reviewer confirmed
   the draft's "the documented cap" was ambiguous: (i) a **visible
   Topic supersede** tops up remaining time bounded by
   `MAX_EXTENSION_ON_SUPERSEDE_SECS` (currently `6`, a `const` in
   `queue.rs` — assert `extension_secs <= 6` after a visible supersede);
   (ii) a **cross-tier waiting supersede** is the invariant-2(c) per-tier
   cap bypass, NOT bounded here. State both separately;
7. `slot_state_if_changed` never returns two consecutive equal states;
8. (if plan 008 landed) after any promotion of a non-High item,
   expanded == false; a High promotion sets it true;
9. (if plan 015 landed) whenever `next_deadline()` is Some it equals
   `promoted_at + window + extension`.

Also add the dated "trigger fired, executing §9" note to §8.

**Verify**: the §9.1 section reads as a spec an executor could implement without this plan; §8 note present.

### Step 3: Implement the queue property suite

New `mod proptest_queue` (inside `queue.rs`'s `#[cfg(test)]` tests module
or a sibling module) implementing Step 2's model: a proptest strategy
generating `Vec<Op>` (length ~0–50), applied against BOTH the real
`SingleSlotQueue` and a simple reference model (or checked directly via
invariant assertions after each op — direct invariant checking is fine
and simpler; a full reference model is only needed for the conservation
invariants). Use a synthetic clock: keep a `now: Instant` in the runner,
advanced by `Tick(secs)`. Keep case counts moderate
(`ProptestConfig { cases: 256, .. }`) so the suite stays fast; CI time
budget: the whole `cargo test` run should stay under ~2 minutes.

**Verify**: `cargo test` → all pass; run twice to shake out flaky
generation; note the wall-time delta in your report.

### Step 4: §9.2 http burst cases

Implement §9.2 as written in the doc (read it — it lists the cases):
burst enqueues to tier caps via `tower::ServiceExt::oneshot` against the
existing test router — the router builds at `http.rs:124` (`Router::new()`)
and the `#[cfg(test)]` module's `oneshot` cases run ~http.rs:280–380;
model new burst tests on those (find the exact helper with
`rg -n "oneshot|Router::new|json_request" src-tauri/src/http.rs`) —
boundary bodies (empty title, 64 KiB limit edge), and paused-state
responses.
Skip any §9.2 case the example suite already covers identically (list
skips in your report).

**Verify**: `cargo test http::` → all pass.

### Step 5: Reconcile the docs

`docs/TESTING_STRATEGY.md`: §0 counts (+N per module); §9.1/§9.2 get
"landed 2026-MM-DD" markers in the §9 preamble (leaving §9.3/§9.4 parked
with one line saying so); §8's table row for the deep-testing item
updated.

**Verify**: `cargo test 2>&1 | grep "test result"` totals match §0.

## Test plan

The plan IS the test plan: the property suite (Step 3) + burst cases
(Step 4). Every pre-existing test stays green and untouched — if a
property test exposes a real queue bug, see the Out-of-scope rule
(report + ignored regression case, no fixes here).

## Done criteria

- [ ] Step 0 decision recorded (either outcome)
- [ ] If executing: `grep -c "proptest" src-tauri/Cargo.toml` → 1;
      property + burst suites present and green
- [ ] `cargo test` exits 0 in < ~2 min; clippy/fmt gates exit 0
- [ ] TESTING_STRATEGY §8/§9/§0 reconciled
- [ ] Any bug found is reported with a minimized failing case (and NOT
      fixed in this plan's diff)
- [ ] `plans/README.md` status row updated

## STOP conditions

- Step 0 not answerable (no operator available) — record the evidence in
  the status row as BLOCKED and stop; do not default to executing an
  L-effort plan without the call.
- A property test fails and you cannot tell whether the model or the
  queue is wrong — commit the minimized case as `#[ignore]`d, report,
  stop.
- The suite exceeds the ~2-minute budget even at reduced cases — report
  timings and options (fewer cases, `--release` test profile) instead of
  shipping slow CI.

## Maintenance notes

- Every future queue-semantics change must extend the `Op`
  model/invariants in the same PR — §9.1's retargeted text should say so
  (it does after Step 2; reviewers enforce it).
- §9.3 (poller fuzz) and §9.4 (frontend timing) remain parked with their
  own §9.6 ordering — next candidates once this tranche has lived a
  while.
- Plans 008/015 add invariants 8/9 — if they land after this plan,
  extending the model is their job (noted in their maintenance sections'
  spirit; cross-check).
