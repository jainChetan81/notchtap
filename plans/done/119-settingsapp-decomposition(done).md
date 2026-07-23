# Plan 119: Decompose SettingsApp.tsx + absorb the three review deletions

> **Executor instructions**: Follow step by step; run every verification. STOP
> on any STOP condition. Do NOT push/merge/PR. Do NOT edit `plans/README.md` —
> the reviewer maintains it.
>
> **Drift check (run first)**: `git diff --stat <planned-at SHA>..HEAD -- src/settings/`
> Line numbers below are anchors, not gospel — re-grep each symbol before editing;
> on a structural mismatch (a named function no longer exists), STOP.

## Status

- **Priority**: P3 (pure maintainability — zero user-visible change)
- **Effort**: M
- **Risk**: LOW (mechanical; the test suite imports only `SettingsApp` + types)
- **Depends on**: none (do not run concurrently with any other settings-window work)
- **Category**: tech-debt
- **Planned at**: commit `dab91d8`, 2026-07-23

## Why this matters

`src/settings/SettingsApp.tsx` is 2,642 lines and grew 1,939 → 2,203 → 2,642
across three plan waves (2026-07-23 thermo-nuclear review, finding 6). The
single-file shape was defensible while plans 108–112 depended on landmark line
numbers; those plans are done. The same review found three concrete deletions
inside it: a segmented toggle implemented three times, a shadow-state remount
hack in AppearanceSection, and a hand-rolled precursor of the `useActionStatus`
mechanism in SecretRow. Decomposition is test-neutral: `SettingsApp.test.tsx`
imports only the `SettingsApp` component plus types.

## Current state (verify with grep — anchors as of `dab91d8`)

- `src/settings/SettingsApp.tsx` (2,642 lines) — everything: 10 section
  components, shared controls, the ActionStatus mechanism, invoke plumbing.
- **The segmented-toggle triplication**: `PriorityToggle` (`:743`),
  `UnitsToggle` (`:804`), `SegmentedControl` (`:2115`) — ~165 near-identical
  lines. Copy-paste wart: `UnitsToggle` renders **2** options into
  `grid-cols-3` (`:825`, same className string as PriorityToggle's `:764`).
- **AppearanceSection shadow-state hack** (`:2168`+): 3 local `useState`s
  seeded from config + a `formGeneration` counter (`:2301`) used as a `key=`
  to force remount on Reset — config is already the source of truth; reading
  it directly deletes the states, the counter, the key, and ~15 lines of
  comments (`:2176`, `:2303`).
- **`SecretRow`** (`:1001`) — hand-rolled pending/ok/error status copy that
  predates `useActionStatus` (9 uses in file); migrate it onto the shared
  mechanism, preserving the plan-108 announcement/dedup semantics.
- **The 11 invoke commands** are called inline as string literals; the review
  recommends `src/settings/ipc.ts` — one typed command map (name → arg/return
  types) as the TS-side mirror of the security-load-bearing `build.rs`
  allowlist (CLAUDE.md "ipc & security").

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Install | `npm ci` | exit 0 |
| Tests | `npx vitest run` | all pass, count unchanged unless a test is deliberately split |
| Typecheck | `npx tsc --noEmit` | 0 |
| Lint | `npx biome ci .` | 0 |
| Build | `npx vite build` | 0 |

## Scope

**In scope**: `src/settings/SettingsApp.tsx` (shrinks), new files under
`src/settings/` (sections/, controls/, ipc.ts — follow the existing folder's
naming), `src/settings/SettingsApp.test.tsx` (import-path updates ONLY — no
assertion changes).

**Out of scope**: `src/settings/base.css` and every visual/styling value (this
plan must be pixel-identical — no className edits beyond moving code);
`src/components/ui/**`; the overlay; `src-tauri/**`; behavior of any control.

## Steps

1. **Extract shared controls** (`src/settings/controls/`): one `Segmented`
   component replacing PriorityToggle/UnitsToggle/SegmentedControl (props:
   options, value, onChange, optional fieldset/legend semantics preserved
   per plan 109 — keep the exact rendered markup/classes each call site has
   today, EXCEPT fix the `grid-cols-3`-for-2-options wart to derive
   `grid-cols-{n}` from the option count). Keep `NumberControl`,
   `TextareaControl`, `ControlCopy`, `ToggleControl` together in one module.
   **Verify**: vitest green; `npx vite build` diff of rendered markup — none
   expected except UnitsToggle's corrected grid column count.
2. **Extract sections** (`src/settings/sections/`): one file per section
   (General, Football, News, Cmux, Weather, Connectors, Shortcuts, Appearance,
   Diagnostics, History), moving each closed component as-is. SettingsApp.tsx
   keeps: shell/nav/footer, config state, ActionStatus, and imports.
   **Verify**: vitest green after each move (move one, run, next).
3. **Delete the AppearanceSection shadow-state**: read config directly,
   remove the 3 useStates + `formGeneration` + `key=`; Reset works because
   config propagation already re-renders. **Verify**: the plan-108 reset tests
   (both Reset buttons, failed-live-apply) stay green unchanged.
4. **Migrate SecretRow onto `useActionStatus`**, preserving announcement
   semantics. **Verify**: the secret-row tests stay green unchanged.
5. **Add `src/settings/ipc.ts`**: typed map of the 11 commands; call sites use
   it. **Verify**: `tsc` catches a deliberately-wrong arg type (prove, revert).
6. Final gates + `wc -l src/settings/SettingsApp.tsx` reported (expect < ~800).

## Done criteria

- [ ] All gates green; **zero test assertion changes** (import paths only)
- [ ] SettingsApp.tsx < ~800 lines; sections + controls extracted
- [ ] One `Segmented` implementation; `grid-cols` derived from option count
- [ ] AppearanceSection has no config-shadow useState/formGeneration/key remount
- [ ] SecretRow uses `useActionStatus`
- [ ] `ipc.ts` typed command map used for all 11 invokes
- [ ] Pixel parity: settings screenshots (plan-111 harness) before/after — only
      permitted delta is UnitsToggle's corrected column count

## STOP conditions

- Any test needs an ASSERTION change (means behavior drifted — this plan is
  pure motion of code).
- The AppearanceSection direct-read breaks a reset test.
- Screenshot diff shows any unexpected visual delta.

## Maintenance notes

- After this, per-section work touches one small file; the god-file review
  finding is closed. Reviewer should scrutinize the screenshot parity and the
  zero-assertion-change rule.
