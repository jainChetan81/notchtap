# 173 — card-chrome width→transform feasibility spike

- **Status**: DONE (2026-08-02), verdict **NO-GO** — keep animating
  `width`. Investigation only, zero code changed (per the spike's own
  brief: "do not blindly convert"). Evidence below is measured, not
  argued from first principles.
- **Severity**: N/A (finding closed as won't-fix, with reopen
  conditions)
- **Category**: Perf investigation (the /review-animations pass's
  "animates a layout property" finding on `.card-assembly`)

## The finding being investigated

`.card-assembly` (card-chrome.css) transitions `width` — a real layout
property — for its promotion/expand width-grow, in several state
variants. Textbook guidance says animate `transform` instead, because
layout properties invalidate layout every frame on the main thread
while transforms composite off it.

## Why every transform-based shape fails here

1. **The fixed-hardware cutout law (fatal).** `.card-assembly` is a
   3-column grid: `1fr | var(--notchtap-cutout-width) | 1fr`. The
   center column IS the synthetic notch cutout, and the repo's standing
   law (recorded at `BOARD_SUMMON_MS`'s doc in animationTiming.ts, born
   from an operator on-sight rejection) is that the cutout must NEVER
   scale, translate, or fade — it impersonates fixed hardware. A
   whole-shell `transform: scaleX(...)` scales the cutout column by
   construction. The width animation exists precisely so the 1fr flanks
   absorb ALL the growth around a pixel-fixed center; no whole-element
   transform can reproduce that.
2. **FLIP + counter-scale (correctness collapse).** Counter-scaling the
   center column and children every frame would need per-frame inverse
   factors on a grid with text, borders, and the ROUNDING-LAW corner
   radii (an x-scale distorts radius-x, visibly flattening the outer
   corners mid-flight). Trades a one-line transition for a large,
   fragile choreography surface guarding against exactly the seams and
   double-curve bugs card-chrome.css's header documents.
3. **Per-flank transforms (can't create width).** Translating
   `.flank-left/right` outward doesn't grow their painted boxes —
   it reveals gaps against the cutout column. Scaling them per-flank
   re-imports problem 2's radius distortion on the outer corners.
4. **clip-path reveal over a pre-laid-out max-width shell (changes the
   motion, doesn't buy anything).** Content would lay out at end-width
   from frame 1 and get revealed by an animated rounded-polygon clip —
   a "reveal" character instead of the shipped "grow" character, an
   animated clip-path is itself paint-bound, and the shell's
   `filter: drop-shadow` (which follows the painted alpha) repaints
   every frame regardless — the repaint the conversion would supposedly
   save is unavoidable here anyway.

## What the animation actually costs (measured)

Chrome trace over the expand-toggle window, real `StatusRailCard`, the
plan-172 harness (`research/plan172-harness/`, `trace.mjs` variant),
2026-08-02, this mac mini:

| pipeline stage | runs | avg | max | total over the whole gesture |
| --- | --- | --- | --- | --- |
| Layout | 19 | 0.09ms | 0.11ms | 1.7ms |
| UpdateLayoutTree | 29 | 0.15ms | 1.07ms | 4.3ms |
| Paint | 92 | 0.05ms | 0.12ms | 4.8ms |
| PrePaint + Layerize | 58 | ~0.05ms | 0.11ms | 2.6ms |

Worst full-pipeline frame ≈ 1.3ms against the 16.7ms/60fps budget
(~8%); the TYPICAL frame's layout leg is 0.09ms (~0.5%). The plan-172
frame samples over the same gesture held ~60fps (worst gap 22ms, one
56ms hiccup across six scenarios). The invalidation scope textbook
advice worries about is one tiny fixed-size overlay window whose entire
content IS this card — there is no surrounding page to thrash.

## Verdict

NO-GO. The conversion's only viable shapes either break the
fixed-cutout law outright or replace a 0.09ms/frame cost with a large
correctness surface (radius distortion, seam/gap regressions, clip
choreography) — and the drop-shadow repaint dominates paint cost either
way. `contain: layout` was considered and skipped too: the assembly is
effectively the window's whole content, so there is nothing outside it
to protect, and the `filter` already isolates it as a containing block.

**Reopen only if**: a real-hardware manual-checklist pass ever shows
dropped frames during promotion/expand (the macbook notch machine, not
this dev box), or a ProMotion/120Hz target halves the frame budget, or
the card ever gets embedded in a larger layout where width invalidation
actually propagates somewhere.
