# Plan 081: TTL progress bar on every rotating card — wire timing fields + rAF bar

> **Executor instructions**: This is a build plan with one explicitly
> gated half. The UNGATED half: a thin 2px progress bar at the bottom of
> every rotating card showing time until rotate-out, driven by real
> timing data added to the slot-state payload, resetting per promotion,
> reduced-motion safe. The GATED half: hover pausing the bar AND the
> card lifecycle — that half ships ONLY if plan 086 (hover/cursor
> tracking spike) has concluded hover is feasible; if 086 is unresolved
> or says no, the bar still renders and counts down, unpaused, exactly
> as the locked decision's non-hover half describes. Do not block the
> ungated half on 086. Follow the steps in order. The
> preview-overlay.css mirror law applies: CSS changes land in
> `src/settings/preview-overlay.css` in the same commit. When done,
> update the status row for this plan in `plans/README.md` — unless a
> reviewer dispatched you and told you they maintain the index.
>
> **Drift check (run first)**: `git diff --stat 3de785a..HEAD -- src-tauri/src/event.rs src-tauri/src/queue.rs src/useSlotState.ts src/components/StatusRailCard.tsx src/styles.css src/settings/preview-overlay.css prototype/notch-states.html`
> Any diff in the source files means line refs below have shifted —
> re-read before editing. Any diff in the prototype is a STOP condition
> (see below). Baseline `3de785a` already INCLUDES plan 080 (merged
> 2026-07-21 as `d21d689`) — the StatusRailCard.tsx DOM refs below are
> post-080. (Baseline history: `9a954b0` → `4fb3af9` for 063's merge —
> styles.css priority-accent rules at :70-83, lib.rs full literal at
> :1057 — then → `3de785a` for 080's merge. All refreshed and
> re-verified by direct read.)

## Status

- **Priority**: P2
- **Effort**: M (wire addition across the rust↔TS seam + a rAF-driven
  component; the pausable-lifecycle half, if ungated, adds real
  queue-side work — re-estimate then)
- **Risk**: MED — touches the slot-state wire shape (both validators,
  the rust emitter, and the eval-seed path) and adds a per-frame render
  loop to an always-on overlay; idle-CPU discipline (plans 015/018)
  applies
- **Depends on**: 080 (news card — the bar mounts at the bottom of the
  card 080 restyles; landing them in one step series avoids CSS
  conflicts). Hover half gated on 086's outcome.
