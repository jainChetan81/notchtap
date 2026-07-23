# Plan 129: deep-review fix batch for the animation wave (125–127)

> Filed 2026-07-24 from the three-lens deep review of `c7b6f83..HEAD`
> (choreography vs motion source, CSS/tokens, tests/perf). Single
> executor, sequential — small fixes spanning overlay + settings +
> ledger. Authored at master `aeec9d7`. Every finding was verified by a
> reviewer AND re-verified by the coordinator.

## K — choreography defects (do first)

**K1. Clear must actually collapse rows.** `QueueSection.tsx:103-133`
and `HistorySection.tsx:298-315` render
`length === 0 ? <p>empty</p> : <ul><AnimatePresence>…</ul>` — pressing
Clear (or skipping the only item) flips to the empty branch and
synchronously unmounts the `<ul>` WITH its AnimatePresence, so no exit
ever plays on the wholesale removal (the exact parent-level-conditional
bug 127 Step 2 fixed for the peek). Fix in BOTH files: keep the
`<ul>` + `AnimatePresence` mounted when the array is empty; render the
empty-state `<p>` as a sibling gated on `length === 0` (the
`items === null` loading branch may stay a ternary — nothing exits
from it). Ensure the empty `<ul>` adds no visual gap (no
margin/padding when empty — check the classes). Tests: after a Clear,
the row nodes are STILL in the DOM on the click render (exiting), gone
after the exit window; empty-state text appears.

**K2. Live region must exist BEFORE its content.** The wave moved
`role="status"`/`aria-live` onto the below-block wrapper
(`StatusRailCard.tsx:661-662`), which MOUNTS at t=175ms in the same
commit as the title/body inside it — a live region inserted
pre-populated is the canonically unreliable ARIA pattern (old code
flipped the attribute at t=0 on the always-mounted root, content
arrived 175ms later into an established region; rotations only work
today because mode="wait" delays the child past the flip). Fix: a
STATIC `<div style={{ display: "contents" }}>` wrapping the
below-block's outer `AnimatePresence`, carrying
`role="status"`/`aria-live="polite"` gated on
`showing && !isLiveCard` (attribute flips at t=0 again). Remove the
attributes from the inner below-block wrapper. `display: contents`
keeps grid layout and the `:has(.below-block)` rounding law intact
(the element stays in the DOM tree; the class stays on the same node).
Move/extend the wave's doc comment accordingly.

## C — CSS fixes

**C1.** `.card-assembly.exiting`'s `transform var(--reveal-ms, 260ms)`
leg (`overlay-card.css:373-378`) is semantically wrong — the only
transform consumer is the hover breathe, which the wave moved to
`var(--hover-ms, 160ms)`; hover-out mid-exit currently runs 260ms.
Change to `var(--hover-ms, 160ms)` AND fix both comment passages: the
stale ":355-356 same transform duration as the base rule" sentence and
the plan-127 note — they currently contradict each other.

**C2.** `.status-dot`'s new transition (`:1435-1445`) softens
opacity/box-shadow/border-radius but the enabled↔disabled shape flip
ALSO changes `background` (`:1487-1490`) and `border` (`:1496-1504`) —
corners morph while fill/border snap, a two-phase glitch. Add
`background-color`, `border-color`, and `border-width` legs (same
`var(--hover-ms, 160ms) var(--ease-notchtap)`).

**C3.** The plan-124 INVARIANT comment block (`:402-412` and echoes at
`:414`, `:421`, `:447`) cites hard line numbers (`:132-134`,
`:135-159`, `:501-504`) that this wave's comment insertions shifted.
Replace every hard line reference in that block with rule/selector
names (the way the F4 pin test locates rules) — substance unchanged.

**C4.** `.track span` (`:1246`) runs `background var(--reveal-ms,
260ms) ease` — built-in `ease` while everything else on the reveal
tier runs `var(--ease-notchtap)`, and the new comment claims "same
reveal-tier motion". Change `ease` → `var(--ease-notchtap)`.

