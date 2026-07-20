# Plan 016: Add a frontend lint/format gate (Biome) to match the fully-gated rust side

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report. When done,
> update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat af9be44..HEAD -- package.json .github/workflows/ci.yml`
> Coordinate with plan 007 (it also edits ci.yml's web job) — 007 is
> already DONE and landed; add the biome step alongside its steps.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW (first run may flag existing code; fixes are mechanical — see Step 2's containment rule)
- **Depends on**: none — 007 is DONE (ci.yml's web job now runs on
  `ubuntu-latest` with `npm audit` + `sh -n notchtap` steps; keep them and
  add the biome step alongside)
- **Category**: dx
- **Planned at**: commit `d40445e`, 2026-07-17; drift baseline refreshed to `b43a7ca` 2026-07-18; **review-plan pass 2026-07-18 at `af9be44`** — drift since `b43a7ca` verified trivial (package.json: only the lottie-react removal from plan 023, scripts unchanged; ci.yml: unchanged). Config sketch updated to Biome v2 current keys (`linter.rules.preset`, deprecated `recommended` dropped) and the scope narrowed to TS/TSX only — `src/**` would also pull in the hand-tuned CSS files (see Step 1's CSS note). Vitest count pinned to 66.

## Why this matters

The rust side is gated by `cargo fmt --check` + `clippy -D warnings` in
CI. The TS/React side (~15 source files, hook- and effect-heavy) has only
`tsc`: no lint (no exhaustive-deps check on hooks — the classic stale-
closure bug class in exactly this kind of code), no formatter (and this
repo is written by many different agent sessions, each picking its own
style). One tool — Biome — provides both with near-zero config.

## Current state

- No `.eslintrc*`, `eslint.config.*`, `biome.json`, `.prettierrc*`, or
  `.editorconfig` anywhere in the repo (verified).
- `package.json` scripts: `dev`, `build`, `preview`, `tauri`,
  `test: vitest run` (verified unchanged at `af9be44`).
- CI web job (`.github/workflows/ci.yml`, post-007): `npm ci`,
  `npm audit --audit-level=high`, `npx tsc --noEmit`, `npx vitest run`,
  `npx vite build`, `sh -n notchtap` — add the biome step after `npm ci`;
  keep every existing step.
- Frontend layout: `src/*.ts(x)`, `src/components/*.tsx`, `src/lib/*.ts`,
  `src/settings/*` — plus root `vite.config.ts`. `dist/` is build output
  (git-ignored); `node_modules/` present. Agent worktrees may exist under
  `.claude/worktrees/` (vite.config.ts excludes them from vitest — Biome
  must ignore them too).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Install (dev-dep) | `npm install --save-dev --save-exact @biomejs/biome` | exit 0 |
| Check | `npx biome check .` | exit 0 (after Step 2) |
| CI form | `npx biome ci .` | exit 0 |
| Frontend tests | `npx vitest run` && `npx tsc --noEmit` | all pass |

## Scope

**In scope**:
- `package.json` + `package-lock.json` (dev-dependency + `lint` script)
- `biome.json` (new, repo root)
- `.github/workflows/ci.yml` (one step in the web job)
- `src/**/*.ts`, `src/**/*.tsx`, `vite.config.ts` — ONLY mechanical
  changes produced by `biome check --write` / explicitly-listed rule
  fixes (see Step 2's containment rule). **Never `src/**/*.css`** — see
  Step 1's CSS note.
- `AGENTS.md` + `CLAUDE.md` commands section (one line documenting the
  lint command)
- `plans/README.md` (status row)

**Out of scope**:
- `src-tauri/**`, `notchtap-detect/**`, `docs/**` (except the AGENTS/
  CLAUDE line), `dist/`, `.claude/`, `.agents/`, `.opencode/`
- Any behavioral change: if a lint fix would alter logic (not just
  style/imports), suppress with an inline ignore + comment instead, and
  list it in your report.

## Git workflow

- Current branch; two commits:
  1. `dx: add biome (lint+format) with ci gate`
  2. `style: mechanical biome fixes across src/` (keep separate so the
     tool-config diff is reviewable apart from the churn)
- Do NOT push.

## Steps

### Step 1: Install and configure

`npm install --save-dev --save-exact @biomejs/biome`, then create
`biome.json` at repo root (the `$schema` below is the local,
version-independent form the Biome v2 docs recommend — no URL to adjust):

```json
{
  "$schema": "./node_modules/@biomejs/biome/configuration_schema.json",
  "files": {
    "includes": ["src/**/*.ts", "src/**/*.tsx", "vite.config.ts"],
    "ignoreUnknown": true
  },
  "formatter": { "enabled": true, "indentStyle": "space", "indentWidth": 2, "lineWidth": 100 },
  "linter": {
    "enabled": true,
    "rules": {
      "preset": "recommended",
      "correctness": {
        "useExhaustiveDependencies": "error"
      }
    }
  },
  "javascript": { "formatter": { "quoteStyle": "double" } }
}
```

(Keys verified against the Biome v2 configuration reference 2026-07-18:
`files.includes` / `files.ignoreUnknown` are current; `linter.rules.preset:
"recommended"` replaced the deprecated `linter.rules.recommended`;
`useExhaustiveDependencies` lives in the `correctness` group.
**`lineWidth: 100` is an operator decision (2026-07-18), not a default**:
the codebase's prevailing style is ~100 cols (147 lines >80 vs 33 >100 at
`af9be44`), and Biome's default 80 churned 782 lines across 16 files in
the first executor pass vs 343 at 100 — the narrower wrap was rejected as
unreviewable noise. **Expected first-run findings** (from that pass, all
verified): ~12 a11y diagnostics in `SettingsApp.tsx` (useSemanticElements
& friends — fixing means migrating div+ARIA markup to semantic elements,
which breaks role-based test queries; suppress per the containment rule,
the markup migration is a separate future task), 2 noNonNullAssertion in
`SettingsApp.test.tsx` (Biome's `?.` suggestion provably breaks tsc's
control-flow narrowing — suppress), 2 useExhaustiveDependencies
(deliberate re-trigger keys — suppress with the documented reason), and
3 mechanical useIterableCallbackReturn in test files (safe to apply).
**CSS is deliberately out of scope**: `src/styles.css` and
`src/settings/*.css` use hand-tuned compact one-line rules (e.g. the
`.cat-*` category blocks) that any CSS formatter would explode into
multi-line churn — the gate is for the TS/React side only, matching "Why
this matters". If you believe CSS should be included, STOP and report
rather than widening the glob. 2-space/double-quote matches the existing
code's prevailing style — verify by eyeballing `src/useSlotState.ts`.)

Add to package.json scripts: `"lint": "biome check .",
"lint:fix": "biome check --write ."`.

**Verify**: `npx biome check . 2>&1 | tail -5` runs and reports (pass or a finite list of issues).

### Step 2: Fix or contain the initial findings

Run `npx biome check --write .` (mechanical fixes: formatting, import
order). Then re-run `npx biome check .` and triage what remains:
- Style/format leftovers: fix.
- `useExhaustiveDependencies` findings: **do not blindly add
  dependencies** — a wrong dep on an effect in `useSlotState`/`useClock`/
  `StatusRailCard` can change behavior (re-subscribes, timer resets).
  For each: if the fix is provably behavior-identical (e.g. adding a
  stable setState fn), apply it; otherwise add the inline suppression
  `// biome-ignore lint/correctness/useExhaustiveDependencies: <reason>`
  and list it in your report for the maintainer.
- Anything else that would change logic: suppress + report, per the
  containment rule.

**Verify**: `npx biome check .` → exit 0. Then the full frontend gate: `npx tsc --noEmit && npx vitest run && npx vite build` → all pass (proves the mechanical churn broke nothing).

### Step 3: CI + docs

Add to the web job in ci.yml, after `npm ci`: `- run: npx biome ci .`.
Add the `npx biome check .` command to AGENTS.md's and CLAUDE.md's
commands section (one line each).

**Verify**: `npx biome ci .` locally → exit 0.

## Test plan

No new tests. The gate is: biome exit 0 + the full existing frontend
suite (66 vitest tests) + tsc + vite build green after the churn commit.

## Done criteria

- [ ] `biome.json` exists; `npx biome ci .` exits 0
- [ ] `grep -c "biome" .github/workflows/ci.yml` → ≥1
- [ ] `grep -c '"lint"' package.json` → 1
- [ ] `npx vitest run`, `npx tsc --noEmit`, `npx vite build` all exit 0
- [ ] Every suppression listed in the executor report with its reason
- [ ] AGENTS.md/CLAUDE.md commands updated
- [ ] `plans/README.md` status row updated

## STOP conditions

- The formatter churn commit exceeds ~500 changed lines (with
  `lineWidth: 100` the expected churn is ~343 across 16 files — measured
  in the first executor pass). If you see materially more than that,
  something drifted: report the diffstat and wait for a human call
  before committing.
- Any `useExhaustiveDependencies` fix changes observable behavior
  (a vitest test fails) — revert that fix to a suppression.
- A suppression category appears that isn't in Step 1's expected-
  findings list (a11y cluster, noNonNullAssertion, exhaustive-deps) —
  report it before suppressing.

## Maintenance notes

- Future agents: run `npx biome check --write .` before committing
  frontend changes; CI enforces it.
- CSS was deliberately excluded from Biome's scope (hand-tuned compact
  one-line rules in `styles.css` / `src/settings/*.css` would be
  exploded by any CSS formatter). Enabling CSS later is its own
  decision with a big one-time churn commit — do not widen the glob in
  passing.
- If the repo ever wants rust-style strictness parity, Biome's
  `noExplicitAny`/`noNonNullAssertion` rules are the next ratchet — note
  `noNonNullAssertion` IS in the recommended preset (it fired twice in
  the first pass and was suppressed where the non-null is provably
  safe); the ratchet question is whether to keep those suppressions or
  fix the code.
- The suppressed a11y cluster in `SettingsApp.tsx` (12 findings:
  div+ARIA-role markup vs semantic elements) is a real, separate task —
  fixing it means migrating markup AND the role-based test queries AND
  the hand-tuned class CSS. Re-raise with `improve next` when wanted;
  do not fold it into lint churn.
- Plan 007 landed first in the same ci.yml web job (audit step, ubuntu
  runner, `sh -n`) — the biome step goes alongside, after `npm ci`.
