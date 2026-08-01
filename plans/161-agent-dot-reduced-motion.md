# 161 — Add reduced-motion coverage to the Agent Board's dot pulse/breathe

- **Status**: DONE (2026-07-31) — reduced-motion block added after `@keyframes agent-dot-breathe`, dropping the tick while keeping the breathe. `npx vitest run` 573/573.
- **Commit**: ef91a0f
- **Severity**: MEDIUM
- **Category**: Accessibility (AUDIT.md §6)
- **Estimated scope**: 1 file, ~1 new rule block

## Problem

`src/overlay/agent-board.css:219-233` animates a real `transform: scale(...)`
tick plus an opacity breathe on every agent session state change, with **no**
`@media (prefers-reduced-motion: reduce)` coverage anywhere in the file. A
repo-wide check confirms every other animated overlay CSS file
(`manifest.css`, `choreography.css`, `weather-art.css`, `ttl-bar.css`,
`news-category.css`, `idle-peek.css`) has its own reduced-motion override —
`agent-board.css` is the one exception.

Current code, `src/overlay/agent-board.css:219-241`:

```css
.card-root .agent-dot.pulse {
  animation:
    agent-dot-state-tick 240ms var(--ease-notchtap),
    agent-dot-breathe 2.2s ease-in-out 4;
}

@keyframes agent-dot-state-tick {
  from {
    transform: scale(1.35);
  }
  to {
    transform: scale(1);
  }
}

@keyframes agent-dot-breathe {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.35;
  }
}
```

Per AUDIT.md §6: "Reduced motion means fewer and gentler animations, not
zero — keep transitions that aid comprehension, remove position changes."
The tick (`scale(1.35)` → `scale(1)`) is a real transform/position-adjacent
change that should be dropped under reduced motion; the breathe (opacity
only, bounded to 4 cycles per plan 149's own doc comment above this rule)
is comprehension-aiding "this just changed" feedback that should be kept,
per the rubric's own "keep transitions that aid comprehension... remove
position changes" split.

## Target

Add a reduced-motion override that removes the transform tick but keeps a
gentler, opacity-only comprehension cue. Rather than fully disabling all
feedback (which the rubric explicitly warns against), keep the breathe
animation but drop the tick:

```css
@media (prefers-reduced-motion: reduce) {
  .card-root .agent-dot.pulse {
    animation: agent-dot-breathe 2.2s ease-in-out 4;
  }
}
```

This removes `agent-dot-state-tick` (the transform/scale component) from
the animation shorthand entirely under reduced motion, while leaving
`agent-dot-breathe` (opacity-only, already bounded to 4 cycles, ~8.8s total)
running exactly as it does today — this satisfies "remove position changes"
(no more transform) while keeping the "this just changed" comprehension
signal (the breathe) intact, exactly per AUDIT.md §6's own split.

## Repo conventions to follow

- Exemplar: `src/overlay/choreography.css:307-313` (the pulse-goal/pulse-red
  reduced-motion block):
  ```css
  /* the two pulses above are plain CSS and were never covered by any
     reduced-motion mechanism — they need this override in their own right. */
  @media (prefers-reduced-motion: reduce) {
    .card-root .card-assembly.pulse-goal,
    .card-root .card-assembly.pulse-goal::before,
    .card-root .card-assembly.pulse-goal::after,
    .card-root .card-assembly.pulse-red {
      animation: none;
    }
  }
  ```
  This plan's situation differs slightly — choreography.css's celebration
  pulses fully disable under reduced motion (`animation: none`), because
  they're purely decorative celebration effects with no comprehension
  value. `agent-board.css`'s breathe genuinely carries state-change
  information (per its own plan-149 doc comment reasoning about *why* it
  exists), so per AUDIT.md §6's "not zero... keep transitions that aid
  comprehension" instruction, this plan keeps the breathe rather than
  fully zeroing the animation. Follow choreography.css's *placement and
  comment style* (a one-line rationale comment directly above the `@media`
  block, block placed near the rule it modifies), not its "disable
  everything" outcome.

## Steps

1. In `src/overlay/agent-board.css`, locate the `@keyframes
   agent-dot-breathe` rule (currently ending around line 241).
2. Immediately after it, add a one-line rationale comment plus the
   `@media (prefers-reduced-motion: reduce)` block shown in the Target
   section above.
3. Do not modify `.card-root .agent-dot.pulse`'s own (non-media-query)
   rule, nor either `@keyframes` block — this is purely an additive
   override.

## Boundaries

- Do NOT touch `.card-root .agent-dot.large` or any other rule in this
  file.
- Do NOT touch `src/components/AgentBoard.tsx` (the React side that keys
  `.agent-dot` on session state to trigger remounts) — this plan is
  CSS-only.
- Do NOT fully disable the breathe animation (`animation: none`) — per
  AUDIT.md §6, that would over-correct; the target keeps the opacity
  breathe and only drops the transform tick.
- If the current code at the cited lines doesn't match the quoted excerpt
  (drift since commit `58cccd9`), STOP and report instead of improvising.

## Verification

- **Mechanical**: `npx vitest run` (repo root) → no test currently asserts
  on `agent-dot-breathe`'s reduced-motion behavior (confirm via grep for
  `agent-dot` in `*.test.ts(x)` before assuming; if a test does exist and
  now fails, that's expected — update its expectation to match the new
  reduced-motion CSS, don't revert this plan's change to make it pass).
  `npx biome ci .` clean (CSS isn't biome-linted, but confirm no adjacent
  file was accidentally touched).
- **Feel check**: run the app with an active agent session, in Chrome
  DevTools Rendering panel set "Emulate CSS media feature
  prefers-reduced-motion" to `reduce`, then trigger a session state change
  (waiting → working → completed, or simulate via the settings window's
  "Send test event"):
  - Confirm the dot's `scale(1.35)`-to-`scale(1)` tick no longer plays.
  - Confirm the opacity breathe (4 cycles, dimming to ~35% and back) still
    plays.
  - Set emulation back to "No preference" and confirm both the tick and
    breathe play exactly as before this change.
- **Done when**: the new `@media (prefers-reduced-motion: reduce)` block
  exists in `agent-board.css`, `npx vitest run` is clean, and the
  feel-check confirms the tick is suppressed while the breathe survives
  under reduced motion.
