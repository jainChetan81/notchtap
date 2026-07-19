# Plan 067: `rotation_order` self-heal doesn't dedupe pre-existing duplicates

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat f6c2f46..HEAD -- src-tauri/src/config.rs`
> If the file changed since this plan was written, compare the "Current
> state" excerpt against the live code before proceeding; on a mismatch,
> treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `f6c2f46`, 2026-07-20

## Why this matters

Commit `fb4acce` ("config: heal a rotation_order missing a source at load
time", 2026-07-19) added a self-heal in `Config::parse` so that a
`rotation_order` array written before a `SourceKind` variant existed
(e.g. an install predating plan 040's `weather` source) gets the missing
variant appended automatically, rather than permanently failing
`settings::validate`'s "must be a permutation of all five sources" check
with no in-UI way to fix it (the rotation-order widget only reorders
existing entries; it can't add one back).

The heal loop only appends missing variants — it never dedupes existing
ones:

```rust
for source in default_rotation_order() {
    if !config.rotation_order.contains(&source) {
        config.rotation_order.push(source);
    }
}
```

If a config's `rotation_order` has a duplicate entry *and* is missing a
different one (e.g. `["football", "football", "manual", "weather"]` —
duplicate `football`, missing `news` and `cmux`), the heal appends the
two missing sources and produces a 6-element array that still contains
the duplicate. `settings::validate`'s permutation check requires
`len() == 5`, so this "healed" config fails validation forever — exactly
the "no in-UI escape hatch short of Reset to defaults" lockout the
`fb4acce` fix exists to prevent, just via a different root cause. This
was verified live during the audit: `Config::parse` on the array above
heals to `[Football, Football, Manual, Weather, Cmux, News]` (6 entries,
duplicate still present).

This is a narrow, hand-edited-TOML-only edge case (unreachable through
the Settings UI, which only reorders) — low day-to-day likelihood, but it
directly defeats the stated purpose of a fix that was shipped the day
before this plan was written, so it's worth closing now while the
context is fresh rather than letting it surface as a confusing "reset to
defaults didn't even fully work" report later.

## Current state

- `src-tauri/src/config.rs:394-410` — the heal, with its own doc
  comment already explaining the failure mode it exists to prevent
  (quoted here so you don't need to re-derive the rationale):

  ```rust
  // heal a `rotation_order` written before a `SourceKind` variant
  // existed (e.g. an install from before plan 040 Part B added
  // `weather`): `settings::validate` requires a permutation of all
  // five variants, but the settings UI's rotation-order list is a
  // fixed reorder-only widget (it just renders whatever's already in
  // the array) with no way for the user to add a missing one back —
  // so a stale array fails validation on every save, permanently,
  // with no in-UI escape hatch short of "Reset to defaults" (which
  // also discards every other customized setting). Append whatever's
  // missing, preserving the file's existing relative order for
  // everything it already had; this self-heals for any future
  // newly-added source too, not just this one.
  for source in default_rotation_order() {
      if !config.rotation_order.contains(&source) {
          config.rotation_order.push(source);
      }
  }
  ```

- `src-tauri/src/config.rs:249` — `default_rotation_order()`, the
  canonical 5-source list this heal iterates (don't need to change it,
  just confirm its shape: `Vec<SourceKind>` covering all 5 variants).

- `src-tauri/src/config.rs` `mod tests` — the two existing tests for
  this heal (locate via
  `grep -n "rotation_order_is_overridable\|rotation_order_missing_a_source_is_healed_by_appending_it" src-tauri/src/config.rs`):
  one confirms a full 5-entry override is respected verbatim, the other
  confirms a 4-entry (missing `weather`) array heals by appending
  `Weather` at the end. Neither exercises a duplicate-plus-missing case —
  this plan adds that.

- `src-tauri/src/settings.rs:181-200` (approximate — re-locate via
  `grep -n "rotation_order must contain" src-tauri/src/settings.rs`) —
  the permutation check this heal must satisfy:

  ```rust
  // rotation_order must be a permutation of all five SourceKind variants
  let is_permutation = c.rotation_order.len() == expected_sources.len()
      && expected_sources
          .iter()
          .all(|source| c.rotation_order.contains(source));
  ```

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests, scoped | `cd src-tauri && cargo test --locked config::` | all pass, including the new test |
| Full suite | `cd src-tauri && cargo test --locked` | all pass |
| Clippy | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Format | `cd src-tauri && cargo fmt --check` | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/config.rs` (the heal fix + new test)

**Out of scope**:
- `src-tauri/src/settings.rs`'s permutation check — it's correct as-is;
  the fix belongs entirely on the healing side, not the validation side.
- Any change to `default_rotation_order()`'s canonical ordering.

## Steps

### Step 1: Dedupe before appending

