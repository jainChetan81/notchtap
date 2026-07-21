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
- **Review-plan pass (2026-07-20)**: own read (zero drift — `weather_poller.rs`
  byte-identical since planning). Found a real gap in the original
  audit, not introduced by drift: a test named
  `lookahead_rounds_to_the_nearest_hour` (`weather_poller.rs:557-566`)
  already exists and already covers 2 of this plan's 3 originally
  proposed cases (both are "minute == 30, rounds up" — one against an
  exact-hour target, one against a genuine `:30` boundary). This plan's
  original "round-up-at-30" test (Step 2's second sketch) would have
  been redundant — dropped. The two genuinely uncovered cases
  (round-down just under 30, and day-rollover) remain and are now
  written out as concrete, verified-compiling code instead of comment
  placeholders — see Steps 2/3. Also fixed the same verify-command
  union-not-intersection issue found in every other plan in this batch's
  review, and confirmed the exact field types needed for the
  day-rollover fixture (`OpenMeteoCurrent.time: String`,
  `OpenMeteoHourly.time: Vec<String>` /
  `.precipitation_probability: Vec<u8>` — `weather_poller.rs:47-65`).
- **Review-plan pass (2026-07-21)**: fresh cold read at `647f6d0`. The
  07-20 pass's "zero drift" no longer holds — plan 082 (weather art)
  landed, adding `is_day: u8` (`#[serde(default)]`) to
  `OpenMeteoCurrent`, `is_day: 1` to the Bangalore fixture JSON, and
  `condition`/`is_day` parameters to `alert_event`, shifting every
  cited line in this file. All citations re-fixed to directly-verified
  current locations: `lookahead_rain_probability` 115-128 (**function
  body byte-identical** — the rounding logic this plan tests is
  untouched), structs 47-76 (excerpt updated to include `is_day`),
  `fixture()` 360-362, `fixture_with_rain_probability` 471-479 (body
  unchanged), `lookahead_rounds_to_the_nearest_hour` 646-654 (content
  unchanged, still asserts `Some(16)`/`Some(22)`/`Some(33)`). The
  fixture's `current.time` is still `"2026-07-19T06:30"` and index 7 is
  still 16%, so Step 2's arithmetic (`+59min → 07:29 → rounds down →
  Some(16)`) and Step 3's rollover fixture (`23:45 + 30 → 00:15 →
  2026-07-20T00:00 → Some(42)`) both still check out. Explicit ruling
  on 082's new field: `is_day` needs NO representation in this plan's
  test fixtures — both new tests clone `fixture()` (which now carries
  `is_day: 1`), the field is serde-defaulted, and the function under
  test never reads it (note added under the struct excerpt). Verdict:
  ready to execute after these fixes.

## Why this matters

`lookahead_rain_probability` (`src-tauri/src/weather_poller.rs:115-128`)
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

- `src-tauri/src/weather_poller.rs:115-128` — the full function:

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

- `src-tauri/src/weather_poller.rs:646-654` — the existing
  `lookahead_rounds_to_the_nearest_hour` test, already landed (not part
  of this plan, don't duplicate it):

  ```rust
  #[test]
  fn lookahead_rounds_to_the_nearest_hour() {
      // 06:30 + 30min = 07:00 exactly → index 7 (16% in the fixture).
      assert_eq!(lookahead_rain_probability(&fixture(), 30), Some(16));
      // 06:30 + 60min = 07:30 → rounds to 08:00 → index 8 (22%).
      assert_eq!(lookahead_rain_probability(&fixture(), 60), Some(22));
      // 06:30 + 120min = 08:30 → minute 30 rounds up to 09:00 →
      // index 9 (33%).
      assert_eq!(lookahead_rain_probability(&fixture(), 120), Some(33));
  }
  ```

  This already covers "target lands exactly on an hour" and "minute ==
  30 rounds up" (twice). It does NOT cover a target landing just under
  the `:30` boundary (rounds down) or a day rollover — those are this
  plan's actual scope now.

- `src-tauri/tests/fixtures/open-meteo-bangalore.json` — the fixture
  data `fixture()` deserializes (`weather_poller.rs:360-362`:
  `serde_json::from_str(BANGALORE).unwrap()`, where `BANGALORE =
  include_str!(...)`). It's a **fixed** JSON file, not a parameterized
  builder — `current.time` is hardcoded `"2026-07-19T06:30"`, and
  `hourly.time` covers only that single day's 24 hours
  (`"2026-07-19T00:00"` through `"2026-07-19T23:00"`, with
  `precipitation_probability` `[2,1,0,0,2,5,10,16,22,33,52,75,88,85,72,
  57,42,25,12,6,3,2,1,0]` at matching indices — index 7 = 07:00 = 16%,
  matching the existing test above). There is **no next-day entry** —
  relevant for Step 3.

- `src-tauri/src/weather_poller.rs:471-479` — `fixture_with_rain_probability`,
  the established pattern for building a scenario the static JSON
  fixture doesn't have: clone `fixture()`'s result and mutate the
  already-deserialized struct directly (not the JSON), reusing the same
  technique `poller.rs` uses elsewhere:

  ```rust
  fn fixture_with_rain_probability(probability: u8) -> OpenMeteoResponse {
      // (live version carries an explanatory comment here — elided)
      let mut response = fixture();
      response.hourly.as_mut().unwrap().precipitation_probability[7] = probability;
      response
  }
  ```

- `src-tauri/src/weather_poller.rs:47-76` — `OpenMeteoResponse`'s field
  types, needed to construct the day-rollover fixture in Step 3
  (derive attributes and doc comments elided; note `is_day` was added
  by plan 082 after this plan was written):

  ```rust
  pub struct OpenMeteoResponse {
      pub current: OpenMeteoCurrent,
      #[serde(default)]
      pub hourly: Option<OpenMeteoHourly>,
  }
  pub struct OpenMeteoCurrent {
      pub time: String,
      pub temperature_2m: f64,
      pub weather_code: u8,
      #[serde(default)]
      pub is_day: u8,   // plan 082 — 1/0 integer, serde-defaulted
  }
  pub struct OpenMeteoHourly {
      pub time: Vec<String>,
      #[serde(default)]
      pub precipitation_probability: Vec<u8>,
  }
  ```

  The new `is_day` field needs **no handling in this plan's tests**:
  both new tests build on a clone of `fixture()` (whose JSON now
  carries `is_day: 1`), the field is `#[serde(default)]`, and
  `lookahead_rain_probability` never reads it — do not add it to the
  Step 3 builder.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests, scoped | `cd src-tauri && cargo test --locked weather_poller::` | all pass, including 2 new tests |
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

### Step 1: Confirm the fixture and existing test match "Current state"

Read `weather_poller.rs:47-76` (`OpenMeteoResponse`/`OpenMeteoCurrent`/
`OpenMeteoHourly`), `:360-362` (`fixture()`), `:471-479`
(`fixture_with_rain_probability`), and `:646-654`
(`lookahead_rounds_to_the_nearest_hour`) yourself and confirm they match
the excerpts already inlined in "Current state" above — all four are
reproduced there verbatim so you don't need to re-derive them, just
verify no drift.

**Verify**: no command — reading/confirmation step.

### Step 2: Add the round-down boundary test

The existing fixture's poll time (`06:30`) already makes this a
zero-mutation test — no fixture builder needed, just the right
`lookahead_mins`: `06:30 + 59min = 07:29`, minute `29 < 30`, must round
DOWN to `07:00` (index 7, `16%` — same cell the existing test's first
assertion already uses, just reached via a genuinely-boundary lookahead
instead of a coincidentally-exact-hour one):

```rust
#[test]
fn lookahead_rounds_down_just_under_the_boundary() {
    // 06:30 + 59min = 07:29 — minute 29 is just under the >=30 rounding
    // threshold, so this must round DOWN to 07:00 (index 7, 16%), not
    // up to 08:00 (index 8, 22%). The existing
    // lookahead_rounds_to_the_nearest_hour test only exercises the
    // round-UP side of this boundary (minute == 30) and an exact-hour
    // target (minute == 0) — this is the missing round-DOWN case.
    assert_eq!(lookahead_rain_probability(&fixture(), 59), Some(16));
}
```

**Verify**: `cd src-tauri && cargo test --locked weather_poller::` → all
pass, including `lookahead_rounds_down_just_under_the_boundary`.

### Step 3: Add a day-rollover test

The base fixture has no next-day entry, so this needs its own small
builder, following `fixture_with_rain_probability`'s established
mutate-the-deserialized-struct pattern (not a second JSON file):

```rust
fn fixture_with_late_poll_time_and_next_day_hour() -> OpenMeteoResponse {
    // mirrors fixture_with_rain_probability's technique: mutate the
    // already-deserialized struct rather than editing the JSON fixture,
    // since the base fixture has no next-day hourly entry to round into.
    let mut response = fixture();
    response.current.time = "2026-07-19T23:45".to_string();
    let hourly = response.hourly.as_mut().unwrap();
    hourly.time.push("2026-07-20T00:00".to_string());
    hourly.precipitation_probability.push(42);
    response
}

#[test]
fn lookahead_rolls_over_to_next_day_at_midnight() {
    // poll_time 23:45 + 30min lookahead = next-day 00:15, minute 15 < 30,
    // rounds DOWN to next-day 00:00 — only found if the date component
    // actually rolled over. If only the hour wrapped in place (23→00 on
    // the SAME day), the produced string would be "2026-07-19T00:00",
    // which IS in this fixture — it's the hourly index-0 entry, with
    // probability 2% — so a buggy lookup would still succeed, returning
    // Some(2). That's exactly why the assertion below checks the
    // specific pushed value (42), not just `is_some()`: an `is_some()`
    // assertion would pass under precisely that bug.
    let response = fixture_with_late_poll_time_and_next_day_hour();
    assert_eq!(lookahead_rain_probability(&response, 30), Some(42));
}
```

**Verify**: `cd src-tauri && cargo test --locked weather_poller::` → all
pass, including `lookahead_rolls_over_to_next_day_at_midnight`.

### Step 4: Full suite + lint

**Verify**:
- `cd src-tauri && cargo test --locked` → all pass, rust total baseline + 2
- `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` → exit 0
- `cd src-tauri && cargo fmt --check` → exit 0

## Test plan

- 2 new tests in `src-tauri/src/weather_poller.rs`'s `mod tests`:
  `lookahead_rounds_down_just_under_the_boundary`,
  `lookahead_rolls_over_to_next_day_at_midnight` (both written out in
  full in Steps 2/3).
- Pattern: `fixture_with_rain_probability` (`weather_poller.rs:471-479`)
  for the mutate-the-deserialized-struct technique Step 3's new builder
  follows.
- Verification: `cargo test --locked weather_poller::` → all pass,
  including the 2 new cases and the pre-existing
  `lookahead_rounds_to_the_nearest_hour` unmodified.

## Done criteria

Machine-checkable. ALL must hold. The file-scope bullet below is about
*source* files only — `plans/README.md` and (conditionally)
`docs/TESTING_STRATEGY.md` are the standard bookkeeping exemption every
plan in this repo's index carries, not a contradiction of it:

- [ ] `cargo test --locked` exits 0; rust total is baseline + 2
- [ ] `cargo clippy --locked --all-targets -- -D warnings` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] The pre-existing `lookahead_rounds_to_the_nearest_hour` test still passes unmodified
- [ ] No *source* files outside `src-tauri/src/weather_poller.rs` modified (`git status` — `plans/README.md` and, if applicable, `docs/TESTING_STRATEGY.md` are expected to change too; everything else is out of scope)
- [ ] `plans/README.md` status row for 074 updated
- [ ] Update `docs/TESTING_STRATEGY.md` §0's `weather_poller` count if this plan lands before plan 071 (the docs truth pass) — otherwise note it in this plan's completion note

## STOP conditions

- The function at `weather_poller.rs:115-128` doesn't match the excerpt
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

---

**Review-plan pass (2026-07-21, second pass at `74fabc7`)**: zero source
drift since the same-day `647f6d0` pass (the only commit between them is
docs-only), and every citation re-confirmed by direct read —
`lookahead_rain_probability` 115-128 byte-identical, structs 47-76,
`fixture()` 360-362, `fixture_with_rain_probability` 471-479,
`lookahead_rounds_to_the_nearest_hour` 646-654, and the fixture JSON
(`current.time` `"2026-07-19T06:30"`, 24 hourly entries
`2026-07-19T00:00`–`23:00`, index 7 = 16%, no next-day entry). One real
error found and fixed: Step 3's sketch comment claimed the
wrong-rollover string `"2026-07-19T00:00"` "exists nowhere in this
fixture" — it is in fact the fixture's **index-0 entry (2%)**, so a
date-rollover bug would return `Some(2)`, not `None`. The test's
assertion (`Some(42)`) was already the right guard either way — it fails
under that bug — but the comment's justification was factually wrong and
is now corrected to describe the actual failure mode (and why
`is_some()` alone would falsely pass). No other change needed. Verdict:
ready to execute.
