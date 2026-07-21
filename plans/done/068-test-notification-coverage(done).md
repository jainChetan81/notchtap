# Plan 068: `build_test_event`/`send_test_notification` have zero test coverage

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat f6c2f46..HEAD -- src-tauri/src/settings.rs`
> **This WILL show changes** — since planning, plans 066 (`cmux_ttl_secs`
> validation), 076 (Telegram connector health: `ConnectorHealthDto`/
> `get_connector_health`), 077 (`get_recent_log_lines`), 085
> (`resting_state` in the appearance payload), and 083 (one line inside
> `build_test_event` itself: the News arm's `EventMeta` literal gained
> `espn: None`) have all landed. That's expected and already accounted
> for in the citations below (572/758, not the original 542/727 or the
> previously-corrected 564/749) — not a STOP condition on its own. Only
> treat this as a STOP condition if `build_test_event`'s or
> `send_test_notification`'s own content differs from what's quoted
> below beyond the noted `espn: None` line, not just their line numbers.

## Status

- **Priority**: P2
- **Effort**: S–M
- **Risk**: LOW
- **Depends on**: none
- **Category**: tests
- **Planned at**: commit `f6c2f46`, 2026-07-20
- **Review-plan pass (2026-07-20)**: own read (zero drift — `settings.rs`
  byte-identical since planning at the cited lines; read all 5 match
  arms in full, not just the Football excerpt). Confirmed the Football
  sketch in Step 2 is accurate as written. Found one real gap: the plan
  treated all 5 branches as symmetric ("assert the branch pulls its
  priority/ttl-or-display-secs from the *matching* config field... set a
  distinct weather_priority-equivalent field and a distinct ttl/display
  value"), but they aren't — Football/News/Cmux each read a *dedicated*
  per-source ttl field (`espn_ttl_secs`/`rss_ttl_secs`/`cmux_ttl_secs`),
  while Manual and Weather both read the *shared* `default_ttl` (there
  is no `manual_ttl_secs`/`weather_ttl_secs`). An executor following the
  original "set a distinct ttl value" instruction literally for Weather
  would hunt for a field that doesn't exist. Step 2 is corrected below
  with the verified real field names for all 5 branches, so nothing is
  left to guess.
- **Second review-plan pass (2026-07-20, same day)**: re-verified drift
  — plans 066 and 076 landed *after* the first pass (which had correctly
  found zero drift at the time), shifting `build_test_event` from
  `settings.rs:542` to `:564` and `send_test_notification` from `:727-741`
  to `:749-763`. Confirmed both functions' actual content is still
  byte-identical to what this plan already quotes — pure line-number
  drift, not a content change. Fixed both citations below.

## Why this matters

The Settings window's per-source "test notification" buttons (one per
`SourceKind`, added in v5.1) are backed by two functions in
`src-tauri/src/settings.rs`: `build_test_event` (a 5-branch match
constructing a representative `Event` per source) and
`send_test_notification` (the `#[tauri::command]` that calls it and
routes through `Engine::accept`). Neither has any test coverage anywhere
in the codebase — confirmed via `grep -n "build_test_event\|send_test_notification" src-tauri/src/settings.rs`,
which returns only the two production-code definition/call sites, no
`#[cfg(test)]` references. The one frontend test that exercises the
"click a test-notification button" flow mocks `invoke` away entirely, so
it never actually runs `build_test_event`'s branch logic.

This matters because `build_test_event` is a 5-way match with real
per-source differences (different `EventType`, different `RotationSpec`,
different `EventPayload` bodies, at minimum) — exactly the shape most
likely to have one branch silently wrong (a copy-paste artifact, a wrong
`ttl_secs` field reused from the wrong config source) without anyone
noticing, since the only way this code runs today is a human clicking a
button in the Settings window and eyeballing the result.

## Current state

