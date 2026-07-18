# Plan 033: Queue slider track + auto-expand-all lifecycle

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report. When done,
> update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat d926977..HEAD -- src-tauri/src/queue.rs src-tauri/src/event.rs src/components/Track.tsx src/useSlotState.ts`
> On any change, re-verify the excerpts below; mismatch = STOP.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED (touches rotation/expanded semantics — the suite must stay green)
- **Depends on**: 032 (both touch the compact DOM/CSS region);
  036 (soft, but land it first — it fixes the plan-015 heartbeat's
  lost-wakeup race in `lib.rs`, and this plan's retract emission +
  disarm hotkey ride that same wake mechanism; executing 033 first
  means rebasing over 036's rewritten wait loop and debugging retract
  tests against a known-racy heartbeat)
- **Reviewed**: 2026-07-18 at `1add02e` (review-plan pass) — excerpts
  re-verified (incl. that the expand assignment lives in the single
  `set_expanded_for_promotion` helper, which Step 2's no-arg-setter
  instruction already matches); invariant-9 rewrite added to Step 2,
  036 ordering surfaced, git-status criterion added
- **Category**: queue/ui
- **Planned at**: commit `d926977`, 2026-07-18, prototype rev-3 session.
  The rev-2 draft omitted to assign the auto-expand lifecycle to any group;
  it lands here because it is queue/rotation semantics, not presentation.

## Decisions locked (operator, 2026-07-18)

1. **Track = queue slider, not priority load.** One segment per item in
   the current batch; consumed items dim, current bright, advancing as the
   queue rotates ("5 items, one by one, like an automated slider").
2. **Slider ceiling**: batches render at most 10 segments; beyond that the
   index maps proportionally (`current = floor(done * 10 / total)`). No
   "+N" labels — proportional compression only.
3. **Every promotion starts expanded, regardless of priority** ("it expands
   regardless of priority, then retracts, then moves on"). This reverses
   plan 008's High-only auto-expand (008's per-item reset and idle no-op
   stay). **Total turn length is unchanged**: the item's rotation window
   stays its configured ttl — auto-expansion is display-only and *free*.
   The card auto-collapses at **half the base window** and finishes its
   turn compact (operator decision 2026-07-18, over the alternative of
   giving every item the 3× expanded window). The 3× window rule becomes
   **manual-expand-only**: a ⌃⇧N press that expands an item extends its
   window exactly as today (and any press disarms the auto-retract).
4. **Batch semantics** (exact, pin them in tests): a batch starts when an
   event is accepted while the engine is fully idle (nothing visible,
   every tier empty) — `batch_done = 0`, `batch_total = 0`. Every accepted
   enqueue increments `batch_total`. Every item completion (rotated out,
   dismissed, skipped) increments `batch_done`. Supersession is not a
   completion. Fully idle again → counters reset for the next batch.

## Why this matters

The slider makes queue depth visible at a glance (the operator's
instagram-story metaphor), and expand-all makes every promotion
self-summarizing — the two changes share `queue.rs` and the slot-state
payload, so they ship together.

## Current state (verified at `d926977`)

- `queue.rs:224` — `self.expanded = priority == Priority::High;` (plan
  008's auto-expand), with the same assignment at both promotion sites
  (immediate-enqueue fast path + `promote_next`); per-item reset and the
  idle no-op toggle are covered by tests at `queue.rs:1243+`.
- `queue.rs:346-351` — `toggle_expanded` flips `self.expanded` when
  something is visible; `next_deadline` (:364-369) returns the rotation
  deadline used by the plan-015 heartbeat.
- `SlotState::Showing` fields at `event.rs:148-160`; the camelCase
  snapshot test at `event.rs:317+` pins them.
- `Track.tsx` renders 3 segments, `LIT_SEGMENTS[priority]` lit.
- `useSlotState.ts` validator (:68-90) rejects unknown field shapes.
- `slot_state_if_changed` (queue.rs:375-383) re-emits whenever any
  `SlotState` field changes — an enqueue while visible therefore updates
  the slider with no new emit machinery (the motion key is the item id,
  so the card does not re-animate, only the track re-renders).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `cargo test` from `src-tauri/` | all pass |
| Frontend tests | `npx vitest run` | all pass |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Build | `npx vite build` | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/queue.rs` — batch counters, `expanded = true` at both
  promotion sites, auto-retract deadline + disarm flag, slot-state fields
