# Plan 115: Settings-window token hygiene (radius scale, shadow token, font-size + preview-frame hex)

> **Executor instructions**: Follow step by step; run every verification. STOP
> and report on any STOP condition. Do NOT push/merge/PR. Do NOT edit
> `plans/README.md` — the reviewer maintains it.
>
> **Drift check (run first)**: `git diff --stat 2c79e36..HEAD -- src/settings/SettingsApp.tsx src/settings/base.css`

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none. Independent of 114/116/117 (settings-only; touches different files than 114/116).
- **Category**: tech-debt
- **Planned at**: commit `2c79e36`, 2026-07-22

## Why this matters

The settings window was migrated to shared-ui tokens in plans 108–113, but a few
stragglers bypass the scales it defines: ~11 `rounded-[Npx]` literals ignore the
`--radius-*` scale (4 of them exactly equal a token), a selected-pill drop-shadow
is a hardcoded `rgba()` repeated 3× right next to an already-tokenized `--ring`
shadow, two font-sizes sit off the `--fs-*` type floor, and three raw hex remain
in `:root` for the Appearance-preview frame. None is a bug, but each is a place
the "one knob" of the token system doesn't actually control the UI. Cleaning them
makes the settings window fully token-driven and consistent.

## Current state

- **Radius scale** (`src/settings/base.css:86-92`): `--radius-sm: calc(var(--radius)*0.6)` (=6px), `--radius-md: *0.8` (=8px), `--radius-lg: var(--radius)` (=10px), etc., bridged to Tailwind `rounded-sm/md/lg` utilities. `--radius` comes from shared-ui.
  Straggler call-sites in `src/settings/SettingsApp.tsx`:
  - `rounded-[6px]` (== `rounded-sm`) ×3 — lines 638, 916, 998.
  - `rounded-[8px]` (== `rounded-md`) ×1 — line 556.
  - off-scale `rounded-[4px]` ×4 — lines 724, 777, 983, 2051.
  - off-scale `rounded-[7px]` ×3 — lines 712, 765, 2044.
  (Confirm exact lines with grep; line numbers may have shifted slightly.)
- **Selected-pill shadow** (`SettingsApp.tsx:726, 779, 2053`): `shadow-[0_1px_2px_rgba(0,0,0,0.4)]` repeated identically ×3. The SIBLING focus shadow on the same elements (`:724/:777/:2051`) already correctly uses `shadow-[0_0_0_2px_var(--ring)]` — so the tokenize-the-shadow pattern already exists here.
- **Off-scale font-sizes**: `base.css:377` `.preview-label { font-size: 12px; }` (the `--fs-*` scale is 9/10/11/19px); `SettingsApp.tsx:556` `text-[16px]` on the markdown card (== `text-base`).
- **Preview-frame raw hex** (`base.css:177-179`): `--text-secondary: #a2a6ad; --surface: #0d0f12; --divider: #202329;`. Readers: `.preview-label` color (`:377`), `.preview-stage` bg/border (`:398-399`). A Plan-112 comment (`base.css:144-176`) explains they were kept because shared-ui had no same-named token and they have live preview-only readers. Candidate mappings: `--text-secondary`→`--muted-foreground`, `--surface`→`--card`, `--divider`→`--border` (the section-scroll thumb already swapped its `#2a2d33` for `var(--border)` at `base.css:269`).

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Install | `npm ci` | exit 0 |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Tests | `npx vitest run` | 295 pass |
| Lint | `npx biome ci .` | exit 0 |
| Build | `npx vite build` | exit 0 |

## Scope

**In scope**: `src/settings/SettingsApp.tsx`, `src/settings/base.css`.
**Out of scope**: everything else. In particular do NOT touch the overlay
(`src/overlay-card.css`, `styles.css`, `App.tsx`), `src-tauri/**`, or the
`--fs-*`/`--radius-*` scale DEFINITIONS themselves (you consume them, you don't
redefine them — except adding one `--shadow-selected` token in Step 2).

## Git workflow

- Worktree from stale base: **FIRST** `git reset --hard 2c79e36`, verify Step 0.
- Commit per sub-area (radius / shadow / font-size / preview-hex), conventional-commit style (e.g. `refactor(settings): adopt --radius-* scale utilities (plan 115)`).

## Steps

### Step 0: Base-sync + baseline
`git reset --hard 2c79e36`; confirm HEAD, the grep counts above (`rounded-[6px]`×3, `[8px]`×1, `[4px]`×4, `[7px]`×3; `shadow-[0_1px_2px_rgba(0,0,0,0.4)]`×3). Baseline gates green. Mismatch → STOP.

### Step 1: Adopt the radius scale
- Replace exact-match arbitraries: `rounded-[6px]`→`rounded-sm` (3 sites), `rounded-[8px]`→`rounded-md` (1 site). These are pixel-identical.
- For off-scale `rounded-[4px]` (4) and `rounded-[7px]` (3): these have no exact token. Make a conscious call — EITHER snap each to the nearest defined step (`rounded-[4px]`→`rounded-sm` would shift +2px; likely too much — probably KEEP 4px as an intentional tight radius) OR leave them as documented intentional exceptions with a one-line comment. Default: **leave 4px/7px as-is with a brief comment** noting they're intentional off-scale values; only convert the exact-match ones. Do not introduce visual shifts to chase purity.