- `src-tauri/src/settings.rs:572-664` — `build_test_event`, a
  `match source { ... }` over `SourceKind` (Football shown; read the full
  function yourself — it has one arm per `SourceKind` variant, so 5
  branches total after plan 040 added `Weather`):

  ```rust
  fn build_test_event(config: &Config, source: SourceKind) -> Event {
      let now_ms = Local::now().timestamp_millis();
      match source {
          SourceKind::Football => Event {
              id: uuid::Uuid::new_v4(),
              event_type: EventType::ScoreUpdate,
              priority: config.espn_priority,
              rotation: RotationSpec::OneShot {
                  ttl_secs: config.espn_ttl_secs,
              },
              topic: None,
              payload: EventPayload {
                  title: "Test score update".into(),
                  body: timestamp_body(),
              },
              meta: EventMeta::default(),
              // ... (signal/origin fields — read the full arm)
          },
          // ... 4 more arms: Manual, Cmux, News, Weather
      }
  }
  ```

- `src-tauri/src/settings.rs:758-772` — `send_test_notification`, the
  command that calls it (the live function also carries a plan-037
  explanatory comment before the `engine.accept` line, elided here):

  ```rust
  pub async fn send_test_notification(
      window: tauri::WebviewWindow,
      state: tauri::State<'_, StdMutex<Config>>,
      engine: tauri::State<'_, Engine>,
      source: SourceKind,
  ) -> Result<(), String> {
      ensure_settings_window(&window)?;
      let config = state.inner().lock().unwrap().clone();
      let event = build_test_event(&config, source);
      engine.accept(event, true).await.map_err(|e| e.to_string())
  }
  ```

- `src-tauri/src/settings.rs`'s existing `mod tests` — has extensive
  coverage of `validate()` and other pure functions; find an example of
  how it constructs a `Config` fixture for a test (likely
  `Config::default()` with targeted field overrides) and follow the same
  style for the new tests below. `build_test_event` is already a pure
  function (`&Config, SourceKind) -> Event`, no async/IO — directly
  unit-testable without any mock.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests, scoped | `cd src-tauri && cargo test --locked settings::` | all pass, including 5 new tests |
| Full suite | `cd src-tauri && cargo test --locked` | all pass |
| Clippy | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Format | `cd src-tauri && cargo fmt --check` | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/settings.rs` (new tests only — no production code
  change is needed unless Step 1 actually turns up a real bug in one of
  the 5 branches, in which case see the STOP condition below)

**Out of scope**:
- `send_test_notification` itself — it's a thin `#[tauri::command]`
  wrapper; testing it end-to-end would require a live `Engine`/tauri
  runtime, which is out of proportion for what this plan needs (the
  playbook's own guidance: unit-test the pure logic, don't chase
  integration coverage for a thin wrapper). Unit-testing
  `build_test_event` directly covers the actual per-source branch logic
  this finding is about.
- Frontend (`SettingsApp.tsx`/its test file) — the existing mocked-invoke
  test already covers "clicking the button calls invoke with the right
  source"; that's a different, already-covered concern from "does the
  constructed Event look right per source."

## Steps

### Step 1: Read all 5 branches of `build_test_event` in full

Before writing tests, read the complete function (all 5 match arms) —
don't rely on the single Football excerpt above. Note, for each branch:
the `EventType`, `RotationSpec` (and which config field feeds its
`ttl_secs`/`display_secs`), `priority` source, and `origin`. Cross-check
each against the corresponding production event-construction code for
that source (e.g. Football's test event should plausibly resemble what
`poller.rs`'s real `make_event` produces for a score update — same
`EventType::ScoreUpdate`, same `RotationSpec::OneShot`, same ttl field).

**Verify**: no command — this is a reading step. If you find a branch
that looks wrong (e.g. uses the wrong config field's ttl, or an
`EventType` that doesn't match what that source's real poller emits),
note it in your final report as a candidate follow-up finding, but do
NOT fix it as part of this plan — see the STOP condition below. This
plan's job is coverage, not a bugfix; conflating them makes both harder
to review.

### Step 2: Add one test per `SourceKind` branch

Add a `mod tests` block near the top of `build_test_event` (or extend the
existing `mod tests` if `settings.rs` has one shared block — check
first) with 5 tests, one per source, asserting the branch's key fields:

```rust
#[test]
fn build_test_event_football_uses_espn_config() {
    let config = Config {
        espn_priority: Priority::High,
        espn_ttl_secs: 42,
        ..Config::default()
    };
    let event = build_test_event(&config, SourceKind::Football);
    assert_eq!(event.event_type, EventType::ScoreUpdate);
    assert_eq!(event.priority, Priority::High);
    assert_eq!(event.rotation, RotationSpec::OneShot { ttl_secs: 42 });
    assert_eq!(event.origin, SourceKind::Football);
}
```

Repeat for `News`, `Cmux`, `Manual`, `Weather`, using the verified real
field names (confirmed by reading all 5 arms directly — not a guess):

- **News**: `priority: config.rss_priority`, ttl from
  `config.rss_ttl_secs` — its own dedicated field, same shape as
  Football.
- **Cmux**: `priority: config.cmux_priority`, ttl from
  `config.cmux_ttl_secs` — its own dedicated field.
- **Manual**: `priority: config.manual_default_priority`, ttl from
  `config.default_ttl` — **shared** with Weather (see below), not a
  dedicated `manual_ttl_secs` field. Assert the branch reads
  `default_ttl` specifically (set a distinct value there and confirm it
  lands on the event), not "a distinct Manual-only field" — there isn't
  one.
- **Weather**: `priority: config.weather_priority`, ttl also from
  `config.default_ttl` — the **same shared field Manual uses**. This
  means a test asserting "Weather reads its own ttl field, not a
  sibling's" can't be built the way Football/News/Cmux's can (there's no
  `weather_ttl_secs` to distinguish it from `default_ttl`); assert
  Weather's `priority` comes from `weather_priority` specifically
  (that field IS dedicated) and that its `rotation` reflects
  `default_ttl`, but don't write an assertion implying Weather has a
  private ttl field — it would be testing something that isn't true of
  the code.

