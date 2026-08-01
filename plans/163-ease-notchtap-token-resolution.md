# 163 — Fix `--ease-notchtap` never resolving in the overlay build

- **Status**: DONE (2026-07-31) — plain `--ease-notchtap: cubic-bezier(0.22, 1, 0.36, 1);` redeclared on `:root` in src/styles.css, mirroring `--ease-notchtap-pop`'s existing defense-in-depth pattern. `npx vitest run` 573/573, `npx tsc --noEmit`/`npx biome ci .` clean. Re-verified against a fresh `npx vite build`: the overlay's `main-*.css` bundle now carries `--ease-notchtap` inside a real `:root{...}` rule (previously only the broken, bare unwrapped `@theme` leftover text existed).
- **Commit**: ef91a0f
- **Severity**: CRITICAL (P0 — prerequisite for plans 164–167 and effectively for plan 161)
- **Category**: Interruptibility & timing / cohesion (review-animations skill, "feel-breaking regression")
- **Estimated scope**: 1 file, 1 line

## Problem

`--ease-notchtap` is defined in exactly one place in the whole repo:
`vendor/shared-ui/design/tokens.css`, inside a Tailwind v4 `@theme inline
{ ... }` block:

```css
/* vendor/shared-ui/design/tokens.css */
@theme inline {
  ...
  /* motion — signature notchtap easing + durations */
  --ease-notchtap: cubic-bezier(.22, 1, .36, 1);
  --duration-fast: 220ms;
  --duration-normal: 300ms;
  --duration-slow: 400ms;
}
```

The file's own header comment documents the required consumption contract:

```
 * consumption (desktop, tailwind v4 css-first):
 *   1. in your app's globals.css, import tailwind first, then this file
 *      (tokens second so their :root values win the cascade):
 *        @import "tailwindcss";
 *        @import "@chetanjain/shared-ui/design/tokens.css";
 *      (the @theme inline block below is picked up by the tailwind vite plugin,
 *       so every --color-* token becomes a utility: bg-background, text-muted-foreground, ...)
```

`@theme inline` blocks are only expanded into real `:root` custom properties
by Tailwind v4's Vite plugin for CSS reachable through an `@import
"tailwindcss";` root. The overlay's real entry point, `src/main.tsx`,
imports the token file directly with no such root:

```tsx
// src/main.tsx (current)
import "@chetanjain/shared-ui/design/tokens.css";
import "./overlay-card.css";
import "./styles.css";
```

Neither `overlay-card.css` nor any file it `@import`s (`src/overlay/*.css`)
nor `src/styles.css` ever contains `@import "tailwindcss";` — only
`src/settings/base.css` (the settings window's entry) does. As a result,
in the overlay's real production build, `--ease-notchtap` never becomes a
usable custom property.

**This was independently verified three ways, not just asserted:**

1. `npx vite build` (default config, no code changes) followed by grepping
   the output: `main-*.css` (the overlay entry's own bundle) ships the
   literal text `--color-overlay-fg: var(--overlay-fg); --ease-notchtap:
   cubic-bezier(.22, 1, .36, 1);` as **bare property declarations sitting
   outside any selector block** — invalid CSS, silently dropped by the
   parser. `settings-*.css` (which does route through `@import
   "tailwindcss"`) has the identical token correctly nested inside a real
   rule.
2. Serving that build and loading `index.html` in a real browser, then
   running:
   ```js
   getComputedStyle(document.documentElement).getPropertyValue('--ease-notchtap')
   ```
   returns an empty string (unresolved). The same check for
   `--ease-notchtap-pop` (which is NOT gated behind `@theme` — see Target
   section) correctly returns `cubic-bezier(.3, 1.36, .44, 1)`.
3. On that same live page, `.card-assembly`'s computed style reads:
   `transitionProperty: "all"`, `transitionDuration: "0s"` —
   i.e. completely unset/default. Confirms the CSS Custom Properties spec
   behavior: a `transition`/`animation` shorthand is invalidated in its
   **entirety** the instant any one referenced custom property inside it
   is unresolved with no fallback — not just the specific sub-value that
   references it.

A repo-wide grep confirms the blast radius: **45 usages of
`var(--ease-notchtap)` across 10 files, zero of which have a fallback
value**:

```
src/overlay/manifest.css
src/overlay/card-chrome.css
src/styles.css
src/overlay/live-scorecard.css
src/overlay/choreography.css
src/overlay/status-dots.css
src/overlay/agent-board.css
src/overlay/news-category.css
src/overlay/idle-peek.css
src/overlay/ttl-bar.css
```

Concretely, this currently breaks (among others): the shell's width
transition AND hover "breathe" transform (`card-chrome.css:103-105` — one
`transition` declaration, so the unresolved `transform` leg poisons the
otherwise-fine `width` leg too), the flank reveal/padding fade, the
border-radius round-in, both exit legs (`.exiting`/`.exiting.exit-to-bare`),
the manifest disclosure, the goal/red-card celebration keyframes
(`animation: goal-overshoot 1.24s var(--ease-notchtap)` and siblings), the
Agent Board's dot pulse+breathe (`agent-board.css:219-222` — the same
animation plan 161 targets for reduced-motion coverage), ttl-bar, and
news's `shade-drift`. None of these plain-CSS animations currently play in
the shipped app; only `motion/react`-driven animations (which read the
separate, correctly-working `NOTCHTAP_EASE` numeric array in
`animationTiming.ts`, not this CSS variable) are unaffected.

## Target

Redeclare `--ease-notchtap` as a plain, non-`@theme` custom property
directly on `:root` in `src/styles.css` — mirroring exactly how
`--ease-notchtap-pop` already protects itself in this same file:

```css
/* src/styles.css (current, for reference — do not restructure this,
   only add the new declaration alongside it) */
