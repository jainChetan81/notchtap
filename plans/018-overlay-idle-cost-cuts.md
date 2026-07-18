# Plan 018: Overlay idle-cost cuts — lazy-load the goal celebration, composite the news shader

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report. When done,
> update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat b43a7ca..HEAD -- src/components/GoalCelebration.tsx src/components/StatusRailCard.tsx src/styles.css`
> On any change, compare excerpts below; mismatch = STOP.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW (visual verification needed — the shader change must look identical)
- **Depends on**: none. NOTE: plan 023 (goal celebration redesign) touches
  the same component — execute 023 FIRST if both are planned for the same
  session, then this plan's Step 1 wraps whatever 023 produced.
- **Category**: perf
- **Planned at**: commit `d40445e`, 2026-07-17; drift baseline refreshed
  to `b43a7ca` 2026-07-18 (excerpts re-verified unchanged);
  **review-plan pass 2026-07-18**: code excerpts and CSS all re-verified
  unchanged (zero drift `b43a7ca..HEAD` on the three in-scope files), but
  the chunk-layout numbers in "Why this matters" / Step 1 were written
  against a layout that no longer exists — today's two-entry build puts
  lottie in a shared 679 KB `StatusRailCard-*.js` chunk loaded by BOTH
  windows, with `main-*.js` a 2.6 KB stub. Premise and remedy unchanged
  (the boot cost is real, and bigger than stated); all measurement
  commands rewritten against the real layout. Also pinned the exact test
  that needs async adjustment (one, not "check first").

## Why this matters

Two measured idle costs in the permanent overlay webview:

1. **lottie-web is statically bundled into the boot-loaded chunk** —
   re-measured 2026-07-18 against a fresh `dist/`: the two-entry build
   (`index.html` + `settings.html`, see `vite.config.ts` rollupOptions)
   hoists shared code into a chunk rollup names
   `StatusRailCard-*.js` (679,093 bytes), which contains the bodymovin
   markers and which BOTH `dist/index.html` and `dist/settings.html`
   load at startup (`main-*.js` itself is only ~2.6 KB — do not measure
   it). So lottie is parsed at every overlay boot AND every settings
   open, resident forever, to support an animation that plays only when
   `signal === "goal"`. `React.lazy` splits it into an async chunk
   loaded on the first goal — both windows stop paying it at boot.
2. **`.news-shade`'s drift animates `background-position`** — not a
   compositor-accelerated property, so WebKit repaints the card layer up
   to 60 fps for the whole time any news card is visible (with
   `rss_enabled`, news cards can occupy the slot most of the time). The
   same visual is achievable with a `transform` animation on an oversized
   pseudo-element (compositor-only, near-zero paint).

## Current state

`src/components/GoalCelebration.tsx:1-12`:

```tsx
import Lottie from "lottie-react";
// Self-authored, bundled locally — never fetched from a CDN at runtime …
import goalCelebration from "../assets/lottie/goal-celebration.json";

