# Plan 068: `build_test_event`/`send_test_notification` have zero test coverage

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

- **Priority**: P2
- **Effort**: S–M
- **Risk**: LOW
- **Depends on**: none
- **Category**: tests
- **Planned at**: commit `f6c2f46`, 2026-07-20

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

- `src-tauri/src/settings.rs:542-...` — `build_test_event`, a
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

- `src-tauri/src/settings.rs:727-741` — `send_test_notification`, the
  command that calls it:

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

Repeat for `Manual`, `Cmux`, `News`, `Weather` — for each, assert the
branch pulls its `priority`/ttl-or-display-secs from the *matching*
config field (e.g. Weather's test should set a distinct
`weather_priority`-equivalent field and a distinct ttl/display value in
the fixture `Config`, then assert those exact values landed on the
constructed `Event` — this is the check that would have caught a
copy-paste bug like "Weather's branch accidentally reads
`espn_ttl_secs`"). Match whatever the actual per-source config field
names turn out to be once you've read all 5 branches in Step 1 — don't
guess field names from this sketch.

**Verify**: `cd src-tauri && cargo test --locked settings:: -- build_test_event` (adjust filter) → 5 new tests pass.

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

- [ ] `cargo test --locked` exits 0; rust total is baseline + 5
- [ ] `cargo clippy --locked --all-targets -- -D warnings` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] Each of the 5 new tests asserts at least one config-field-sourced value (not just the constant fields like `event_type`) — a test that only checks `event_type` wouldn't catch the copy-paste bug this plan exists to guard against
- [ ] No files outside `src-tauri/src/settings.rs` modified (`git status`)
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
