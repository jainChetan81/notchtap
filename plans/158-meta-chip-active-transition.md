# 158 — Give MetaChip's active state a transition

- **Status**: DONE (2026-07-31) — `transition-colors duration-[150ms] ease-notchtap` added to MetaChip's base class. `npx vitest run` 573/573.
- **Commit**: ef91a0f
- **Severity**: MEDIUM
- **Category**: Missed opportunities (AUDIT.md §8) / Purpose & frequency (§1)
- **Estimated scope**: 1 file, 1 class list

## Problem

`src/components/ui/meta-chip.tsx:40-45` flips border/background/text color
when the `active` prop changes, with no `transition` declared anywhere on
the element — the state change snaps instantly.

Current code, `src/components/ui/meta-chip.tsx:38-46`:

```tsx
return (
  <span
    data-slot="meta-chip"
    className={cn(
      "meta-chip min-w-0 rounded-full border border-border px-[7px] py-0.5 font-mono text-fs-caption font-[650] leading-[1.5] text-muted-foreground",
      uppercase && "tracking-[0.06em] uppercase",
      active && "border-ring/40 bg-input/40 text-foreground",
      className,
    )}
```

Two real consumers where this fires on a genuine, user-relevant state
change (not just a hypothetical):

- `src/settings/sections/ConnectorsSection.tsx:67`:
  `<MetaChip aria-live="polite" uppercase active={!!status} className="status-chip flex-none">`
  — flips from "unset" to a saved-secret status the instant a user saves a
  secret. The `aria-live="polite"` on the same element signals this moment
  is meant to be noticed; the visual channel currently gives it nothing.
- `src/settings/sections/AgentsSection.tsx:246`:
  `<MetaChip uppercase active={health.status === "available"}>` — inside a
  component whose `useEffect` (`AgentsSection.tsx:392-396`) polls
  `get_agent_health` every 5000ms, so a real availability change (an
  adapter coming online) currently teleports.

## Target

Add a `transition-colors` utility class (Tailwind's shorthand for
`color`/`background-color`/`border-color`/`fill`/`stroke`, which covers
every property this `active` flip touches) with the repo's house duration
and easing token.

```tsx
return (
  <span
    data-slot="meta-chip"
    className={cn(
      "meta-chip min-w-0 rounded-full border border-border px-[7px] py-0.5 font-mono text-fs-caption font-[650] leading-[1.5] text-muted-foreground transition-colors duration-[150ms] ease-notchtap",
      uppercase && "tracking-[0.06em] uppercase",
      active && "border-ring/40 bg-input/40 text-foreground",
      className,
    )}
```

150ms is the repo's own "state flip" duration — see `switch.tsx`'s
`transition-colors duration-150` on its track (same class of "toggled
state, color-only change" animation). `ease-notchtap` is the house Tailwind
utility already used by `Segmented.tsx` and `SettingsApp.tsx`'s nav items
for the identical `transition-colors` pattern (see Repo conventions below)
— it maps to the `--ease-notchtap: cubic-bezier(.22, 1, .36, 1)` token
defined in `vendor/shared-ui/design/tokens.css:183`.

## Repo conventions to follow

- The identical fix was already shipped once for a different component:
  `src/overlay/status-dots.css:15-46` (plan 127/129) added a transition to
  an analogous instant state-flip on the overlay's status dots. Match that
  precedent's intent (color/border transition on a discrete state change),
  translated to this component's Tailwind-class idiom.
- Exemplar for the exact Tailwind syntax to copy:
  `src/settings/controls/Segmented.tsx:105` uses
  `"...transition-colors duration-[140ms] ease-notchtap hover:bg-accent..."`
  — same `transition-colors` + `duration-[Nms]` + `ease-notchtap` triplet,
  just a different duration value (140ms there, vs. 150ms here to match
  `switch.tsx`'s convention for a toggled/set state rather than a hover).

## Steps

1. In `src/components/ui/meta-chip.tsx`, locate the `className={cn(...)}`
   call inside the returned `<span>` (currently lines 40-45).
2. Add `transition-colors duration-[150ms] ease-notchtap` to the base
   (always-applied) class string — the first string argument to `cn(...)`
   — so it applies regardless of `active`/`uppercase` state. Do not add it
   conditionally.
3. Leave every other class, prop, and the component's JSDoc-style comments
   above it untouched.

## Boundaries

- Do NOT touch `ConnectorsSection.tsx` or `AgentsSection.tsx` — the fix
  lives entirely in the shared `MetaChip` component; both consumers inherit
  it automatically.
- Do NOT touch the `dotColor` swatch rendering below the class string
  (lines 46+) — out of scope.
- Do NOT change the `active`/`uppercase` conditional logic itself, only add
  the transition classes to the base string.
- If `meta-chip.tsx`'s current content doesn't match the quoted excerpt
  (drift since commit `58cccd9`), STOP and report instead of improvising.

## Verification

- **Mechanical**: `npx vitest run` (repo root) → `MetaChip`-related tests
  (search for `MetaChip` usage in `ConnectorsSection.test.tsx`,
  `AgentsSection.test.tsx`, or a dedicated `meta-chip.test.tsx` if one
  exists) must stay green — a class-string addition should not break any
  DOM-structure assertion. `npx biome ci .` and `npx tsc --noEmit` clean.
- **Feel check**: run the settings window, open Connectors & Keys, save a
  secret, and confirm the status chip's color/border now glides rather than
  snapps. Open Agents section and watch an adapter's health chip through a
  real or simulated state change (or temporarily flip the `active` prop
  value in DevTools React inspector) and confirm the same smooth transition.
  - In DevTools Animations panel at 10% playback, confirm no `box-shadow`
    or non-color property moves — this should be a pure color/border
    glide, nothing else.
- **Done when**: `MetaChip`'s base class string includes `transition-colors
  duration-[150ms] ease-notchtap`, `npx vitest run` is clean, and both real
  consumers (Connectors secret-status chip, Agents health chip) visibly
  transition instead of snapping.
