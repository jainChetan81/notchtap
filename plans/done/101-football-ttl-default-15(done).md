# Plan 101: Football card default TTL becomes 15s (without breaking the v6.1 inherit heal)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to
> the next step. If anything in "STOP conditions" occurs, stop and report
> — do not improvise. The reviewer maintains `plans/README.md` — do not
> edit it.
>
> **Worktree preflight (run first)**: agent worktrees can branch from a
> stale HEAD. Run `git log --oneline master ^HEAD`; if it prints
> anything, run `git merge --ff-only master` and confirm success.
>
> **Drift check (run second)**: `git diff --stat 39f0fb1..HEAD -- src-tauri/src/config.rs`
> On any change, compare the excerpts below against live code; on a
> content mismatch, STOP.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW (one default + one heal condition; the subtlety is fully specced)
- **Depends on**: none (do not run concurrently with another plan editing `config.rs`)
- **Category**: config/polish
- **Planned at**: commit `39f0fb1`, 2026-07-21

## Why this matters

Operator feedback from first real use (2026-07-21): football cards
rotate away too fast to read a scoreline — the default should be ~15s
(it is already configurable; only the DEFAULT changes). Today
`espn_ttl_secs` defaults to 8, and — the subtlety — an absent
`espn_ttl_secs` key currently inherits `default_ttl` unconditionally,
which would silently override a new 15s default back to 8 on any config
that omits the key. The heal must become conditional on the file
actually customizing `default_ttl`, which is the only case the heal was
built for.

## Current state

