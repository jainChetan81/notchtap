# 162 — Add reduced-motion coverage to the card's hover "breathe" scale

- **Status**: DONE (2026-07-31) — `transform: none` reduced-motion override added immediately after `.hovered`. `npx vitest run` 573/573.
- **Commit**: ef91a0f
- **Severity**: MEDIUM
- **Category**: Accessibility (AUDIT.md §6)
- **Estimated scope**: 1 file, ~1 new rule block

## Problem

`src/overlay/choreography.css:30-33` applies a `transform: scale(1.02)` on
`.card-assembly.hovered` — the card's "breathe" effect when the cursor is
near the notch. The transition duration lives on the base rule,
`src/overlay/card-chrome.css:104-105`:

```css
transition:
    width var(--expand-ms, 320ms) var(--ease-notchtap-pop, cubic-bezier(0.3, 1.36, 0.44, 1)),
    transform var(--hover-ms, 160ms) var(--ease-notchtap);
```

Current code, `src/overlay/choreography.css:30-33`:

```css
.card-root .card-assembly.hovered {
  transform: scale(1.02);
  /* plan 129 (C5): `transform-origin: top center` moved to the base
     `.card-assembly` rule above — see that declaration's own doc. Was
     declared here (and echoed on `.pulse-goal` below) until this plan;
     kept working for AS LONG AS `.hovered` stayed applied, but the
     origin snapped back to the base rule's implicit `50% 50%` default
     the instant the class cleared, mid the 160ms un-hover shrink. */
}
```

No `@media (prefers-reduced-motion: reduce)` block anywhere in the file
covers `.hovered`. This is set from a rust-derived tracking-area signal
(cursor proximity to the notch), not CSS `:hover` — meaning it can fire
very frequently as the user's cursor moves near the menu bar, making it
(per the file's own surrounding comments) the single highest-frequency
transform trigger in the app. Per AUDIT.md §6: "Hunt for: movement with no
`prefers-reduced-motion` handling."

## Target

Add a reduced-motion override that neutralizes the scale entirely. Unlike
plan 161's agent-dot pulse (which has a separate comprehension-carrying
opacity component worth keeping), this hover breathe is a pure transform
with no other component — so the correct minimal fix is to disable it
outright under reduced motion, not to partially soften it:

```css
@media (prefers-reduced-motion: reduce) {
  .card-root .card-assembly.hovered {
    transform: none;
  }
}
```

## Repo conventions to follow

- Exemplar and placement convention: `src/overlay/choreography.css:307-313`
  (the pulse-goal/pulse-red reduced-motion block), which uses the same
  "one-line rationale comment directly above the `@media` block" style:
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
  Follow this same comment-then-block placement pattern, positioned
  immediately after the `.hovered` rule this plan modifies (i.e., right
  after line 33's closing brace), not down at the file's existing
  celebration-pulse `@media` block near line 307 — keep the override
  physically adjacent to the rule it's overriding, matching where plan 161
  places its own block relative to `agent-dot-breathe`.
- `transform: none` (rather than omitting the property or setting `scale:
  1`) is the correct override value because the base `.card-assembly` rule
  (per its own plan 129 doc comment) sets `transform-origin` but not a
  resting `transform` — `.hovered` is the only rule that ever sets
  `transform` on this element, so `transform: none` under reduced motion
  fully and correctly neutralizes it back to the element's un-transformed
  resting state.

## Steps

1. In `src/overlay/choreography.css`, locate the `.card-root
   .card-assembly.hovered` rule (currently lines 30-36, including its
   trailing doc comment and closing brace).
2. Immediately after its closing brace, add a one-line rationale comment
   plus the `@media (prefers-reduced-motion: reduce)` block shown in the
   Target section above.
3. Do not modify the `.hovered` rule itself, nor `card-chrome.css`'s base
   transition declaration.

## Boundaries

- Do NOT touch `card-chrome.css` — the transition duration/easing that
  drives the (still-present, non-reduced-motion) hover breathe stays
  exactly as-is; this plan only adds the override for the reduced-motion
  case.
- Do NOT touch the celebration pulse rules (`pulse-goal`, `pulse-red`) or
  their existing `@media` block near line 307 — those are already covered,
  out of scope.
- Do NOT add a resting `transform` declaration to the base `.card-assembly`
  rule — `transform: none` in the new block is sufficient and matches how
  the element already behaves when `.hovered` is absent.
- If the current code at the cited lines doesn't match the quoted excerpt
  (drift since commit `58cccd9`), STOP and report instead of improvising.

## Verification

- **Mechanical**: `npx vitest run` (repo root) → confirm no test currently
  asserts on `.hovered`'s computed transform under reduced motion (grep for
  `hovered` in `*.test.ts(x)` first); if one exists and now needs updating,
  update its expectation to match this plan's change rather than reverting
  it. `npx biome ci .` clean.
- **Feel check**: run the app on real or emulated hardware, in Chrome
  DevTools Rendering panel set "Emulate CSS media feature
  prefers-reduced-motion" to `reduce`, then move the cursor near the notch
  (or toggle the `hovered` state via whatever test/debug hook the app
  exposes) and confirm:
  - The card no longer scales up on hover.
  - Any other hover-driven behavior (e.g. content reveal, if hover also
    triggers something besides the scale — check `StatusRailCard.tsx`'s
    `hovered` prop usage) is unaffected — this plan only touches the
    transform, not hover-driven content changes.
  - Set emulation back to "No preference" and confirm the 1.02 scale
    breathe returns exactly as before this change.
- **Done when**: the new `@media (prefers-reduced-motion: reduce)` block
  exists in `choreography.css` immediately after the `.hovered` rule, `npx
  vitest run` is clean, and the feel-check confirms the scale is
  suppressed under reduced motion and unchanged otherwise.
