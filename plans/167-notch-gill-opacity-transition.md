# 167 — Fade the concave "gill" corners instead of hard-popping them

- **Status**: DONE (2026-07-31) — `.notch-gill` visibility within HUD mode is now opacity-driven; the outer real-hardware `display: none` gate is untouched. `npx vitest run` 573/573 (no existing test asserted on the old display-toggle behavior, so none needed updating).
- **Commit**: ef91a0f
- **Severity**: MEDIUM
- **Category**: Interruptibility & timing / physicality (review-animations skill — `display` cannot be transitioned)
- **Estimated scope**: 1 file, restructure one small rule group
- **Depends on**: plan 163 (this plan adds a real transition; without 163 the curve it uses is inert)

## Problem

The concave "gill" corners (`.notch-gill`) are the piece of synthetic art
specifically built to make the HUD idle rail read as a real hardware notch
rather than, per the file's own comment, "a flat rounded black bar" —
extensive doc comments (`card-chrome.css:481-534`) describe two rounds of
operator feedback that led to this exact technique.

They currently toggle visibility via `display: none` ↔ `display: block`:

```css
/* src/overlay/card-chrome.css:562-595 (current) */
.card-root .notch-gill {
  display: none;
  position: absolute;
  top: 0;
  width: 10px;
  height: 10px;
  pointer-events: none;
}
:root[data-notchtap-mode="hud"] .card-root .card-assembly:not(:has(.below-block)) .notch-gill,
:root[data-notchtap-mode="hud"] .card-root .card-assembly.exiting:not(.bare) .notch-gill {
  display: block;
}
.card-root .notch-gill-left {
  left: -10px;
  background: radial-gradient(circle at bottom left, transparent 10px, #000 10px);
}
.card-root .notch-gill-right {
  right: -10px;
  background: radial-gradient(circle at bottom right, transparent 10px, #000 10px);
}
```

`display` cannot be transitioned by CSS at all — there is no
intermediate state between `none` and `block`. This toggles at exactly
the same DOM boundary as plan 166's flank corner-radius fix (a
below-block newly mounting/unmounting), so right now the one piece of art
whose entire purpose is selling the "real notch" illusion is also the
single most abrupt element in the whole shell during exactly the
transitions under review.

## Target

Convert the below-block-driven show/hide from `display` to `opacity`,
while keeping the OUTER real-hardware gate (`data-notchtap-mode="hud"`)
as a `display: none` toggle, unchanged — real notch hardware must
continue to never render this element at all (per the same "sits behind
the physical camera housing, nothing to gain by painting it" reasoning
the file already documents for the analogous idle-face element). Only
*within* HUD mode does the element now always stay in the render tree,
with `opacity` governing whether it's visible:

```css
/* src/overlay/card-chrome.css (target) */
.card-root .notch-gill {
  display: none;
  position: absolute;
  top: 0;
  width: 10px;
  height: 10px;
  pointer-events: none;
  /* plan 167: visibility within HUD mode is now `opacity`-driven (below),
     not this `display: none` — that stays exactly as-is here, still the
     permanent, unconditional gate that keeps this element fully out of
     the render tree on real notch hardware (unaffected by this plan;
     `display` genuinely cannot be transitioned, so it must stay a hard
     boundary for the "does this mode exist at all" question — only the
     "is it currently showing" question below moves to opacity). */
  opacity: 0;
  transition: opacity var(--expand-ms, 320ms) var(--ease-notchtap);
}
/* plan 167: within HUD mode, the gill stays permanently in the render
   tree (a tiny, absolutely-positioned, pointer-events:none pair of
   elements — negligible cost) so its opacity can actually transition;
   previously `display: none` here meant the element didn't exist to
   animate at the exact moment it needed to fade. */
:root[data-notchtap-mode="hud"] .card-root .notch-gill {
  display: block;
}
:root[data-notchtap-mode="hud"] .card-root .card-assembly:not(:has(.below-block)) .notch-gill,
:root[data-notchtap-mode="hud"] .card-root .card-assembly.exiting:not(.bare) .notch-gill {
  opacity: 1;
}
.card-root .notch-gill-left {
  left: -10px;
  background: radial-gradient(circle at bottom left, transparent 10px, #000 10px);
}
.card-root .notch-gill-right {
  right: -10px;
  background: radial-gradient(circle at bottom right, transparent 10px, #000 10px);
}
```

