# 151 — Scorecard morphs, score odometer, media-bar truthfulness, ambient drift shape

- **Status**: DONE (2026-07-27)
- **Commit**: 0c5ae11
- **Severity**: MEDIUM (M3/F7/F6) + one LOW delight item (M4)
- **Category**: Missed opportunities, Interruptibility, Physicality
- **Estimated scope**: 6 files, ~120 lines

## Problem & Target, per item

### A. Match-state pill snaps (LIVE → HT → FT)

`src/components/LiveMatchScorecard.tsx:57-61` swaps `pillVariant`/label;
`src/overlay/live-scorecard.css:56-72` gives `.chip-live.break`/`.final`
different color/background/border with NO transition on the base rule,
and `.live-dot` vanishes the same frame.

Target: on the base `.chip-live` rule add
`transition: color var(--reveal-ms, 260ms) var(--ease-notchtap), background-color var(--reveal-ms, 260ms) var(--ease-notchtap), border-color var(--reveal-ms, 260ms) var(--ease-notchtap);`
and make `.live-dot` leave via `opacity` (transition
`opacity var(--reveal-ms) var(--ease-notchtap)`; keep the element
mounted with `opacity: 0` in break/final variants rather than
unmounting — smallest structural change that lets it fade).

### B. Score digits never move on a goal (the payload is the one thing that doesn't animate)

`src/components/LiveMatchScorecard.tsx:68-72` renders `homeScore`/
`awayScore` as bare text while `goal-overshoot`/`goal-burst`/ripple
celebrate around them.

Target: a single-digit odometer roll, goal-gated:
- wrap each score in a fixed-height `overflow: hidden` clip span;
- key the inner `motion.span` on the score value inside
  `<AnimatePresence initial={false} mode="popLayout">`;
- incoming digit `initial={{ y: "100%" }} animate={{ y: 0 }}`, outgoing
  `exit={{ y: "-100%" }}` (percentage translate, compositor-only),
  `transition={{ duration: 0.36, ease: NOTCHTAP_EASE, delay: 0.12 }}`
  — landing well inside the 1240ms celebration;
- restraint guards: ONLY animate when the value CHANGES (keying by
  value gives this) — and since the scorecard re-renders each minute
  for the clock pill, confirm the clock tick cannot remount the score
  spans (the key is the score value; assert it in a test).

### C. Media progress bar glides backwards on track change / keeps moving on pause

`src/overlay/idle-peek.css:291-298` — `transition: transform 1s linear`
on `.media-bar-fill`, correct for the 1s playback tick
(`src/components/IdleHoverPeek.tsx:262-271`, `:318`), but also applied
to discontinuities: a track change slides the fill leftward over a full
second; a pause lets it advance up to 1s after the transport glyph
(`:303-307`) has flipped instantly.

Target: suppress the transition on discontinuities — in
`IdleHoverPeek.tsx`, detect (a) `media.playing === false` or (b) a
progress DECREASE or (c) a title change, and for that render set inline
`transition: "none"` on the fill (clearing back to the CSS rule on the
next steady tick). Track the previous progress/title via a ref. Comment
with the rule: the 1s linear glide is for steady playback only; resets
must read as resets.

### D. Ambient drifts are straight lines pretending to wander

1. `src/overlay/news-category.css:31-38` — `shade-drift 12s ease-in-out
   infinite alternate` between `(0,0)` and `(-8%,-6%)`: a pendulum on a
   rail. Target: reshape the keyframes into a non-collinear path so the
   wander never reads as a straight retrace, same cost:

```css
@keyframes shade-drift {
  0%   { transform: translate3d(0, 0, 0); }
  33%  { transform: translate3d(-7%, -2%, 0); }
  66%  { transform: translate3d(-3%, -5%, 0); }
  100% { transform: translate3d(-8%, -6%, 0); }
}
```

(keep `alternate`, duration, and the existing `will-change`; keep the
settled existence/ease decisions — this changes SHAPE only).

