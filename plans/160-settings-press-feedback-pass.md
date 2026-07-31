# 160 — Add press feedback to the switch, sidebar nav, and history disclosure

- **Status**: DONE (2026-07-31) — `active:scale-[0.97]` added to switch.tsx's track, SettingsApp.tsx's nav item, and HistorySection.tsx's summary, each with `transform` folded into their transition list. `npx vitest run` 573/573.
- **Commit**: 58cccd9
- **Severity**: MEDIUM
- **Category**: Physicality & origin (AUDIT.md §3)
- **Estimated scope**: 3 files, 1 class-string edit each

## Problem

Three pressable elements in the settings window have no press feedback at
all. Per AUDIT.md §3: "Press feedback: `transform: scale(0.97)` on
`:active` with `transition: transform 160ms ease-out`. Keep it subtle
(0.95–0.98)." / hunt list: "pressable elements with no press feedback."

**1. The toggle switch** — `src/components/ui/switch.tsx:42` (track,
inside `SwitchPrimitive.Root`'s `className`, current full string):

```tsx
"peer group/switch relative inline-flex shrink-0 items-center rounded-full border border-transparent p-0 transition-colors duration-150 ease-out outline-none after:absolute after:-inset-x-1 after:-inset-y-1.5 focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 aria-invalid:border-destructive aria-invalid:ring-3 aria-invalid:ring-destructive/20 data-[size=default]:h-[22px] data-[size=default]:w-9 data-[size=sm]:h-4 data-[size=sm]:w-7 dark:aria-invalid:border-destructive/50 dark:aria-invalid:ring-destructive/40 data-checked:bg-primary data-unchecked:bg-input data-disabled:cursor-not-allowed data-disabled:opacity-50",
```

No `:active` state anywhere in the file. This is the shadcn-derived toggle
used throughout the settings window's config rows — a frequently-clicked
control with zero tactile confirmation of a click registering, beyond the
(already-animated) checked/unchecked color+position change.

**2. The settings sidebar nav buttons** — `src/settings/SettingsApp.tsx:385`
(inside the `cn(...)` call, current):

```tsx
"nav-item relative grid min-h-[38px] min-w-0 grid-cols-[16px_minmax(0,1fr)] items-center gap-2 rounded-md border-0 border-l-2 border-l-transparent bg-transparent py-[7px] pr-2 pl-[6px] text-left text-muted-foreground outline-none transition-colors duration-[140ms] ease-notchtap hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring",
```

Only `hover:` and `focus-visible:` states are styled — no `active:` class.
This is the primary section-switcher for the whole settings window.

**3. The history disclosure toggle** — `src/settings/sections/HistorySection.tsx:174`
(current, full element):

```tsx
<summary className="cursor-pointer text-fs-caption font-[650] text-muted-foreground">
  More details
</summary>
```

No hover or active state of any kind — lower-traffic than the other two,
but still a clickable element with zero feedback.

## Target

Each element gets the same subtle `active:scale-[0.97]` press treatment,
with its existing `transition-colors` (or, where absent, a new
`transition-transform`) extended to cover `transform` too, so the scale
itself animates smoothly rather than snapping.

**1. `switch.tsx` track** (only the `transition-colors` → property-list and
trailing `active:` class are new):

```tsx
"peer group/switch relative inline-flex shrink-0 items-center rounded-full border border-transparent p-0 transition-[color,background-color,border-color,transform] duration-150 ease-out outline-none after:absolute after:-inset-x-1 after:-inset-y-1.5 focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 aria-invalid:border-destructive aria-invalid:ring-3 aria-invalid:ring-destructive/20 data-[size=default]:h-[22px] data-[size=default]:w-9 data-[size=sm]:h-4 data-[size=sm]:w-7 dark:aria-invalid:border-destructive/50 dark:aria-invalid:ring-destructive/40 data-checked:bg-primary data-unchecked:bg-input data-disabled:cursor-not-allowed data-disabled:opacity-50 active:scale-[0.97]",
```

**2. `SettingsApp.tsx` nav item**:

```tsx
"nav-item relative grid min-h-[38px] min-w-0 grid-cols-[16px_minmax(0,1fr)] items-center gap-2 rounded-md border-0 border-l-2 border-l-transparent bg-transparent py-[7px] pr-2 pl-[6px] text-left text-muted-foreground outline-none transition-[color,background-color,border-color,transform] duration-[140ms] ease-notchtap hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring active:scale-[0.97]",
```

**3. `HistorySection.tsx` summary**:

```tsx
<summary className="cursor-pointer text-fs-caption font-[650] text-muted-foreground transition-transform duration-150 ease-out active:scale-[0.97]">
  More details
</summary>
```

(150ms/ease-out for the summary matches AUDIT.md §3's own literal example
duration — this element has no pre-existing `duration-[Nms]` convention to
match, unlike the other two.)

## Repo conventions to follow

- `src/components/ui/button.tsx:8` is the one component in the repo that
  already has real press feedback (`active:not-aria-[haspopup]:translate-y-px`
  on a `transition-[color,background-color,border-color,box-shadow,transform]`
  list) — this plan intentionally does NOT copy button.tsx's `translateY`
  technique (a separate, lower-severity finding about *which* technique to
  standardize on exists independently — out of scope here); it uses the
  `scale(0.97)` technique AUDIT.md §3 prescribes, matching plan 159's choice
  for `Segmented.tsx` so every *newly added* press treatment in this batch
  is numerically consistent with itself.
- `switch.tsx`'s Radix `data-checked`/`data-unchecked` attribute selectors
  are the existing state-variant convention in that file — this plan adds
  a plain Tailwind `active:` pseudo-class, which composes fine alongside
  them (confirmed: Tailwind's `active:` and Radix's `data-*:` variants are
  independent, non-conflicting selector prefixes).

## Steps

1. In `src/components/ui/switch.tsx`, locate the track's `className`
   string (currently line 42). Replace `transition-colors` with
   `transition-[color,background-color,border-color,transform]`, and add
   `active:scale-[0.97]` at the end of the string.
2. In `src/settings/SettingsApp.tsx`, locate the nav button's class string
   inside the `cn(...)` call (currently around line 385). Replace
   `transition-colors` with
   `transition-[color,background-color,border-color,transform]`, and add
   `active:scale-[0.97]` at the end.
3. In `src/settings/sections/HistorySection.tsx`, locate the `<summary>`
   element (currently line 174). Add `transition-transform duration-150
   ease-out active:scale-[0.97]` to its existing `className` string.
4. For each file, leave every other class and prop untouched.

## Boundaries

- Do NOT touch the `SwitchPrimitive.Thumb`'s own className (below the
  track in `switch.tsx`) — only the track (`SwitchPrimitive.Root`) gets
  press feedback; the thumb's existing transform-based checked/unchecked
  transition is unrelated and out of scope.
