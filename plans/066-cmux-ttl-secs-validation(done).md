# Plan 066: `cmux_ttl_secs` is missing from `settings::validate` — a saved `0` rotates cmux cards out instantly

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat f6c2f46..HEAD -- src-tauri/src/settings.rs`
> If the file changed since this plan was written, compare the "Current
> state" excerpt against the live code before proceeding; on a mismatch,
> treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `f6c2f46`, 2026-07-20

## Why this matters

`settings::validate` (`src-tauri/src/settings.rs`) range-checks every
other rotation-window field to `1..=3600` seconds — `default_ttl`
(line 58), `espn_ttl_secs` (line 76), `rss_ttl_secs` (line 99) — but has
no check at all for `cmux_ttl_secs`. The Settings window's "Rotation
seconds" input for the Cmux Relay section is a plain
`<input type="number" min={1} max={3600}>`; `min`/`max` are cosmetic HTML
attributes the browser doesn't strictly enforce on programmatic value
changes, and the save path never re-validates them beyond what
`validate()` itself checks — so a user can type `0`, save, and the config
persists with `cmux_ttl_secs = 0`.

Downstream, `src-tauri/src/http.rs:172-179` feeds `state.cmux_ttl_secs`
directly into `RotationSpec::OneShot { ttl_secs }` for every cmux-origin
`/notify` push, and `queue.rs`'s `rotate_out_if_elapsed` rotates an item
out as soon as `elapsed_secs < window` is false — with `window == 0`
that's true on the very next tick, so a cmux-relayed card gets promoted
and evicted almost instantly instead of holding for the intended
duration. Per this repo's own memory notes, cmux is the one connector
confirmed live-wired and in active use on the dev machine — this is a
user-facing gap on a real, actively-used path, not a theoretical one.

## Current state

- `src-tauri/src/settings.rs:49-105` — `validate()`'s existing ttl
  checks (three of the four ttl-bearing config fields; `cmux_ttl_secs` is
  the missing fourth):

  ```rust
  pub fn validate(c: &Config) -> Result<(), Vec<String>> {
      let mut errors = Vec::new();

      if c.port < 1024 {
          errors.push(format!(
              "port must be 1024–65535 (got {}) — privileged ports fail to bind at boot",
              c.port
          ));
      }
      if !(1..=3600).contains(&c.default_ttl) {
          errors.push(format!(
              "default_ttl must be 1–3600 seconds (got {})",
              c.default_ttl
          ));
      }
      if !(1..=1000).contains(&c.max_queued_per_tier) {
          errors.push(format!(
              "max_queued_per_tier must be 1–1000 (got {})",
              c.max_queued_per_tier
          ));
      }
      if !(5..=3600).contains(&c.espn_poll_secs) {
          errors.push(format!(
              "espn_poll_secs must be 5–3600 (got {}) — below 5s is abuse of a free endpoint",
              c.espn_poll_secs
          ));
      }
      if !(1..=3600).contains(&c.espn_ttl_secs) {
          errors.push(format!(
              "espn_ttl_secs must be 1–3600 seconds (got {})",
              c.espn_ttl_secs
          ));
      }
      // ... (leagues, rss_poll_secs, rss_ttl_secs, etc. — cmux_ttl_secs is
      // never checked anywhere in this function)
  ```

  Confirm this yourself with `grep -n "cmux_ttl_secs" src-tauri/src/settings.rs`
  before writing the fix — at planning time this returns zero matches
  inside `validate()`.

- `src-tauri/src/settings.rs:822-838` (approximate — re-locate via
  `grep -n "espn_ttl_secs = 3600\|espn_ttl_secs = 3601" src-tauri/src/settings.rs`)
  — the exemplar test pattern for a boundary-tested ttl field, to copy
  for `cmux_ttl_secs`:

  ```rust
  c.espn_ttl_secs = 3600;
  // ... assert validate(&c).is_ok()
  c.espn_ttl_secs = 3601;
  // ... assert validate(&c) rejects, error mentions "espn_ttl_secs"
  ```

  Read the full test (both the accept-at-boundary and reject-past-boundary
  halves, plus whatever `0` boundary test exists for a sibling field) and
  mirror its exact structure — same `Config::default()` base, same
  assertion style.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests, scoped | `cd src-tauri && cargo test --locked settings::` | all pass, including the new tests |
| Full suite | `cd src-tauri && cargo test --locked` | all pass |
| Clippy | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Format | `cd src-tauri && cargo fmt --check` | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/settings.rs` (the `validate()` fix + new tests)

**Out of scope**:
- `src/settings/SettingsApp.tsx` — the frontend's `min={1} max={3600}`
  attributes are already correct as a UX hint; this plan's server-side
  fix is the actual enforcement boundary (same division of labor as the
  three existing ttl checks, which also have no frontend-side duplicate
  logic beyond the same cosmetic `min`/`max`). Don't add frontend
  validation logic — it's deliberately not this repo's pattern (see
  `plans/README.md`'s rejected-findings note on "advisory min/max props
  duplicating validate ranges" — accepted duplication, enforcement is
  server-side).
- `src-tauri/src/http.rs`, `src-tauri/src/queue.rs` — no change needed;
  once `validate()` rejects out-of-range values at save time, the
  downstream rotation logic never sees an invalid `cmux_ttl_secs`.

## Steps

### Step 1: Add the missing range check

Insert a check for `c.cmux_ttl_secs` into `validate()`, matching the
exact style of the three existing ttl checks. Put it near
`espn_ttl_secs`'s check (both are per-source ttl fields) rather than at
the end of the function, so the four ttl checks stay visually grouped:

```rust
if !(1..=3600).contains(&c.cmux_ttl_secs) {
    errors.push(format!(
        "cmux_ttl_secs must be 1–3600 seconds (got {})",
        c.cmux_ttl_secs
    ));
}
```

**Verify**: `cd src-tauri && cargo build` → exit 0.

### Step 2: Add boundary tests

Add two tests in `settings.rs`'s `mod tests`, modeled exactly on the
`espn_ttl_secs` boundary tests you read in "Current state" above — same
structure, swap the field name:

- Accept at `3600`, reject at `3601` (upper boundary)
- Reject at `0` (lower boundary) — check whether the existing
  `espn_ttl_secs` tests already cover a `0` case; if so, mirror that
  exact test too, if not, add one anyway since `1..=3600` excludes `0`
  and this is the most likely real-world mistake (an empty/cleared input
  field parsing as `0`).

**Verify**: `cd src-tauri && cargo test --locked settings:: -- cmux_ttl_secs` (adjust the filter to your actual test names) → new tests pass.

### Step 3: Full suite + lint

**Verify**:
- `cd src-tauri && cargo test --locked` → all pass, rust total baseline + 2 or 3 (depending how many tests you added in Step 2)
- `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` → exit 0
- `cd src-tauri && cargo fmt --check` → exit 0

## Test plan

- 2-3 new tests in `src-tauri/src/settings.rs`'s `mod tests`: accept at
  `3600`, reject at `3601`, reject at `0` — following the `espn_ttl_secs`
  boundary-test pattern already in the file (locate via
  `grep -n "espn_ttl_secs = 36" src-tauri/src/settings.rs`).
- Verification: `cargo test --locked settings::` → all pass, including
  the new cases.

## Done criteria

- [ ] `cargo test --locked` exits 0; rust total is baseline + 2 or 3
- [ ] `cargo clippy --locked --all-targets -- -D warnings` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] A scratch test confirms `validate(&Config { cmux_ttl_secs: 0, ..Default::default() })` now returns `Err(...)` (remove the scratch test before finishing — it's covered by Step 2's permanent tests)
- [ ] No files outside `src-tauri/src/settings.rs` modified (`git status`)
- [ ] `plans/README.md` status row for 066 updated
- [ ] Update `docs/TESTING_STRATEGY.md` §0's `settings` count if this plan lands before plan 071 (the docs truth pass) — otherwise note it in this plan's completion note

## STOP conditions

- The code at `settings.rs:49-105` doesn't match the excerpt above
  (drift since planning) — re-locate the ttl checks by `grep` and adjust.
- `cmux_ttl_secs` turns out to already be checked somewhere else in the
  file you didn't find via the grep above — if so, this finding is
  stale; report it and don't make a redundant change.

## Maintenance notes

- If a future ttl-bearing source is added (mirroring plan 040's
  `weather` addition, which did NOT introduce a new ttl field — weather
  alerts reuse `default_ttl`), make sure its config field gets the same
  `1..=3600` treatment at the time it's added, not discovered later by a
  future audit the way this one was.
