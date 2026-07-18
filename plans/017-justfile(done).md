# Plan 017: Add a justfile — one-command local verification across the split working directories

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report. When done,
> update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 4af5e8e..HEAD -- AGENTS.md CLAUDE.md package.json .github/workflows/ci.yml`
> (Read-only inputs; this plan creates a new file plus doc edits.)

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none — both 007 and 016 are DONE. Their gates
  (`--locked` cargo from 007; `sh -n notchtap`; `npx biome ci .` from
  016) are all baked into the sample below unconditionally — no
  execution-time judgment call remains
- **Category**: dx
- **Planned at**: commit `d40445e`, 2026-07-17; drift baseline refreshed to `b43a7ca` 2026-07-18; **review-plan pass 2026-07-18 at `cd64e19`** — drift verified trivial (package.json: only 023's lottie removal; AGENTS.md:96 / CLAUDE.md:98 "consider adding" paragraphs confirmed in place; ci.yml unchanged since 007). Sample justfile corrected to mirror CI exactly: `--locked` baked in (007 already landed — the plan's conditional was stale), `audit-web` + `check-swift` recipes added (they're CI gates the sample had omitted), `just -n` fallback row fixed to name a real recipe. **Pass 3 same day, also at `cd64e19`** — re-verified ci.yml gate list, AGENTS.md:96/CLAUDE.md:98 (line refs corrected from stale ~76–80), 016 still TODO/no biome step in ci.yml, `"tauri"` script present in package.json; added: `just`-absent fact (confirmed on this machine) with named manual-fallback commands, cargo PATH-prefix quirk, stage-by-name git guidance for the shared working tree, red-baseline STOP condition, grep exit-code clarification in Step 3's verify, and a brew-install note requirement for the doc edits. **Pass 4 (execute pre-dispatch reconcile) 2026-07-18 at `4af5e8e`** — plan 016 (biome) landed + committed since pass 3: ci.yml web job now runs `npx biome ci .` (line 49, before `npm audit`/`tsc`); AGENTS.md:83 + CLAUDE.md:85 gained an `npx biome check .` command line; the "consider adding" paragraphs remain (AGENTS.md:97 / CLAUDE.md:99). Reconciled: baked `npx biome ci .` into `check-web` unconditionally (removed the 016 conditional), refreshed drift baseline to `4af5e8e`, updated Depends-on. Full CI web-job order to mirror: `biome ci` → `npm audit` → `tsc` → `vitest` → `vite build` → `sh -n`.

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
- `AGENTS.md:96` and `CLAUDE.md:98` carry the "consider adding a
  `justfile` (or `Makefile`)" paragraph suggesting recipes: `dev`,
  `test-rust`, `test-web`, `test-all`, `build`, `push "title" "body"`
  (one occurrence of "consider adding" per file — verified 2026-07-18).
- CI (`.github/workflows/ci.yml`) is the authoritative gate list; the
  justfile must mirror it, not invent new gates.
- The CLI script is `./notchtap --title <t> --body <b>` (flags only).
- `just` is NOT installed on the dev mac mini as of 2026-07-18
  (`which just` → not found; install is `brew install just`, but do NOT
  install it yourself; write the file and take the manual-verification
  path in Steps 1–2). Re-check with `which just` in case the operator
  installed it since.
- On this machine `cargo` may not be on PATH in non-interactive shells —
  if `cargo: command not found`, prefix the command with
  `PATH="$HOME/.cargo/bin:$PATH"` (this is a shell-environment quirk,
  not a plan drift; do not "fix" anything for it).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Syntax check | `just --list` (if just installed) | lists recipes |
| Full local gate | `just test-all` (if installed) | exit 0 |
| Fallback check | `just -n test-all` (dry-run, if installed) | prints the recipe commands without running |
| Manual rust gate | `cd src-tauri && cargo test --locked` | exit 0, all pass |
| Manual web gate | `npx vitest run` (repo root) | exit 0, all pass |
| Manual cli gate | `sh -n notchtap` (repo root) | exit 0, no output |

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
- **Stage only the in-scope files by name** (`git add justfile AGENTS.md
  CLAUDE.md plans/README.md`) — never `git add -A`/`git add .`.
  Concurrent agent sessions share this working tree, so `git status`
  will likely show unrelated modified files (e.g. under `src-tauri/src/`);
  leave them alone and re-check `git status` right before committing.
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

# rust gates (run from src-tauri, as CI does — `--locked` per plan 007)
test-rust:
    cd src-tauri && cargo test --locked

check-rust:
    cd src-tauri && cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings

# frontend gates
test-web:
    npx vitest run

# lint/format + typecheck (biome from plan 016, then tsc — CI order)
check-web:
    npx biome ci . && npx tsc --noEmit

audit-web:
    npm audit --audit-level=high

build-web:
    npx vite build

# script gate
check-cli:
    sh -n notchtap

# swift detect-binary compile check
check-swift:
    cd notchtap-detect && swift build

# everything CI runs, locally (except cargo-audit — see note below)
test-all: check-rust test-rust check-web audit-web test-web build-web check-cli check-swift

# manual push against the local endpoint
push title body:
    ./notchtap --title "{{title}}" --body "{{body}}"
```

