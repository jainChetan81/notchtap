# Plan 047: test-only backfill for the newest modules — card team-id mismatch, weather threshold boundaries, real-timer test note

> **Executor instructions**: Follow this plan step by step. This is a
> test-only plan — no production code changes anywhere. Run every
> verification command and confirm the expected result before moving to
> the next step. If anything in "STOP conditions" occurs, stop and
> report — do not improvise. When done, update this plan's status row in
> `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat f2cbae6..HEAD -- src-tauri/src/poller.rs src-tauri/src/weather_poller.rs src-tauri/src/engine.rs`
> If any of these files changed since this plan was written, re-read the
> live code at the cited lines before adapting the new tests — several
> steps below quote exact helper function names and fixture values that
> must still match.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW — every change in this plan adds a test; no production
  code in `poller.rs`, `weather_poller.rs`, or `engine.rs` is modified.
- **Depends on**: none (independent of plan 044, which has now landed —
  `5c1ca36` — cleanly, not mid-edit; it changed a different condition in
  the same file, `poller.rs`, but a different function region and
  doesn't conflict with this plan's Step 1. Its landing did shift two of
  this plan's own line-number citations, corrected in the review-plan
  pass note above — always re-run this plan's own drift check rather
  than trusting any citation, this plan's or otherwise)
- **Category**: tests
- **Planned at**: commit `f2cbae6`, 2026-07-19
- **Review-plan pass (2026-07-20)**: independently hand-traced every
  proposed test's compilation and runtime behavior against live code
  (not just the citations) — confirmed all four tests would compile
  and pass exactly as predicted: Step 1's drop-test relies on
  `SbTeam`'s two fields being `pub` (confirmed) and the bucketing
  loop's `if let Some((yellows, reds)) = side { ... } ` producing no
  side effect on the `None` arm (confirmed, read directly); Step 2's
  three boundary tests were traced through `diff_weather`'s full body
  (`weather_poller.rs:143-196`) line by line — with
  `WeatherAlertState::default()` as `prev`, all three `>=`/`<=`
  comparisons evaluate true at the exact literal threshold values with
  no floating-point-precision risk (the test sets the same literal the
  threshold constant holds, not a computed value), each fires exactly
  one event (the other two conditions are unaffected by fixtures that
  mutate only one axis), and all three `payload.title` strings ("High
  temperature", "Low temperature", "Rain expected soon") were
  confirmed byte-exact against the existing passing tests. Also caught
  live, mid-review: **plan 044 landed while this pass was running**
  (`5c1ca36`, poller.rs 30→31 tests), which shifted two of this plan's
  own citations — corrected below: `ucl_fixture_cards_bucket_per_side_
  and_color` moved from `poller.rs:1100` to `poller.rs:1123` (a new
  test landed between it and `red_card_emits_red_card_signal`, which
  stayed at `1068`), and the drift-check header's premise ("if these
  files changed... re-read before adapting") was itself exercised for
  real, not hypothetically — this is a live demonstration that the
  drift check is load-bearing, not boilerplate, on this actively-worked
  repo. Separately, plan 048's `engine.rs` refactor (named-struct
  signature for `StatusState::snapshot`) shifted the two real-timer
  test citations from `engine.rs:496,536` to `engine.rs:501,541` — also
  corrected below. Step 3's target text in `plans/README.md` was
  independently confirmed to still contain the exact stale wording this
  plan describes (`heartbeat_rotates_out_via_deadline_sleep_not_polling`,
  no mention of the `engine.rs` port or the sibling test) — the fix is
  still needed, unchanged by any concurrent landing.

## Why this matters

Three small, independent test-coverage gaps in the newest/least-audited
modules, bundled into one plan since they're all cheap, test-only
additions with no production-code risk:

1. **`poller.rs`'s per-side card bucketing silently drops a card whose
   `team.id` matches neither competitor** — untested. If ESPN ever
   emits a card detail with an unexpected/malformed `team.id` (a
   different competition shape, a neutral-venue entry, a transient feed
   glitch — plausible, since this same poller already treats `own_goal`/
   `red_card` as needing exactly this kind of structural defensiveness),
   that card vanishes from `total_cards()` with zero observability, and
   no test would catch a regression that made the drop worse (e.g.
   silently misattributing it to the wrong side instead).
2. **Weather alert thresholds are inclusive comparisons
   (`>=`/`<=`) with no test at the exact boundary value.** A future
   `>=`→`>` typo would compile clean and pass every existing test,
   silently un-firing the operator's specifically-calibrated 36°C hot /
   14°C cold / 60% rain thresholds (locked in plan 040's `/grill-me`
   session for the operator's actual climate — a generic threshold
   would be nearly useless there, so a silent off-by-one-comparison
   regression matters more here than a typical config default would).
3. **Plan 037's Engine refactor quietly doubled the one previously-
   accepted "real-timer test" exception.** `plans/README.md`'s
   "Findings considered and rejected" section names exactly one test,
   `heartbeat_rotates_out_via_deadline_sleep_not_polling`, as an accepted
   real-timer exception — that test (and its sibling) no longer exist at
   that name/location; plan 037 ported both to `engine.rs` under new
   names. The port is a legitimate, reasonable carry-over, not a new bug
   — but the accepted-findings note now understates the real-timer
   surface (it names a since-deleted test), so a future audit session
   would have to re-derive this instead of reading it. This step is a
   documentation correction, not a test addition.

## Current state

### 1. `poller.rs` — untested card-bucketing drop branch

`src-tauri/src/poller.rs:262-283` (inside the match-diffing function
that builds a `MatchView`):

```rust
let details = comp.map(|c| c.details.as_slice()).unwrap_or(&[]);
// per-side (yellow, red) bucketing (plan 042 — replaces the old
// aggregate count): cross-reference each card detail's own `team.id`
// against the competitor ids above — structural, same discipline as
// `red_card`/`own_goal`. a card whose team id matches neither side
// is not counted (no verified payload does this).
let mut home_cards = (0u32, 0u32);
let mut away_cards = (0u32, 0u32);
for d in details.iter().filter(|d| {
    d.detail_type
        .as_ref()
        .map(|t| t.text.contains("Card"))
        .unwrap_or(false)
}) {
    let team_id = d.team.as_ref().map(|t| t.id.as_str()).unwrap_or("");
    let side = if !team_id.is_empty() && team_id == home_id {
        Some(&mut home_cards)
    } else if !team_id.is_empty() && team_id == away_id {
        Some(&mut away_cards)
    } else {
        None
    };
    // (the loop body below increments `side`'s (yellow, red) tuple
    // when `Some`, and does nothing when `None` — read the few lines
    // after this excerpt to confirm before writing your test)
}
```

The relevant fixture-backed struct fields (`src-tauri/src/poller.rs:76-114`):

```rust
pub struct SbTeam {
    pub id: String,
    pub abbreviation: String,
}

pub struct SbDetail {
    pub detail_type: Option<SbDetailType>,   // .text, e.g. "Yellow Card"
    pub scoring_play: bool,
    pub red_card: bool,
    pub own_goal: bool,
    pub clock: Option<SbClock>,
    pub athletes: Vec<SbAthlete>,
    pub team: Option<SbTeam>,
}
```

The one existing test covering this bucketing,
`ucl_fixture_cards_bucket_per_side_and_color` (`poller.rs:1123-1135` as
of `5c1ca36` — plan 044 landed a new test between it and
`red_card_emits_red_card_signal` mid-review, shifting it from its
original `1100-1112`; re-locate by name with `rg`, don't trust either
number blindly), exercises only the happy path against the real
fixture:

```rust
#[test]
fn ucl_fixture_cards_bucket_per_side_and_color() {
    // plan 042 ground truth, verified directly against the raw
    // fixture json: 6 yellows, 0 reds — home PSG (team.id 160) 2Y,
    // away ARS (team.id 359) 4Y. if these numbers come out different,
    // the `team.id` cross-reference is misattributing cards.
    let sb = parse_scoreboard(UCL).unwrap();
    let snap = view(&sb.events[0]).snap;
    assert_eq!(snap.home_abbrev, "PSG");
    assert_eq!(snap.away_abbrev, "ARS");
    assert_eq!(snap.home_cards, (2, 0));
    assert_eq!(snap.away_cards, (4, 0));
    assert_eq!(snap.total_cards(), 6);
}
```

No test constructs a card detail whose `team.id` matches neither
competitor (PSG's `"160"` nor ARS's `"359"` in the `UCL` fixture).

### 2. `weather_poller.rs` — untested threshold boundary

`src-tauri/src/weather_poller.rs:150,167,181`:

```rust
let crossed = probability >= rain_threshold_pct;   // :150
let hot = temp_c >= temp_hot_c;                     // :167
let cold = temp_c <= temp_cold_c;                   // :181
```

The test module's fixed thresholds, from `default_args()`
(`weather_poller.rs:316-318`):

```rust
fn default_args() -> (Units, u8, u16, f64, f64, u64, Priority) {
    (Units::Celsius, 60, 30, 36.0, 14.0, 8, Priority::Medium)
}
```

— i.e. `rain_threshold_pct = 60`, `temp_hot_c = 36.0`, `temp_cold_c =
14.0` for every test that uses the `diff()` test helper
(`weather_poller.rs:320-328`, which threads `default_args()` into
`diff_weather`).

Existing edge-trigger tests use values well clear of these thresholds:
`fixture_with_temp(37.5)` for hot (`hot_crossing_fires_once_then_stays_silent_then_rearms`,
line 450), `fixture_with_temp(12.0)` for cold
(`cold_crossing_fires_once_then_stays_silent_then_rearms`, line 471),
`fixture_with_rain_probability(75)` for rain
(`rain_crossing_threshold_fires_exactly_once`, line 400). None test the
exact threshold value itself (`36.0`, `14.0`, or `60`).

The two fixture-mutation helpers you'll reuse:

```rust
fn fixture_with_rain_probability(probability: u8) -> OpenMeteoResponse {
    let mut response = fixture();
    response.hourly.as_mut().unwrap().precipitation_probability[7] = probability;
    response
}

fn fixture_with_temp(temp: f64) -> OpenMeteoResponse {
    let mut response = fixture();
    response.current.temperature_2m = temp;
    response
}
```

### 3. `engine.rs` — the ported real-timer tests

`src-tauri/src/engine.rs:501,541` (shifted from the original `496,536`
by plan 048's named-struct refactor of `StatusState::snapshot`, which
touched `engine.rs` between when this plan was written and this
review-plan pass — re-locate by name with `rg` if it's moved again):

```rust
async fn rotation_loop_parked_idle_wakes_on_accept() {
    // plan 036 regression, ported from lib.rs's
    // heartbeat_parked_idle_wakes_on_enqueue_and_rotates_out: ...
```
```rust
async fn rotation_loop_rotates_out_via_deadline_sleep_not_polling() {
    // plan 015, ported from lib.rs's
    // heartbeat_rotates_out_via_deadline_sleep_not_polling: ...
```

Both use real `tokio::time::sleep` calls inside a
`tokio::time::timeout(Duration::from_secs(3), ...)`, same shape as the
one test `plans/README.md`'s "Findings considered and rejected" section
(third-audit-session entry) names as the suite's single accepted
real-timer exception — but by the exact name
`heartbeat_rotates_out_via_deadline_sleep_not_polling`, which no longer
exists at that name or in `lib.rs`.

## Commands you will need

| Purpose | Command (from `src-tauri/`) | Expected on success |
|---|---|---|
| Build | `cargo build --locked` | exit 0 |
| Rust tests | `cargo test --locked` | all pass; §0 count updated |
| Targeted | `cargo test --locked poller::` / `cargo test --locked weather_poller::` | all pass |
| Lint/format | `cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |

## Scope

**In scope** (the only files you should modify):
- `src-tauri/src/poller.rs` — `#[cfg(test)] mod tests` only (add one
  test).
- `src-tauri/src/weather_poller.rs` — `#[cfg(test)] mod tests` only (add
  three tests).
- `plans/README.md` — the "Findings considered and rejected" note about
  the accepted real-timer test (wording fix only, no code implication).
- `docs/TESTING_STRATEGY.md` §0 — bump counts for the four new tests.

**Out of scope** (do NOT touch, even though they look related):
- Any production code in `poller.rs`, `weather_poller.rs`, or
  `engine.rs` — this plan adds tests only; the card-drop behavior and
  the threshold comparisons are correct as-is (this plan is about
  proving they're correct with a test, not changing them).
- Converting the two `engine.rs` real-timer tests to a simulated/paused
  clock — that's a real but separate, higher-effort call (the tests
  specifically prove real deadline-driven wakeup timing; a paused clock
  would fake past the exact thing they exist to prove). This plan only
  fixes the stale documentation reference, it does not attempt that
  conversion.
- `src-tauri/src/queue.rs`'s own accepted real-timer test — unrelated to
  this plan, do not touch.

## Git workflow

- Branch: `advisor/047-newer-module-test-coverage-backfill` (or work
  directly if the operator dispatched you that way).
- Commit style (from `git log`): lowercase `area: imperative summary`,
  e.g. `tests(poller+weather): backfill card-mismatch and threshold-boundary cases`.
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Add the card team-id-mismatch test

In `src-tauri/src/poller.rs`'s `#[cfg(test)] mod tests`, add a test
modeled on `ucl_fixture_cards_bucket_per_side_and_color`
(`poller.rs:1123` at last check, re-locate by name — see "Current
state" above) and on `red_card_emits_red_card_signal`'s technique
for hand-mutating a fixture's card detail (search for how that test
locates `.details.last_mut()` and sets `red_card`/`detail_type.text`,
around `poller.rs:1082-1091` — reuse the same mutation style):

```rust
#[test]
fn card_with_unrecognized_team_id_is_dropped_not_misattributed() {
    let mut sb = parse_scoreboard(UCL).unwrap();
    let baseline_total = view(&sb.events[0]).snap.total_cards();

    // mutate the last card detail's team.id to a value that matches
    // neither PSG ("160") nor ARS ("359")
    let comp = &mut sb.events[0].competitions[0];
    let last_detail = comp
        .details
        .iter_mut()
        .rev()
        .find(|d| {
            d.detail_type
                .as_ref()
                .map(|t| t.text.contains("Card"))
                .unwrap_or(false)
        })
        .expect("fixture has at least one card detail");
    last_detail.team = Some(SbTeam {
        id: "999999".to_string(),
        abbreviation: String::new(),
    });

    let snap = view(&sb.events[0]).snap;
    // the mutated card is dropped, not misattributed to either side —
    // total_cards() is exactly one less than baseline (the mutated
    // detail no longer counts for anyone), not equal to baseline
    // (which would mean it landed somewhere by accident)
    assert_eq!(snap.total_cards(), baseline_total - 1);
}
```

Adjust field/method names if your read of the live code differs
slightly from the excerpts above (e.g. if `SbTeam` isn't directly
constructible from the test module due to visibility — check `pub`
modifiers first; per the earlier excerpt, `SbTeam`'s fields are `pub`,
so this should compile as written). The key assertion is `baseline_total
- 1`, not just "some card is missing" — this proves the drop is total-count-accurate,
which is what would break silently if a future change turned a drop into
an accidental misattribution instead.

**Verify**: `cargo test --locked poller::card_with_unrecognized_team_id`
(from `src-tauri/`) → passes.

### Step 2: Add the three weather-threshold boundary tests

In `src-tauri/src/weather_poller.rs`'s `#[cfg(test)] mod tests`, add
three tests using the exact threshold values from `default_args()`
(`36.0`, `14.0`, `60`):

```rust
#[test]
fn hot_boundary_at_exactly_threshold_fires() {
    let at_threshold = fixture_with_temp(36.0);
    let (_, events, fired) = diff(&at_threshold, WeatherAlertState::default());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload.title, "High temperature");
    assert!(fired.hot_fired);
}

#[test]
fn cold_boundary_at_exactly_threshold_fires() {
    let at_threshold = fixture_with_temp(14.0);
    let (_, events, fired) = diff(&at_threshold, WeatherAlertState::default());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload.title, "Low temperature");
    assert!(fired.cold_fired);
}

#[test]
fn rain_boundary_at_exactly_threshold_fires() {
    let at_threshold = fixture_with_rain_probability(60);
    let (_, events, state) = diff(&at_threshold, WeatherAlertState::default());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload.title, "Rain expected soon");
    assert!(state.rain_fired);
}
```

Check the cold-alert event's exact title text against the existing
`cold_crossing_fires_once_then_stays_silent_then_rearms` test (line
471-ish, not fully quoted above — read it) before assuming "Low
temperature" is correct; use whatever the real title string is.

**Verify**: `cargo test --locked weather_poller::` (from `src-tauri/`) →
all pass, including the three new tests.

### Step 3: Fix the real-timer-test accepted-finding note

In `plans/README.md`'s "Findings considered and rejected" section (third
audit session entry), find the note about
`heartbeat_rotates_out_via_deadline_sleep_not_polling`. Update it to
reflect the current state: the test was ported by plan 037 to
`src-tauri/src/engine.rs` as `rotation_loop_rotates_out_via_deadline_sleep_not_polling`,
and the port brought a sibling
(`rotation_loop_parked_idle_wakes_on_accept`) that also uses real
sleeps — so the accepted exception now covers **two** real-timer tests
in `engine.rs`, not one in `queue.rs`/`lib.rs`. Keep the same "accepted
as-is, revisit only if it flakes in CI" disposition — this step is a
factual correction, not a re-litigation of the original acceptance
decision.