- **Category**: direction (locked 2026-07-20 — "TTL progress bar +
  hover-pause", `plans/frontend-ui-consolidated.html` Locked decisions)
  → build
- **Planned at**: commit `9a954b0`, 2026-07-20 (reviewed same date:
  drift baseline corrected, bar placement/fill-color/reduced-motion
  pinned, 079-checklist done-criterion fixed). **Review-plan pass 2
  (2026-07-21, against `4fb3af9`)**: rust/frontend/prototype citations
  re-verified exact (`queue.rs` :10/:454-466/:472-480/:482-515/:532-533,
  `event.rs` :177-207/:217/:230-235/:320/:376, `useSlotState.ts`
  :25-49/:77-127, notch-states.html :152-153/:392/:394, the `..`-rest
  claim for the queue/engine/http/poller test modules); three stale
  refs fixed — the lib.rs full literal (`:1016` → `:1057`, shifted by
  063's +41-line block), the priority-accent rules (`styles.css:61-74`
  → `:70-83`), and the supersede constant name (the MIN constant is
  `MIN_REMAINING_ON_SUPERSEDE_SECS`, not `MIN_EXTENSION_…`). Drift
  baseline re-stamped to `4fb3af9`, then again to `3de785a` when plan
  080 merged mid-review (StatusRailCard.tsx DOM refs refreshed to
  post-080: Track :162, Manifest :164-174, compact-hint :157-161 — the
  original text's "then the compact-hint" ordering misdescription also
  fixed: the hint lives INSIDE `.compact`, before `Track`).

## ⚠️ Execution attempt 1 STOPPED — read this before Step 1

**2026-07-21, executor stopped correctly at its own STOP condition; the
plan was wrong, not the executor.** Everything below is the refinement.

**What it found** (verified empirically, then fully reverted — the
worktree was left clean at `725040b` with zero commits): implementing
Step 1 as originally written **doubles `slot-state` emission volume for
every mutation system-wide** — every accepted notification, poller
event, manual expand, everything.

**The mechanism**, confirmed by the reviewer against live code:

1. `slot_state_if_changed` (`queue.rs:472-480`) dedups by comparing the
   WHOLE `SlotState` with derived `PartialEq` (`event.rs:178`):
   `if self.last_emitted.as_ref() == Some(&current) { None } else { … }`.
2. The rotation loop (`engine.rs::spawn_rotation`, the `loop` at
   `engine.rs:279-311`) is woken by `notify_waiters()` after **every**
   mutation, then independently re-locks the queue and calls
   `q.tick(Instant::now())` + `q.slot_state_if_changed()` — a second
   time, downstream of the mutation site's own emit. This is the
   shipped plan-036/037 mutate→wake→emit protocol, not a bug.
3. Today that second call always finds the state byte-identical and
   returns `None`. **Once `remaining_ms` is computed from
   `Instant::now()` inside `current_slot_state()`, the two calls happen
   milliseconds apart, so the value always differs, so the comparison
   never matches, so it always emits again.**

The executor proved it rather than reasoning about it: a temporary test
spawning the real rotation loop, calling one `engine.accept(...)`, and
counting `SLOT_STATE_EVENT` emissions returned **2 emissions for 1
accept**, reproducibly.

The plan's original STOP condition was worded for a *periodic* caller.
This caller is not periodic — it is structural and fires on every
mutation, which is worse. The wording is corrected below.

### The prescribed fix (reviewer decision, 2026-07-21)

**Split the two fields' dedup treatment: `remaining_ms` is EXCLUDED
from the dedup comparison; `ttl_ms` STAYS IN it.**

This works because the two fields have fundamentally different natures:

- **`ttl_ms` is time-free.** It derives only from stored fields —
  `rotation_window(window_expanded)` (a pure function of the rotation
  spec and a bool, `event.rs:23-33`) plus `extension_secs`. Those change
  only at discrete lifecycle moments: promotion (`queue.rs:271` resets
  `extension_secs`), supersede top-up (`queue.rs:348` grows it), and
  expand (`queue.rs:426`). Between the mutation-site emit and the
  rotation loop's recheck, with nothing genuinely changed, `ttl_ms` is
  *identical* — so it is safe in the comparison and keeps deduping.
- **`remaining_ms` is a pure function of `now`.** It can never dedupe.

And the split is not merely a workaround — it is **exactly the right
semantics**. `ttl_ms` changes precisely at the discrete events where the
bar must re-anchor (extension, supersede, manual expand, new promotion).
So the dedup gate still fires on every one of those, emitting a fresh
`remaining_ms` with it. Between those events the frontend counts down
locally via rAF, which is what Step 3 specifies anyway ("the frontend
anchors its countdown at receipt"). Nothing is lost.

**Do NOT implement this by hand-writing `PartialEq` for `SlotState`.**
Tests use `assert_eq!` on `SlotState`, and silently ignoring a wire
field in equality would make those assertions lie. Instead add an
explicit, documented dedup comparison — e.g. a
`fn dedup_eq(&self, other: &SlotState) -> bool` on `SlotState`, or
compare clones with `remaining_ms` normalized to a fixed value — and
have `slot_state_if_changed` use *that* while `PartialEq` stays derived
and honest. Document the asymmetry at both definition sites.

## Why this matters

Locked 2026-07-20: every rotating card carries a thin progress bar
showing time until rotate-out; hovering pauses the bar and the
lifecycle. The bar is mocked in both lifecycle demos
(`prototype/notch-states.html` §4 and `prototype/news-card.html` §3,
`.ttl-bar`/`.ttl-fill` tracking a pausable rotate timer). Today the
frontend can't build it honestly: the slot-state payload carries no
timing at all, so any bar would be a fake animation detached from the
real rotation deadline — visibly wrong exactly when it matters (manual
⌃⇧N extension, Topic supersede extension, Recurring vs OneShot
windows). This plan's real work is the wire addition; the bar itself
is small once honest numbers flow.

## Current state

- `src-tauri/src/queue.rs:10` — the visible item already records
  `promoted_at: Option<Instant>`; `queue.rs:454-466` (`next_deadline`)
  already computes the real rotation deadline as `promoted_at +
  rotation_window(window_expanded) + extension_secs` — all the timing
  truth exists rust-side, using the same `Instant` math the rotation
  engine itself uses.
- `src-tauri/src/event.rs:177-207` — `SlotState::Showing` (the wire
  payload): id/title/body/…/queue_total/queue_done, NO timing fields.
  Emitted via `emit_slot_state` (`event.rs:217`) from
  `Queue::current_slot_state` (`queue.rs:482-515`) and re-emitted only
  when the payload changes (`slot_state_if_changed`, `queue.rs:472-480`).
- `src/useSlotState.ts:25-49` (TS `SlotState` type) and
  `useSlotState.ts:77-127` (`isValidSlotState` — every field checked,
  unknown/missing fields fall back to empty). Both must grow in
  lockstep with the rust enum; `event.rs:230-235`'s pinned-seam comment
  names this file.
- `src/components/StatusRailCard.tsx` — the showing card's DOM order
  today is: `.compact` content (the `compact-hint` at :157-161 inside
  it, ending in the `Track` queue slider at :162), then `Manifest`
  (:164-174 — mounted but zero-height when collapsed). The `Track` is a
  DIFFERENT
  element with a different meaning (batch position, not time). The TTL
  bar is additional, per the prototype (`.ttl-bar` sits immediately
  after the compact content block, BEFORE the manifest wrapper —
  notch-states.html:392 vs `.manifest-wrap` at :394 — full card width,
  2px). Mount it in exactly that position: between the compact block
  and `Manifest`.
- Prototype reference (`prototype/notch-states.html`): CSS at lines
  152-153 (`.ttl-bar` 2px track + `.ttl-fill` colored fill, no CSS
  transition — JS-driven), markup at line 392, and the pausable-timer
  JS at lines 480-563 (bar width = remaining/rotate-window per rAF
  tick, frozen while paused, reset to 100% on rotate-out). **Fill
  color decision (reviewer-pinned, deliberate)**: use `var(--accent)`
  on every card — the priority accent the card shell already sets
  (styles.css:70-83). This deviates from news-card.html:145, which
  uses `var(--cat)` on the news demo: the bar is card chrome (like the
  priority border), not news content, and one rule for all card types
  beats a per-branch exception. The prototypes' hardcoded/`--cat` demo
  colors are both superseded by this line.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `cd src-tauri && cargo test --locked` | all pass |
| Clippy | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Rust format | `cd src-tauri && cargo fmt --check` | exit 0 |
| Frontend unit tests | `npx vitest run` | all pass |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Lint + format gate | `npx biome ci .` | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/event.rs` — two new fields on `SlotState::Showing`
  (see Step 1 for the chosen shape), serialized camelCase like the rest.
- `src-tauri/src/queue.rs` — compute those fields in
  `current_slot_state` from the same `promoted_at`/`rotation_window`/
  `extension_secs` math `next_deadline` uses.
- `src/useSlotState.ts` — TS type + validator for the new fields.
- `src/components/StatusRailCard.tsx` (+ possibly a small new
  `src/components/TtlBar.tsx`) — the rAF-driven bar.
- `src/styles.css` + `src/settings/preview-overlay.css` — `.ttl-bar`/
  `.ttl-fill`, mirrored same commit.
- Tests per the Test plan below.
- **Gated (only if 086 concluded hover is feasible AND its mechanism
  exists to build against)**: pause-on-hover for bar + lifecycle. If
  ungated, this becomes its own follow-on steps — re-plan the
  lifecycle-pause half then (it touches `queue.rs`'s deadline engine,
  which is exactly the careful-review module; do not squeeze it in
  here).

**Out of scope**:
- Changing the rotation engine's deadlines themselves — this plan READS
  the same math, it doesn't alter rotate-out/retract behavior by a
  millisecond.
- The idle rail (no card visible = no bar; the bar exists only in
  `state: "showing"`).
- The queue-slider `Track` (batch position) — unchanged, both elements
  coexist as in the prototype.
- Hover expand/collapse of the card (086 territory), the weather peek
  (082 preps it), the football card's no-track variant (084).

## Steps

### Step 1: Wire shape — add timing to `SlotState::Showing`

Add two fields to `SlotState::Showing` in `event.rs` (camelCase on the
wire via the existing `rename_all_fields`):

```rust
/// Total rotation window for this showing, milliseconds — includes
/// `extension_secs` and resolves OneShot ttl vs Recurring display_secs
/// exactly as `rotation_window(self.window_expanded)` does.
ttl_ms: u64,
/// Milliseconds remaining until rotate-out AT EMISSION TIME, computed
/// from the same `promoted_at` Instant math as `next_deadline`
/// (saturating at 0). The frontend anchors its countdown at receipt —
/// `Instant` isn't wall-clock and can't cross the wire, so remaining-
/// at-emit + local elapsed is the honest shape.
remaining_ms: u64,
```

Compute both in `queue.rs::current_slot_state` where the visible item
and `promoted_at` are in scope. Use `Instant` durations (as `queue.rs`
already does), `as_millis() as u64` with a `try_from`/`u64::MAX`
defensive cap matching the file's existing `queue_total` style. If
`promoted_at` is somehow `None` on a visible item (defensive — every
real path sets it), emit `remaining_ms = ttl_ms` (full bar), never
panic. Note in the completion report: supersede/extension/expansion
changes all re-emit slot-state already, so the countdown re-anchors
honestly at each — no new emission sites needed.

**Then implement the dedup split** described in the "Execution attempt 1
STOPPED" section above — `remaining_ms` excluded from
`slot_state_if_changed`'s comparison, `ttl_ms` kept in it, `PartialEq`
left derived and honest. This is NOT optional and NOT a follow-up: without
it this plan doubles system-wide emission volume, which is why attempt 1
stopped here.

**Verify**: `cd src-tauri && cargo test --locked` → all pass (existing
SlotState serialization tests will fail to compile until Step 2 updates
their literals — that's expected; do Steps 1+2 before running).

### Step 2: Update rust-side SlotState literals/tests

Fix every construction of `SlotState::Showing` in rust tests for the
two new fields. The full literals that WILL fail to compile live at
`event.rs:320`, `event.rs:376`, and `lib.rs:1057`; the
`queue.rs`/`engine.rs`/`http.rs`/`poller.rs` test modules use `..`
rest patterns and compile unchanged — do not "fix" files the compiler
doesn't name. Add one new rust test: a visible item 2s into an 8s window
emits `ttl_ms == 8000` and `remaining_ms` in a narrow band around 6000.
There is no clock-injection harness in this repo — in-module tests use
`Instant::now()` arithmetic directly and can set
`visible.promoted_at = Some(Instant::now() - Duration::from_secs(2))`
before calling `current_slot_state` (see how `queue.rs`'s existing
tests construct visible items, e.g. around `queue.rs:1857`).

**Also add the single-emit regression test — REQUIRED, this is the
tripwire for attempt 1's failure.** Attempt 1 wrote it as a throwaway
experiment; it becomes permanent. Shape: spawn the real rotation loop,
make ONE `engine.accept(...)` call for a long-ttl (e.g. 30s) item, wait
~300ms, and count `SLOT_STATE_EVENT` emissions via
`app.handle().listen(...)`. **Assert exactly 1.** Before the dedup split
this asserts 2; after it, 1. Name it so its purpose is obvious (e.g.
`one_accept_emits_exactly_one_slot_state_despite_live_remaining_ms`) and
comment it with a pointer to this plan's "Execution attempt 1 STOPPED"
section, so nobody later "simplifies" the dedup split and silently
reintroduces double emission.

**Verify**: `cd src-tauri && cargo test --locked` → all pass;
`cargo clippy --locked --all-targets -- -D warnings` → exit 0;
`cargo fmt --check` → exit 0.

### Step 3: Frontend — type, validator, TtlBar component

`src/useSlotState.ts`: add `ttlMs: number; remainingMs: number` to the
showing variant and validate both as non-negative numbers in
`isValidSlotState` (same discipline as `queueTotal`/`queueDone` —
reject fractional/negative, `useSlotState.ts:125-127`). New component
(prefer `src/components/TtlBar.tsx`): props `{ slotId, ttlMs,
remainingMs }`; on mount and whenever `remainingMs` changes, anchor
`deadline = performance.now() + remainingMs` (a supersede/extension
re-emit resets the bar honestly), then a `requestAnimationFrame` loop
setting `fill.style.width` via a ref (NO React state per frame — the
prototype mutates the DOM node directly, follow that); cancel the rAF
on unmount and on `slotId` change. Width
= `max(0, deadline - performance.now()) / ttlMs * 100%`. Under
`prefers-reduced-motion` the bar must NOT tick per frame — a CSS-only
rule cannot stop the rAF loop, so gate the loop itself in JS:
`matchMedia("(prefers-reduced-motion: reduce)")` checked when arming
the rAF; reduced-motion renders a static full-width fill (CSS
presentation) and skips the loop entirely (idle-CPU discipline, plans
015/018). Mount it in
`StatusRailCard.tsx` BETWEEN the compact content block and the
`Manifest` element (the prototype's exact position —
notch-states.html:392 before `.manifest-wrap` at :394), keyed by
`renderedSlot.id` so promotion resets it (belt-and-braces on top of
`card-content`'s `swapKey`, which already re-keys on the same id).

**Verify**: `npx vitest run` → all pass; `npx tsc --noEmit` → exit 0.

### Step 4: styles.css + preview-overlay.css mirror (same commit)

Add `.ttl-bar` (2px, full card width, `rgba(255,255,255,0.08)` track)
and `.ttl-fill` (100%→0 width, `var(--accent)` fill) per prototype
lines 152-153, plus the reduced-motion rule. Mirror both in
`src/settings/preview-overlay.css` under `.appearance-preview` in the
same commit.

**Verify**: `npx vite build` → exit 0; mirror grep —
`grep -c 'ttl-bar\|ttl-fill' src/settings/preview-overlay.css` ≥ 2.

### Step 5 (GATED — only if plan 086 unblocked hover): hover-pause

If 086 concluded hover is feasible and its mechanism exists: wire
hover-in/out to (a) freeze the bar's anchor and (b) pause the card's
lifecycle deadlines. Part (b) is queue-engine work (deadline
suspension, not just UI) — STOP and write it up as its own plan rather
than expanding this one, unless 086's conclusion explicitly scoped it
here. If 086 is unresolved or negative: skip this step entirely; the
bar ships unpaused. Record which branch applied in the completion
report.

**Verify**: only if executed — defined by the follow-on plan, not here.

### Step 6: Full gate

**Verify**: every command in the Commands table exits 0.

## Test plan

- **Rust (cargo)**: the Step 2 timing test; a supersede case (the
  extension governed by the `MIN_REMAINING_ON_SUPERSEDE_SECS`/
  `MAX_EXTENSION_ON_SUPERSEDE_SECS`
  constants, `queue.rs:532-533`) re-emits with `ttl_ms` including
  `extension_secs`; existing pinned-serialization tests updated, none
  deleted.
- **Frontend (vitest)**: bar renders only when `state === "showing"`;
  resets (re-anchors) when `slotId` changes; a same-id re-emit with new
  `remainingMs` re-anchors without remounting the card (the plan-078
  no-remount regression test must keep passing unchanged); validator
  rejects a payload missing/with-fractional `ttlMs`. rAF in jsdom:
  assert the fill element exists and the anchor math via injected fake
  timers/mocked rAF — do NOT assert pixel-perfect animation.
- **Manual-only** (operator, TESTING_STRATEGY §5): bar visibly drains
  and matches actual rotate-out timing on the real overlay, including
  after a ⌃⇧N manual-expand extension; reduced-motion presentation.

## Done criteria

- [ ] `SlotState::Showing` carries `ttlMs`/`remainingMs`; rust tests pass with the new fields (`cargo test --locked`)
- [ ] **The dedup split is implemented**: `remaining_ms` excluded from `slot_state_if_changed`'s comparison, `ttl_ms` kept in it, `PartialEq` still derived (not hand-written to lie)
- [ ] **The single-emit regression test exists and passes**: one `engine.accept()` → exactly 1 `SLOT_STATE_EVENT`, with a comment pointing at attempt 1's finding
- [ ] `useSlotState.ts` type + validator accept/reject correctly (vitest)
- [ ] Bar renders on every showing card, resets per promotion, re-anchors on re-emit, no per-frame React state
- [ ] `prefers-reduced-motion` presentation implemented
- [ ] `.ttl-bar`/`.ttl-fill` in both `src/styles.css` and `src/settings/preview-overlay.css`, same commit
- [ ] Rotate-out timing unchanged (`cargo test --locked` queue suite passes unmodified except SlotState literals)
- [ ] Hover-pause half: shipped, or explicitly deferred per Step 5's gate — recorded either way
- [ ] `plans/frontend-ui-consolidated.html` "TTL progress bar + hover-pause" entry updated (shipped/partially-shipped per the Step 5 branch); `plans/079-checklist.html` gains a TTL-bar entry (none exists today — add one row naming 081 rather than hunting for it); `plans/README.md` row for 081 updated

## STOP conditions

- **Prototype drift**: the drift-check diff on
  `prototype/notch-states.html` (or `prototype/news-card.html`) is
  non-empty, or the prototypes' TTL mechanics no longer match this
  plan's description — stop and confirm what was approved.
- **Mirror-law risk**: a `.ttl-*` rule can't be scoped into
  `preview-overlay.css` — stop; do not land unmirrored CSS.
- The honest-wire approach turns out wrong — e.g. slot-state does NOT
  re-emit on some lifecycle event that visibly changes the deadline
  (find one: extension, supersede, manual expand) — stop and surface
  the emission gap rather than faking the countdown.
- **[KNOWN AND RESOLVED — do not re-stop here]** A non-mutation caller
  of `slot_state_if_changed` whose re-emit would never dedupe once
  `remaining_ms` is live. Attempt 1 found exactly this (the rotation
  loop's post-wake recheck, `engine.rs:279-311` — structural, not
  periodic) and the reviewer prescribed the dedup split now written into
  Step 1. **Implement the split and the Step 2 single-emit regression
  test; do NOT stop again for this.** Stop only if the split fails to
  restore deduping — i.e. the regression test still sees >1 emission per
  `accept()` — which would mean a *second*, different non-deduping
  caller exists. Report what you measured if so.
- 086's outcome is ambiguous but you find yourself writing hover-pause
  code anyway — that's the gated half leaking into the ungated one;
  stop.

## Maintenance notes

- Update `plans/frontend-ui-consolidated.html` (the "TTL progress bar +
  hover-pause" locked-decision entry → shipped/partially-shipped
  depending on the Step 5 branch) and add the TTL-bar row to
  `plans/079-checklist.html` (no such entry exists at filing time).
- The `remaining_ms`-at-emit + local-anchor pattern is the repo's
  answer for any future time-on-the-wire need (e.g. football
  event-dwell in 084) — point future plans at this one rather than
  re-deriving it.
- If 086 later unblocks hover, the lifecycle-pause follow-on plan
  should be written against `queue.rs`'s deadline engine with the same
  care plan 037's history documents — it's the most-reviewed module in
  the repo.
