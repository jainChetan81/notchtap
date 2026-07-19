# Plan 074: Test `lookahead_rain_probability`'s minute-rounding/day-rollover boundary

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat f6c2f46..HEAD -- src-tauri/src/weather_poller.rs`
> If the file changed since this plan was written, compare the "Current
> state" excerpt against the live code before proceeding; on a mismatch,
> treat it as a STOP condition.

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: tests
- **Planned at**: commit `f6c2f46`, 2026-07-20

## Why this matters

`lookahead_rain_probability` (`src-tauri/src/weather_poller.rs:104-117`)
computes a target timestamp by rounding the current poll time forward to
the nearest hour (per Open-Meteo's hourly forecast granularity) based on
a configurable lookahead window, then looks up that hour's precipitation
probability. The rounding logic has two edge cases neither of the
existing weather-threshold tests (added in plan 047's test backfill)
exercises:

1. **Minute-boundary rounding**: `target.minute() >= 30` rounds up to the
   next hour, `< 30` rounds down to the current hour — the exact `== 30`
   boundary and values just either side of it are untested.
2. **Day rollover**: if `poll_time + lookahead` crosses midnight (e.g. a
   poll at 23:45 with a 30-minute lookahead lands at 00:15 the next day),
   the rounding must correctly roll the date forward, not just the hour —
   untested.

This function fails safe today (an out-of-range lookup returns `None` via
`?`, which the caller already treats as "no rain-incoming alert this
poll" — not a crash), so this is a coverage gap, not a live bug. But it's
unpinned: a future change to the rounding logic (e.g. adjusting the `>=
30` threshold, or the `%Y-%m-%dT%H:%M` format string) could silently
break the day-rollover case with no test to catch it.

## Current state

- `src-tauri/src/weather_poller.rs:104-117` — the full function:

  ```rust
  fn lookahead_rain_probability(response: &OpenMeteoResponse, lookahead_mins: u16) -> Option<u8> {
      let hourly = response.hourly.as_ref()?;
      let poll_time = NaiveDateTime::parse_from_str(&response.current.time, "%Y-%m-%dT%H:%M").ok()?;
      let target = poll_time + chrono::Duration::minutes(i64::from(lookahead_mins));
      let hour_start = target.with_minute(0)?.with_second(0)?.with_nanosecond(0)?;
      let rounded = if target.minute() >= 30 {
          hour_start + chrono::Duration::hours(1)
      } else {
          hour_start
      };
      let target_str = rounded.format("%Y-%m-%dT%H:%M").to_string();
      let index = hourly.time.iter().position(|t| *t == target_str)?;
      hourly.precipitation_probability.get(index).copied()
  }
  ```

- `src-tauri/src/weather_poller.rs`'s existing `mod tests` — find the
  plan-047 threshold-boundary tests (search for a test with
  `lookahead_rain_probability` or `rain_threshold` in its name) to see
  how this codebase constructs an `OpenMeteoResponse` test fixture (the
  `hourly.time`/`hourly.precipitation_probability` arrays need
  matching-length, ISO-timestamp-formatted entries) — copy that fixture
  construction pattern exactly rather than inventing a new one.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests, scoped | `cd src-tauri && cargo test --locked weather_poller::` | all pass, including 3 new tests |
| Full suite | `cd src-tauri && cargo test --locked` | all pass |
| Clippy | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Format | `cd src-tauri && cargo fmt --check` | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/weather_poller.rs` (new tests only — no production code
  change; the function fails safe already, this is pure coverage)

**Out of scope**:
- The function's actual rounding logic — don't change it as part of this
  plan unless Step 1's reading turns up a genuine bug (see the STOP
  condition below; this plan's job is coverage, not a fix).
- `diff_weather` or any other function in this file — scoped to
  `lookahead_rain_probability` only.

## Steps

### Step 1: Read the existing fixture-construction pattern

