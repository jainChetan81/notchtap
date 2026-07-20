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
> **Drift check (run first)**: `git diff --stat 71e54a7..HEAD -- src-tauri/src/event.rs src-tauri/src/queue.rs src/useSlotState.ts src/components/StatusRailCard.tsx src/styles.css src/settings/preview-overlay.css prototype/notch-states.html`
> Any diff in the source files means line refs below have shifted —
> re-read before editing. Any diff in the prototype is a STOP condition
> (see below). Plan 080 lands before this one — expect
> `StatusRailCard.tsx`/`styles.css` to differ from 71e54a7 by exactly
> 080's diff; anything MORE is drift to reconcile.

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
- **Planned at**: commit `71e54a7`, 2026-07-20

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
- `src/components/StatusRailCard.tsx` — the card's bottom edge today is
  `.compact`'s `Track` (queue slider) — a DIFFERENT element with a
  different meaning (batch position, not time). The TTL bar is
  additional, per the prototype (`.ttl-bar` sits after the content
  block, full card width, 2px).
- Prototype reference (`prototype/notch-states.html`): CSS at lines
  152-153 (`.ttl-bar` 2px track + `.ttl-fill` colored fill, no CSS
  transition — JS-driven), markup at line 392, and the pausable-timer
  JS at lines 480-563 (bar width = remaining/rotate-window per rAF
  tick, frozen while paused, reset to 100% on rotate-out). The fill
  color follows the card's accent — use `var(--accent)` (the
  priority/category accent the card already sets), not the prototype's
  hardcoded demo color.

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

**Verify**: `cd src-tauri && cargo test --locked` → all pass (existing
SlotState serialization tests will fail to compile until Step 2 updates
their literals — that's expected; do Steps 1+2 before running).

### Step 2: Update rust-side SlotState literals/tests

Fix every construction of `SlotState::Showing` in rust tests (search
`queue.rs`, `event.rs`, `engine.rs`, `lib.rs` test modules) for the two
new fields. Add one new rust test: a visible item 2s into an 8s window
emits `ttl_ms == 8000` and `remaining_ms` in a narrow band around 6000
(use the queue's existing clock-injection/test-time patterns — check
how current tests construct `Instant` math rather than inventing wall-
clock sleeps).

**Verify**: `cd src-tauri && cargo test --locked` → all pass;
`cargo clippy --locked --all-targets -- -D warnings` → exit 0;
`cargo fmt --check` → exit 0.

### Step 3: Frontend — type, validator, TtlBar component

`src/useSlotState.ts`: add `ttlMs: number; remainingMs: number` to the
showing variant and validate both as non-negative numbers in
`isValidSlotState` (same discipline as `queueTotal`/`queueDone` —
reject fractional/negative, `useSlotState.ts:125-127`). New component
(prefer `src/components/TtlBar.tsx`): props `{ slotId, ttlMs,
remainingMs }`; on mount/prop change, anchor `deadline = performance.now()
+ remainingMs`, then a `requestAnimationFrame` loop setting
`fill.style.width` via a ref (NO React state per frame — the prototype
mutates the DOM node directly, follow that); cancel the rAF on
unmount and on `slotId` change, and re-anchor whenever `remainingMs`
changes (a supersede/extension re-emit resets the bar honestly). Width
= `max(0, deadline - performance.now()) / ttlMs * 100%`. Under
`prefers-reduced-motion` (media query in JS or a CSS rule that hides
the animated fill and shows a static one — pick the CSS-only variant if
it covers it): the bar must not tick per frame; a static full-width
fill is the reduced-motion presentation. Mount it in
`StatusRailCard.tsx` at the bottom of the showing branch, after the
`Manifest` element, keyed by `renderedSlot.id` so promotion resets it
(the same re-key discipline as `card-content`'s `swapKey`).

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

- **Rust (cargo)**: the Step 2 timing test; a supersede case (extension
  path, `queue.rs:532-533`) re-emits with `ttl_ms` including
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
- [ ] `useSlotState.ts` type + validator accept/reject correctly (vitest)
- [ ] Bar renders on every showing card, resets per promotion, re-anchors on re-emit, no per-frame React state
- [ ] `prefers-reduced-motion` presentation implemented
- [ ] `.ttl-bar`/`.ttl-fill` in both `src/styles.css` and `src/settings/preview-overlay.css`, same commit
- [ ] Rotate-out timing unchanged (`cargo test --locked` queue suite passes unmodified except SlotState literals)
- [ ] Hover-pause half: shipped, or explicitly deferred per Step 5's gate — recorded either way
- [ ] `plans/079-checklist.html` and `plans/frontend-ui-consolidated.html` statuses updated; `plans/README.md` row for 081 updated

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
- 086's outcome is ambiguous but you find yourself writing hover-pause
  code anyway — that's the gated half leaking into the ungated one;
  stop.

## Maintenance notes

- Update `plans/079-checklist.html` and
  `plans/frontend-ui-consolidated.html` (the "TTL progress bar +
  hover-pause" locked-decision entry → shipped/partially-shipped
  depending on the Step 5 branch).
- The `remaining_ms`-at-emit + local-anchor pattern is the repo's
  answer for any future time-on-the-wire need (e.g. football
  event-dwell in 084) — point future plans at this one rather than
  re-deriving it.
- If 086 later unblocks hover, the lifecycle-pause follow-on plan
  should be written against `queue.rs`'s deadline engine with the same
  care plan 037's history documents — it's the most-reviewed module in
  the repo.