- Do NOT touch `Segmented.tsx` here — it's covered by plan 159, which
  additionally fixes a shadow-transition bug this plan doesn't touch.
- Do NOT add hover styling to `HistorySection.tsx`'s `<summary>` — the
  audited finding is specifically about press feedback; adding a hover
  state would be scope creep beyond what was found.
- If any of the three cited excerpts don't match current code (drift since
  commit `58cccd9`), STOP and report instead of improvising for that file
  — the other two files in this plan can still proceed independently.

## Verification

- **Mechanical**: `npx vitest run` (repo root) → tests touching `Switch`,
  `SettingsApp`'s nav, or `HistorySection`'s details/summary must stay
  green (class-string additions shouldn't affect DOM structure or
  `data-*`/`aria-*` assertions). `npx biome ci .` and `npx tsc --noEmit`
  clean.
- **Feel check**: run the settings window and:
  - Click several config toggles and confirm a brief, subtle scale-down on
    press, both in checked and unchecked starting states.
  - Click through sidebar section tabs and confirm the same subtle press
    feedback, distinct from (and layered on top of) the existing hover
    color change.
  - Open a History entry with "More details" and click the disclosure;
    confirm a subtle press scale before/independent of the expand.
  - In DevTools Animations panel at 10% playback on each, confirm the
    scale stays in the 0.95–0.98 range visually (not exaggerated) and that
    keyboard-only activation (`Space`/`Enter`) still works and still shows
    the existing focus ring correctly.
- **Done when**: all three class strings include the new `active:scale-[0.97]`
  (with `transform` added to whichever transition list the element already
  had, or a new one added to the summary), `npx vitest run` and `npx tsc
  --noEmit` are clean, and the feel-check confirms press feedback on all
  three without regressing existing hover/focus/checked-state behavior.
