# Plan 018: Overlay idle-cost cut — composite the news shader (transform, not background-position)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report. When done,
> update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat ede063a..HEAD -- src/styles.css src/components/StatusRailCard.tsx`
> On any change, compare excerpts below; mismatch = STOP.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW (visual verification needed — the shader change must look identical)
- **Depends on**: none
- **Category**: perf
- **Planned at**: commit `d40445e`, 2026-07-17; **rescoped 2026-07-18 at
  `ede063a`** — the original plan had two halves. Half 1 (lazy-load
  lottie out of the boot chunk) is **moot**: plan 023 (`16a804f`,
  DONE) replaced the goal celebration with pure CSS and deleted
  `lottie-react`, the JSON asset, and `GoalCelebration.tsx` outright —
  a strictly bigger win than the code-split 018 asked for. This file
  now carries only half 2 (the news-shader repaint), with every excerpt,
  line number, and clipping assumption re-verified against `ede063a`.

## Why this matters

One measured idle cost remains in the permanent overlay webview:

**`.news-shade`'s drift animates `background-position`** — not a
compositor-accelerated property, so WebKit repaints the card layer up
to 60 fps for the whole time any news card is visible (with
`rss_enabled`, news cards can occupy the slot most of the time). The
same visual is achievable with a `transform` animation on an oversized
pseudo-element (compositor-only, near-zero paint). As of `ede063a` this
is the **only** `infinite` animation in the stylesheet
(`grep -c infinite src/styles.css` → 1), so it is the whole of the
overlay's steady-state animation cost.

## Current state

`src/styles.css:397-406` — the shader and its keyframes:

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
```

Facts verified at the drift baseline (state them, don't re-derive them):

- `news-shade` is applied **on the same element as `rail-card`**
  (`src/components/StatusRailCard.tsx:53-62` builds one `cardClass`
  list containing both), and `.rail-card` has `overflow: hidden` +
  `border-radius` (`src/styles.css:27-28`). An oversized `::before`
  is therefore clipped by the card's own bounds — no extra overflow
  rule is needed on `.news-shade`.
- `.rail-card { isolation: isolate; }` (`src/styles.css:83`) pins the
  stacking context, and `.news-shade .compact` / `.manifest` sit at
  `z-index: 1` above the pseudo-element (`src/styles.css:408-409`).
  The CSS swap below keeps `z-index: 0` so this ordering is untouched.
- The reduced-motion override lives at `src/styles.css:438-440`:
  `@media (prefers-reduced-motion: reduce) { .news-shade::before { animation: none; } }` —
  keep it exactly as-is; it neutralizes the transform animation the
  same way it neutralized the background-position one.
- No test asserts on these styles (CSS look is manual-verify territory —
  `docs/TESTING_STRATEGY.md` §5). `StatusRailCard.test.tsx` does not
  reference `news-shade`'s animation.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Frontend tests (regression gate) | `npx vitest run` (repo root) | all pass |
| Typecheck (regression gate) | `npx tsc --noEmit` | exit 0 |
| Build | `npx vite build` | exit 0 |

## Scope

**In scope**:
- `src/styles.css` — ONLY the `.news-shade::before` rule, the
  `shade-drift` keyframes, and (if visual bleed appears, which the
  verified clipping facts predict it won't) `.news-shade` itself
- `plans/README.md` (status row)

**Out of scope**:
- `src/components/StatusRailCard.tsx` — no markup or class changes are
  needed; the swap is pure CSS.
- The goal-celebration CSS (`pulse-goal`, confetti burst, ring) — plan
  023's landed territory; it shares the stylesheet but not these rules.
- Every other animation, the settings window, vite config.

## Git workflow

- Current branch; single commit:
  `overlay: news shader drifts via transform, not background-position`
- Do NOT push.

## Steps

### Step 1: Transform-based shader drift

In `src/styles.css`, replace the rule and keyframes at 397-406 — same
gradients, same visual drift range, compositor-only. The pseudo-element
becomes oversized and moves by transform; percentages are relative to
its own (larger) box, so the excursion is small:

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

(`background-size: 200%` is dropped — the oversized box replaces it.
Clipping is already guaranteed by `.rail-card`'s `overflow: hidden`, see
Current state. Keep the `prefers-reduced-motion` override at 438-440
exactly as-is.)

**Verify**: `npx vite build` → exit 0; `npx vitest run` → all pass;
`npx tsc --noEmit` → exit 0 (the latter two are regression gates — this
step must not have touched anything they check).

### Step 2: Visual check (dev machine, `rss_enabled = true`)

`npm run tauri dev`, wait for a news card: the category shader still
drifts subtly, stays inside the card's rounded bounds, and the compact +
manifest content still renders above it (z-order unchanged). If a Web
Inspector is handy, Rendering → "Paint flashing" should show no repaints
from the drift (transform animates without paint).

**Verify**: operator/dev-machine eyeball; report done or handed off.

## Test plan

No new tests (visual/animation is manual-only by recorded design —
`docs/TESTING_STRATEGY.md` §5). Existing suite must stay green,
untouched — no test references these styles.

## Done criteria

- [ ] `grep -c "background-position" src/styles.css` → 0 (baseline
      today: 1 — the single `shade-drift` keyframes line)
- [ ] `grep -c "translate3d" src/styles.css` → 2 (the two keyframe stops)
- [ ] `npx vitest run`, `npx tsc --noEmit`, `npx vite build` all exit 0
- [ ] No files outside the in-scope list modified (`git status`)
- [ ] Visual check reported (done or handed to operator)
- [ ] `plans/README.md` status row updated

## STOP conditions

- The excerpts at `src/styles.css:397-406` don't match Current state
  (the stylesheet drifted again — concurrent sessions land daily here;
  re-verify before improvising).
- The shader visually escapes the card's rounded bounds in Step 2
  despite `.rail-card`'s `overflow: hidden` — report what you see
  rather than adding containment rules beyond the in-scope list.
- A second `infinite` animation has appeared in `src/styles.css`
  (baseline: exactly 1) — the premise "this is the whole idle cost"
  has drifted; do the swap anyway, but report the new animation so it
  gets its own look.

## Maintenance notes

- The `-8%/-6%` transform excursion approximates the old
  `60%/40% background-position` drift on a 200% canvas; if the look
  drifted too far from the original, tune the keyframe values — the
  compositor-only property is the requirement, the numbers are taste.
- Rule for future stylists: idle (`infinite`) animations in the overlay
  must animate compositor-only properties (`transform`, `opacity`).
  Reviewers should flag any keyframes touching `background-*`,
  `box-shadow`, `filter`, or layout properties on a looping animation.
- History: this plan originally also covered lazy-loading lottie-web
  out of the boot chunk; plan 023 deleted lottie entirely (`16a804f`),
  which is why no chunk-size work appears here anymore.