**C5.** `transform-origin: top center` lives only inside `.hovered`
(`:752`) and `.pulse-goal` (`:833`) — when `.hovered` clears, the
origin snaps back to center while the 160ms shrink is mid-flight,
pivoting the unhover around the wrong point. Move
`transform-origin: top center;` to the base `.card-assembly` rule
(`:71-118`, with a one-line comment) and DELETE the two now-redundant
declarations. Behavior-neutral elsewhere (origin only matters while
transform ≠ none; all whole-card transforms want the top anchor).

**C6.** `IdleFace.tsx:187` hand-types
`cubic-bezier(0.22, 1, 0.36, 1)` with a comment claiming interpolation
needed a new export — false: build it as
`` `transform 200ms cubic-bezier(${NOTCHTAP_EASE.join(", ")})` ``
(the array is already imported). Fix the comment. Update the test to
build its expectation from the import too
(`IdleFace.test.tsx:52`-area).

## T — test hardening

**T1.** Real live-region tripwires (against K2's NEW placement), in
StatusRailCard.test.tsx: idle →
`container.querySelector('[role="status"], [aria-live]')` is null;
showing (non-live-match) → `querySelectorAll('[role="status"]')` has
length 1 AND the clock/dots are outside it
(`clockEl.closest('[role="status"]') === null` or equivalent); the
attribute is present at t=0 of a promotion (before the 175ms content
mount — fake timers).

**T2.** Replace the queue-row-key stability test
(`SettingsApp.test.tsx:1370-1387`) with a discriminating one: second
`get_queue` mock returns the list MINUS THE FIRST row; grab the
surviving row's DOM node before refresh, assert the SAME node after
(old positional keys remount it — test fails pre-fix; verify that).

**T3.** Export `contentExitVariants` from StatusRailCard.tsx
(test-only export, `iconForBundleId` precedent) and pin:
`exit(true).transition.duration === ROTATION_EXIT_MS / 1000`,
`exit(false).transition.duration === CONTENT_EXIT_MS / 1000`, both
`ease === NOTCHTAP_EASE`.

**T4.** GENUINE §0 re-reconcile of docs/TESTING_STRATEGY.md's frontend
line — not another delta. Run `npx vitest run` and per-file counts;
fix: the duplicated "+8 plan 122 / +2 plan 123" clauses (line
currently sums to 364 vs real 354), the stale per-suite claims
(measure the real StatusRailCard file count incl. the convergence
describe; IdleHoverPeek real count; add IdleFace 4, button 1), and the
double celebrationStacking listing (6 and 4 — collapse to the real
current one). Keep the line's established narrative style; anchor with
"re-reconciled 2026-07-24 at <count>".

**T5.** IdleFace: a glance actually HAPPENS — after reveal,
`advanceTimersByTime(11001)`, assert the eyes' transform changed from
its initial value (first gaze step is deterministic).

**T6.** StatusDots CSS pins (new describe, reuse the `ruleBody`
helper pattern): `.status-dot`'s rule body contains
`border-radius var(--hover-ms` (and, post-C2, `background-color`);
`@keyframes pause-glyph-fade-in` exists and `.pause-glyph`'s body
references that exact name.

**T7.** The same-id no-remount test (`StatusRailCard.test.tsx:
1109-1117`): add `expect(node.getAttribute("data-rotation-swap"))
.toBe("false")`.

**T8.** One-line badge pin next to the button test: badge's class
string contains `transition-colors` and not `transition-all`.

## Verification

`npx tsc --noEmit`, `npx vitest run`, `npx biome ci .`, `npx vite
build` — clean. No rust. Every pre-existing test green unmodified
except the explicitly-sanctioned T1/T2 replacements. The plan-124
convergence pins stay green. STOP if K2's `display: contents` wrapper
breaks any grid/`:has` behavior a test catches — report, don't
restructure the grid.