- `src-tauri/src/event.rs` — `SlotState::Showing` gains
  `queue_total: u32, queue_done: u32`; snapshot test update
- `src-tauri/src/lib.rs` — heartbeat handles the retract wake (plan 015's
  `next_deadline` path); `toggle_manual_expand`'s High no-op guard is
  deleted (hotkey always flips under expand-all)
- `src/components/Track.tsx`, `src/components/StatusRailCard.tsx`,
  `src/styles.css`, `src/settings/preview-overlay.css` (mirror)
- `src/useSlotState.ts` validation; frontend tests; **every TS
  `SlotState` constructor/fixture/preview sample gains
  `queueTotal`/`queueDone`** (the compiler lists them all)
- `docs/TESTING_STRATEGY.md` §0 counts; `CONTEXT.md` Expanded entry gains
  the auto-retract sentence

**Out of scope**:
- Priority-load track (032's removal of the chip already landed — this
  plan repurposes what remains), idle status rail (034), cmux details (035)
- Any change to promotion *ordering* (priority → rotation order → FIFO
  is untouched)

## Git workflow

- Current branch; one commit for queue+wire, one for frontend. Do NOT push.

## Steps

### Step 1: Batch counters (rust)

Add `batch_total: usize, batch_done: usize` to `SingleSlotQueue`
(constructor zeroes). Increment `batch_total` at every accepted enqueue
site (`enqueue`, `enqueue_test`); increment `batch_done` at every
completion site (rotation-out in the tick, `dismiss_current`, skip).
Reset both when the engine is fully idle (check after every mutation:
`visible.is_none() && waiting.iter().all(|t| t.is_empty())`).

`SlotState::Showing` gains `queue_total: u32, queue_done: u32` where
`total = max(batch_total, 1)` and `done = min(batch_done, total - 1)`.
`Empty` stays tag-only. Update the event.rs snapshot test
(`queueTotal`/`queueDone` camelCase).

Tests: the four decision-4 semantics (start/increment/complete/reset),
plus supersession-not-a-completion.

### Step 2: Auto-expand-all + retract (window/render decoupling)

**The trap to avoid**: `rotate_out_if_elapsed` currently computes the
window from the render flag (`rotation_window(self.expanded)`). If the
retract simply flipped `expanded` to `false`, the window would shrink
from under the visible item and it would rotate out *at the retract
moment* — the "finishes its turn compact" half of the lifecycle would
never exist. So expansion splits into two flags:

- `expanded: bool` — **render state** (what the card looks like). Set
  `true` at every promotion; flipped `false` by the auto-retract and by
  manual toggles.
- `window_expanded: bool` — **rotation arithmetic** (how long the turn
  is). Set `false` at every promotion (auto-expand never extends); set
  `true` only by a manual expand, and sticky for the rest of the turn.
  `rotation_window(window_expanded)` replaces every current
  `rotation_window(expanded)` call (rotation-out, supersede top-up,
  `next_deadline`) — including the proptest harness's `vis_snapshot`
  window math in the same file.