Find the plan-047 weather threshold tests (search
`grep -n "fn.*threshold\|fn.*lookahead" src-tauri/src/weather_poller.rs`)
and read one in full to learn: how an `OpenMeteoResponse` fixture is
built (likely a helper function constructing `hourly.time`/
`hourly.precipitation_probability` as parallel arrays), what timestamp
format the fixture uses (must match `%Y-%m-%dT%H:%M` for
`lookahead_rain_probability`'s own parsing to succeed), and how
`response.current.time` gets set as the "poll time" anchor.

**Verify**: no command — reading step.

### Step 2: Add minute-boundary tests

Add 2-3 tests asserting the `>= 30` rounding boundary, using the fixture
pattern from Step 1:

```rust
#[test]
fn lookahead_rounds_down_at_29_minutes_past() {
    // poll_time + lookahead lands at HH:29 — must round DOWN to HH:00
    let response = /* fixture: current.time such that poll_time + lookahead == "...T14:29", hourly containing both "...T14:00" and "...T15:00" with distinguishable probability values */;
    let result = lookahead_rain_probability(&response, /* lookahead_mins */);
    assert_eq!(result, /* the T14:00 entry's probability */);
}

#[test]
fn lookahead_rounds_up_at_30_minutes_past() {
    // poll_time + lookahead lands at HH:30 exactly — must round UP to (HH+1):00
    // ... same shape, target minute == 30
}
```

Adjust exact helper/fixture calls to match Step 1's discovered pattern —
this sketch shows the intent (distinguishable probability values at the
two candidate hours, so the assertion proves which one was actually
picked), not literal code to paste.

**Verify**: `cd src-tauri && cargo test --locked weather_poller:: -- lookahead_rounds` (adjust filter) → 2 tests pass.

### Step 3: Add a day-rollover test

```rust
#[test]
fn lookahead_rolls_over_to_next_day_at_midnight() {
    // poll_time = "...T23:45", lookahead_mins = 30 → target = next day
    // 00:15, rounds down to next-day "...T00:00" — hourly must contain
    // an entry for the NEXT calendar day to be found at all
}
```

**Verify**: `cd src-tauri && cargo test --locked weather_poller:: -- lookahead_rolls_over` (adjust filter) → passes.

### Step 4: Full suite + lint

**Verify**:
- `cd src-tauri && cargo test --locked` → all pass, rust total baseline + 3
- `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` → exit 0
- `cd src-tauri && cargo fmt --check` → exit 0

## Test plan

- 3 new tests in `src-tauri/src/weather_poller.rs`'s `mod tests`:
  round-down-at-29, round-up-at-30, day-rollover-at-midnight.
- Pattern: whatever fixture-construction helper the existing plan-047
  threshold tests already use (located in Step 1) — reuse it, don't
  duplicate a second fixture-building approach in the same file.
- Verification: `cargo test --locked weather_poller::` → all pass,
  including the 3 new cases.

## Done criteria

- [ ] `cargo test --locked` exits 0; rust total is baseline + 3
- [ ] `cargo clippy --locked --all-targets -- -D warnings` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] Each new test's fixture contains at least two candidate hourly
      entries with distinguishable probability values, so the assertion
      actually proves which hour was selected (a fixture with only one
      hourly entry can't distinguish "rounded correctly" from "got lucky")
- [ ] No files outside `src-tauri/src/weather_poller.rs` modified (`git status`)
- [ ] `plans/README.md` status row for 074 updated
- [ ] Update `docs/TESTING_STRATEGY.md` §0's `weather_poller` count if this plan lands before plan 071 (the docs truth pass) — otherwise note it in this plan's completion note

## STOP conditions

- The function at `weather_poller.rs:104-117` doesn't match the excerpt
  above (drift since planning).
- While writing the day-rollover test, you find the function actually
  DOES have a bug at that boundary (e.g. `with_minute(0)` on a
  `NaiveDateTime` doesn't roll the date forward the way `chrono`'s API
  might be assumed to) — STOP, do not silently fix it inside this
  coverage plan; report what you found so the operator can decide if it
  needs its own bugfix plan (this function currently fails safe via `?`,
  so even a real edge-case bug here is low-severity, but still worth a
  deliberate decision rather than a drive-by fix).

## Maintenance notes

- If Open-Meteo's response format or this function's rounding logic ever
  changes, these 3 boundary tests are exactly the ones most likely to
  need updating alongside it — they're intentionally testing the
  "interesting" edges, not just a happy path.
