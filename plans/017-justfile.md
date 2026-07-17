# Plan 017: Add a justfile — one-command local verification across the split working directories

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report. When done,
> update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat d40445e..HEAD -- AGENTS.md CLAUDE.md package.json .github/workflows/ci.yml`
> (Read-only inputs; this plan creates a new file plus doc edits.)

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none (if plans 007/016 have landed, mirror their added
  gates in the recipes — see Step 1's note)
- **Category**: dx
- **Planned at**: commit `d40445e`, 2026-07-17

## Why this matters

Local verification is a six-command, two-working-directory dance: `cargo
fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
(all from `src-tauri/`); `npx tsc --noEmit`, `npx vitest run`, `npx vite
build` (from repo root). AGENTS.md itself asks for this ("consider adding
a justfile … prevents 'oops i ran vitest from src-tauri' errors"), and in
an agent-driven repo every session currently re-derives the dance. The
"one-command way to know the codebase works" exists only in CI today.

## Current state

- No `justfile` or `Makefile` at repo root.
- `AGENTS.md` (~lines 76–80) and `CLAUDE.md` carry the "consider adding a
  `justfile` (or `Makefile`)" paragraph suggesting recipes: `dev`,
  `test-rust`, `test-web`, `test-all`, `build`, `push "title" "body"`.
- CI (`.github/workflows/ci.yml`) is the authoritative gate list; the
  justfile must mirror it, not invent new gates.
- The CLI script is `./notchtap --title <t> --body <b>` (flags only).
- `just` may not be installed on the machine (`which just` to check —
  install is `brew install just`, but do NOT install it yourself if
  absent; write the file and verify with `make`-free dry checks per
  Step 2).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Syntax check | `just --list` (if just installed) | lists recipes |
| Full local gate | `just test-all` (if installed) | exit 0 |
| Fallback check | `just -n check` (dry-run, if installed) | prints commands |

## Scope

**In scope**:
- `justfile` (new, repo root)
- `AGENTS.md` + `CLAUDE.md`: replace the "consider adding" paragraph with
  the actual recipe list, and add `just test-all` to the commands section
- `plans/README.md` (status row)

**Out of scope**:
- ci.yml (CI keeps its explicit steps — the justfile mirrors CI, CI does
  not call just)
- package.json scripts
- Installing `just` system-wide (operator's call; note it in the report)

## Git workflow

- Current branch; commit style: `dx: justfile — dev/test/check recipes mirroring ci`.
- Do NOT push.

## Steps

### Step 1: Write the justfile

Create `justfile` at repo root:

```just
# notchtap task runner — mirrors .github/workflows/ci.yml exactly.
# `just test-all` before calling any phase done (IMPLEMENTATION_PLAN.md §6).

default:
    @just --list

# run the app in dev mode
dev:
    npm run tauri dev

# rust gates (run from src-tauri, as CI does)
test-rust:
    cd src-tauri && cargo test

check-rust:
    cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings

# frontend gates
test-web:
    npx vitest run

check-web:
    npx tsc --noEmit

build-web:
    npx vite build

# script gate
check-cli:
    sh -n notchtap

# everything CI runs, locally
test-all: check-rust test-rust check-web test-web build-web check-cli

# manual push against the local endpoint
push title body:
    ./notchtap --title "{{title}}" --body "{{body}}"
```

Notes: `cd src-tauri &&` inside a recipe is correct just-usage for this
repo's split (each recipe line runs in its own shell). **If plans 007/016
landed first**, add their gates: `--locked` on the cargo invocations
(007) and an `npx biome ci .` line in `check-web` (016) — check
`.github/workflows/ci.yml`'s current steps and mirror exactly what it
runs at the time you execute this plan.

**Verify**: if `just` is installed: `just --list` shows all recipes and `just check-cli` exits 0. If not installed: verify the file manually — every command in it, run by hand from the stated directory, must exit 0 (run at least `sh -n notchtap` and one rust + one web gate); state in your report that `just` itself was unavailable.

### Step 2: Run the full gate once

`just test-all` (or the manual equivalent) — everything green.

**Verify**: exit 0 across all recipes.

### Step 3: Update the agent docs

In AGENTS.md and CLAUDE.md: delete the "consider adding a justfile"
paragraph, add `just test-all` (and `just push "t" "b"`) to the commands
list with one line noting the justfile mirrors CI.

**Verify**: `grep -c "consider adding a" AGENTS.md CLAUDE.md` → 0 in each.

## Test plan

None — the justfile is itself a test runner; Step 2 is the proof.

## Done criteria

- [ ] `justfile` exists at root; recipes match CI's current gate list
- [ ] `just test-all` exits 0 (or manual-equivalent run documented)
- [ ] AGENTS.md/CLAUDE.md updated, "consider adding" paragraph gone
- [ ] `plans/README.md` status row updated

## STOP conditions

- CI's gate list at execution time differs from both this plan's recipe
  set AND what plans 007/016 describe — report the delta rather than
  guessing which is authoritative.

## Maintenance notes

- The justfile mirrors CI — every future CI step addition gets a matching
  recipe edit (reviewers: check both files move together).
- Plan 004 also edits AGENTS.md/CLAUDE.md (project-state paragraph) —
  different sections, trivial merge.
