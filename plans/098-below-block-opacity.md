# Plan 098: Make the Opacity setting work again — applied to the below-block only, shell stays solid

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. The reviewer maintains `plans/README.md` — do
> not edit it.
>
> **Worktree preflight (run before anything else)**: agent worktrees can
> branch from a stale HEAD. Run `git log --oneline master ^HEAD`; if it
> prints anything, run `git merge --ff-only master` and confirm it
> succeeds before starting. Then run `npm ci` (fresh worktrees have no
> node_modules).
>
> **Drift check (run second)**: `git diff --stat 0056f38..HEAD -- src/styles.css src/settings/preview-overlay.css`
> If either changed since `0056f38`, compare the "Current state" excerpts
> below against the live code before proceeding; on a mismatch, treat it
> as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `0056f38`, 2026-07-21

## Why this matters

The Settings window ships a user-facing Opacity control (Glass 0.7 /
Default 0.9 / Solid 1.0). Its value is validated, persisted, emitted to
the overlay, and written to the CSS variable `--card-opacity`
(`src/App.tsx:25`) — but since plan 091 replaced the old `.rail-card`
(whose background was `rgba(5, 6, 7, var(--card-opacity))`) with the
`.card-assembly` grid whose surfaces are hardcoded `#000`, **no CSS rule
reads the variable**. The setting is a silent no-op.

Operator decision (2026-07-21): re-scope opacity to the **below-block
only**. The shell — the two flanks and the synthetic cutout, i.e. the
whole top grid row — must stay solid `#000` so the app-drawn shape stays
indistinguishable from the hardware notch (plan 091's "shell always
opaque" decision is unchanged). Only the content block hanging below the
cutout becomes translucent.

## Current state

- `src/styles.css` — the overlay stylesheet. The `.card-assembly` grid:
  flanks at `:82-97` (`background: #000;` at `:89`), synthetic cutout at
  `:147-155` (`background: #000;` at `:154`), and the content block:
  ```css
  /* styles.css:157-172 */
  .below-block {
    grid-column: 1 / -1;
    grid-row: 2;
    position: relative;
    box-sizing: border-box;
    overflow: hidden;
    width: 100%;
    min-width: 0; /* see the flank rule above — same grid-blowout guard */
    background: #000;
    color: #f5f7fa;
    box-shadow: 0 14px 28px rgba(0, 0, 0, 0.5);
    border-bottom-left-radius: var(--card-radius, 14px);
    border-bottom-right-radius: var(--card-radius, 14px);
  }
  ```
  `--card-opacity: 1;` is already declared at `styles.css:4`.
  The idle weather peek (`.below-block.idle-peek`, around `:1340+`) is a
  `.below-block` modifier and paints no background of its own — it
  inherits whatever `.below-block` does, which is the desired behavior.
- `src/settings/preview-overlay.css` — the settings-window preview
  mirror. `--card-opacity: 1;` at `:4`; `.appearance-preview .below-block`
  at `:83` with `background: #000;` at `:91`. The preview already feeds
  the live slider value into `--card-opacity` inline
  (`src/settings/SettingsApp.tsx:1594`), so once the CSS consumes the
  var, the preview shows the effect with no TS change.
- **Mirror law** (repo convention): `styles.css` and
  `preview-overlay.css` must stay in visual lockstep for shared card
  classes — every change here lands in both files in the same commit.

## Commands you will need

| Purpose | Command (repo root) | Expected |
|---|---|---|
| Frontend tests | `npx vitest run` | all pass |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Lint (CI gate) | `npx biome ci .` | exit 0 |
| Build | `npx vite build` | success |

## Scope

**In scope** (the only files you should modify):
- `src/styles.css`
- `src/settings/preview-overlay.css`

**Out of scope** (do NOT touch):
- The flank backgrounds (`styles.css:89`, preview `:47`) and the
  synthetic cutout (`styles.css:154`, preview `:80`) — these are the
  shell and MUST remain literal `#000` (notch-blend decision, plan 091).
- `src/App.tsx`, `src/settings/SettingsApp.tsx`, all rust code — the
  plumbing already works end-to-end.
- `docs/TESTING_STRATEGY.md` §0 — plan 099 reconciles counts.

## Git workflow

- Branch: your dispatched worktree branch.
- One commit, conventional style:
  `fix(overlay): consume --card-opacity on the below-block (plan 098)`.
- Do NOT push.

## Steps

### Step 1: Consume the variable on the below-block, both files

In `src/styles.css:165`, change
`background: #000;` → `background: rgba(0, 0, 0, var(--card-opacity, 1));`
and add a short comment above it: opacity (the Settings Glass/Default/
Solid control, `App.tsx --card-opacity`) applies ONLY here — the flanks
and synthetic cutout above are the shell and stay solid `#000` so the
drawn shape matches the hardware notch (plan 091 decision; re-scoped by
plan 098).

Make the identical change (and a one-line pointer comment) in
`src/settings/preview-overlay.css:91` under
`.appearance-preview .below-block`.

**Verify**:
`grep -n "rgba(0, 0, 0, var(--card-opacity" src/styles.css src/settings/preview-overlay.css`
→ exactly one hit per file, on the `.below-block` rule.

### Step 2: Confirm the shell stayed solid and nothing else moved

**Verify** (all from repo root):
- `grep -c "background: #000;" src/styles.css` → 2 (flanks + synthetic cutout only)
- `npx vitest run` → all pass, same count as before your change
- `npx tsc --noEmit` → exit 0
- `npx biome ci .` → exit 0
- `npx vite build` → success
- `git status` → only the two in-scope files modified

## Test plan

No new automated test: jsdom does not resolve `var()` in computed
backgrounds, so a vitest assertion here would test the test. The
machine-checkable guards are the greps in Steps 1–2; the visual check
joins the operator's batched smoke hour (set Opacity to Glass, confirm
the below-block goes translucent while the flanks/cutout stay solid, in
both notch and HUD modes).

## Done criteria

ALL must hold:

- [ ] Both greps from Step 1 and Step 2 return exactly the stated counts
- [ ] `npx vitest run`, `npx tsc --noEmit`, `npx biome ci .`, `npx vite build` all clean
- [ ] `git status` shows only `src/styles.css` and `src/settings/preview-overlay.css` modified

## STOP conditions

Stop and report back (do not improvise) if:

- `styles.css:165` / `preview-overlay.css:91` no longer contain
  `background: #000;` under `.below-block` (drift).
- You find any OTHER rule already consuming `--card-opacity` (the premise
  "no consumer exists" would be wrong).
- The grep in Step 2 finds a third `background: #000;` you'd have to
  reclassify as shell-vs-content — report it instead of deciding.

## Maintenance notes

- If a future plan adds any new painted surface to the below-block
  family (a new card type, a new peek), its background must either
  inherit `.below-block`'s or consume `var(--card-opacity)` itself —
  never a fresh opaque `#000` inside the content area.
- Reviewer: diff both CSS files side by side (mirror law), and confirm
  the flank/cutout rules are byte-unchanged.
- The `.below-block.cmux-origin` border-top hairline (styles.css:182)
  keeps its own alpha — it is chrome, not surface; deliberately not
  multiplied by the opacity setting.