**Verify**: `grep -n "rotation_loop_rotates_out_via_deadline_sleep_not_polling\|rotation_loop_parked_idle_wakes_on_accept" plans/README.md`
→ at least one match in the updated note.

### Step 4: Full suite + docs count reconciliation

**Verify**: `cargo test --locked` (from `src-tauri/`) → all pass,
including the four new tests. `cargo fmt --check && cargo clippy
--locked --all-targets -- -D warnings` → exit 0.

Update `docs/TESTING_STRATEGY.md` §0: bump `poller`'s sub-count by 1 and
`weather_poller`'s sub-count by 3, and the rust total by 4 — but
**recount from the actual `cargo test --locked` summary line**, not by
arithmetic on the numbers quoted in this plan (they drift with every
merged plan).

## Test plan

- New tests: `card_with_unrecognized_team_id_is_dropped_not_misattributed`
  (`poller.rs`), `hot_boundary_at_exactly_threshold_fires`,
  `cold_boundary_at_exactly_threshold_fires`,
  `rain_boundary_at_exactly_threshold_fires` (all three in
  `weather_poller.rs`), modeled on the existing tests named throughout
  the Steps above.
- Verification: `cargo test --locked` (from `src-tauri/`) → all pass,
  including the four new tests; existing tests unaffected (no production
  code changed).

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `cargo test --locked` exits 0; the four new tests exist and pass
- [ ] `cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings`
      exits 0
