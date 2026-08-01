# 159 — Segmented control: add press feedback, fix the selection-shadow pop

- **Status**: DONE (2026-07-31) — transition property list expanded to cover box-shadow/transform, `active:scale-[0.97]` added. `npx vitest run` 573/573.
- **Commit**: 58cccd9
- **Severity**: MEDIUM
- **Category**: Physicality & origin (AUDIT.md §3) + Cohesion & tokens (§7)
- **Estimated scope**: 1 file, 1 class list

## Problem

`src/settings/controls/Segmented.tsx` is the one shared segmented-control
component in the app (per its own doc comment, it replaced three
near-duplicate implementations — Priority toggle, Units toggle, and
Appearance's Scale/Radius/Opacity rows all use it). It has two related gaps:

1. **No press feedback at all.** Per AUDIT.md §3: "Press feedback:
   `transform: scale(0.97)` on `:active` with `transition: transform 160ms
   ease-out`. Keep it subtle (0.95–0.98)." / hunt list: "pressable elements
   with no press feedback."
2. **The selection halo pops in while color glides.** The button's own
   `transition-colors` only covers color/background-color/border-color —
   not `box-shadow` — so when a segment becomes selected, its background
   and text color animate over 140ms but its `shadow-[var(--shadow-selected)]`
   selection ring appears/disappears instantly on the same class flip. Per
   AUDIT.md §8/§7, this is the "two-phase glitch" class of finding: one
   property of a single logical state change animates while a sibling
   property snaps.

Current code, `src/settings/controls/Segmented.tsx:87-111` (the button):

```tsx
<button
  key={option.value}
  type="button"
  className={cn(
    // plan 115: rounded-[4px] is intentionally off-scale (no
    // --radius-* rung is 4px; --radius-sm is 6px) — left as a
    // literal arbitrary value rather than shifting the
    // visible corner radius.
    buttonClass,
    // S3 consistency fix: was a hard, opaque 2px
    // `shadow-[0_0_0_2px_var(--ring)]` ring — the only focus
    // treatment in the settings window that didn't match the
    // soft 3px/50%-opacity ring every shadcn primitive
    // (button.tsx, switch.tsx, input/textarea) uses via
    // `focus-visible:border-ring focus-visible:ring-3
    // focus-visible:ring-ring/50`. Same vocabulary here so
    // keyboard-tabbing reads as one consistent focus style
    // across the whole window.
    "rounded-[4px] border border-transparent bg-transparent px-1.5 py-px font-mono text-fs-secondary font-[620] tracking-[0.03em] text-muted-foreground outline-none transition-colors duration-[140ms] ease-notchtap hover:bg-accent hover:text-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50",
    value === option.value &&
      "is-selected bg-accent text-foreground shadow-[var(--shadow-selected)]",
  )}
  aria-pressed={value === option.value}
  onClick={() => onChange(option.value)}
>
  {option.label}
</button>
```

## Target

Two additions to the base (always-applied) class string, both extending the
existing `transition-colors` to a `transition-[color,background-color,
border-color,box-shadow,transform]` (so the shadow glides along with color,
and so a future `:active` transform also animates), plus a subtle
`active:scale-[0.97]`:

```tsx
"rounded-[4px] border border-transparent bg-transparent px-1.5 py-px font-mono text-fs-secondary font-[620] tracking-[0.03em] text-muted-foreground outline-none transition-[color,background-color,border-color,box-shadow,transform] duration-[140ms] ease-notchtap hover:bg-accent hover:text-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 active:scale-[0.97]",
```

(Only `transition-colors` → `transition-[color,background-color,
border-color,box-shadow,transform]` and the trailing `active:scale-[0.97]`
are new; every other class stays exactly as-is, same order.)

## Repo conventions to follow

- `src/components/ui/button.tsx:8` already has a Tailwind
  multi-property transition list in the same style:
  `transition-[color,background-color,border-color,box-shadow,transform]`
  — copy that exact property list rather than inventing a new one, so this
  matches the one other component in the repo with a real press-feedback
  transition.
- AUDIT.md §3's scale target is `0.95–0.98`; `0.97` (Tailwind's
  `scale-[0.97]`) sits in the middle of that range and matches this plan's
  sibling plan 160's choice for other pressable elements — keep them
  numerically identical across the settings window rather than picking a
  different value per component.

## Steps

1. In `src/settings/controls/Segmented.tsx`, locate the button's class
   string (currently line 105, inside the `cn(...)` call at lines 90-106).
2. Replace `transition-colors` with
   `transition-[color,background-color,border-color,box-shadow,transform]`.
3. Add `active:scale-[0.97]` to the end of the same class string (after
   `focus-visible:ring-ring/50`, before the closing quote).
4. Leave `duration-[140ms] ease-notchtap` and every other class unchanged.

## Boundaries

- Do NOT touch the `is-selected` conditional class or its
  `shadow-[var(--shadow-selected)]` value — only the transition property
  list on the base class changes; the shadow itself already fades in
  correctly once it's included in the transitioned property list.
- Do NOT touch `buttonClass` (a prop passed into this component from
  call sites) — this plan only edits the literal class string inside
  `Segmented.tsx` itself.
- Do NOT touch the three call sites (Priority toggle, Units toggle,
  Appearance rows) — they inherit the fix automatically via the shared
  component.
- If the current code at the cited lines doesn't match the quoted excerpt
  (drift since commit `58cccd9`), STOP and report instead of improvising.

## Verification

- **Mechanical**: `npx vitest run` (repo root) → any `Segmented`-related
  test (search `Segmented` usage across `*.test.tsx`) stays green — a class
  string change should not affect DOM structure or `aria-pressed`
  assertions. `npx biome ci .` and `npx tsc --noEmit` clean.
- **Feel check**: run the settings window, click through the Priority
  toggle (or Units/Appearance segmented rows) and confirm:
  - The selection halo (`box-shadow`) now glides in/out over the same
    140ms as the color change — no more instant pop.
  - Pressing (mouse-down) a segment gives a subtle, brief scale-down before
    release — check in DevTools Animations panel at 10% playback that the
    scale is genuinely subtle (~97% width), not jarring.
  - Keyboard `Tab`-focus and `Space`/`Enter` activation still shows the
    existing focus ring correctly (unaffected by this change).
- **Done when**: the button's class string includes the expanded
  `transition-[...]` list and `active:scale-[0.97]`, `npx vitest run` and
  `npx tsc --noEmit` are clean, and the feel-check confirms both the
  shadow-glide fix and the new press feedback.