**Verify**: `grep -c 'rounded-\[6px\]' src/settings/SettingsApp.tsx` → 0; `grep -c 'rounded-\[8px\]'` → 0; `grep -c 'rounded-sm' ` increased by 3; build clean; preview-equivalence harness (see Test plan) zero delta (exact-match swaps are pixel-identical).

### Step 2: Tokenize the selected-pill shadow
Define one `--shadow-selected: 0 1px 2px rgba(0, 0, 0, 0.4);` (in `base.css`, near the other settings-scoped tokens) and reference it as `shadow-[var(--shadow-selected)]` at the 3 sites (`SettingsApp.tsx:726, 779, 2053`). Same computed value.

**Verify**: `grep -c 'shadow-\[0_1px_2px_rgba(0,0,0,0.4)\]' src/settings/SettingsApp.tsx` → 0; `grep -c 'shadow-selected'` → 4 (1 def + 3 uses); preview-equivalence zero delta.

### Step 3: Snap the off-scale font-sizes
- `SettingsApp.tsx:556` `text-[16px]` → `text-base` (== 16px, pixel-identical).
- `base.css:377` `.preview-label { font-size: 12px; }`: 12px is off the `--fs-*` scale. EITHER map to the nearest `--fs-*` step (decide which, noting the visual delta) OR keep 12px with a comment marking it an intentional preview-only exception. Default: **keep with a comment** unless a `--fs-*` step is exactly 12px (it isn't) — a snap here would shift the preview label size, which is a visual change the operator must approve; prefer documenting over shifting.

**Verify**: `grep -c 'text-\[16px\]' src/settings/SettingsApp.tsx` → 0 (only the swap; the `:552` comment mention doesn't count if it's in a `//` comment — check); build clean; preview-equivalence zero delta on the `text-base` swap.

### Step 4: Map the preview-frame hex to tokens (guarded)
Investigate mapping `--text-secondary`→`--muted-foreground`, `--surface`→`--card`, `--divider`→`--border`. For EACH, compare the raw hex against the shared-ui token's computed color; only swap if the visual result is unchanged or the operator would accept the shift. If a mapping shifts the preview look noticeably, KEEP the raw hex with the existing Plan-112 comment (it's already documented as an intentional keep) and report that you kept it and why.

**Verify**: whichever you swapped, `node`/build clean and preview-equivalence harness reports the delta (zero if you only swapped visual-equivalent mappings; if non-zero, you must either revert that mapping or flag it for operator approval — do NOT silently ship a preview color change). Report exactly which of the 3 you swapped vs kept.

### Step 5: Final gates
`npm ci`, `npx vitest run` (295), `npx tsc --noEmit` (0), `npx biome ci .` (0), `npx vite build` (0). Scope diff limited to the 2 in-scope files.

## Test plan

- No new product tests required (these are equivalence-preserving token swaps).
- **Preview-equivalence harness** (Plan 111 technique, scratchpad at
  `/private/tmp/claude-501/-Users-chetanjain-Desktop-code-mac-notification-nudge/d23330e2-89b5-4fe9-8f29-80e31db32a04/scratchpad/`):
  run after each step. Exact-match swaps (Steps 1 partial, 2, 3 `text-base`) must be
  **zero delta**. Any non-zero delta means a swap wasn't equivalent — revert or
  flag for operator, don't ship silently.
- Optionally add a focused test asserting no `rounded-[6px]`/`rounded-[8px]` and
  no raw `shadow-[0_1px_2px_rgba…]` literal remains in `SettingsApp.tsx` (guards
  regression), in the repo's existing string-scan style — skip if it doesn't fit.

## Done criteria

- [ ] `rounded-[6px]`/`rounded-[8px]` exact-match sites → scale utilities; 4px/7px consciously kept-or-snapped with rationale recorded
- [ ] `--shadow-selected` token defined and used at all 3 selected-pill sites; no raw shadow literal remains
- [ ] `text-[16px]` → `text-base`; `.preview-label` 12px kept-or-snapped with rationale
- [ ] Preview-frame hex: each of the 3 either mapped to a token (visual-equivalent) or kept with recorded rationale
- [ ] Preview-equivalence: zero delta on all exact-match swaps; any intentional shift flagged for operator
- [ ] vitest 295+ / tsc 0 / biome 0 / vite build 0; scope diff limited to 2 files

## STOP conditions

- Base-sync/Step-0 grep counts don't match (drift since planning).
- A token mapping in Step 4 shifts the preview visibly and you're unsure whether it's acceptable — STOP and report rather than shipping a silent color change.
- A gate fails twice after a reasonable fix.

## Maintenance notes

- After this, the settings window is fully radius/shadow-token-driven except any
  consciously-documented off-scale exceptions (4px/7px radius, 12px preview label).
- Reviewer: confirm every "equivalent" swap actually produced zero preview delta,
  and that any kept exception carries a rationale comment.
