# Plan 007: Pin the git dependency, lock CI resolution, add vuln scanning, and stop paying macOS rates for Node CI

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report. When done,
> update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat d40445e..HEAD -- src-tauri/Cargo.toml .github/workflows/ci.yml docs/TESTING_STRATEGY.md`
> On any change, compare excerpts below against live files; mismatch = STOP.
> Note: as of commit `b1981c9`, `Cargo.toml` already drifted once (a
> `chrono = "0.4"` line was added above the `[target...macos]` section),
> shifting `tauri-nspanel` from line 38 to line 37. Locate the line by its
> **content** (`tauri-nspanel = {`), never by the line number quoted below
> — that number is stale the moment any earlier line in the file changes.

## Status

- **Priority**: P1
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: deps / dx
- **Planned at**: commit `d40445e`, 2026-07-17

## Why this matters

Four independent supply-chain/CI gaps, all cheap to close:

1. `tauri-nspanel` — a crate running unsafe Objective-C window swizzling
   inside the app process — is pulled from a **branch** of a third-party
   personal GitHub repo. The committed `Cargo.lock` pins a rev today, but
   any `cargo update` (or lockfile regeneration) silently re-floats to
   branch HEAD: unreviewed native code on the next dependency refresh.
2. CI runs `cargo test` without `--locked`, so a drifted/regenerated
   lockfile would be silently re-resolved instead of failing loudly.
3. No dependency-advisory scanning exists anywhere (no `cargo audit`, no
   `npm audit` step) — a RUSTSEC advisory against tokio/reqwest/feed-rs/
   axum would go unnoticed.
4. The CI `web` job (npm ci, tsc, vitest, vite build — pure Node, jsdom
   test env) runs on `macos-latest`: ~10× the billing rate and slower
   queues than Linux, for work with zero macOS dependency. Also,
   `docs/TESTING_STRATEGY.md` claims an `sh -n` syntax gate for the
   `notchtap` CLI script that does not actually exist in CI — add it, and
   the doc becomes true.

## Current state

`src-tauri/Cargo.toml` (under `[target.'cfg(target_os = "macos")'.dependencies]`
— line 37 as of `b1981c9`, but locate by content, not line number; see the
drift-check note above):

```toml
tauri-nspanel = { git = "https://github.com/ahkohd/tauri-nspanel", branch = "v2" }
```

`src-tauri/Cargo.lock` (committed) pins:

```
source = "git+https://github.com/ahkohd/tauri-nspanel?branch=v2#18ffb9a201fbf6fedfaa382fd4b92315ea30ab1a"
```

`.github/workflows/ci.yml` (whole file is ~55 lines; three jobs):
- `rust` job (macos-latest, workdir `src-tauri`): `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test` — none use
  `--locked`.
- `web` job: `runs-on: macos-latest`, node 22, `npm ci`,
  `npx tsc --noEmit`, `npx vitest run`, `npx vite build`.
- `swift` job (macos-latest): `swift build` in `notchtap-detect/` —
  genuinely needs macOS; leave it.

`docs/TESTING_STRATEGY.md:397-399`:

```
- **cli** (`notchtap --priority low|medium|high`): manual only, same
  as the rest of the script (§4.8) — `sh -n` syntax check is the only
  automated gate
```

(No such gate exists in ci.yml today.)

`npm audit` at planning time: 0 vulnerabilities. `cargo-audit` is not
installed on the dev machine.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust build resolves identically | `cargo build --locked` (from `src-tauri/`) | exit 0, no re-resolution |
| Rust tests | `cargo test --locked` (from `src-tauri/`) | all pass |
| Script syntax | `sh -n notchtap` (repo root) | exit 0, silent |
| Workflow syntax | push to a branch / `gh workflow view ci` after push, or `actionlint` if installed | parses |

## Scope

**In scope**:
- `src-tauri/Cargo.toml` (one line: add `rev = ...`)
- `.github/workflows/ci.yml`
- `docs/TESTING_STRATEGY.md` (only if wording needs the "gate now exists
  in ci" tense fix — coordinate with plan 004, which touches nearby lines)
- `plans/README.md` (status row)

**Out of scope**:
- `src-tauri/Cargo.lock` — the pinned commit must NOT change (that is
  the whole point of the pin: same resolution, now durable). One
  cosmetic 1-line diff IS expected and correct: the `source =` string
  for `tauri-nspanel` switches from `?branch=v2#<sha>` to
  `?rev=<sha>#<sha>` — same commit hash on both sides, cargo just
  encodes which key (`branch` vs `rev`) was used in `Cargo.toml`. If
  the *sha itself* changes, or any other package's entry changes,
  that's the real STOP condition.
- Bumping tauri-nspanel to a newer rev — separate, deliberate work
  needing the manual notch/hud hardware checklist.
- `package.json` / npm deps.
- The `swift` and `rust` jobs' runner choice (both need macOS).

## Git workflow

- Current branch; commit style: `ci: pin nspanel rev, --locked, audit scans, linux web job, sh -n gate`
  (or split into two commits: `deps:` + `ci:`).
- Do NOT push unless the operator asked; note that CI verification
  ultimately requires a push.

## Steps

### Step 1: Pin the rev in Cargo.toml

Find the `tauri-nspanel = { ... }` line under
`[target.'cfg(target_os = "macos")'.dependencies]` (locate by content —
see the drift-check note at the top of this plan, the line number has
already moved once). Cargo rejects `branch` and `rev` together as an
"ambiguous" dependency specification (confirmed error: "Only one of
`branch`, `tag` or `rev` is allowed" — this is not toolchain-specific,
it's a hard cargo rule). Drop `branch` and keep only `rev`:

```toml
tauri-nspanel = { git = "https://github.com/ahkohd/tauri-nspanel", rev = "18ffb9a201fbf6fedfaa382fd4b92315ea30ab1a" }
```

**Verify**: `cargo build --locked` from `src-tauri/` → exit 0 AND `git diff d40445e -- src-tauri/Cargo.lock` → only a `tauri-nspanel` `source =` line changes, and only the query key (`?branch=v2#<sha>` → `?rev=<sha>#<sha>`) — the sha itself (`18ffb9a201fbf6fedfaa382fd4b92315ea30ab1a`) must be identical on both sides. Any other line changing, or the sha differing, is a STOP.

### Step 2: `--locked` in CI

In `.github/workflows/ci.yml`'s rust job, change:
- `cargo clippy --all-targets -- -D warnings` → `cargo clippy --locked --all-targets -- -D warnings`
- `cargo test` → `cargo test --locked`

(`cargo fmt --check` takes no resolution; leave it.)

**Verify**: run both commands locally from `src-tauri/` → exit 0.

### Step 3: Add advisory scanning

Add to ci.yml:
- In the rust job, add a `permissions:` block at the job level (the
  workflow currently has none, and the default token permission for
  this repo is read-only):
  ```yaml
  permissions:
    contents: read
    checks: write
  ```
  Then add a step using the pinned tag (not the `v2` branch ref, to
  stay consistent with this plan's own "stop floating on unpinned
  refs" thesis):
  ```yaml
  - uses: rustsec/audit-check@v2.0.0
    with:
      token: ${{ secrets.GITHUB_TOKEN }}
      working-directory: src-tauri
  ```
  (Confirmed inputs: `token` required, `ignore` optional
  comma-separated advisory-id list, `working-directory` optional,
  default `.`. Without `checks: write`, the action still runs — it
  falls back to printing advisories to stdout instead of posting a
  Check — but grant the permission so it produces its normal Check
  annotation.)
- In the web job, after `npm ci`: `npm audit --audit-level=high`.

**Verify**: locally, `npm audit --audit-level=high` → exit 0 (0 vulns at planning time). `rustsec/audit-check` itself only runs inside GitHub Actions (it shells out to `cargo-audit` via the `@actions/github`/`@actions/core` toolkit and needs the Actions runtime) — CI is the verification for that step; note it as pending push in your report.

### Step 4: Move the web job to Linux and add the script gate

- `web` job: `runs-on: macos-latest` → `runs-on: ubuntu-latest`.
- Add a step to the web job (before or after tests): `sh -n notchtap`
  (working-directory: repo root — the default).

**Verify**: `sh -n notchtap` locally → exit 0. `npx vitest run` locally → all pass (jsdom is platform-neutral; the CI run on Linux is the final proof — flag in your report that one green CI run on a PR is required before this plan is DONE).

## Test plan

No new unit tests. Verification is: unchanged Cargo.lock, all local
gates green, and one green CI run on the modified workflow (all three
jobs plus the new steps).

## Done criteria

- [ ] `grep -c 'rev = "18ffb9a' src-tauri/Cargo.toml` → 1
- [ ] `git diff d40445e -- src-tauri/Cargo.lock` → only the `tauri-nspanel` `source =` line's query key changed (`branch=v2` → `rev=<sha>`), same sha both sides; no other package touched
- [ ] `grep -c '\-\-locked' .github/workflows/ci.yml` → ≥2
- [ ] `grep -c 'sh -n notchtap' .github/workflows/ci.yml` → 1
- [ ] `grep -c 'ubuntu-latest' .github/workflows/ci.yml` → 1 (web job only)
- [ ] `grep -c 'audit' .github/workflows/ci.yml` → ≥2 (cargo + npm)
- [ ] `cargo test --locked` from `src-tauri/` exits 0
- [ ] One green CI run (or explicitly reported as pending push)
- [ ] `plans/README.md` status row updated

## STOP conditions

- `cargo build --locked` fails, or `Cargo.lock` shows any change beyond
  the expected `tauri-nspanel` source-string query-key switch (see
  Scope) — a changed sha or any other package's entry means the
  lockfile and the pin disagree; report, do not `cargo update`.
- `vitest` fails on ubuntu-latest (would indicate a real platform
  dependency the audit judged absent) — report; do not paper over with
  `continue-on-error`.

## Maintenance notes

- Future nspanel upgrades are now a deliberate rev bump in Cargo.toml +
  Cargo.lock together, followed by the manual notch/hud checklist
  (`docs/IMPLEMENTATION_PLAN.md` §6) — the crate swizzles the overlay
  panel; treat every bump as hardware-affecting.
- Upstream health check (deferred, needs network): whether newer
  `tauri-nspanel` revs migrate off the deprecated `objc`/`cocoa` crates
  to `objc2` — worth folding into the next deliberate bump.
- If `cargo audit` starts failing on an advisory in a transitive dep with
  no fix available, add an `--ignore RUSTSEC-...` with a dated comment
  rather than removing the step.
- Plan 016 (frontend lint) also edits the web job in ci.yml — land these
  in either order but expect a trivial textual merge.