For each of the 5, assert `event_type`, `origin`, and whichever of
`priority`/ttl-source is source-specific enough to catch a copy-paste
swap (per the field list above) — not every field needs asserting, just
enough to distinguish "this branch reads its own config" from "this
branch accidentally reads a sibling's."

**Verify**: `cd src-tauri && cargo test --locked settings::` → all
settings tests pass (49 `#[test]`/`#[tokio::test]` fns exist in
`settings.rs` at review time, 2026-07-21, so expect 54 after this
plan), including your 5 new tests (confirm each appears in
the output as `ok`). Don't rely on a second filter after `--` to isolate
them — verified empirically (in this batch's other plan reviews) that
cargo/libtest's pre-`--` and post-`--` filters union rather than
intersect, so `settings::` alone already pulls in the entire module
regardless of what follows.

### Step 3: Full suite + lint

**Verify**:
- `cd src-tauri && cargo test --locked` → all pass, rust total baseline + 5
- `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` → exit 0
- `cd src-tauri && cargo fmt --check` → exit 0

## Test plan

- 5 new tests in `src-tauri/src/settings.rs`, one per `SourceKind`
  branch of `build_test_event`, each asserting that branch reads its
  *own* source's config fields (priority, ttl/display-secs) rather than
  a sibling's — the specific mistake this kind of copy-paste-prone
  5-way match is most likely to hide.
- No existing test to model structurally beyond `settings.rs`'s general
  `Config`-fixture-plus-assert pattern already used throughout its
  `validate()` tests — this is new coverage for a previously-untested
  function, not an extension of an existing test.
- Verification: `cargo test --locked settings::` → all pass, including
  the 5 new cases.

## Done criteria

Machine-checkable. ALL must hold. The file-scope bullet below is about
*source* files only — `plans/README.md` and (conditionally)
`docs/TESTING_STRATEGY.md` are the standard bookkeeping exemption every
plan in this repo's index carries, not a contradiction of it:

- [ ] `cargo test --locked` exits 0; rust total is baseline + 5
- [ ] `cargo clippy --locked --all-targets -- -D warnings` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] Each of the 5 new tests asserts at least one config-field-sourced value (not just the constant fields like `event_type`) — a test that only checks `event_type` wouldn't catch the copy-paste bug this plan exists to guard against; for Manual/Weather that means asserting the shared `default_ttl` landed correctly, not a nonexistent per-source ttl field
- [ ] No *source* files outside `src-tauri/src/settings.rs` modified (`git status` — `plans/README.md` and, if applicable, `docs/TESTING_STRATEGY.md` are expected to change too; everything else is out of scope)
- [ ] `plans/README.md` status row for 068 updated
- [ ] Update `docs/TESTING_STRATEGY.md` §0's `settings` count if this plan lands before plan 071 (the docs truth pass) — otherwise note it in this plan's completion note

