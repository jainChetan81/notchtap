# Plan 116: Replace rotation-reorder unicode triangles with lucide chevrons

> **Executor instructions**: Follow step by step; run every verification. STOP
> on any STOP condition. Do NOT push/merge/PR. Do NOT edit `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 2c79e36..HEAD -- src/settings/SettingsApp.tsx`

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none. Independent of 114/115/117 (touches only the two glyph lines in SettingsApp.tsx).
- **Category**: dx / tech-debt
- **Planned at**: commit `2c79e36`, 2026-07-22

## Why this matters

The rotation-order "move earlier / move later" icon-buttons render bare unicode
triangles (`▲`/`▼`) as their entire content, inside a file that already imports
and renders lucide icons two dozen lines above. The triangles render at the
font's weight/metrics, not the crisp 1.5px-stroke look of every other icon in the
settings window — a visible consistency seam. This is the single clean,
in-pattern icon conversion the audit found (all other glyph sites are either
wire-driven content or in the overlay window, which has no lucide today).

## Current state

- `src/settings/SettingsApp.tsx:845` — literal `▲` is the entire child of a
  `<Button variant="outline" size="icon-xs">` (the "Move earlier" reorder button).
- `src/settings/SettingsApp.tsx:856` — literal `▼` in the sibling "Move later"
  `<Button size="icon-xs">`.
  (Confirm exact lines with `grep -nE '[▲▼]' src/settings/SettingsApp.tsx`.)
- The buttons already carry `aria-label`s (accessibility does not depend on the
  glyph). Confirm this before editing — the glyphs are `aria-hidden`-equivalent
  decoration on a labeled button.
- lucide is already imported in this file — the named-import block is around
  `src/settings/SettingsApp.tsx:2-14`. Read it to match the exact import style,
  sizing (`className="size-4"` or the file's convention), and color handling used
  by the file's other icons.

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Tests | `npx vitest run` | 295 pass |
| Lint | `npx biome ci .` | exit 0 |
| Build | `npx vite build` | exit 0 |

## Scope

**In scope**: `src/settings/SettingsApp.tsx` only (the import block + the two glyph sites).
**Out of scope**: every overlay glyph (media ▶/⏸, the CSS pause glyph, the
`glyphForBundleId` map) — those are a separate gated decision (plan for overlay
lucide adoption comes later, if at all). Keyboard keycap glyphs (`⌃⇧N` etc.),
weather SVGs, football event marks — all content/art, never touch. `src-tauri/**`.

## Git workflow

- Worktree from stale base: **FIRST** `git reset --hard 2c79e36`.
- Single commit, conventional-commit style: `refactor(settings): rotation-reorder triangles to lucide chevrons (plan 116)`.

## Steps

### Step 0: Base-sync + baseline
`git reset --hard 2c79e36`; `grep -nE '[▲▼]' src/settings/SettingsApp.tsx` → exactly the two lines (845, 856 or nearby). Confirm both buttons have `aria-label`s. Baseline gates green. Mismatch → STOP.

### Step 1: Add the chevron imports
Add `ChevronUp` and `ChevronDown` to the existing `lucide-react` named-import
block (keep the block alphabetized if it currently is). Do NOT add a second
import statement — extend the existing one.

**Verify**: `grep -c 'ChevronUp' src/settings/SettingsApp.tsx` ≥1; `npx tsc --noEmit` → 0.

### Step 2: Swap the glyphs
- Replace the `▲` text node (move earlier) with `<ChevronUp className="size-4" />`
  (match the size/className convention the file's other in-button icons use — if
  the file sizes icons differently, follow that).
- Replace the `▼` (move later) with `<ChevronDown className="size-4" />`.
- Leave each `<Button>`'s `aria-label`, `variant`, `size`, `onClick`, and
  disabled logic untouched — only the child glyph changes.

**Verify**: `grep -cE '[▲▼]' src/settings/SettingsApp.tsx` → 0. `npx vitest run` → all pass (if a test asserts on the button by its accessible name / aria-label, it still passes since those are unchanged; if any test asserted on the literal `▲`/`▼` text, update it to assert on the accessible name instead — that's an allowed focused test edit).

### Step 3: Final gates
`npx tsc --noEmit` (0), `npx biome ci .` (0), `npx vite build` (0), `npx vitest run` (295). Scope diff = only `SettingsApp.tsx`.

## Test plan

- Existing tests should stay green (buttons are identified by `aria-label`, not glyph).
- If any test referenced the `▲`/`▼` literals, repoint it to the button's
  accessible name (allowed focused edit). Otherwise no test changes.
- Optionally add a focused assertion that the reorder buttons render an SVG icon
  (lucide renders `<svg>`) rather than a text triangle — only if it fits cleanly.

## Done criteria

- [ ] `ChevronUp`/`ChevronDown` imported from the existing lucide block
- [ ] No `▲`/`▼` literal remains in `SettingsApp.tsx`
- [ ] Button `aria-label`s / behavior unchanged
- [ ] vitest 295 / tsc 0 / biome 0 / vite build 0; scope diff = 1 file

## STOP conditions

- The glyphs aren't where "Current state" says, or the buttons lack `aria-label`s
  (then the glyph might be load-bearing for accessibility — STOP, report).
- A gate fails twice after a reasonable fix.

## Maintenance notes

- This establishes chevrons as the reorder affordance; if the overlay later adopts
  lucide (deferred gated decision), reuse the same icon vocabulary there.
- Reviewer: trivial change; just confirm accessible names are intact and no other
  glyph was swept up.
