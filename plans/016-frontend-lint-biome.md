# Plan 016: Add a frontend lint/format gate (Biome) to match the fully-gated rust side

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report. When done,
> update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat d40445e..HEAD -- package.json .github/workflows/ci.yml`
> Coordinate with plan 007 (it also edits ci.yml's web job) — land in
> either order; expect a trivial textual merge.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW (first run may flag existing code; fixes are mechanical — see Step 2's containment rule)
- **Depends on**: none (007 recommended first to settle ci.yml)
- **Category**: dx
- **Planned at**: commit `d40445e`, 2026-07-17

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
  `test: vitest run`.
- CI web job (`.github/workflows/ci.yml`): `npm ci`, `npx tsc --noEmit`,
  `npx vitest run`, `npx vite build`.
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
- `src/**`, `vite.config.ts` — ONLY mechanical changes produced by
  `biome check --write` / explicitly-listed rule fixes (see Step 2's
  containment rule)
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
`biome.json` at repo root (adjust the schema URL to the installed
version — `npx biome --version`):

```json
{
  "$schema": "./node_modules/@biomejs/biome/configuration_schema.json",
  "files": {
    "includes": ["src/**", "vite.config.ts"],
    "ignoreUnknown": true
  },
  "formatter": { "enabled": true, "indentStyle": "space", "indentWidth": 2 },
  "linter": {
    "enabled": true,
    "rules": {
      "recommended": true,
      "correctness": {
        "useExhaustiveDependencies": "error"
      }
    }
  },
  "javascript": { "formatter": { "quoteStyle": "double" } }
}
```

(Biome's config format has evolved across majors — if `files.includes` is
rejected, consult `npx biome check --help` / the generated schema and use
the installed version's equivalent keys. The intent that must survive:
only `src/**` + `vite.config.ts` are in scope; hooks exhaustive-deps is an
error; 2-space/double-quote matches the existing code's prevailing style
— verify by eyeballing `src/useSlotState.ts`.)

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
suite (60 vitest tests) + tsc + vite build green after the churn commit.

## Done criteria

- [ ] `biome.json` exists; `npx biome ci .` exits 0
- [ ] `grep -c "biome" .github/workflows/ci.yml` → ≥1
- [ ] `grep -c '"lint"' package.json` → 1
- [ ] `npx vitest run`, `npx tsc --noEmit`, `npx vite build` all exit 0
- [ ] Every suppression listed in the executor report with its reason
- [ ] AGENTS.md/CLAUDE.md commands updated
- [ ] `plans/README.md` status row updated

## STOP conditions

- Biome cannot express the file scoping for this layout (unlikely) or
  its formatter fights the existing style so hard the churn commit
  exceeds ~500 changed lines — report the diffstat and wait for a human
  call before committing.
- Any `useExhaustiveDependencies` fix changes observable behavior
  (a vitest test fails) — revert that fix to a suppression.

## Maintenance notes

- Future agents: run `npx biome check --write .` before committing
  frontend changes; CI enforces it.
- If the repo ever wants rust-style strictness parity, Biome's
  `noExplicitAny`/`noNonNullAssertion` rules are the next ratchet — off
  by default here to keep the initial diff small.
- Plan 007 edits the same ci.yml web job (audit step, ubuntu runner) —
  trivial merge either direction.
