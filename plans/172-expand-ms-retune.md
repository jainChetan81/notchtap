# 172 — EXPAND_MS feel-check and retune (320ms → 300ms)

- **Status**: DONE (2026-08-02) — real headless frame-sample taken
  (technique of plans 156/163: throwaway harness mounting the actual
  `StatusRailCard` with the actual settings-preview fixtures, Playwright
  chromium, per-rAF `getBoundingClientRect()` sampling). Decision: 300ms.
  `EXPAND_MS` and every CSS `var(--expand-ms, ...)` fallback moved
  together. `npx vitest run` 625/625 at this commit, `npx tsc --noEmit`
  clean. (A later commit on this branch, 17d3933, adds one more guard
  test — the branch tip is 626/626; see plan 174's Status.)
- **Severity**: LOW
- **Category**: Animation-review follow-up (the last open duration
  finding from the /review-animations pass)
- **Prerequisite that made this measurable at all**: plan 163 (merged)
  fixed `--ease-notchtap` never resolving in the overlay bundle, so
  before it, nobody had ever actually felt/measured this curve play.
  Plan 174 (the shared-ui 0.4.0 motion-trio adoption) landed FIRST on
  this branch on purpose, so the numbers below describe the curve the
  app actually ships — `cubic-bezier(0.23, 1, 0.32, 1)`.

## The question

`EXPAND_MS = 320` (animationTiming.ts) drives the shell's promotion
width-grow, the manifest disclosure's expand/collapse, the masthead
min-height, a border-radius leg, and an opacity leg — all via the
injected `--expand-ms`. The animation review's UI-duration ceiling is
300ms; 320 sits above it. Is that a real feel problem, or nominal-value
pedantry that the strong ease-out makes moot?

## Measurement

Harness: `research/plan172-harness/` (gitignored research dir —
`index.html` + `harness.tsx` mounting the real component with the real
entry import order and `applyAnimationTiming()`, `measure.mjs` driving
headless chromium via a vite dev server). Per-frame width/height of
`.card-assembly`; `tXX` = ms from state-flip to XX% of the pixel delta;
`settled` = first frame within 0.5px of the end value that stays there.
~60fps throughout (worst frame gap 17-22ms; one 56ms hiccup in C).

| scenario | Δpx (width) | t50 | t90 | t99 | settled |
| --- | --- | --- | --- | --- | --- |
| A promotion empty→expanded, stock 320 | 130 | 62 | 95 | 95 | (contaminated, see below) |
| B promotion empty→compact, stock 320 | 30 | 50 | 100 | 134 | 250 |
| C expand toggle on a showing card, stock 320 | 100 | 65 | 131 | 231 | **248** |
| D same as A at `--expand-ms: 300` | 130 | 54 | 87 | 103 | (contaminated) |
| E same as A at `--expand-ms: 260` | 130 | 43 | 76 | 92 | (contaminated) |
| F repeat of A (stability) | 130 | 64 | 97 | 97 | (contaminated) |

Honesty notes, checked rather than hand-waved:

- A/D/E/F used fixture 0 ("GOAL", `signal: "goal"`), whose celebration
  pulse keeps nudging the bounding rect sub-pixel long after the width
  transition ends — their `settled` values (~845ms) measure the
  celebration, not the expand, so they're excluded. B (compact news, no
  celebration) and C (same-id expanded toggle — a same-id update never
  replays a celebration, pinned by plan 127's tests) are the clean
  reads. A/D/E remain valid relative to each other in the early window
  (same contamination), which is all the 320-vs-300-vs-260 comparison
  needs.
- A reads faster than C for a similar delta because the base
  `.card-assembly` width transition rides `--ease-notchtap-pop` (the
  overshoot variant — see styles.css's plan-163 FEEL-CHECK note), while
  the expanded-state legs ride the plain house curve.

## The numbers say

1. **Perceived completion is ~250ms at nominal 320.** The strong
   ease-out front-loads motion so hard that 90% of the pixels have
   moved by ~95-131ms and the gesture is visually settled ~80ms before
   the nominal duration even elapses. The review ceiling — which is
   about how long a UI response *feels* — was never actually being
   violated in feel terms.
2. **320 → 300 is imperceptible.** t90 shifts 95→87ms: less than one
   frame at 60fps. 260 starts to visibly clip the tail (t90 76ms) —
   sharper than the house character, rejected.

## Decision: 300ms

Not because 320 felt slow (it measurably doesn't), but because the
change is free and buys cohesion: 300ms is exactly shared-ui's
`--duration-normal` token, it closes the review's last open duration
finding without argument, and no consumer can feel the difference
(point 2). One constant edit + the CSS fallback literals
(`var(--expand-ms, 320ms)` → `300ms` in card-chrome.css ×5,
manifest.css ×2, masthead-content.css ×1, and the two comment echoes)
in the same commit, per the fallback-tracks-the-constant discipline.
Tests reference `EXPAND_MS` symbolically — zero test edits needed,
which is itself the plan-117 single-sourcing working as designed.

Execution note (honesty record, from this branch's own review round):
the fallback sed in the landing commit also rewrote one HISTORICAL
quote in card-chrome.css's wave-C note into nonsense and missed three
prose echoes of the old value (two present-tense card-chrome comments,
useExitChoreography's rebound estimate) — all repaired by the
follow-up truth-up commit on this same branch before the PR opened.

## Boundaries

- The `--ease-notchtap-pop` overshoot on the base width transition is
  character, not a bug — untouched.
- `--duration-fast`/`--duration-slow` adoption (mapping the whole
  animationTiming.ts constant set onto shared-ui duration tokens) is a
  separate, larger cohesion question — not opened here.
- The real-hardware notch-mode feel check remains manual-checklist
  territory (CLAUDE.md: notch behaviour verifies on the macbook, this
  dev box has no notch). Nothing here changes geometry, only 20ms of
  coast time.