:root {
  --card-scale: 1;
  --card-radius: 8px;
  --card-opacity: 1;
  /* FEEL-CHECK (item 2, card-entry mass): ... */
  --ease-notchtap-pop: cubic-bezier(0.3, 1.36, 0.44, 1);
}
```

```css
/* src/styles.css (target) */
:root {
  --card-scale: 1;
  --card-radius: 8px;
  --card-opacity: 1;
  /* plan 163: `--ease-notchtap` is only ever defined inside
     vendor/shared-ui/design/tokens.css's `@theme inline {}` block, which
     Tailwind's Vite plugin only expands for CSS reachable through an
     `@import "tailwindcss";` root — the overlay's entry (main.tsx ->
     overlay-card.css/styles.css) never has one (only
     src/settings/base.css does), so the token silently never resolves
     here. Same defense-in-depth convention `--ease-notchtap-pop` already
     uses in this file: redeclare the plain value directly so every
     overlay consumer (45 usages across 10 files) gets a real, working
     custom property regardless of Tailwind's theme-scoping. Numeric
     value is the exact twin of tokens.css's own `--ease-notchtap` and of
     animationTiming.ts's `NOTCHTAP_EASE` array — keep all three in sync
     if this curve is ever retuned.
     FEEL-CHECK (item 2, card-entry mass): a dedicated ease with a slight
     overshoot (vs. the house `--ease-notchtap`'s pure settle), used ONLY
     by `.card-assembly`'s base width transition (card-chrome.css) so
     promotion/expand width growth lands with a touch of physical mass.
     Also declared with an inline fallback at its one consumer (shared
     file, read by the settings preview too, which never loads this
     stylesheet) — same defense-in-depth convention animationTiming.ts's
     own injected vars use. Revert = swap that consumer's timing function
     back to `var(--ease-notchtap)`. */
  --ease-notchtap: cubic-bezier(0.22, 1, 0.36, 1);
  --ease-notchtap-pop: cubic-bezier(0.3, 1.36, 0.44, 1);
}
```

## Repo conventions to follow

- `--ease-notchtap-pop` in this exact file is the working exemplar of the
  pattern this plan applies to `--ease-notchtap`: a plain `:root`
  redeclaration as defense-in-depth against a token that might not resolve
  in every context.
- `src/animationTiming.ts:90`'s `NOTCHTAP_EASE` array and
  `animationTiming.test.ts` already guard that the JS numeric twin stays
  in sync with tokens.css's CSS value — this plan does not touch that
  pairing, it only makes the CSS side of the SAME curve actually reach the
  browser in the overlay window.

## Steps

1. In `src/styles.css`, inside the existing `:root { ... }` block
   (currently lines 1-14), add `--ease-notchtap: cubic-bezier(0.22, 1,
   0.36, 1);` immediately before the existing `--ease-notchtap-pop`
   declaration, with the comment shown in the Target section above.
2. Do not modify any other file — every one of the 45 consumers across the
   10 overlay CSS files already references `var(--ease-notchtap)` and will
   pick up the new, real `:root` value automatically once it exists.

## Boundaries

- Do NOT touch `vendor/shared-ui/design/tokens.css` — it is a vendored
  shared-ui package; changing its consumption contract or moving values
  out of its `@theme inline` block is out of scope and would affect the
  settings window too (where the token currently works correctly via a
  real `@import "tailwindcss"` chain).
- Do NOT touch `src/animationTiming.ts` or its `NOTCHTAP_EASE` array —
  already correct, unaffected by this bug.
- Do NOT touch `vite.config.ts` or attempt to add an `@import
  "tailwindcss"` to the overlay's CSS chain — that would pull in Tailwind's
  full preflight/utility generation into the overlay bundle, a much larger
  and riskier change than this plan's scope.
- Do NOT retune the curve's numeric value — this plan restores the
  existing, already-designed value to working order; it does not change
  what the curve is.
- If `src/styles.css`'s current `:root` block doesn't match the quoted
  excerpt (drift since commit `58cccd9`), STOP and report instead of
  improvising.

## Verification

- **Mechanical**: `npx vitest run` (repo root) — this bug predates any
  test in the current suite catching it (CSS custom-property resolution
  isn't something jsdom-based component tests exercise), so expect no
  test to fail or newly pass; confirm the suite stays green regardless.
  `npx tsc --noEmit` clean (no TS touched). `npx biome ci .` clean.
- **Build verification (required, not optional for this plan)**:
  1. `npx vite build --outDir /tmp/plan163-verify --emptyOutDir`
  2. `grep -o '.\{0,40\}ease-notchtap:[^;]*;' /tmp/plan163-verify/assets/main-*.css` —
     confirm the value now appears **inside** a real rule body (e.g.
     nested under a `:root{...}` or similar), not as bare text outside any
     selector.
  3. Serve the build (`npx vite preview --outDir /tmp/plan163-verify
     --port 5411 --strictPort`) and load `http://localhost:5411/index.html`
     in a browser. Run:
     ```js
     getComputedStyle(document.documentElement).getPropertyValue('--ease-notchtap').trim()
     ```
     Confirm it returns `cubic-bezier(0.22, 1, 0.36, 1)` (or `.22, 1, .36,
     1` depending on serialization), not an empty string.
  4. On the same page, find (or trigger) a `.card-assembly` element and
     check `getComputedStyle(el).transitionProperty` /
     `.transitionDuration` — confirm they now report the real declared
     values (`width, transform` / non-zero durations), not `"all"` /
     `"0s"`.
  5. Clean up: kill the preview server, delete `/tmp/plan163-verify`.
- **Feel check**: with the fix applied, run the app (or the same build) and
  observe: the shell's hover "breathe" (cursor near the notch) should now
  visibly scale by 1.02 with a smooth 160ms transition, where before it
  did nothing. A card promotion's width growth should now visibly ease in
  with the pop-overshoot curve, where before it snapped instantly. This is
  expected and correct — it is the animation system actually running for
  the first time, not a new visual bug. Do not be alarmed if some
  transitions now look more prominent than screenshots/prior review notes
  suggested; those were made against a state where the curves never
  played. Note anything that looks clearly wrong (a curve that's too
  strong, a duration that now feels slow) for a follow-up plan rather than
  reverting this fix.
- **Done when**: `--ease-notchtap` resolves to a real value in the overlay
  build (steps 1-4 above pass), `npx vitest run`/`tsc`/`biome` are clean,
  and the feel-check confirms plain-CSS overlay animations are now
  actually running.