Notes: `cd src-tauri &&` inside a recipe is correct just-usage for this
repo's split (each recipe line runs in its own shell). The sample above
already mirrors CI as of 2026-07-18 (`--locked` from 007, `sh -n`,
`npm audit`, swift compile). Two deliberate omissions: **cargo-audit**
(runs in CI as an action; locally it needs a separate `cargo install
cargo-audit` — if the binary exists on your machine, add an
`audit-rust: cargo audit` recipe to `test-all` and note it in your
report; otherwise skip it and say so) and **`npm ci`** (local dev uses
the existing `node_modules`). Plan 016 (biome) has already landed —
`npx biome ci .` is baked into `check-web` above, matching ci.yml's
web job (biome runs first, before `npm audit` and `tsc`). No
execution-time conditional remains; the sample is CI-exact as written.

**Verify**: if `just` is installed: `just --list` shows all recipes and `just check-cli` exits 0. If not installed (the expected case — see Current state): verify manually by running, by hand from the stated directory, at minimum `sh -n notchtap` (repo root), `cd src-tauri && cargo test --locked`, and `npx vitest run` (repo root) — each exit 0; state in your report that `just` itself was unavailable so the recipes were hand-verified.

### Step 2: Run the full gate once

`just test-all` — or, with `just` unavailable, the manual equivalent:
every `test-all` prerequisite's command, run by hand from its stated
directory, in the listed order (`check-rust`, `test-rust`, `check-web`,
`audit-web`, `test-web`, `build-web`, `check-cli`, `check-swift`).

**Verify**: exit 0 across all recipes. A failure here on code you did
not touch is a STOP condition (pre-existing red baseline), not
something to fix.

### Step 3: Update the agent docs

In AGENTS.md and CLAUDE.md: delete the "consider adding a justfile"
paragraph, add `just test-all` (and `just push "t" "b"`) to the commands
list with one line noting the justfile mirrors CI and that `just` needs
`brew install just` (it is not currently installed on the dev machine —
without that note, the docs would tell future sessions to run a command
that fails).

**Verify**: `grep -c "consider adding a" AGENTS.md CLAUDE.md` prints
`AGENTS.md:0` and `CLAUDE.md:0` and exits 1 — the non-zero exit code IS
the pass condition here (grep exits 1 when nothing matches). Then
`grep -c "just test-all" AGENTS.md CLAUDE.md` → at least 1 in each,
exit 0.

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
- Any Step 2 gate fails on code this plan did not touch (this repo
  hosts concurrent agent sessions, so a red baseline from in-flight
  work is plausible): report which gate failed and its output. Do NOT
  fix source files — they are all out of scope — and do NOT commit a
  justfile whose `test-all` has never been seen green.

## Maintenance notes

- The justfile mirrors CI — every future CI step addition gets a matching
  recipe edit (reviewers: check both files move together).
- Plan 016 (biome) already landed: it added the `npx biome check .`
  command line to AGENTS.md:83 / CLAUDE.md:85 and the `npx biome ci .`
  ci.yml step. This plan's `check-web` mirrors that. The doc edits in
  Step 3 touch a different region (the "consider adding" paragraph +
  commands list), so no textual conflict with 016's lines.
- Plan 004 has landed; its AGENTS/CLAUDE edits are simply the text you
  see — no reconciliation conditional remains.
