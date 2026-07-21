# Plan 100: TTL bar becomes the card's bottom edge; celebration animations run at 2× duration

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to
> the next step. If anything in "STOP conditions" occurs, stop and report
> — do not improvise. The reviewer maintains `plans/README.md` — do not
> edit it.
>
> **Worktree preflight (run first)**: agent worktrees can branch from a
> stale HEAD. Run `git log --oneline master ^HEAD`; if it prints
> anything, run `git merge --ff-only master` and confirm success. Then
> `npm ci` (fresh worktrees have no node_modules).
>
> **Drift check (run second)**: `git diff --stat 39f0fb1..HEAD -- src/styles.css src/settings/preview-overlay.css src/components/StatusRailCard.tsx src/components/TtlBar.tsx`
> On any change, compare the "Current state" excerpts against live code;
> on a content mismatch, STOP.

## Status

- **Priority**: P2
- **Effort**: S–M
- **Risk**: LOW (visual-only; the one moving part is DOM order in StatusRailCard)
- **Depends on**: none
- **Category**: bug (double-border) + polish (pacing)
- **Planned at**: commit `39f0fb1`, 2026-07-21

## Why this matters

First real-use feedback (operator, 2026-07-21): (1) the TTL bar renders
mid-card, directly above the manifest wrap's `border-top`, producing a
double-line artifact; the operator wants the TTL bar to BE the card's
bottom edge — the floor of the card visibly draining is also the better
metaphor. (2) The goal/red-card celebration animations read too fast at
real size; the operator asked for exactly 2× on each.

## Current state

