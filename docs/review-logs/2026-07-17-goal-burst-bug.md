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
