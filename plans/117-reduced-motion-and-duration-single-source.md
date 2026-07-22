# Plan 117: Single-source reduced-motion + coupled animation durations (overlay)

> **Executor instructions**: Follow step by step; run every verification. STOP on
> any STOP condition. Do NOT push/merge/PR. Do NOT edit `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 2c79e36..HEAD -- src/components/TtlBar.tsx src/components/IdleHoverPeek.tsx src/components/StatusRailCard.tsx src/useDelayedSwap.ts src/overlay-card.css`

## Status

- **Priority**: P2
- **Effort**: S–M
- **Risk**: MED (touches the load-bearing overlay animation timing)
- **Depends on**: none. Independent of 114/115/116. (If 114 also lands, both touch `overlay-card.css` but different lines — coordinate merge order; no logical dependency.)
- **Category**: correctness / tech-debt
- **Planned at**: commit `2c79e36`, 2026-07-22

## Why this matters

Two correctness-adjacent duplications live in the overlay animation code:

1. **Reduced-motion is checked in 3 places, byte-identically**, with no shared
   definition. A fourth animated component is one copy-paste away from silently
   omitting the check — an accessibility regression no test would catch by
   construction.
2. **Animation durations are duplicated across CSS ↔ JS (↔ rust)**: the 220ms
   content-swap window is authored both in `overlay-card.css` and as the literal
   `220` passed to `useDelayedSwap(...)`; the 260ms idle-peek close lives in CSS
   and as `CLOSE_DELAY_MS = 260`. The JS timer and the CSS animation must stay
   equal or the content swaps before/after the visual exit finishes (a flash or a
   premature snap — exactly the "grows before shrinking" race the plan-105/107
   comments describe). Today they're kept in lockstep only by memory.

This plan single-sources both without touching the visual art itself.

## Current state

- **Reduced-motion JS checks** (3 byte-identical inline expressions
  `typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches`):
  - `src/components/TtlBar.tsx:66-68`
  - `src/components/IdleHoverPeek.tsx:131-134`
  - `src/components/IdleHoverPeek.tsx:228-230`
  The CSS half is 9 `@media (prefers-reduced-motion: reduce)` blocks in
  `overlay-card.css` (`:457,484,539,810,843,1133,1177,1470,1832`) — **leave those**;
  they are the correct layer for the pure-CSS art. This plan only unifies the JS half.
- **220ms swap**: authored in `overlay-card.css` (7 occurrences of `220ms`, e.g.
  `:510,513,516,519`) AND as the literal `220` at
  `src/components/StatusRailCard.tsx:190` — `useDelayedSwap(slot, swapKey, 220)`.
  `src/useDelayedSwap.ts` is the freeze-timer hook (self-described stand-in for
  `AnimatePresence mode="wait"`).