- [ ] `grep -n "rotation_loop_rotates_out_via_deadline_sleep_not_polling"
      plans/README.md` shows the corrected accepted-finding note
- [ ] `docs/TESTING_STRATEGY.md` §0 counts reconciled against a live
      `cargo test --locked` run (poller +1, weather_poller +3, total +4)
- [ ] No files outside the in-scope list are modified (`git status`)
- [ ] `plans/README.md` status row for this plan updated

## STOP conditions

Stop and report back (do not improvise) if:

- `SbTeam`/`SbDetail`'s field visibility or shape doesn't match the
  excerpts in "Current state" (the codebase has drifted since
  `f2cbae6`) — re-read the live structs before adapting Step 1's test.
- The weather alert event's exact `payload.title` strings ("High
  temperature", "Rain expected soon", and whatever the cold one actually
  is) don't match what Step 2's tests assert — read the real strings
  from the existing passing tests rather than guessing, and correct the
  new tests to match.
- Any of the three new boundary tests reveal the comparison is actually
  NOT inclusive as documented (i.e. the boundary value does NOT fire) —
  that would mean the "Current state" section's read of `>=`/`<=` was
  wrong, or the code changed; STOP and report rather than flipping the
  test's expected assertion to match, since that would silently paper
  over a real behavior change from what plan 040's `/grill-me` session
  locked in.

## Maintenance notes

- The card-bucketing test (Step 1) documents a real, if narrow,
  observability gap: a dropped card produces no log line today. This
  plan does not add one (out of scope — test-only), but a future
  session touching `poller.rs`'s card logic should consider whether a
  `tracing::warn!` on the drop path is worth adding, now that there's a
  test pinning the drop's existence so a future observability addition
  has a regression test to build alongside.
- The weather threshold tests (Step 2) will need updating if plan 040's
  operator-configured defaults (36°C/14°C/60%) are ever changed in
  `config.rs`'s `default_weather_temp_hot_c`/`default_weather_temp_cold_c`/
  `default_weather_rain_threshold_pct` — the test module's `default_args()`
  hardcodes these same values independently for its own harness, so a
  config-default change and a test-harness change are two separate
  edits; a reviewer of any future threshold-default change should check
  whether `default_args()` needs updating too.
- If plan 048 (StatusState::snapshot hardening) or plan 053 (Topic
  supersession generalization spike) are executed after this plan,
  neither touches the same test regions — no coordination needed.