- `src/styles.css`:
  - `:704-714` — the bar (mounted "between the compact content and the
    manifest wrap"):
    ```css
    .ttl-bar {
      height: 2px;
      background: rgba(255, 255, 255, 0.08);
    }
    .ttl-fill {
      height: 100%;
      width: 100%;
      background: var(--accent);
    }
    ```
    `.ttl-fill` width is rAF-driven from `TtlBar.tsx`; no CSS transition
    (deliberate — keep it that way).
  - `:763` — `border-top: 1px solid rgba(255, 255, 255, 0.1);` on the
    manifest wrap: the second line of the double-border. It stays — it
    is a legitimate compact/manifest separator once the bar moves away.
  - `.below-block` (`:157-176`) is `position: relative` with
    `overflow: hidden` and bottom-corner radii — an absolutely
    positioned child at `bottom: 0` clips to the rounded corners for
    free.
  - Celebration/pulse blocks (durations to double — the exhaustive list):
    - `:293` region — `animation: goal-ring 620ms ease-out;` plus the
      `goal-overshoot` / `goal-burst` animations in the same plan-023
      confetti block (`:290-344`): grep that block for every `ms`
      duration.
    - `:365` — `.cele-ripple span { animation: ripple-out 720ms ... }`
      with `animation-delay: 140ms` / `280ms` on children 2/3.
    - the `pulse-red` block (after `:394`) — `red-alert` animation.
    - `:1580-1642` — `cele-goal` (`goal-glow 900ms` + `cele-ring
      900ms`), `cele-yc` (`cele-ring 700ms`), `cele-rc`
      (`red-strobe 900ms`).
- `src/settings/preview-overlay.css` — mirror file: `.appearance-preview
  .ttl-bar`/`.ttl-fill` at `:486-499`; celebration mirrors at
  `:220-260` (`cele-ripple`) and `:1226+` (`cele-goal` etc.). Every
  change lands in both files (MIRROR LAW, same commit).
- `src/components/StatusRailCard.tsx`:
  - `:514` — `<TtlBar ...>` mounted between the compact content and the
    manifest wrap inside `.below-block`.
  - `:49-52` `CELEBRATION_END_ANIMATION` and `:84-89`
    `PULSE_END_ANIMATION` — celebration/pulse classes are removed on
    `animationend` matched BY ANIMATION NAME, not by timers. Doubling
    CSS durations therefore requires NO TS changes. (Verify: grep the
    file for `setTimeout` near pulse/celebration — there are none.)
- `src/components/TtlBar.tsx` — `:110-111` renders
  `<div className="ttl-bar"><div className="ttl-fill" ref={fillRef}/></div>`.
  No change needed to this component.

## Commands you will need

| Purpose | Command (repo root) | Expected |
|---|---|---|
| Frontend tests | `npx vitest run` | all pass |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Lint (CI gate) | `npx biome ci .` | exit 0 |
| Build | `npx vite build` | success |

## Scope

**In scope**:
- `src/styles.css`, `src/settings/preview-overlay.css`
- `src/components/StatusRailCard.tsx` (TtlBar mount position only)
- `src/components/StatusRailCard.test.tsx` / `src/components/TtlBar.test.tsx`
  — ONLY if an existing assertion pins the bar's old DOM position/order;
  update that structural assertion, change nothing behavioral.

**Out of scope**:
- `TtlBar.tsx` internals, the rAF loop, `hoverPaused` handling.
- Every non-celebration animation (idle-peek open/close, `pill-enter`,
  width/radius transitions) — durations unchanged.
- The manifest wrap's `border-top` at `:763` — keep it.
- `docs/TESTING_STRATEGY.md` §0 — only touch if you add/remove a test,
  and then update the exact counts.

## Git workflow

- Branch: your dispatched worktree branch. Conventional commits, e.g.
  `fix(overlay): ttl bar as the card bottom edge + 2x celebration pacing (plan 100)`.
  Do NOT push.

## Steps

### Step 1: Move the TTL bar to the bottom edge

In `StatusRailCard.tsx`, move the `<TtlBar .../>` element (currently
`:514`, between compact content and manifest wrap) to be the LAST child
inside the `.below-block` element (after the manifest wrap, after any
shade/effects siblings — last in DOM order within `.below-block`). Add a
one-line comment citing plan 100 (bar = the card's floor).

In `src/styles.css`, change `.ttl-bar` to:
```css
.ttl-bar {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  height: 2px;
  background: rgba(255, 255, 255, 0.08);
}
```
(keep the existing comment block, extend it: plan 100 moved the bar to
the card's bottom edge — `.below-block`'s `overflow: hidden` clips it to
the rounded corners; it must never regain static-flow position or it
recreates the double-border above the manifest wrap.)

Mirror the same `.ttl-bar` change in `preview-overlay.css`.

**Verify**: `npx vitest run` → all pass (update a structural assertion
only if one pins the old order); `grep -A3 "^\.ttl-bar" src/styles.css | grep -c "position: absolute"` → 1.

### Step 2: Double every celebration/pulse duration

In BOTH CSS files, multiply by exactly 2 every `animation` duration and
`animation-delay` inside these blocks ONLY: the plan-023 goal
confetti/ring block (`goal-ring`, `goal-overshoot`, `goal-burst`), the
`.cele-ripple` block (`ripple-out` 720→1440ms; delays 140→280ms,
280→560ms), the `pulse-red`/`red-alert` block, and the plan-084 block
(`goal-glow`+`cele-ring` 900→1800ms, `cele-yc`'s `cele-ring`
700→1400ms, `red-strobe` 900→1800ms). Keyframe PERCENTAGES and all
easing stay unchanged — only durations/delays double.

**Verify**:
- `grep -n "620ms\|720ms\|140ms\|280ms" src/styles.css` → no hits inside
  the celebration blocks (the only permitted survivors are matches in
  UNRELATED blocks — check each remaining hit's context and list them in
  your report).
- `grep -c "1440ms" src/styles.css src/settings/preview-overlay.css` →
  ≥1 in each file.

### Step 3: Full gates

**Verify**: `npx vitest run`, `npx tsc --noEmit`, `npx biome ci .`,
`npx vite build` → all clean; `git status` → only in-scope files.

## Test plan

No new tests: the bar's position and animation pacing are visual (jsdom
resolves neither layout nor animation timing). Existing TtlBar behavior
tests (freeze/resume/re-anchor) must stay green untouched — they prove
the move didn't break the rAF wiring. Visual confirmation joins the
operator's next look: single line at card bottom, bar flush with rounded
corners, goal celebration visibly slower.

## Done criteria

- [ ] All Step 1–3 greps/gates return the stated results
- [ ] No behavioral test changed (only structural assertions, if any — list them)
- [ ] `git status` clean, only in-scope files

## STOP conditions

- An existing test asserts the bar's old position in a BEHAVIORAL way
  (not just DOM order) — report instead of rewriting behavior.
- You find a `setTimeout`-based celebration/pulse timer in
  StatusRailCard.tsx after all (the animationend premise would be wrong).
- `.below-block` turns out not to be `position: relative` (absolute bar
  would anchor to the wrong ancestor).

## Maintenance notes

- Any future element added as the last child of `.below-block` will
  paint OVER the bar; keep the bar last (or bump its z-index then).
- If a future celebration is added, its class must join
  `CELEBRATION_END_ANIMATION`/`PULSE_END_ANIMATION` — the name-keyed
  animationend contract is what let this plan skip TS entirely.
- Reviewer: eyeball both CSS diffs side-by-side for mirror-law identity.
