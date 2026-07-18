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
- **Planned at**: commit `d40445e`, 2026-07-17; drift baseline refreshed to `b43a7ca` 2026-07-18 (excerpts re-verified unchanged)

## Why this matters

Two measured idle costs in the permanent overlay webview:

1. **lottie-web is statically bundled into the overlay's entry chunk**
   (~250 KB of the 330 KB main chunk — verified by finding bodymovin
   markers in `dist/assets/main-*.js`), parsed at every app boot and
   resident forever, to support an animation that plays only when
   `signal === "goal"`. `React.lazy` defers the cost to the first goal.
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

`src/styles.css:340-349` + the reduced-motion override at 383-385:

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

Test context: `src/components/StatusRailCard.test.tsx` (14 tests) renders
the card incl. the goal-signal path — read it before Step 1 to see how the
goal state is driven; lazy-loading must not break those tests
(vitest + Suspense: the test may need `await screen.findBy...` instead of
sync queries for the lottie mount — only if a test actually asserts on the
celebration's presence; check first).

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

**Verify**: `npx vitest run` → all pass (adjust to `findBy*` only where a
test asserts the celebration's mount — see Current state). Then
`npx vite build` and check the split:
`ls dist/assets/ | grep -i goal` (or a new chunk appears) AND the main
chunk shrank: `wc -c dist/assets/main-*.js` — expect roughly 250 KB
smaller than 330 KB. Quote both numbers in your report.

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
- [ ] `grep -c "background-position" src/styles.css` → 0
- [ ] `npx vitest run`, `npx tsc --noEmit`, `npx vite build` all exit 0
- [ ] Main chunk size reduction reported (before/after bytes)
- [ ] Visual check reported (done or handed to operator)
- [ ] `plans/README.md` status row updated

## STOP conditions

- The main chunk does NOT shrink after Step 1 (lottie got pulled into the
  shared chunk anyway) — report the chunk layout; the fix may need the
  JSON import moved inside the lazy module (it already is) or vite
  `manualChunks`, which is out of scope.
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
