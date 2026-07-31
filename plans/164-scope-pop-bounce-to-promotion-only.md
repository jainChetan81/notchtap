# 164 — Scope the shell's pop-bounce curve out of the hover-reveal leg

- **Status**: DONE (2026-07-31) — `.bare:has(.idle-peek)` now has its own `transition: width var(--reveal-ms, 260ms) var(--ease-notchtap);`. `npx vitest run` 573/573.
- **Commit**: 58cccd9
- **Severity**: HIGH
- **Category**: Physicality & origin / frequency-appropriateness (review-animations skill, Standards #2, #10)
- **Estimated scope**: 1 file, 1 new rule
- **Depends on**: plan 163 (this curve currently never plays at all — see that plan's Problem section; this plan's effect is invisible until 163 lands)

## Problem

`.card-assembly`'s base rule (`card-chrome.css:35-106`) is the ONLY place
`.card-assembly`'s `width` transition is declared:

```css
/* src/overlay/card-chrome.css (current) */
.card-root .card-assembly {
  ...
  transition:
    width var(--expand-ms, 320ms) var(--ease-notchtap-pop, cubic-bezier(0.3, 1.36, 0.44, 1)),
    transform var(--hover-ms, 160ms) var(--ease-notchtap);
}
```

`--ease-notchtap-pop` is a genuine overshoot curve (`cubic-bezier(0.3,
1.36, 0.44, 1)` — the `y=1.36` control point mathematically overshoots
past the target before settling). The rule's own FEEL-CHECK comment
explains the intent: "a slight overshoot so promotion/expand width growth
lands with a touch of physical mass... `transform` (the hover breathe) is
untouched: hover never animates `width` at all, so this can never be felt
as part of the hover response — only a real promotion or manual expand
feels it."

That claim is incorrect for the minimal(bare)→idle hover-reveal leg
specifically. `.card-assembly.bare:has(.idle-peek)` (`card-chrome.css:164-166`)
changes `--cw` purely as a function of hover state:

```css
/* src/overlay/card-chrome.css:164-166 (current) */
.card-root .card-assembly.bare:has(.idle-peek) {
  --cw: min(calc(var(--notchtap-cutout-width, 200px) + (2 * 85px * var(--card-scale))), 100%);
}
```

This selector never redeclares `transition`, so it inherits the base
rule's bouncy `--ease-notchtap-pop` width transition — meaning every time
a user hovers the minimal/bare notch to reveal the idle rail (a
cursor-proximity gesture that can retrigger many times per session, not a
rare "arrival" event), the shell overshoots past its target width and
settles back, exactly like a genuine new-notification promotion does.
Measured (once plan 163 is applied): width overshoots to 377.2px against
a 370px target (a ~7px/~2% overshoot), settling ~376ms after the hover
starts.

Meanwhile the SAME hover gesture's companion content — the flank
clock/status-dots fade (`StatusRailCard.tsx:779`, `REVEAL_MS` = 260ms,
`NOTCHTAP_EASE`, no bounce) — runs on a different clock entirely (260ms vs
the shell's 320ms), so the shell is still visibly wobbling toward its
target for ~60ms+ after the clock/dots have already finished fading in.

Per review-animations Standard #2 (frequency-appropriateness): "Tens of
times/day (hover effects...) → Remove or drastically reduce." The most
emphatic curve in the system (the one reserved for a genuinely rare
"physical mass" arrival) is currently applied to the highest-frequency
trigger in this whole chain.

## Target

Give `.bare:has(.idle-peek)` its own width transition: the plain,
non-bounce house ease, retimed to `REVEAL_MS` (260ms) so it matches the
duration of the clock/dots fade it's paired with — resolving both the
misapplied-bounce issue and the clock-desync issue in one edit, since
they share the same file and the same fix shape.

```css
/* src/overlay/card-chrome.css (target) — insert immediately after the
   existing .bare:has(.idle-peek) rule at :164-166 */
/* plan 164: this is a HOVER-driven width change (cursor proximity, can
   retrigger many times a session), not a genuine new-notification
   arrival — it must not inherit the base rule's `--ease-notchtap-pop`
   overshoot, which the base rule's own FEEL-CHECK comment reserves for
   "promotion/expand width growth" specifically. Retimed to `--reveal-ms`
   (260ms) to match this exact gesture's OTHER moving part — the flank
   clock/status-dots opacity fade (StatusRailCard.tsx, same REVEAL_MS) —
   so the shell and its content finish growing/fading on the same clock
   instead of the shell's width still settling ~60ms after the content
   has already finished. Mirrors `.exiting`'s own precedent
   (card-chrome.css, "exits must not bounce") — the same scoping
   principle applied to the growing/reveal direction instead of the
   shrinking direction. */
.card-root .card-assembly.bare:has(.idle-peek) {
  transition: width var(--reveal-ms, 260ms) var(--ease-notchtap);
}
```

Note: this ADDS a `transition` declaration to the existing rule (it
currently only sets `--cw`) — do not remove or alter the existing `--cw`
line.

## Repo conventions to follow

- `.card-assembly.exiting` (`card-chrome.css:374-379`) is the exact
  precedent for this pattern — it already overrides the base rule's
  `transition` to swap out the bounce curve for the plain
  `--ease-notchtap`, for exactly the same reason ("exits must not
  bounce"). This plan applies the identical technique to the opposite
  (growing) direction's hover-driven leg.
- `var(--reveal-ms, 260ms)` is the existing single-sourced token (`REVEAL_MS`
  in `src/animationTiming.ts:121`) already used by this exact gesture's
  other properties — e.g. the flank background-color/padding fade
  (`card-chrome.css:221-223`, same `.flank-left`/`.flank-right`
  rule) already uses `var(--reveal-ms, 260ms) var(--ease-notchtap)`. This
  plan brings the shell's own width onto the same token, not a new one.

## Steps

1. In `src/overlay/card-chrome.css`, locate `.card-root
   .card-assembly.bare:has(.idle-peek)` (currently lines 164-166).
2. Add a `transition: width var(--reveal-ms, 260ms) var(--ease-notchtap);`
   declaration inside that same rule block, alongside the existing `--cw`
   line, with the comment shown in the Target section.
3. Do not touch any other selector in this file.

## Boundaries

- Do NOT touch the base `.card-assembly` rule's own `transition`
  declaration — it must keep the bounce curve for the genuine
  idle/bare→showing promotion entrance, which is correct and unaffected by
  this plan.
- Do NOT touch `.card-assembly.exiting`/`.exiting.exit-to-bare` — already
  correctly scoped away from the bounce.
- Do NOT touch `StatusRailCard.tsx`'s flank-clock/status-dots
  `motion.span`/`motion.div` `transition` props — they already correctly
  use `REVEAL_MS`; this plan is bringing the CSS side to match them, not
  the other way around.
- Do NOT retune `REVEAL_MS` itself — reuse the existing 260ms value.
- If the current code at `card-chrome.css:164-166` doesn't match the
  quoted excerpt (drift since commit `58cccd9`), STOP and report instead
  of improvising.

## Verification

- **Mechanical**: `npx vitest run` (repo root) clean. `npx biome ci .`
  clean (CSS isn't biome-linted, but confirm no adjacent file changed).
- **Prerequisite**: plan 163 must be applied first, or this change will
  have no visible effect (the underlying curve doesn't resolve at all
  without it).
- **Feel check**: with plan 163 also applied, run the app, put the app in
  bare/minimal notch mode (`restingState: "notch"`, not hovered), then
  hover it:
  - Confirm the shell's width growth from minimal to idle no longer
    visibly overshoots/settles back — it should ease in and stop, not
    wobble.
  - In DevTools Animations panel at 10% playback, confirm the shell's
    width and the flank clock/status-dots opacity now finish within a few
    ms of each other, not ~60ms apart.
  - Confirm a genuine notification promotion (idle/bare → showing, NOT
    triggered by hover) still visibly overshoots/settles with the pop
    curve — this plan must not affect that leg (it's driven by the base
    rule alone, untouched by this change).
  - Confirm the reverse (hover-out, idle-peek closing back to bare) also
    now transitions on the plain ease at 260ms rather than the bounce —
    this rule applies in both directions since `:has()` is a live,
    continuously-evaluated selector.
- **Done when**: `.bare:has(.idle-peek)` has its own non-bounce, 260ms
  width transition, `npx vitest run` is clean, and the feel-check (with
  plan 163 applied) confirms the hover-reveal no longer bounces while a
  genuine promotion still does.