// Mounted by <StatusRailCard> only while signal === "goal" …
export function GoalCelebration() {
```

`src/components/StatusRailCard.tsx` imports it at top-level (find the
import + the conditional mount site with
`rg -n "GoalCelebration" src/components/StatusRailCard.tsx`).

`src/styles.css:348-357` + the reduced-motion override at ~388-390:

```css
.news-shade::before {
  content: ""; position: absolute; inset: 0; z-index: 0;
  background:
    radial-gradient(120% 140% at 0% 0%, var(--cat-deep), transparent 52%),
    linear-gradient(135deg, var(--cat-deep) 0%, transparent 46%);
  background-size: 200% 200%;
  animation: shade-drift 12s ease-in-out infinite alternate;
  pointer-events: none;
}
@keyframes shade-drift { from { background-position: 0% 0%; } to { background-position: 60% 40%; } }
…
@media (prefers-reduced-motion: reduce) {
  .news-shade::before { animation: none; }
}
```

`.news-shade .compact` / `.manifest` sit at `z-index: 1` above the
pseudo-element; the card has `isolation: isolate` (added by the goal-burst
z-index fix — verify with `rg -n "isolation" src/styles.css`).

Test context: `src/components/StatusRailCard.test.tsx` (14 tests).
Checked 2026-07-18 — exactly ONE test breaks under lazy loading:
`it("applies pulse-goal and mounts the celebration on a goal signal")`
(~line 92) asserts `container.querySelector(".goal-celebration")`
synchronously right after `render()`, which returns null while the lazy
chunk resolves — convert that assertion to an async wait
(`await waitFor(...)` or `await screen.findBy...`-style, matching the
file's existing testing-library idiom). The negative assertion in
`it("applies pulse-red (without mounting the goal celebration) …")`
(~line 109, expects `.goal-celebration` to be null) stays valid as-is —
the lazy component is never rendered on that path. The two
animation-end tests don't touch the celebration.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Frontend tests | `npx vitest run` (repo root) | all pass |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Build + chunk check | `npx vite build` | exit 0; see Step 1 verify |

## Scope

**In scope**:
- `src/components/StatusRailCard.tsx` (lazy import + Suspense wrapper)
- `src/components/GoalCelebration.tsx` (default export if lazy needs it)
- `src/styles.css` (`.news-shade::before` + keyframes + reduced-motion
  block only)
- `src/components/StatusRailCard.test.tsx` (only if the async mount
  requires findBy adjustments)
- `plans/README.md` (status row)

**Out of scope**:
- The lottie JSON asset, the pulse/burst CSS (plan 023's territory), any
  other animation, the settings window, `index.html`/vite config
  (code-splitting via dynamic import needs no config).

## Git workflow

- Current branch; commits:
  1. `overlay: lazy-load GoalCelebration — lottie-web out of the boot chunk`
  2. `overlay: news shader drifts via transform, not background-position`
- Do NOT push.

## Steps

### Step 1: Code-split the celebration

In `GoalCelebration.tsx`, add a default export (`export default
GoalCelebration;` — keep the named export for tests). In
`StatusRailCard.tsx`:

```tsx
import { lazy, Suspense } from "react";
const GoalCelebration = lazy(() => import("./GoalCelebration"));
```

and wrap the existing conditional mount site:

```tsx
<Suspense fallback={null}>
  <GoalCelebration />
</Suspense>
```

(only rendered when the existing goal condition holds — the dynamic import
therefore resolves on the first goal, from local disk, no network. The CSS
burst/overshoot on the card fires instantly regardless; a few frames of
lottie delay on the very first goal is the accepted trade.)

**Verify**: `npx vitest run` → all pass (the one async-adjusted test —
see Current state). Then `npx vite build` and check the split (do NOT
measure `main-*.js` — it's a ~2.6 KB entry stub; the boot-loaded weight
lives in the shared `StatusRailCard-*.js` chunk):
1. `for f in dist/assets/*.js; do printf "%s: " "$f"; grep -c bodymovin "$f" || true; done`
   → bodymovin markers appear in exactly ONE chunk, a NEW one (rollup
   will name it `GoalCelebration-*.js` or similar), and that chunk is
   referenced by NEITHER `dist/index.html` nor `dist/settings.html`
   (`grep -o 'assets/[^"]*\.js' dist/index.html dist/settings.html` must
   not list it — it loads only via dynamic import).
2. `wc -c dist/assets/StatusRailCard-*.js` (or whatever the shared chunk
   is now named) → expect roughly 250–300 KB smaller than the current
   679,093 bytes. Quote before/after bytes in your report.

### Step 2: Transform-based shader drift

Replace the animated properties — same gradients, same visual drift
range, compositor-only. The pseudo-element becomes oversized and moves by
transform; percentages are relative to its own (larger) box, so keep the
excursion small and re-eyeball:

```css
.news-shade::before {
  content: ""; position: absolute; inset: -30% -30% -30% -30%; z-index: 0;
  background:
    radial-gradient(120% 140% at 0% 0%, var(--cat-deep), transparent 52%),
    linear-gradient(135deg, var(--cat-deep) 0%, transparent 46%);
  animation: shade-drift 12s ease-in-out infinite alternate;
  pointer-events: none;
  will-change: transform;
}
@keyframes shade-drift {
  from { transform: translate3d(0, 0, 0); }
  to   { transform: translate3d(-8%, -6%, 0); }
}
```

(`background-size: 200%` is dropped — the oversized box replaces it. The
card's `overflow: hidden` / border-radius clipping must still contain the
pseudo-element: check the `.rail-card` rules for `overflow` and add
`overflow: hidden` to `.news-shade` only if the oversized layer bleeds —
inspect visually in Step 3.) Keep the `prefers-reduced-motion` override
exactly as-is (`animation: none`).

**Verify**: `npx vite build` → exit 0. No test asserts on these styles (CSS is manual-verify territory in this repo).

### Step 3: Visual check (dev machine, `rss_enabled = true`)

`npm run tauri dev`, wait for a news card: the category shader still
drifts subtly, stays inside the card's rounded bounds, and the compact +
manifest content still renders above it (z-order unchanged). If a Web
Inspector is handy, Rendering → "Paint flashing" should show no repaints
from the drift (transform animates without paint).

**Verify**: operator/dev-machine eyeball; report done or handed off.

## Test plan

No new tests (visual/animation is manual-only by recorded design —
`docs/TESTING_STRATEGY.md` §5). Existing StatusRailCard suite must stay
green, async-adjusted at most.

## Done criteria

- [ ] `grep -c 'lazy(() => import' src/components/StatusRailCard.tsx` → 1
      (baseline today: 0)
- [ ] `grep -c "background-position" src/styles.css` → 0 (baseline
      today: 1 — the single `shade-drift` keyframes line)
- [ ] `npx vitest run`, `npx tsc --noEmit`, `npx vite build` all exit 0
- [ ] bodymovin in exactly one non-boot chunk (Step 1's check 1) and the
      shared-chunk shrink reported (before/after bytes vs 679,093)
- [ ] Visual check reported (done or handed to operator)
- [ ] `plans/README.md` status row updated

## STOP conditions

- After Step 1, bodymovin markers still appear in a chunk that
  `dist/index.html` or `dist/settings.html` references (rollup pulled
  lottie back into the shared chunk) — report the chunk layout; the fix
  may need the JSON import moved inside the lazy module (it already is)
  or vite `manualChunks`, which is out of scope.
- A StatusRailCard test asserts synchronous celebration mount in a way
  `findBy*` can't fix cleanly — report rather than weakening the test.
- Plan 023 landed first and restructured the celebration into pure CSS
  (no lottie at all) — Step 1 may be moot; verify and mark accordingly.

## Maintenance notes

- Anyone adding a second lottie animation should follow the same lazy
  pattern — lottie-web must never return to the boot chunk (reviewers:
  check `dist/assets/main-*` size on animation PRs).
- The `-8%/-6%` transform excursion approximates the old
  `60%/40% background-position` drift on a 200% canvas; if the look
  drifted too far from the original, tune the keyframe values — the
  compositor-only property is the requirement, the numbers are taste.
