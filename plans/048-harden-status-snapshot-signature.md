# Plan 048: replace `StatusState::snapshot`'s positional bool/Option signature with named construction

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to
> the next step. If anything in "STOP conditions" occurs, stop and
> report — do not improvise. When done, update this plan's status row in
> `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat f2cbae6..HEAD -- src-tauri/src/status.rs src-tauri/src/engine.rs`
> If either file changed since this plan was written, re-read the live
> function/call sites before adapting — this plan changes a function
> signature and all of its call sites, so an accurate current-state read
> matters more than usual.

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW — pure refactor (signature + call-site shape change),
  no behavior change; every call site is already covered by existing
  tests that would catch a mechanical mistake during the change itself.
- **Depends on**: none
- **Category**: tech-debt
- **Planned at**: commit `f2cbae6`, 2026-07-19

## Why this matters

`StatusState::snapshot` grew from a 4-argument function (plan 034:
`queue, live, espn_enabled, rss_enabled`) to 6 (plan 040 Part B added
`weather, weather_enabled`) while staying purely positional. Three of
the six parameters are plain `bool` and two are same-shaped
`Option<T>` — nothing in the type system distinguishes
`espn_enabled`/`rss_enabled`/`weather_enabled` from each other at a call
site, or `live`/`weather` from each other. A future edit adding a fourth
ambient source (or refactoring one of the two call sites) that
transposes two same-typed arguments would compile cleanly; only a test
whose fixture happens to give the swapped pair different values would
catch it. This isn't a bug today — both real call sites are correct as
verified below — but it's a shape that invites one on the next change,
and the fix is a small, low-risk refactor available right now while
there are still only two production call sites and three test call
sites to update.

## Current state

- `src-tauri/src/status.rs:82-89` — the function signature:

  ```rust
  pub fn snapshot(
      queue: &SingleSlotQueue,
      live: Option<LiveMatchSummary>,
      espn_enabled: bool,
      rss_enabled: bool,
      weather: Option<WeatherSummary>,
      weather_enabled: bool,
  ) -> Self {
      Self {
          paused: queue.is_paused(),
          waiting: queue.total_waiting(),
          football: FootballStatus {
              enabled: espn_enabled,
              live,
          },
          news: NewsStatus {
              enabled: rss_enabled,
          },
          weather: WeatherStatus {
              enabled: weather_enabled,
              current: weather,
          },
          // (paused/waiting fields shown above; football/news/weather follow)
      }
  }
  ```

- Two production call sites in `src-tauri/src/engine.rs`:

  Rotation loop, `engine.rs:277-284`:
  ```rust
  let status = StatusState::snapshot(
      &q,
      live_summary,
      espn_enabled,
      rss_enabled,
      weather_summary,
      weather_enabled,
  );
  ```

  `emit_current_status_blocking`, `engine.rs:331-338`:
  ```rust
  StatusState::snapshot(
      &q,
      live_summary,
      self.espn_enabled,
      self.rss_enabled,
      weather_summary,
      self.weather_enabled,
  )
  ```

- Three test call sites in `src-tauri/src/status.rs`'s
  `#[cfg(test)] mod tests`:
  - `status.rs:244`: `StatusState::snapshot(&queue, None, true, false, None, false)`
  - `status.rs:258`: `StatusState::snapshot(&queue, None, true, false, None, false).waiting`
  - `status.rs:266`: `StatusState::snapshot(&queue, None, true, false, Some(weather_summary()), true)`

## Commands you will need

| Purpose | Command (from `src-tauri/`) | Expected on success |
|---|---|---|
| Build | `cargo build --locked` | exit 0 |
| Rust tests | `cargo test --locked` | all pass, same count as before (pure refactor) |
| Lint/format | `cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Targeted | `cargo test --locked status::` | all `status` tests pass |

## Scope

**In scope** (the only files you should modify):
- `src-tauri/src/status.rs` — the `snapshot` signature and its
  `#[cfg(test)] mod tests` call sites.
- `src-tauri/src/engine.rs` — the two `StatusState::snapshot` call
  sites.

**Out of scope** (do NOT touch, even though they look related):
- `StatusState`'s own field shape (`paused`, `waiting`, `football`,
  `news`, `weather`) or any of `FootballStatus`/`NewsStatus`/
  `WeatherStatus`/`LiveMatchSummary`/`WeatherSummary` — this plan
  changes how `snapshot`'s *inputs* are passed, not the output shape or
  the wire format the frontend consumes.
- `src-tauri/src/lib.rs` or any other file that reads `StatusState`
  fields after construction — nothing downstream of `snapshot`'s return
  value changes.
- `src/useStatusState.ts` or any frontend file — the serialized JSON
  shape is unaffected; this is a rust-internal constructor ergonomics
  change only.

## Git workflow

- Branch: `advisor/048-harden-status-snapshot-signature` (or work
  directly if the operator dispatched you that way).
- Commit style (from `git log`): lowercase `area: imperative summary`,
  e.g. `status: replace snapshot's positional bool/Option args with a named struct`.
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Introduce a named-fields input struct

In `src-tauri/src/status.rs`, add a struct grouping the five non-queue
inputs, placed near `StatusState`'s own definition:

