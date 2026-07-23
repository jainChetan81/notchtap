# Plan 125: idle face — kill the 24/7 idle cost, align the character motion

> Filed 2026-07-23 from the /improve-animations audit (findings #1 HIGH
> perf, #11 LOW character). Authored at master `304d078`. Scope: ONE
> file — `src/components/IdleFace.tsx`. Do not touch overlay-card.css
> (plan 127 owns it) or any other file except the §0 frontend count
> line in docs/TESTING_STRATEGY.md and IdleFace's own test file if one
> exists (check; add `src/components/IdleFace.test.tsx` if absent).

## Why (verified)

The idle face mounts 4.5s into every idle on the Mac mini
(`idleFaceEligible = presentationFacts().mode !== "notch"`) and then:
- `useGazeCycle` (`IdleFace.tsx:40-60`) re-arms a `setTimeout` every
  `randomBetween(1500, 2500)` ms FOREVER, each firing `setState` →
  React re-render → a motion spring on the eyes.
- `useBlink` (`:64-94`) adds more of the same every 3–6s.
- The eyes animate via `motion` props `x`/`y`/`scaleY` with
  `{ type: "spring", stiffness: 140, damping: 16 }` (`:137`) — a
  main-thread rAF loop per movement, damping ratio ≈0.68 (visibly
  wobblier than the app's house spring, IdleHoverPeek's 480/37 ≈0.84).
- The reveal is `duration: 0.5, ease: "easeOut"`, `scale: 0.85`
  (`:123, :132`) — over the 300ms UI ceiling, built-in curve instead
  of the house curve, and below the 0.9 scale floor.

Net: the overlay's main thread never sleeps longer than ~2.5s, 24/7,
in exactly the "bare notch ≈ zero cost" state. The fix keeps the
face's personality; it changes cost and curve vocabulary only.

## Changes

1. **Sparser wakeups.** `useGazeCycle`: `randomBetween(1500, 2500)` →
   `randomBetween(6000, 11000)`. `useBlink`: `randomBetween(3000,
   6000)` → `randomBetween(6000, 12000)` (keep any double-blink logic
   as-is, just on the sparser cadence). The face still glances and
   blinks — a glance every ~8s reads as calm rather than fidgety, and
   wakeups drop ~4×.
2. **Eyes off the spring, onto self-ending CSS transitions.** Replace
   the eyes' `motion.div` with a plain `div` that sets
   `style={{ transform: \`translate(\${offset.x}px, \${offset.y}px) scaleY(\${blinking ? 0.12 : 1})\` }}`
   and a CSS transition declared inline on the same element:
   `transition: "transform 200ms cubic-bezier(0.22, 1, 0.36, 1)"` —
   import nothing new; use the `NOTCHTAP_EASE` values via the existing
   `animationTiming.ts` export if a string form exists, else write the
   literal WITH a comment naming `NOTCHTAP_EASE` as its source (do not
   add a new export; plan 127 owns animationTiming.ts). Browser-driven,
   composited-eligible, ends after 200ms — no rAF loop, no spring.
   Blink at 0.12 scaleY over the same 200ms reads as a quick blink at
   the sparser cadence; if the blink needs to be snappier, 120ms for
   the blink leg only is acceptable — pick one and comment the choice.
3. **Reveal on the house curve.** `initial={{ opacity: 0, scale:
   0.85 }}` → `scale: 0.92`; `transition={{ duration: 0.5, ease:
   "easeOut" }}` → `{ duration: 0.24, ease: NOTCHTAP_EASE }` (import
   from `../animationTiming` — the array export already exists). Keep
   the 0.1s exit override exactly as-is (it's a documented review fix).
4. **Doc comments.** This file's comments are design records — extend
   them in the same voice: why the cadence is sparse (idle cost), why
   the eyes are CSS transitions not springs (main-thread wakeups), why
   0.92/0.24/notchtap (house vocabulary; audit finding refs).

## Tests

- Existing IdleFace/StatusRailCard tests must stay green unmodified.
- Add (in IdleFace's test file): with fake timers, no gaze/blink state
  change occurs before 6000ms (pins the sparser floor); the eyes
  element carries an inline `transition` containing `transform` and
  `cubic-bezier(0.22, 1, 0.36, 1)`; the reveal's motion props carry
  `scale: 0.92` (string-level or props-level pin, match the repo's
  style of pinning motion values).

## Verification

`npx tsc --noEmit`, `npx vitest run`, `npx biome ci .`, `npx vite
build` — all clean. Update the §0 frontend count line ONLY. Final
feel (glance cadence, blink weight, reveal pop) is operator-verified
on device — flag it in your report.

## STOP conditions

- Any existing test needs weakening.
- The eyes' CSS-transition approach can't reproduce the blink without
  a keyframe loop (do NOT add an infinite CSS animation — that trades
  timer wakeups for a permanently-awake compositor).