2. `src/overlay/weather-art.css:100-123` — the comment promises "a
   gentle diagonal sway rather than a straight fall", the keyframes
   deliver a straight `translate(0,0) → (22px,66px)`. Target: split
   the axes into two stacked animations with non-harmonic periods so
   the sway actually exists, preserving the tiling contract the
   comment at `:97-99` establishes (each axis must end on its own tile
   multiple):

```css
/* target shape — adjust to the element's actual structure: */
animation:
  snow-fall-y 6.6s linear infinite,
  snow-sway-x 14s ease-in-out infinite alternate;
@keyframes snow-fall-y { from { translate: 0 0; } to { translate: 0 66px; } }
@keyframes snow-sway-x { from { translate: 0 0; } to { translate: 22px 0; } }
```

CAUTION: two animations composing `translate` on one element override
each other — if the current code animates a single `transform`, either
use the separate `translate` property axes via two elements (nest a
wrapper), or animate `background-position-x`/`-y` independently if the
layer is a tiled background. Read the actual structure first; keep the
loop seamless (no visible snap at either period boundary). `rain-fall`
(`:91`) stays untouched — a straight fall is correct for rain.

## Repo conventions to follow

- Timing via the CSS vars (`--reveal-ms`, `--ease-notchtap`) — see
  news-category.css for var-with-fallback usage.
- Motion components: `AnimatePresence initial={false}` idioms as in
  StatusRailCard/AgentBoard.
- The `.rotation-swap` suppression idiom (news-category.css:143-154):
  check whether the scorecard participates; if the odometer could fire
  on a same-slot rotation re-emit, gate it the same way.

## Boundaries

- Files: `src/components/LiveMatchScorecard.tsx`, `src/overlay/live-scorecard.css`, `src/components/IdleHoverPeek.tsx`, `src/overlay/idle-peek.css` (media-bar rules ONLY — the celebration blocks belong to plan 150), `src/overlay/news-category.css` (keyframes only), `src/overlay/weather-art.css` (snow only), plus their test files.
- Do NOT touch choreography.css or StatusRailCard.tsx (plan 150 owns them).
- No reduced-motion variants (permanent non-goal).
- If cited code has drifted, STOP and report.

## Verification

- **Mechanical**: `npx vitest run` green; `npx tsc --noEmit`; `npx biome ci .`; `npx vitest run src/overlayCardMirror.test.ts` still green (CSS chunk edits).
- **Feel check**: simulate HT/FT via the football test path — the pill must morph colours over ~260ms and the dot fade, not blink out; push a goal — the changed score digit rolls up while the other side holds still; play music, skip a track — the bar resets instantly (no leftward glide); watch a snow card for 30s — flakes visibly sway, loop has no snap; news card for 30s — the shade wanders (no straight retrace).
- **Done when**: gates green + all five feel checks pass.

## Execution notes (2026-07-27)

- Cited code had NOT drifted; all four sites matched the plan.
- (A) the always-mounted `.live-dot` also collapses (`width: 0` +
  `margin-right: -5px`, cancelling `.chip-live`'s 5px gap) in the
  `.final` rule — without it the finished chip kept a permanent 10px
  phantom hole where the dot used to be. Transitioned alongside the
  opacity, one-shot, on a chip this small.
- (D2) the two snow axes are split across the `translate` (fall) and
  `transform` (sway) PROPERTIES, not two `translate` animations as the
  plan's sketch had them — individual transform properties compose with
  `transform`, whereas two animations on one property just override each
  other (the plan's own CAUTION). The element is a pseudo-element over a
  tiled background, so a nested wrapper wasn't available and
  `background-position` was rejected as main-thread work.
- One line outside the listed files: StatusRailCard.test.tsx's
  "full-time: Final pill" test asserted the dot's ABSENCE, which (A)
  necessarily invalidates — flipped to assert presence, with the
  fade/collapse contract now pinned on the CSS rule in
  LiveMatchScorecard.test.tsx.
- Gates: `npx vitest run` 570/570 green (27 files), `npx tsc --noEmit`
  clean, `npx biome ci .` clean. Feel checks are hardware/manual and
  remain owed.
