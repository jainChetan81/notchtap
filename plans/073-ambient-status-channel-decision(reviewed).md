# Plan 073: Decide whether to generalize the ambient status side-channel before News becomes its third copy

> **Executor instructions**: This is a decide/investigate-scoped plan, not
> a build-everything plan — its deliverable is a recommendation (and,
> conditionally, a small generalizing refactor) gating the still-open plan
> 052 spike, not a new feature. Follow the steps in order. If the
> investigation in Step 1 doesn't clearly favor one option, STOP and
> present both options to the operator rather than picking one yourself —
> this is exactly the kind of judgment call this plan exists to surface,
> not resolve unilaterally. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat f6c2f46..HEAD -- src-tauri/src/engine.rs src-tauri/src/status.rs`
> **`engine.rs` WILL show a diff** — plan 076 (Telegram connector health)
> landed a `telegram_health` field/accessor, and plan 070 (ingest
> tracing) added log lines inside `accept`, both before
> `update_live_match`/`update_weather` — shifting them from the
> planning-time lines 196/216 to their current 216/236 (content of both
> functions otherwise unchanged — verified byte-identical 2026-07-21;
> already corrected below). `status.rs` is byte-identical since
> planning. Those specific shifts are expected, not a STOP condition;
> only treat further engine.rs/status.rs changes as a STOP if the
> ambient-channel functions themselves changed shape, not just moved.

## Status

- **Priority**: P3
- **Effort**: S (investigate) to M (if generalization is the chosen and
  executed outcome)
- **Risk**: MED (if generalization is chosen — touches the shared Engine
  module) / LOW (if the recommendation is "keep duplicating, ship 052
  as-is")
- **Depends on**: none, but directly informs the still-open plan 052
  (SPIKE: News ambient idle-rail status) — read that plan/its design doc
  before starting this one
- **Category**: tech-debt / architecture
- **Planned at**: commit `f6c2f46`, 2026-07-20
- **Review-plan pass (2026-07-21)**: fresh cold read at `647f6d0`.
  Re-checked the urgency question against plan 083's landing: the
  duplication has **not** gotten worse — 083's crests/EspnMeta ride the
  notification-card wire (`EspnMeta` on `EventMeta` in `event.rs`), not
  the ambient channel; `status.rs` is byte-identical since planning and
  `LiveMatchSummary` is still exactly `label`+`minute`, with one
  unchanged `engine.update_live_match(summary)` call site
  (`poller.rs:1376`). Precise current touch-point count per channel in
  `engine.rs`, directly verified: (1) field decl (:36 live / :37
  weather), (2) manual `Clone` impl line (:53/:54), (3) constructor-time
  initialization inside `Engine::new` (:82/:83 — a **correction** to
  this plan's "constructor argument" phrasing: `live`/`weather` are
  initialized internally, NOT parameters; only 076's `telegram_health`
  is a parameter), (4) `update_*` method (:216-229/:236-249), (5) read
  site in `spawn_rotation` (Arc clones :272-273 + lock-clone :291-292 +
  `StatusInputs` fields :301/:304), (6) read site in
  `emit_current_status_blocking` (:350-351 + :357/:360) — i.e. 6
  distinct touch points counting the two read sites separately, plus
  the `status.rs` mirror (summary struct + `*Status` struct +
  `StatusInputs` field + `snapshot()` wiring). Same count as at
  planning. One new adjacent data point strengthening Step 1's
  question: plan 076's `telegram_health` deliberately "mirror[s]
  `live`/`weather`" (its own README row's words) for the
  field/Clone/ctor/accessor half of the pattern (no
  compare-then-store-then-wake) — a third Engine-side per-channel
  plumbing instance to weigh as evidence, though not a third copy of
  the full ambient pattern. Stale line citations fixed (update fns
  216/236; status.rs excerpt corrected to actual struct order incl. the
  pre-existing enabled-only `NewsStatus`; engine test baseline 10→12).
  Verdict: still a valid, correctly-scoped decision plan; urgency
  unchanged (not escalated) — but it should still run before plan 052
  is dispatched.

## Why this matters

`src-tauri/src/engine.rs` carries two parallel, hand-duplicated ambient
status side-channels — one for football (`live`/`update_live_match`) and
one for weather (`weather`/`update_weather`) — each duplicated across 5
touch points: a private field on `Engine`, a `Clone` derive concern, a
constructor argument at `Engine::new`, an `update_*` method, and two read
sites (`spawn_rotation`'s wake-check and
`emit_current_status_blocking`). The same shape is mirrored again in
`status.rs`'s `FootballStatus`/`LiveMatchSummary` and
`WeatherStatus`/`WeatherSummary` structs.

`docs/design/news-ambient-status.md` (the plan 052 spike, already written
and DONE as a design doc, still open as a build decision) proposes
shipping a **third** near-identical copy of this exact pattern
(`NewsSummary { headline, source }` + its own `Arc<Mutex<...>>` +
`update_news` method + `NewsStatus` struct) rather than generalizing.
Three independent, hand-copied implementations of the same
compare-then-store-then-wake pattern is exactly the threshold this
repo's own precedent treats as worth generalizing — the "no `Notifier`
trait" deferral (recorded in `CONTEXT.md` and `plans/README.md`'s
rejected-findings list) is explicitly justified as "until a second
connector exists," implying the bar is real but the count matters. Three
near-identical implementations of the ambient-status pattern crosses that
same bar football+weather (two) didn't.

This plan exists to make that call deliberately, before plan 052 (if
selected) ships the third copy by default — not to force a generalization
that might not be worth it. A `Ambient<T>` (or similarly-named) generic
handle could plausibly collapse the 5-touch-point pattern into one shared
implementation parameterized by the summary type, but that's a real,
non-trivial refactor of a module (`engine.rs`) this repo treats as
load-bearing and carefully reviewed (see plan 037's own extensive review-
pass history) — it deserves an explicit yes/no, not a silent default
either way.

## Current state

- `src-tauri/src/engine.rs:216-249` — the two existing ambient channels,
  side by side (already near-identical — this is the duplication in
  question):

  ```rust
  pub fn update_live_match(&self, summary: Option<LiveMatchSummary>) {
      let changed = {
          let mut guard = self.live.lock().unwrap();
          if *guard == summary {
              false
          } else {
              *guard = summary;
              true
          }
      };
      if changed {
          self.wake.notify_waiters();
      }
  }

  /// The weather twin of `update_live_match` (plan 040 Part B): ...
  pub fn update_weather(&self, summary: Option<WeatherSummary>) {
      let changed = {
          let mut guard = self.weather.lock().unwrap();
          if *guard == summary {
              false
          } else {
              *guard = summary;
              true
          }
      };
      if changed {
          self.wake.notify_waiters();
      }
  }
  ```

  Read the full `Engine` struct definition and `Engine::new` constructor
  yourself to see the other 3 duplicated touch points per channel (field
  declaration, constructor parameter, and the two read sites in
  `spawn_rotation`/`emit_current_status_blocking`) before scoping Step 1's
  investigation.

- `src-tauri/src/status.rs:31-76` — the mirrored duplication on the
  wire-shape side:

  ```rust
  pub struct FootballStatus {          // status.rs:31
      pub enabled: bool,
      pub live: Option<LiveMatchSummary>,
  }

  pub struct LiveMatchSummary {        // status.rs:39
      pub label: String,
      pub minute: String,
  }

  pub struct WeatherSummary {          // status.rs:54
      pub temp_display: String,
      pub condition: String,
  }

  pub struct NewsStatus {              // status.rs:64 — enabled-gate only;
      pub enabled: bool,               // NO ambient summary channel yet.
  }                                    // The 052 spike's NewsSummary would
                                       // be the third summary channel.

  pub struct WeatherStatus {           // status.rs:73
      pub enabled: bool,
      pub current: Option<WeatherSummary>,
  }
  ```

  (Derive/serde attributes and doc comments elided; struct order and
  fields verified against the live file 2026-07-21.)

  Note `LiveMatchSummary` and `WeatherSummary` are NOT structurally
  identical (2 differently-named string fields each, but different field
  names and semantics — `label`/`minute` vs `temp_display`/`condition`),
  so a shared struct isn't a free win on the wire-shape side; a
  generalization would most plausibly land on the `Engine`-internal
  plumbing (the `Arc<Mutex<Option<T>>>` + compare-then-store-then-wake
  method), not necessarily the `status.rs` wire types.

- `docs/design/news-ambient-status.md` — read this in full before Step 1;
  it's the existing, already-written spike design for the proposed third
  copy. Its own reasoning for NOT generalizing (if it gives one) is
  directly relevant evidence for Step 1's investigation — don't
  re-litigate ground it already covered without reading what it said
  first.

- `plans/README.md`'s "Findings considered and rejected" section — the
  "no `Notifier` trait" precedent (search for "Notifier trait" in that
  file) is the closest analogous precedent for how this repo has
  historically decided "duplicate vs. generalize" questions; read it
  before forming a recommendation, since consistency with that precedent
  is part of what makes a recommendation here credible.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests (if generalizing) | `cd src-tauri && cargo test --locked` | all pass |
| Clippy (if generalizing) | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Format (if generalizing) | `cd src-tauri && cargo fmt --check` | exit 0 |

## Scope

**In scope**:
- Step 1: read-only investigation (no file changes)
- Step 2 (conditional — only if Step 1's recommendation is "generalize
  now"): `src-tauri/src/engine.rs` (introduce a shared internal helper
  or generic type for the `Arc<Mutex<Option<T>>>` compare-then-store-
  then-wake pattern; migrate `live`/`weather` onto it)

**Out of scope**:
- `status.rs`'s wire types (`LiveMatchSummary`/`WeatherSummary`) — as
  noted above, they're not structurally identical, so this plan's
  generalization (if chosen) should target the `Engine`-internal
  plumbing only, not force a shared wire struct that would lose the
  distinct field semantics.
- Actually building plan 052 (the News ambient status feature itself) —
  that's a separate plan/decision; this plan's job is to inform *how*
  052 would be built if selected, not whether to select it.
- Any change to `poller.rs`, `weather_poller.rs`, or `rss_poller.rs` —
  none of the producers need to change regardless of which way this
  decision goes.

## Steps

### Step 1: Investigate and recommend

Read `docs/design/news-ambient-status.md` in full, plus `engine.rs`'s
full `live`/`weather` implementation (all 5 touch points each) and
`status.rs`'s corresponding wire types. Answer, with evidence:

1. Would a generic `Ambient<T>`-style handle (or similar) actually
   collapse the `Engine`-internal 5-touch-point duplication cleanly, or
   does something about `live`/`weather`'s specific shapes (different
   read-site logic, different wake conditions) resist a clean
   generalization? Look closely at `spawn_rotation`'s read sites — are
   they truly identical in structure for both channels, or is there a
   subtle difference that would make a shared abstraction leaky?
2. What's the actual effort/risk of generalizing now (before News is a
   third copy) vs. after (once News duplicates the pattern a third time)?
   Generalizing 2→1 shared implementation is usually cheaper than
   generalizing 3→1 (less to migrate at once, but also: is a 2-instance
   pattern really "duplication" worth abstracting, or is 3 the more
   honest trigger point per this repo's own "no Notifier trait until a
   second connector exists" precedent — note that precedent's trigger
   was "a second," and this would be a *third* instance, an even
   stronger case, not a weaker one)?
3. Does `docs/design/news-ambient-status.md` already address this
   question? If it explicitly considered and rejected generalizing (with
   reasoning), that's strong evidence to weigh heavily — don't
   contradict a reasoned decision without a specific new argument the
   spike didn't have.

Write your recommendation as a short (a few paragraphs) note — either
"generalize now, here's the shape" (proceed to Step 2) or "duplication is
fine at 3 instances too, ship 052 as its own copy" (skip Step 2, this
plan is done at this point) or "genuinely unclear, operator should
decide" (STOP, present both options).

**Verify**: no command — this is a reasoning step. The deliverable is
the written recommendation itself, which becomes part of your completion
report either way.

### Step 2 (conditional — only if Step 1 recommends generalizing now)

Introduce a shared internal type (name it something clear — `AmbientSlot<T>`
or similar) wrapping the `Arc<Mutex<Option<T>>>` +
compare-then-store-then-wake pattern currently duplicated in
`update_live_match`/`update_weather`. Migrate both existing channels onto
it. Keep `LiveMatchSummary`/`WeatherSummary` and their distinct
`FootballStatus`/`WeatherStatus` wire types unchanged in `status.rs` —
this generalization targets the `Engine`-internal plumbing only, per the
Scope section above.

**Verify**: `cd src-tauri && cargo test --locked` → all pass, same
behavior (this must be a pure refactor — no test should need to change
its assertions, only possibly its setup if `Engine`'s internal field
names change in a way tests construct against directly; check whether any
test constructs `Engine` fields directly vs. only through its public
methods before assuming zero test impact).

### Step 3 (if Step 2 was executed): Full suite + lint

**Verify**:
- `cd src-tauri && cargo test --locked` → all pass, same test count as
  baseline (pure refactor, no new/removed behavior)
- `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` →
  exit 0
- `cd src-tauri && cargo fmt --check` → exit 0

## Test plan

- If Step 2 executes: no new tests required (pure internal refactor); the
  existing `engine.rs` test suite (12 tests at current HEAD — 10 at
  planning time, +2 landed since, incl. plan 081's emission-dedup
  regression test)
  already exercises `update_live_match`/`update_weather` behaviorally —
  re-running it unchanged after the refactor IS the verification that the
  generalization preserved behavior exactly.
- If Step 2 doesn't execute (recommendation was "don't generalize"): no
  tests needed at all — this plan's deliverable is the written
  recommendation.

## Done criteria

- [ ] Step 1's recommendation is written and included in the completion report, with its reasoning
- [ ] If generalizing: `cargo test --locked` exits 0, same test count as baseline; `cargo clippy`/`cargo fmt --check` exit 0; `status.rs`'s wire types are unchanged (`git diff src-tauri/src/status.rs` is empty)
- [ ] If not generalizing: no source files modified at all — this plan's only artifact is the recommendation
- [ ] `plans/README.md` status row for 073 updated, and (if applicable) a note added to plan 052's row cross-referencing this plan's recommendation

## STOP conditions

- Step 1's investigation doesn't clearly favor one option — present both
  to the operator rather than picking one.
- `docs/design/news-ambient-status.md` already explicitly rejected
  generalizing with specific reasoning this plan doesn't have a genuinely
  new counter-argument to — don't override a reasoned prior decision
  without new evidence.
- If generalizing: any existing test needs its assertions (not just
  setup) changed to pass — that means the refactor changed behavior, not
  just structure; stop and report rather than adjusting the test to match.

## Maintenance notes

- Whatever this plan decides directly determines how plan 052 (News
  ambient status), if ever selected for a build, should be scoped —
  make sure that plan's Scope section is updated to reference this
  plan's outcome before it's dispatched to an executor.
- If a fourth ambient-status source is ever proposed after this, the
  "duplicate vs. generalize" question shouldn't need re-litigating from
  scratch — point back at this plan's reasoning.