- **260ms idle-peek close**: `overlay-card.css:1442,1447` AND
  `src/components/IdleHoverPeek.tsx:48` — `const CLOSE_DELAY_MS = 260;`. The peek's
  100px open height (`overlay-card.css:1456,1459`) is ALSO mirrored in rust
  (`hover.rs`'s `IDLE_PEEK_BELOW_BLOCK_H`, per the comment at `overlay-card.css:1450-1453`)
  — that cross-language pair already carries a lockstep comment; do NOT try to
  unify across the rust boundary, just don't break the existing comment.
- Existing tests to respect: `src/TtlBar.test.tsx`, `src/useDelayedSwap.test.ts`,
  `src/components`/`StatusRailCard.test.tsx` ("compact->idle geometry"),
  `IdleHoverPeek` tests. These pin the current timing behavior.

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Tests | `npx vitest run` | 295 pass |
| Lint | `npx biome ci .` | exit 0 |
| Build | `npx vite build` | exit 0 |

## Scope

**In scope**:
- A new small module, e.g. `src/prefersReducedMotion.ts` (a `usePrefersReducedMotion()` hook and/or a plain `prefersReducedMotion()` helper).
- A new small module for shared durations, e.g. `src/animationTiming.ts` (exported constants `SWAP_EXIT_MS = 220`, `IDLE_PEEK_CLOSE_MS = 260`).
- `src/components/TtlBar.tsx`, `src/components/IdleHoverPeek.tsx`, `src/components/StatusRailCard.tsx`, `src/useDelayedSwap.ts` — consume the new modules.
- `src/overlay-card.css` — OPTIONALLY expose `--swap-ms`/`--peek-close-ms` custom properties and reference them, OR (lower-risk) add a one-line "must equal `SWAP_EXIT_MS` in animationTiming.ts" comment at each of the 4 CSS duration sites. **Prefer the comment approach** unless the CSS→var wiring is clean and testable — do not risk the timing to chase DRY.
- Test files for the new modules + any existing test that hardcodes 220/260.

**Out of scope**:
- The CSS `@media (prefers-reduced-motion)` art blocks — leave them.
- The rust `hover.rs` side of the 100px mirror — do not touch.
- Any actual animation VISUAL (keyframes, easing, art) — this is pure de-duplication of timing/flags.
- `src/settings/**` (settings uses `motion`'s `MotionConfig` for reduced-motion — a different, already-single-sourced mechanism; do not converge them).
- The larger `motion`-orchestration rewrite (that's the deferred An-D spike, explicitly NOT this plan).

## Git workflow

- Worktree from stale base: **FIRST** `git reset --hard 2c79e36`.
- Two commits: (1) reduced-motion helper, (2) duration constants. Conventional-commit style (e.g. `refactor(overlay): single-source prefers-reduced-motion check (plan 117)`).

## Steps

### Step 0: Base-sync + baseline
`git reset --hard 2c79e36`; confirm the 3 reduced-motion sites, `useDelayedSwap(...,220)` at StatusRailCard.tsx:190, `CLOSE_DELAY_MS = 260`, and the CSS `220ms`/`260ms` occurrences. Baseline gates green. Mismatch → STOP.

### Step 1: Single-source the reduced-motion check
Create `src/prefersReducedMotion.ts` exporting a `usePrefersReducedMotion()` React
hook (subscribes to the media query and returns a boolean; SSR/jsdom-safe — guard
`typeof window.matchMedia === "function"`). Replace the 3 inline checks in
`TtlBar.tsx` and `IdleHoverPeek.tsx` (×2) with the hook. Preserve exact current
behavior: where the current code reads the boolean once at a specific point,
ensure the hook's value is available at that point (a hook returning the live
value is fine; if a call site needs the value inside a non-render callback, also
export a plain `prefersReducedMotion()` function reading `matchMedia` on demand and
use that there). Do NOT change WHEN or HOW each component reacts — only where the
boolean comes from.

**Verify**: `grep -rc 'prefers-reduced-motion: reduce")' src/components/` → 0 (no inline JS checks remain in components); the media string now lives only in `prefersReducedMotion.ts` and the CSS art blocks. `npx vitest run` → all existing TtlBar/IdleHoverPeek reduced-motion tests pass unchanged. Add unit tests for the new hook/helper (matches true/false per matchMedia mock).

### Step 2: Single-source the coupled durations (JS side)
Create `src/animationTiming.ts` with `export const SWAP_EXIT_MS = 220;` and
`export const IDLE_PEEK_CLOSE_MS = 260;` (documented as "must equal the CSS
`220ms`/`260ms` in overlay-card.css"). Replace the literal `220` at
`StatusRailCard.tsx:190` with `SWAP_EXIT_MS`, and `CLOSE_DELAY_MS = 260` in
`IdleHoverPeek.tsx` with `IDLE_PEEK_CLOSE_MS` (or keep the local name assigned from
the import). Update any existing test that hardcodes 220/260 to import the constant
(allowed focused edit) so the coupling is asserted in one place.

**Verify**: `grep -rn '\b220\b' src/components/StatusRailCard.tsx` → the literal is gone (now the imported constant); `grep -rn 'CLOSE_DELAY_MS = 260' src/components/IdleHoverPeek.tsx` → gone. `npx vitest run` → the useDelayedSwap / StatusRailCard "compact->idle geometry" / IdleHoverPeek timing tests still pass.

### Step 3: CSS-side coupling note (low-risk)
At each of the 4 CSS duration sites (`overlay-card.css:510,513,516,519` for 220ms
and `:1442,1447` for 260ms — confirm), add a one-line comment `/* must equal
SWAP_EXIT_MS (animationTiming.ts) */` (or peek equivalent). Do NOT convert to
`var(--swap-ms)` unless you can prove via the preview-equivalence harness that the
computed animation timing is byte-identical AND a test guards it — the comment is
the safe default; the var-wiring is optional and only if clearly clean.

**Verify**: preview-equivalence harness → zero delta (comments don't change rendering; if you did the optional var-wiring, prove zero delta). Build clean.

### Step 4: Final gates
`npx vitest run` (295 + new hook/timing tests), `npx tsc --noEmit` (0), `npx biome ci .` (0), `npx vite build` (0). Scope diff limited to in-scope files.

## Test plan

- New unit tests: `src/prefersReducedMotion.test.ts(x)` — hook/helper returns the
  matchMedia result, handles the no-matchMedia (jsdom) path. Model after an
  existing hook test (e.g. `src/useDelayedSwap.test.ts` or the slot-state hook tests).
- The existing TtlBar/IdleHoverPeek/StatusRailCard/useDelayedSwap tests must stay
  green with NO behavioral change — that's the proof this is a pure refactor.
- Repoint any existing test's hardcoded 220/260 to the new constant.

## Done criteria

- [ ] Exactly one JS definition of the `(prefers-reduced-motion: reduce)` query (in `prefersReducedMotion.ts`); zero inline copies in components
- [ ] `SWAP_EXIT_MS`/`IDLE_PEEK_CLOSE_MS` constants exist and are consumed at the JS sites; no bare 220/260 timing literals remain in those components
- [ ] CSS duration sites carry a "must equal <constant>" comment (or proven-equivalent var wiring)
- [ ] New hook/helper tests pass; all existing overlay-animation tests pass unchanged (behavior identical)
- [ ] vitest 295+ / tsc 0 / biome 0 / vite build 0; scope diff limited to in-scope files
- [ ] No change to CSS `@media (prefers-reduced-motion)` art blocks or the rust `hover.rs` mirror

## STOP conditions

- Base-sync/Step-0 evidence doesn't match (drift since planning).
- Any existing overlay-animation test changes behavior (not just import source) —
  that means the refactor altered timing; STOP and report rather than editing the
  test to pass.
- The optional CSS `var(--swap-ms)` wiring produces ANY preview-equivalence delta —
  revert to the comment approach.
- A gate fails twice after a reasonable fix.

## Maintenance notes

- After this, changing the swap/close feel is a one-line constant edit on the JS
  side; the CSS sites carry a pointer so they're changed in lockstep (or read the
  same var if you did the optional wiring).
- This deliberately does NOT attempt the `motion`-orchestration rewrite (the An-D
  spike) — that fights plan 078's decision to keep the overlay CSS-driven and
  carries HIGH regression risk. Keep them separate.
- Reviewer: the whole point is behavior-identical de-duplication — scrutinize that
  no existing timing/geometry test's ASSERTION changed, only its import source.