## Repo conventions to follow

- `var(--expand-ms, 320ms) var(--ease-notchtap)` matches plan 166's
  identical choice for the flank corner-radius, which fires at the exact
  same DOM boundary (below-block mount/unmount) — keeping the gill fade
  and the corner round-in on the same clock, since they're visually one
  "shape resolving" moment.
- The `idle-face` element (`card-chrome.css:632-657`) is the file's own
  precedent for the "permanently gated by `display:none` outside HUD
  mode, but something else drives visibility within HUD mode" pattern
  this plan follows — though idle-face uses a React-side conditional
  (`idleFaceEligible`) rather than CSS, the underlying principle (never
  pay any cost for this art on real hardware) is the same one this plan
  preserves for `.notch-gill`.

## Steps

1. In `src/overlay/card-chrome.css`, locate the `.card-root .notch-gill`
   base rule (currently lines 562-569).
2. Add `opacity: 0;` and `transition: opacity var(--expand-ms, 320ms)
   var(--ease-notchtap);` to that rule, alongside the existing
   `pointer-events: none;` line, with the comment shown in the Target
   section. Leave `display: none;` in this rule exactly as-is.
3. Immediately after that base rule, add a new rule:
   `:root[data-notchtap-mode="hud"] .card-root .notch-gill { display:
   block; }` with the comment shown in the Target section.
4. Locate the existing "show" rule (currently lines 584-587,
   `:root[data-notchtap-mode="hud"] .card-root
   .card-assembly:not(:has(.below-block)) .notch-gill,
   :root[data-notchtap-mode="hud"] .card-root .card-assembly.exiting:not(.bare)
   .notch-gill`). Change its declaration from `display: block;` to
   `opacity: 1;`.
5. Do not touch `.notch-gill-left`/`.notch-gill-right` — unchanged.

## Boundaries

- Do NOT remove or alter the outer `display: none;` on the base rule —
  it is the permanent gate that keeps this element out of the render tree
  entirely on real notch hardware, and must stay a hard (non-transitioned)
  boundary since `display` cannot animate.
- Do NOT touch `.notch-gill-left`/`.notch-gill-right`'s positioning or
  gradient rules.
- Do NOT touch plan 166's flank corner-radius fix or any other rule in
  this file.
- If the current code at `card-chrome.css:562-595` doesn't match the
  quoted excerpt (drift since commit `58cccd9`), STOP and report instead
  of improvising.

## Verification

- **Mechanical**: `npx vitest run` (repo root) clean — in particular,
  re-check any test that asserts on `.notch-gill`'s presence/absence in
  the DOM (search `notch-gill` in `*.test.tsx`); since this element is now
  ALWAYS present in HUD mode (opacity-driven, not mount/unmount-driven),
  any test that previously asserted `display: none` or DOM absence as a
  proxy for "not showing" needs to instead assert `opacity: 0` or the
  computed style — flag and fix any such test rather than silently
  breaking it. `npx biome ci .` clean.
- **Prerequisite**: plan 163 must be applied first, or the curve this fix
  uses won't resolve and no visible fade will occur (it will just always
  render at whatever opacity the currently-matching rule sets, changing
  instantly rather than fading — functionally similar to the current
  display-toggle snap, so verify against 163 to see the real intended
  effect).
- **Feel check**: with plan 163 also applied, put the app in HUD mode
  (`:root[data-notchtap-mode="hud"]`), trigger a genuine idle/bare→showing
  promotion:
  - Confirm the concave gill corners now visibly fade out (rather than
    popping instantly) as the below-block mounts, roughly in step with
    the flank corner-radius round-in from plan 166.
  - Trigger the reverse (showing→idle exit) and confirm the gills fade
    back in smoothly as the below-block clears.
  - Confirm real notch hardware mode (`data-notchtap-mode` NOT `"hud"`,
    if you can toggle/simulate it) never renders this element at all —
    check via DevTools that `.notch-gill` computes `display: none` there,
    unaffected by this plan.
- **Done when**: the gill's visibility is opacity-driven within HUD mode
  (with the outer `display: none` hardware gate untouched), `npx vitest
  run` is clean (any DOM-presence-based test assertions updated to check
  opacity instead), and the feel-check (with plan 163 applied) confirms a
  smooth fade instead of a hard pop.