Then: `queue.rs:224`'s setter becomes no-arg (`expanded=true,
window_expanded=false, auto_retract_armed=true` — drop the
`Priority::High` comparison at both promotion sites). `tick` gains
`retract_if_elapsed` *before* rotate/promote: when armed-and-expanded and
`now >= promoted_at + base_window / 2` (`Duration` math, not seconds
truncation), clear `expanded` + the arm. `next_deadline` returns the
earlier of that retract deadline and the rotation deadline; the plan-015
heartbeat already wakes there and `slot_state_if_changed` emits the
collapse. `toggle_expanded` becomes: any press disarms the retract;
collapse is render-only; expand sets `window_expanded=true` (manual
extension, sticky). `lib.rs:731-733`'s `toggle_manual_expand` High no-op
guard is deleted — with expand-all the hotkey must always flip (a press
on an auto-expanded card collapses it).

Tests: every priority auto-expands; retract fires at half the base window
and emits (slot-state `expanded: false`, item still visible); hotkey
press disarms; manual expand extends the window 3×; per-item reset still
holds; idle no-op toggle still holds. The plan-008 example cases are
*rewritten*, not deleted — specifically
`expanded_resets_when_next_item_promotes` (the next item now starts
**expanded**), `auto_expanded_high_uses_expanded_rotation_window`
(auto-expand no longer extends the window; manual-only), the
`next_deadline_*` cases (an armed retract is the earlier deadline), and
the proptest suite's invariant 8 (`expanded == (priority==High)` at
promotion, asserted at `queue.rs:1630-1631`, becomes
`expanded == true`) — plan 022's property harness lives in the same
file and reads the same flags.

**Invariant 9 must be rewritten too** (verified at review, `1add02e` —
the plan-time draft missed it): the harness asserts
`next_deadline == promoted_at + rotation_window(q.expanded) + extension`
at `queue.rs:1821-1826`, and `vis_snapshot` computes the same window at
`:1582`. Under this plan both are wrong twice over: the window flag
becomes `window_expanded`, and `next_deadline` becomes
`min(retract_deadline_if_armed, rotation_deadline)` — so an armed
retract makes the old equality fail. Rewrite invariant 9 to model the
min-of-two-deadlines rule (and retarget both harness sites to
`window_expanded`); do not weaken it to a `<=` — the exact deadline is
the property being pinned.

### Step 3: Track component + validation

`Track` takes `total`/`done` props: `n = clamp(total, 1, 10)`,
`current = total > 10 ? floor(done * 10 / total) : done`; segments
`< current` get `.done`, `== current` gets `.cur`. CSS (both files):
`grid-template-columns: repeat(var(--queue-n,3),1fr)` set inline,
`.track span { transition: background 260ms … }`,
`.done` = 30%-accent dim, `.cur` = accent + soft glow. `useSlotState`
validates two non-negative integers; tests for accept/reject.

### Step 4: Eyeball + docs

`npm run tauri dev`, `just push` 5 items: slider advances one segment per
rotation; every card enters expanded and retracts mid-turn. `CONTEXT.md`
Expanded entry + TESTING_STRATEGY §0.

## Test plan

- queue.rs: batch semantics (4+ cases), expand-all, retract, disarm,
  slot-state fields present
- event.rs: snapshot update
- frontend: Track segment math (≤10, >10 proportional), validator cases,
  StatusRailCard renders the new track without re-animating on
  waiting-count change
- §0 counts updated

## Done criteria

- [ ] `cargo test`, `npx vitest run`, `npx tsc --noEmit`, `npx vite build` green
- [ ] `queueTotal`/`queueDone` in the slot-state snapshot test
- [ ] plan 008's expanded-semantics tests pass in their expand-all form
- [ ] 5-push manual check: slider advances, expand→retract→rotate observed
- [ ] `git status --short` shows, beyond whatever was already dirty
      before your first edit (concurrent sessions share this checkout —
      snapshot it first, never revert/stage/commit those paths),
      modifications ONLY to in-scope files
- [ ] `plans/README.md` row updated

## STOP conditions

- The plan-015 heartbeat has drifted (no `next_deadline`-driven sleep) —
  the retract mechanism depends on it; report before improvising a second timer
- Batch counters would require touching promotion *ordering* — out of scope
- The >10 proportional mapping looks wrong on a real burst — tune the
  formula with the operator, don't ship a guess

## Maintenance notes

- Batch semantics deliberately say nothing about *why* items arrive —
  a 5-item cmux burst and a slow RSS trickle both read honestly.
- If a future evergreen/Recurring producer lands (plan 031 spike), its
  requeue-to-back is not a completion either — revisit the counter rule
  then, with the suite as the guard.
