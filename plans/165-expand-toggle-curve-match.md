# 165 — Match the expand-toggle shell width curve to the manifest it reveals

- **Status**: DONE (2026-07-31) — `.card-assembly.expanded` now has its own `transition: width var(--expand-ms, 320ms) var(--ease-notchtap);`. `npx vitest run` 573/573.
- **Commit**: ef91a0f, then 9855bfe (follow-up /review-animations fixes)
- **Severity**: HIGH
- **Category**: Cohesion / interruptibility & timing (review-animations skill, "one clock, one gesture")
- **Estimated scope**: 1 file, 1 new rule
- **Depends on**: plan 163 (this curve currently never plays at all — see that plan's Problem section)

## Problem

The compact→expanded disclosure is one user-facing gesture (the manifest
detail panel growing open) implemented as two separately-curved
animations that happen to share a duration but not an easing shape.

The shell's own width grow to 500px, `.card-assembly.expanded`
(`card-chrome.css:118-120`), never redeclares `transition`, so — exactly
like the hover-reveal case plan 164 fixes — it inherits the base rule's
bouncy `--ease-notchtap-pop`:

```css
/* src/overlay/card-chrome.css:118-120 (current) */
.card-root .card-assembly.expanded {
  --cw: min(max(calc(500px * var(--card-scale)), calc(var(--notchtap-cutout-width, 200px) + (2 * 60px * var(--card-scale)))), 100%);
}
```

Meanwhile the manifest content growing INSIDE that shell,
`.manifest-wrap` (`src/overlay/manifest.css:15-22`), uses the plain,
non-bounce curve at the same nominal duration:

```css
/* src/overlay/manifest.css:15-22 (current) */
.card-root .manifest-wrap {
  display: grid;
  grid-template-rows: 0fr;
  opacity: 0;
  transition:
    grid-template-rows var(--expand-ms, 320ms) var(--ease-notchtap),
    opacity var(--expand-ms, 320ms) var(--ease-notchtap);
}
.card-root .manifest-wrap.expanded {
  grid-template-rows: 1fr;
  opacity: 1;
}
```

Measured (once plan 163 is applied — see that plan): the shell width
overshoots to 504.23px against a 500px target, visually reaching its
"looks done" band (~2% of target) by t≈107-115ms; the manifest's
`grid-template-rows`/`opacity`, having no overshoot, doesn't cross that
same band until t≈195-200ms. Both nominally *finish* (transitionend) at
the shared 320ms mark, but for the ~85-90ms in between, the outer shape
reads as "already arrived and wobbling" while the inner content is still
visibly growing — two halves of one disclosure disagreeing about when the
motion is done.

Unlike the hover-reveal case (plan 164), the fix here does not need a
duration change — `EXPAND_MS` (320ms) is already the single-sourced value
both the shell's base rule and the manifest disclosure reference. The only
mismatch is the easing *curve*, not the timing.

## Target

Give `.card-assembly.expanded` its own width transition using the same
plain `--ease-notchtap` curve and the same `EXPAND_MS` duration the
manifest already uses — removing the curve mismatch without changing
either side's timing.

```css
/* src/overlay/card-chrome.css (target) — insert immediately after the
   existing .expanded rule at :118-120 */
/* plan 165: this shell-width grow accompanies `.manifest-wrap`'s own
   grid-template-rows/opacity disclosure (manifest.css), which already
   uses the plain `--ease-notchtap` at the same `--expand-ms` duration —
   but this rule was inheriting the base `.card-assembly` rule's bouncy
   `--ease-notchtap-pop` (reserved for a genuine new-notification
   arrival, per that rule's own FEEL-CHECK comment), so the outer shell
   visibly overshot and wobbled while the inner manifest content grew on
   a plain, non-overshooting curve — two halves of one disclosure
   finishing their "looks done" moment ~85-90ms apart despite sharing a
   nominal 320ms duration. Same duration as the base rule (no retune),
   only the curve changes, so this is a pure desync fix, not a pacing
   change. Mirrors `.exiting`'s own precedent (card-chrome.css) and plan
   164's identical fix for the hover-reveal leg. */
.card-root .card-assembly.expanded {
  transition: width var(--expand-ms, 320ms) var(--ease-notchtap);
}
```

Note: this ADDS a `transition` declaration to the existing rule (it
currently only sets `--cw`) — do not remove or alter the existing `--cw`
line.

## Repo conventions to follow

- Same precedent as plan 164 cites: `.card-assembly.exiting`
  (`card-chrome.css:374-379`) already demonstrates overriding the base
  rule's bounce curve for a specific state class.
- `var(--expand-ms, 320ms)` is the existing single-sourced token
  (`EXPAND_MS` in `src/animationTiming.ts:107`) already shared by the base
  `.card-assembly` rule and `.manifest-wrap` — this plan reuses it
  unchanged, it does not introduce a new duration.

## Steps

1. In `src/overlay/card-chrome.css`, locate `.card-root
   .card-assembly.expanded` (currently lines 118-120).
2. Add a `transition: width var(--expand-ms, 320ms) var(--ease-notchtap);`
   declaration inside that same rule block, alongside the existing `--cw`
   line, with the comment shown in the Target section.
3. Do not touch any other selector in this file, and do not touch
   `src/overlay/manifest.css` — it is already correct.

## Boundaries

- Do NOT touch the base `.card-assembly` rule — the genuine
  idle/bare→showing promotion entrance (a below-block newly mounting from
  nothing) still correctly gets the bounce curve, and is unaffected by
  this plan.
- Do NOT touch `src/overlay/manifest.css` — its curve/duration are already
  correct; this plan brings the shell to match it, not the reverse.
- Do NOT change `EXPAND_MS`'s numeric value in `animationTiming.ts` —
  reused unchanged.
- If the current code at `card-chrome.css:118-120` doesn't match the
  quoted excerpt (drift since commit `58cccd9`), STOP and report instead
  of improvising.

## Verification

- **Mechanical**: `npx vitest run` (repo root) clean. `npx biome ci .`
  clean.
- **Prerequisite**: plan 163 must be applied first, or this change will
  have no visible effect.
- **Feel check**: with plan 163 also applied, run the app, promote a
  notification, then trigger the expand toggle (the ⌃⇧N hotkey, or however
  your test setup drives `slot.expanded`):
  - Confirm the shell's width growth to 500px no longer visibly
    overshoots/settles back independently of the manifest content growing
    inside it — both should read as reaching their final size together.
  - In DevTools Animations panel at 10% playback, confirm the shell width
    and the manifest's `grid-template-rows`/`opacity` now visually
    "finish" within a similar window, not ~90ms apart.
  - Confirm collapsing (expanded → compact) is unaffected by this plan —
    that direction isn't driven by this rule (the shell just reverts to
    the base rule's own `--cw` once `.expanded` no longer applies) and
    should look the same as before.
  - Confirm a genuine notification promotion (idle/bare → compact, no
    `.expanded` class involved) still visibly overshoots/settles with the
    pop curve — unaffected by this plan.
- **Done when**: `.card-assembly.expanded` has its own non-bounce,
  `EXPAND_MS`-duration width transition, `npx vitest run` is clean, and
  the feel-check (with plan 163 applied) confirms the shell and manifest
  now read as one converging motion.