Change the heal to first deduplicate `config.rotation_order` (keeping
the first occurrence of each `SourceKind`, to preserve the "existing
relative order" guarantee the doc comment already promises), then append
any of the 5 canonical sources still missing:

```rust
// dedupe first (keep first occurrence — a malformed hand-edited config
// might repeat a source; without this, a duplicate-plus-missing array
// would grow past 5 elements and fail `validate`'s permutation check
// forever, the same lockout this heal exists to prevent).
let mut seen = std::collections::HashSet::new();
config.rotation_order.retain(|s| seen.insert(*s));
for source in default_rotation_order() {
    if !config.rotation_order.contains(&source) {
        config.rotation_order.push(source);
    }
}
```

Adjust the doc comment above the block to mention the dedupe step too
(one added sentence is enough — don't rewrite the whole comment).

**Verify**: `cd src-tauri && cargo build` → exit 0. Confirm `SourceKind`
implements `Eq + Hash` (it almost certainly already does, since it's used
as a `HashSet`/map key elsewhere in this codebase — check
`src-tauri/src/event.rs`'s `SourceKind` derive list before assuming; if
it doesn't, use a `Vec`-based `contains` dedupe instead of `HashSet`,
mirroring the existing `.contains(&source)` idiom already in this
function, just applied to a `retain`).

### Step 2: Add a regression test

Add a new test in `config.rs`'s `mod tests`, right after
`rotation_order_missing_a_source_is_healed_by_appending_it` (the
existing single-missing-source test), covering the duplicate-plus-missing
case:

```rust
#[test]
fn rotation_order_with_duplicate_and_missing_sources_heals_to_exactly_five() {
    // a malformed config: `football` appears twice, `news` and `cmux`
    // are both missing entirely. The heal must both dedupe the
    // duplicate AND append the two missing sources, landing at exactly
    // 5 unique entries — not 6, which would still fail `validate`'s
    // permutation check.
    let c = Config::parse(
        "rotation_order = [\"football\", \"football\", \"manual\", \"weather\"]\n",
    )
    .unwrap();
    assert_eq!(c.rotation_order.len(), 5);
    let mut sorted = c.rotation_order.clone();
    sorted.sort_by_key(|s| format!("{s:?}"));
    let mut expected = vec![
        SourceKind::Football,
        SourceKind::Manual,
        SourceKind::Weather,
        SourceKind::News,
        SourceKind::Cmux,
    ];
    expected.sort_by_key(|s| format!("{s:?}"));
    assert_eq!(sorted, expected);
    // first-occurrence order preserved for what the file already had:
    // football (deduped to one), manual, weather stay in that relative
    // order; only the appended news/cmux go at the end.
    assert_eq!(c.rotation_order[0], SourceKind::Football);
    assert_eq!(c.rotation_order[1], SourceKind::Manual);
    assert_eq!(c.rotation_order[2], SourceKind::Weather);
}
```

Adjust the exact assertion style (e.g. if `SourceKind` doesn't implement
`Debug`-sortable formatting the way this sketch assumes) to match
whatever's idiomatic elsewhere in this file's existing tests — the two
existing tests you read in "Current state" are the closest pattern.

**Verify**: `cd src-tauri && cargo test --locked config:: -- duplicate` (adjust filter to your test name) → passes.

### Step 3: Full suite + lint

**Verify**:
- `cd src-tauri && cargo test --locked` → all pass, rust total baseline + 1
- `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` → exit 0
- `cd src-tauri && cargo fmt --check` → exit 0

## Test plan

- 1 new test in `src-tauri/src/config.rs`'s `mod tests`:
  `rotation_order_with_duplicate_and_missing_sources_heals_to_exactly_five`
  (or your chosen name) — covers the duplicate-plus-missing case Step 1
  fixes.
- Pattern: model after
  `rotation_order_missing_a_source_is_healed_by_appending_it`, the
  existing single-missing-source test in the same file.
- Verification: `cargo test --locked config::` → all pass, including the
  new case; re-confirm the two pre-existing rotation_order tests still
  pass unchanged (the dedupe must not alter their behavior — neither
  fixture has duplicates).

## Done criteria

- [ ] `cargo test --locked` exits 0; rust total is baseline + 1
- [ ] `cargo clippy --locked --all-targets -- -D warnings` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] The two pre-existing `rotation_order_*` tests in `config.rs` still pass unmodified
- [ ] No files outside `src-tauri/src/config.rs` modified (`git status`)
- [ ] `plans/README.md` status row for 067 updated
- [ ] Update `docs/TESTING_STRATEGY.md` §0's `config` count if this plan lands before plan 071 (the docs truth pass) — otherwise note it in this plan's completion note

## STOP conditions

- The code at `config.rs:394-410` doesn't match the excerpt above (drift
  since planning).
- `SourceKind` doesn't derive `Eq`/`Hash`/`Clone`/`Copy` as assumed by
  Step 1's sketch — check its derive list first; if it's missing a trait
  the sketch needs, use the `Vec`-based dedupe fallback mentioned in
  Step 1 rather than adding a new derive to `SourceKind` (that's a wider
  blast-radius change outside this plan's scope — report it instead).

## Maintenance notes

- This heal (and its dedupe) is a defensive measure for hand-edited or
  corrupted TOML — the Settings UI itself can never produce a
  duplicate-or-missing `rotation_order` today, since it only reorders a
  pre-validated array. If a future settings-UI change ever allows adding/
  removing rotation-order entries directly, re-check whether this
  load-time heal is still the right layer for the fix, or whether it
  should move to the save path instead.
