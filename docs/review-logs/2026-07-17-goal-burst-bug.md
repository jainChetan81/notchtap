# bug: goal celebration burst + confetti never visible (2026-07-17, live smoke)

## symptom

`./notchtap --title "GOAL" --body "..." --priority high --signal goal`
shows the coral H3 card correctly (stamp, tier, expand) but **no radial
burst, no scale overshoot, no lottie confetti** — "just the card".
Observed live on the mac mini (hud mode), dev build, 2026-07-17 evening.
Red-card strobe not visually confirmed either (user saw the card only).

## verified during the session

- signal reaches the frontend end-to-end: wire carries `signal: "goal"`
  (rust tests assert SlotState serialization; card rendered with the
  signal-driven stamp), so this is presentation-layer only.
- one real bug WAS found and fixed (kept, commit alongside this note):
  `.rail-card::after` (burst) and `.goal-celebration` (lottie) sat at
  `z-index: -1` inside a card with an **opaque background** — negative-z
  layers paint behind the parent's background: rendered, invisible. Fixed
  to `z-index: 0` + `isolation: isolate` on `.rail-card`
  (src/styles.css). the news `.news-shade::before` at z-index 0 was the
  working counter-example.
- the fix was hot-reloaded via vite HMR and re-tested: **still no burst**
  — so either HMR didn't actually apply to the webview, or a second
  suppressor exists.

## ranked hypotheses (next session works this list top-down)

~~1. macOS "Reduce Motion" enabled~~ — **eliminated same session**:
`defaults read com.apple.universalaccess reduceMotion` → not set (off).

0. **the moment is under-designed even when it works.** verified same
   session: `goal-celebration.json` is valid lottie but 6 shape
   particles, 120×120, 24 frames @ 30fps = **0.8s** — near-invisible on
   a dark 400px card even with correct stacking. also clarified: the
   confetti never existed in any browser prototype (only in the app),
   so "never saw it in the prototype" is expected, not evidence of a
   second bug. likely resolution is a redesign, not just a fix:
   bigger/longer particle field (or drop lottie for the css burst
   alone, scaled up), and only then re-judge.
1. **HMR never applied the z-index fix** — dev webview may need a full
   reload for pseudo-element changes. restart `npm run tauri dev` and
   re-test before trusting any css conclusion from the live session.
2. **pulse class not applied at all** — verify in devtools that the card
   element gains `pulse-goal` for 620ms on a goal push (StatusRailCard
   keys the effect on `[currentId, currentSignal]`).
3. **lottie autoplay/mount in dev** — confirm `GoalCelebration` mounts
   and plays (react devtools / console log).
4. paint-order edge: `::after` on an `overflow: hidden` card with
   `isolation` — if all above pass, screenshot computed styles/stacking
   in devtools before touching css again.

## how to debug the overlay webview

right-click the overlay → Inspect (devtools enabled in dev builds), or
`npm run tauri dev` then the WebKit inspector via Safari → Develop →
the notchtap webview.

## acceptance for closing this

on a machine with reduce-motion OFF: goal push shows burst + overshoot
+ confetti once, ~620ms, never looping; red card shows the two-pulse
strobe; with reduce-motion ON: whatever fallback the product decision
picks (including deliberately nothing) is documented in
`docs/ARCHITECTURE.md`.

## resolution (2026-07-18, plan 023 — CSS-first redesign)

Worked hypothesis 0 ("under-designed even when it works") as the primary
fix rather than chasing the invisible-lottie suppressor, because the
CSS-first direction **eliminates the whole failure class** the ranked
list was probing:

- **Root cause of "nothing visible" was over-determined, not a single
  bug.** The z-index fix (`5277c55`) was real and necessary, but on top
  of it the lottie layer added its own uncertain mount/autoplay path
  (hypotheses 2–4), and the asset itself was too small/short to read on
  a dark 400px card even when correctly stacked (hypothesis 0). Any one
  of these could leave "just the card". Rather than bisect them live on
  the dev machine, the redesign removes them all at once.
- **Fix: dropped lottie entirely; the celebration is now pure CSS.** The
  confetti is a layered-radial-gradient spray on `.rail-card::after`
  (warm core + six offset colour sparks + outer coral glow, `inset:
  -60px`), an expanding hoop on `.rail-card.pulse-goal::before`, and the
  existing scale overshoot on the card — all driven by the `pulse-goal`
  class the `StatusRailCard` state machine already applies for ~620ms,
  play-once, keyed on `signal === "goal"` only. No element mounts, so
  there is no mount/autoplay path left to fail; no negative z-index, so
  the opaque-background trap can't recur.
- **Removed:** `lottie-react` (dependency + lockfile), the
  `goal-celebration.json` asset, and `GoalCelebration.tsx`. This also
  makes plan 018's lazy-lottie step moot (noted in `plans/README.md`).
- **Red-card strobe** (`pulse-red`) is untouched and still fires; a
  signal-less High-priority push (e.g. cmux "needs input") plays
  neither pulse — covered by `StatusRailCard.test.tsx`.
- **Reduce-motion:** the existing `@media (prefers-reduced-motion:
  reduce)` block now also covers the new `::before` ring; the decision —
  reduce-motion ⇒ *deliberately nothing* (card still renders + announces)
  — is recorded in `docs/ARCHITECTURE.md` §4.
- **Gates:** `npx vitest run` (64 pass), `npx tsc --noEmit`, and
  `npx vite build` all green.

**Still owed by the operator (headless executor cannot do these):** the
live cold-start eyeball on the mac mini per the acceptance script above —
goal push shows an unmissable ~620ms burst + overshoot + ring; red-card
push shows the strobe; signal-less push shows neither — plus the macbook
notch-mode check on the standing hardware checklist.

One thing to judge in that eyeball (raised by the post-merge code
review): the `::after` confetti paints *above* the card content
(non-news content has no `z-index`, so the last-painted pseudo wins), and
the redesigned spray has a near-opaque warm core (`rgba(255,224,130,0.95)`
at the burst peak) — so the GOAL title/body may wash out for the ~200ms
peak. If it reads badly, the smallest fix is to lift the content above
the burst during the pulse (mirror the news pattern:
`.rail-card.pulse-goal .compact, .rail-card.pulse-goal .manifest {
position: relative; z-index: 1; }`), which leaves the confetti blooming
behind readable text. A cheap follow-up noticed alongside it: the
seven-gradient `background` lives on the base `.rail-card::after`, so
every card rasterizes it at `opacity: 0`; moving it onto
`.rail-card.pulse-goal::after` confines the cost to actual goal moments.