- `src-tauri/src/config.rs`:
  - `:219-221`:
    ```rust
    fn default_espn_ttl_secs() -> u64 {
        8
    }
    ```
  - `:428-441` inside `Config::parse` (v6.1 heal — comment explains it
    exists so "an install that had already customized default_ttl before
    this split ... keeps the value it actually configured"):
    ```rust
    if let Ok(raw) = content.parse::<toml::Table>() {
        if !raw.contains_key("espn_ttl_secs") {
            config.espn_ttl_secs = config.default_ttl;
        }
        if !raw.contains_key("cmux_ttl_secs") {
            config.cmux_ttl_secs = config.default_ttl;
        }
    }
    ```
  - Tests pinning current behavior (all in `config.rs`'s test module):
    - `:537` — an empty/absent-key parse asserts `espn_ttl_secs == 8`
      (via the inherit from `default_ttl`'s own default of 8).
    - `:754-758` — a config setting `default_ttl = 20` and no
      `espn_ttl_secs` asserts espn inherits 20 (the heal's core promise
      — MUST keep passing unchanged).
    - `:764-766` — explicit `espn_ttl_secs = 5` wins over
      `default_ttl = 20` (must keep passing unchanged).
    - `:777` — another absent-key assert of `8`.
  - `settings.rs:78-81` validates `espn_ttl_secs` 1–3600 — 15 is in
    range; no validation change.

## Commands you will need

Run from `src-tauri/` with the PATH prefix:

| Purpose | Command | Expected |
|---|---|---|
| Tests | `PATH="$HOME/.cargo/bin:$PATH" cargo test` | all pass |
| Lint | `PATH="$HOME/.cargo/bin:$PATH" cargo clippy --all-targets -- -D warnings` | exit 0 |
| Format | `PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check` | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/config.rs` (default fn, the espn arm of the heal, tests)
- `docs/V3_6_TECHNICAL_SPEC.md` / `docs/V5_TECHNICAL_SPEC.md` — ONLY if
  `grep -n "espn_ttl" docs/*.md` shows a stated default of 8; update that
  number in place, nothing else.

**Out of scope**:
- The `cmux_ttl_secs` arm of the heal — unchanged, unconditional, as is.
- `default_ttl` itself, `rss_ttl_secs`, all validation ranges.
- `settings.rs`, the settings UI (it reads defaults via
  `get_default_config` and follows automatically).
- `docs/TESTING_STRATEGY.md` §0 — unless your test edits change the
  config-suite count; then update §0's config number and the total.

## Git workflow

- Branch: your dispatched worktree branch. One conventional commit, e.g.
  `feat(config): football default TTL 15s, espn inherit-heal made conditional (plan 101)`.
  Do NOT push.

## Steps

### Step 1: New default + conditional heal

1. Change `default_espn_ttl_secs()` to return `15`, with a comment:
   operator decision 2026-07-21 — a scoreline needs longer on screen
   than a generic alert; still configurable via `espn_ttl_secs`.
2. Make the espn arm of the heal conditional on the file having
   customized `default_ttl`:
   ```rust
   if !raw.contains_key("espn_ttl_secs") && raw.contains_key("default_ttl") {
       config.espn_ttl_secs = config.default_ttl;
   }
   ```
   Extend the heal's comment: the inherit exists ONLY for configs that
   customized the old shared `default_ttl`; a config that never touched
   it gets espn's own default (15, plan 101) instead of silently
   re-inheriting the generic default. The cmux arm stays unconditional
   (its default intentionally tracks `default_ttl`).

**Verify**: `PATH="$HOME/.cargo/bin:$PATH" cargo build` → exit 0.

### Step 2: Retarget the absent-key tests, add the new-default pin

1. Update the asserts at `:537` and `:777` from `8` to `15` (these parse
   configs with neither key — they now get espn's own default).
2. The tests at `:754-758` (customized `default_ttl` → inherit) and
   `:764-766` (explicit wins) must pass UNCHANGED — if either fails,
   STOP.
3. Add one test `espn_ttl_defaults_to_15_when_default_ttl_untouched`:
   parse `""` (empty config) → `espn_ttl_secs == 15` AND
   `default_ttl == default_ttl()`'s value (pin that the generic default
   did not move); and parse `"default_ttl = 30\n"` →
   `espn_ttl_secs == 30` (the heal still honors a customized shared
   default).

**Verify**: `PATH="$HOME/.cargo/bin:$PATH" cargo test config` → all pass
including the new test; the two must-not-change tests pass without edits.

### Step 3: Docs sweep for the stated default

`grep -rn "espn_ttl" docs/` — if any doc states the default as 8, update
the number (and only the number) to 15. Report which files, or "none".

**Verify**: `grep -rn "espn_ttl" docs/ | grep -v TESTING` shows no
remaining claim of an 8s default.

### Step 4: Full gates

**Verify**: `cargo test` / `clippy -D warnings` / `fmt --check` all
clean; `git status` → only in-scope files. If your test count changed
the config suite size, update §0 (config number + rust total) to the
observed `cargo test` output.

## Test plan

Covered in Step 2: two retargeted asserts, one new two-case pin, two
pinned-unchanged heal tests. The full config suite is the regression
net.

## Done criteria

- [ ] `default_espn_ttl_secs()` returns 15; heal espn arm is conditional on `raw.contains_key("default_ttl")`
- [ ] Tests at `:754-758` and `:764-766` pass byte-unchanged
- [ ] New test exists and passes; absent-key asserts now expect 15
- [ ] All gates clean; `git status` only in-scope files
- [ ] §0 updated iff the count changed

## STOP conditions

- Either must-not-change test (`:754-758`, `:764-766`) fails after your
  edit — the heal semantics drifted, report.
- The excerpts above don't match live code.
- Any OTHER test outside the config suite fails (espn poller tests
  consume `espn_ttl_secs` — a failure there means a hidden coupling to
  the 8s value; report it, don't patch the other suite).

## Maintenance notes

- The espn and cmux heal arms are now deliberately DIFFERENT (conditional
  vs unconditional) — the comment explains why; a future "simplify the
  duplication" refactor would be a behavior change.
- Operator's real config lives at `~/.config/notchtap/config.toml`
  (never read it): if it has a customized `default_ttl` and no
  `espn_ttl_secs`, football keeps inheriting — expected; they can set
  15 in Settings.
- Reviewer: check the new heal condition against the v6.1 comment's
  stated intent — it should now IMPLEMENT that comment more precisely
  than the original code did.