```rust
/// Named-field inputs for [`StatusState::snapshot`] — replaces five
/// positional bool/Option arguments (three same-typed `bool`s, two
/// same-shaped `Option`s) that a future call-site edit could transpose
/// without a compile error. Construct with field names, not
/// positionally, at every call site.
pub struct StatusInputs {
    pub live: Option<LiveMatchSummary>,
    pub espn_enabled: bool,
    pub rss_enabled: bool,
    pub weather: Option<WeatherSummary>,
    pub weather_enabled: bool,
}
```

Change `snapshot`'s signature to:

```rust
pub fn snapshot(queue: &SingleSlotQueue, inputs: StatusInputs) -> Self {
    Self {
        paused: queue.is_paused(),
        waiting: queue.total_waiting(),
        football: FootballStatus {
            enabled: inputs.espn_enabled,
            live: inputs.live,
        },
        news: NewsStatus {
            enabled: inputs.rss_enabled,
        },
        weather: WeatherStatus {
            enabled: inputs.weather_enabled,
            current: inputs.weather,
        },
    }
}
```

(Keep whatever doc-comment already precedes `snapshot` — the one about
being "recomputed from the live handles on every heartbeat pass" — just
above the new signature.)

**Verify**: `cargo build --locked` (from `src-tauri/`) → will fail at
this point, since call sites haven't been updated yet — that's
expected; the failures should be exactly at the call sites listed below,
not anywhere else. If a failure appears outside `engine.rs`'s two sites
and `status.rs`'s three test sites, STOP (see STOP conditions).

### Step 2: Update the two production call sites in `engine.rs`

Rotation loop (`engine.rs:277-284`):

```rust
let status = StatusState::snapshot(
    &q,
    StatusInputs {
        live: live_summary,
        espn_enabled,
        rss_enabled,
        weather: weather_summary,
        weather_enabled,
    },
);
```

`emit_current_status_blocking` (`engine.rs:331-338`):

```rust
StatusState::snapshot(
    &q,
    StatusInputs {
        live: live_summary,
        espn_enabled: self.espn_enabled,
        rss_enabled: self.rss_enabled,
        weather: weather_summary,
        weather_enabled: self.weather_enabled,
    },
)
```

Add `use crate::status::StatusInputs;` (or extend an existing `use
crate::status::{...}` line) at the top of `engine.rs` if `StatusInputs`
isn't already in scope via a glob/module import — check the existing
`StatusState` import first.

**Verify**: `cargo build --locked` (from `src-tauri/`) → exit 0.

### Step 3: Update the three test call sites in `status.rs`

```rust
// status.rs:244 becomes:
let snap = StatusState::snapshot(
    &queue,
    StatusInputs {
        live: None,
        espn_enabled: true,
        rss_enabled: false,
        weather: None,
        weather_enabled: false,
    },
);

// status.rs:258 becomes:
StatusState::snapshot(
    &queue,
    StatusInputs {
        live: None,
        espn_enabled: true,
        rss_enabled: false,
        weather: None,
        weather_enabled: false,
    },
)
.waiting

// status.rs:266 becomes:
let snap = StatusState::snapshot(
    &queue,
    StatusInputs {
        live: None,
        espn_enabled: true,
        rss_enabled: false,
        weather: Some(weather_summary()),
        weather_enabled: true,
    },
);
```

**Verify**: `cargo test --locked status::` (from `src-tauri/`) → all
pass, same assertions as before (this is a call-site shape change only —
no test's expected value changes).

### Step 4: Full suite + lint

**Verify**: `cargo test --locked` (from `src-tauri/`) → all pass, exact
same total count as before this plan (pure refactor — no test added or
removed). `cargo fmt --check && cargo clippy --locked --all-targets --
-D warnings` → exit 0.

No `docs/TESTING_STRATEGY.md` §0 update needed — this plan adds zero
tests.

## Test plan

No new tests — this plan is a pure signature refactor. The existing
three `status.rs` tests (updated in Step 3 to use named construction)
and the production code path exercised transitively by any test that
runs the rotation loop or `emit_current_status_blocking` are the
regression safety net; all must continue passing with identical
assertions.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `cargo build --locked` exits 0
- [ ] `cargo test --locked` exits 0, exact same test count as before
      this plan
- [ ] `cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings`
      exits 0
- [ ] `grep -n "StatusState::snapshot(" src-tauri/src/engine.rs src-tauri/src/status.rs`
      shows every call site now passing a `StatusInputs { ... }` value,
      not five bare positional arguments
- [ ] No files outside the in-scope list are modified (`git status`)
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- The signature or call sites don't match the excerpts in "Current
  state" (drift since `f2cbae6`) — re-read the live code before
  adapting.
- A compile failure after Step 1 appears anywhere other than the five
  named call sites (`engine.rs` ×2, `status.rs` ×3) — that means another
  call site exists that this plan didn't account for; find and update it
  following the same pattern, but if its context is unclear, report
  instead of guessing at field values.
- Any test's assertion value needs to change (not just its call
  syntax) to pass — a pure refactor should never require that; if it
  does, something about the current behavior wasn't what "Current
  state" described, and that's worth reporting rather than silently
  adjusting.

## Maintenance notes

- The next ambient source (if one is ever added — see plan 052's spike
  on a News ambient summary, which would add a sixth boolean/Option pair
  to this same shape) should extend `StatusInputs` with named fields
  rather than reverting to positional arguments.
- A reviewer should scrutinize: that every field in `StatusInputs` is
  passed by name (not that the struct-literal happens to list fields in
  the same order as the old positional signature, which would defeat
  the point), and that no test's expected assertion values changed — only
  the call syntax should differ from before this plan.