## STOP conditions

- Step 1's read-through finds a branch that appears to construct a
  genuinely wrong `Event` (wrong config field, wrong `EventType` for
  that source) — STOP, do not silently fix it inside this coverage plan.
  Report the specific branch and what looks wrong; let the operator
  decide whether it's a real bug worth its own plan or a deliberate
  choice you're missing context for.
- `build_test_event`'s signature or match arms don't match the shape
  described above (drift since planning) — re-read the live function
  and adjust the test sketch accordingly; the core intent (one test per
  branch, asserting config-field provenance) still applies.

## Maintenance notes

- If a 6th `SourceKind` is ever added, `build_test_event` gains a 6th
  match arm and this plan's pattern (one test per arm) should extend
  with it — flag this in a comment near the test block so a future
  editor adding a source remembers to add the matching test, not just
  the match arm.

---

**Review-plan pass (2026-07-21)**: third cold re-verification at HEAD
`647f6d0`, after plans 080-086 landed. Read all 5 match arms of
`build_test_event` and `send_test_notification` in full against the live
file. Line drift only, plus one benign content change: `build_test_event`
moved `:564` → `:572` and `send_test_notification` `:749-763` → `:758-772`
(pushed down by plan 085's `resting_state` additions to the appearance
payload above them), and plan 083 (`512e6e9`, structured `EspnMeta` on
the wire) added exactly one line *inside* `build_test_event` — the News
arm's explicit `EventMeta` literal gained `espn: None`. No other arm
changed (Football/Cmux/Manual/Weather all use `EventMeta::default()`, so
the new `espn` field flows in via `Default`). Citations fixed above.
Everything else re-verified still true: `SourceKind` still has exactly 5
variants (`event.rs:83-89` — plans 080-086 added NO new source, so no
new branch for this plan to cover; 082's weather `is_day` and 085's
`resting_state` never touched `build_test_event`); the per-source
config-field mapping in Step 2 (espn_priority/espn_ttl_secs,
rss_priority/rss_ttl_secs, cmux_priority/cmux_ttl_secs,
manual_default_priority/default_ttl, weather_priority/default_ttl —
Manual and Weather sharing `default_ttl`) is byte-accurate against HEAD;
zero test coverage still holds (`grep -n "build_test_event\|send_test_notification"
src-tauri/src/settings.rs` → only lines 572, 758, 766 — production
definition/call sites, no `#[cfg(test)]` references); `settings.rs` has
an existing `mod tests` (49 test fns) to extend, as Step 2 anticipates.
One forward-looking note for the executor: the News-arm test may assert
`event.meta.espn.is_none()` if convenient, but it is not required —
`espn` is not a config-sourced field, and this plan's copy-paste guard
is about priority/ttl provenance. In-scope file list still complete
(`src-tauri/src/settings.rs` only). Verdict: ready to execute as
corrected.

**Review-plan pass (2026-07-21, second pass at `74fabc7`)**: zero source
drift since the same-day `647f6d0` pass (the only commit between them is
docs-only). Independently re-read all 5 arms of `build_test_event`
(:572-664) and `send_test_notification` (:758-772) — every citation,
the per-source config-field mapping, the News-arm `espn: None` line, the
grep result (572/758/766 only), the 49-test-fn count, and `SourceKind`'s
5 variants (`event.rs:83-89`) all re-confirmed by direct read. One new
verification this pass adds: the Step 2 sketch's `assert_eq!`
comparisons all compile as written — `EventType` (`event.rs:50`),
`Priority` (:59), `SourceKind` (:81), and `RotationSpec` (:91) each
derive `PartialEq`, so the executor will not hit a missing-derive
compile error and be tempted into an out-of-scope production-code
change. No content changes needed. Verdict: ready to execute.
