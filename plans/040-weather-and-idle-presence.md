# Plan 040: weather source + idle-rail ambient presence (incl. football chip)

> **Executor instructions**: Follow this plan step by step (Part B only
> — Part A is DONE, verified below). Run every verification command and
> confirm the expected result before moving on. If anything in "STOP
> conditions" occurs, stop and report. When done, update this plan's
> status row in `plans/README.md`.
>
> **Drift check (run first)**:
> `git diff --stat 55d799e..HEAD -- src-tauri/src/status.rs src-tauri/src/poller.rs src-tauri/src/rss_poller.rs src-tauri/src/settings.rs src-tauri/src/config.rs src-tauri/src/engine.rs src-tauri/src/event.rs src-tauri/src/net.rs src/components/IdleView.tsx src/useStatusState.ts`
> Re-verify the excerpts below against any drift before starting — this
> review-plan pass re-grounded every citation at `15df3cc` (2026-07-19),
> which is AFTER 037 (the Engine) landed; the original filing predates
> that and its "no rust change needed" / "settings.rs + Config" framing
> is corrected below.

## Status

- **Part A DONE (verified this review-plan pass), Part B TODO** — filed
  2026-07-19 from a live-session product discussion. **Part A**:
  `IdleView.tsx` renders a bright `Football` chip when
  `football.enabled && !live` (dim `Football off` when disabled),
  upgrading to the live green-dot chip at kickoff. Confirmed landed at
  commit `9b5bd62` ("feat(idle): football presence chip when armed
  (plan 040 Part A)"); `IdleView.tsx:24-28` matches the plan's
  description exactly; `docs/TESTING_STRATEGY.md` §0 confirms
  `IdleView rail 6` (was 4) and frontend total 107 (was 105). **No
  further action on Part A.**
- **Priority**: P2
- **Effort**: Part B (weather) M
- **Risk**: LOW-MEDIUM — additive, no queue-semantics change, but this
  review-plan pass found Part B needs one genuine Engine extension (see
  "Re-grounded against the Engine" below) that the original filing
  didn't anticipate, since it predates 037.
- **Depends on**: none for the architecture (037 is landed, not a
  blocker). **Operator decision required before Step 1**: the weather
  provider (see "Provider decision" below) — this plan cannot specify
  the exact fetch shape until that's picked.
- **Review-plan pass (2026-07-19, at `15df3cc`)**: verified Part A DONE
  against live code and test counts (see Status above). For Part B:
  corrected "settings.rs + Config" — the struct is in `config.rs`
  (confirmed during the 039 review-plan pass, same finding applies
  here); found that Part B's original filing predates 037 and doesn't
  address how ambient weather data reaches the rotation loop's
  `StatusState` snapshot — resolved by extending the `Engine`'s
  existing `live`/`update_live_match` pattern (see below) rather than
  inventing a new mechanism; grounded every file reference against live
  code (`status.rs`, `event.rs`, `config.rs`, `net.rs`,
  `useStatusState.ts`, `IdleView.tsx`); wrote concrete numbered Steps
  (none existed before, same gap as 039's original filing).

## Motivation

Two gaps surfaced watching the app during a live match:

1. **Football has no idle presence.** *(Part A — DONE, see Status.)*
2. **No weather.** A glanceable weather readout (and rain/severe alerts)
   is high-value ambient info the notch/HUD is well suited to, and there
   is zero weather code in the tree today.

## Part A — football presence chip (DONE, verified)

No action. `IdleView.tsx:24-28`:
```tsx
<span className={`src-chip${status.football.enabled ? "" : " dim"}`}>
  {status.football.enabled ? "Football" : "Football off"}
</span>
```
matches the plan's design exactly, gated the same way the live-chip
branch is (`live !== null` at `IdleView.tsx:19`).

## Provider decision (operator — REQUIRED before Step 1)

The original filing named "Open-Meteo (no key, lat/lon in, JSON out)"
as an example, flagged for confirmation, not a locked choice. This
review-plan pass did not independently verify Open-Meteo's terms (no
tool access to confirm current ToS) — that verification, and the final
provider pick, is still an operator decision. Whatever is chosen must
be keyless or low-friction (the `net.rs` client posture — 10s timeout,
3 redirects, capped body — assumes a plain public JSON endpoint, not
an OAuth/API-key flow; a keyed provider would need its own secrets
handling, mirroring `notifier.rs`'s telegram-secret pattern, which is
meaningfully more scope than this plan currently sizes for). **Do not
proceed past this gate without the operator confirming**: provider,
its terms, and whether it needs a secret.

## Re-grounded against the Engine (this review-plan pass)

The original filing predates plan 037 and describes Part B's ambient
chip purely in terms of `status.rs`, without addressing **who writes
the ambient weather data and how it reaches the rotation loop's
`StatusState` snapshot**. Plan 037/034 already solved exactly this
problem for football's live-match chip — read that solution and mirror
it, don't invent a second mechanism:

- **Today's football precedent**: `Engine` privately owns
  `live: Arc<StdMutex<Option<LiveMatchSummary>>>` (`engine.rs:32`),
  constructed internally in `Engine::new` (`engine.rs:58-74`, nothing
  outside can hold it). The espn poller calls
  `engine.update_live_match(summary)` (`engine.rs:186-199`) once per
  poll pass — a narrow method that locks `live`, compares, stores, and
  wakes the rotation loop ONLY on change (never touches the queue).
  `spawn_rotation`'s loop (`engine.rs:218-263`) reads `live` (BEFORE
  locking the queue — lock-discipline comment at `engine.rs:236-238`)
  and folds it into `StatusState::snapshot` (`status.rs:58-75`) every
  pass, emitting on change via the loop's own `last_status` dedup.
  `emit_current_status_blocking` (`engine.rs:287-295`, the
  `on_page_load` reload re-emit) does the same read, unconditionally.
- **Weather's ambient chip needs the identical shape**: `Engine` gains
  a `weather: Arc<StdMutex<Option<WeatherSummary>>>` field +
  `weather_enabled: bool` (constructed in `Engine::new` exactly like
  `live`/`espn_enabled`/`rss_enabled` are today), a
  `pub fn update_weather(&self, summary: Option<WeatherSummary>)`
  method with the identical compare-then-store-then-maybe-wake body as
  `update_live_match`, and `StatusState::snapshot`/
  `emit_current_status_blocking`/`spawn_rotation`'s StatusState block
  all thread the new field through. This is a mechanical extension of
  an established pattern, not new design — but it IS a real
  `engine.rs` edit the original filing's "no rust change needed" (which
  only applied to Part A) and "extend `snapshot()`" (true, but
  incomplete — `snapshot()` alone doesn't explain the write path) don't
  cover.
- **Threshold alerts are ordinary `accept()`-routed Events**, NOT part
  of this ambient mechanism — a threshold-crossing weather event is a
  normal `Event { origin: SourceKind::Weather, topic: None,
  rotation: RotationSpec::OneShot { .. }, .. }` passed to
  `engine.accept(event, false)` from the weather poller, identical in
  shape to how the espn/rss pollers push cards today
  (`poller.rs:608-612`, `rss_poller.rs:487-501`). The two mechanisms
  (ambient status vs. queued card) are already cleanly separated in the
  codebase for football; weather should use the same separation, not
  conflate "current conditions" with "a card."
- **`StatusState::snapshot`'s signature will grow to 6 params**
  (`queue, live, espn_enabled, rss_enabled, weather, weather_enabled`)
  — check `cargo clippy` doesn't flag `too_many_arguments` on it once
  added (the default threshold is 7, so this should still be fine, but
  confirm rather than assume).

## Scope

**In scope**:
- `src-tauri/src/event.rs` — add `Weather` to `SourceKind`
  (`event.rs:72-77`, `#[serde(rename_all = "snake_case")]` already on
  the enum, so `"weather"` round-trips for free).
- `src-tauri/src/config.rs` — `weather_enabled: bool` (default `false`),
  location field(s), `weather_poll_secs`, `weather_alert_*` thresholds,
  `weather_priority: Priority`. Add `SourceKind::Weather` to
  `default_rotation_order()` (`config.rs:181-194`) at whatever position
  the operator wants it prioritized relative to
  Football/Manual/Cmux/News.
- `src-tauri/src/settings.rs` — `validate()` (`settings.rs:49-90`
  pattern) gains range checks for the new numeric fields, following the
  existing `espn_poll_secs`/`espn_ttl_secs` checks as the exemplar. No
  new `#[tauri::command]` (confirmed: `get_config`/`get_default_config`
  serialize the whole struct, same as the 039 finding — no per-field
  wiring needed there).
- `src-tauri/src/net.rs` — reuse `build_poll_client()`/
  `read_body_capped()` as-is (`net.rs:8-38`) — no changes, just call
  them from the new poller.
- `src-tauri/src/weather_poller.rs` (new) — mirrors
  `rss_poller.rs`'s shape: a `spawn_weather_poller(engine: Engine,
  ...)` function, a poll loop, a pure diff/parse function (fixture-
  tested, no live network — same discipline as `poller.rs`'s
  `diff_scoreboard`/`rss_poller.rs`'s `diff_feed`), calling
  `engine.update_weather(...)` every pass and `engine.accept(event,
  false)` only for threshold-crossing events.
- `src-tauri/src/engine.rs` — the `weather`/`weather_enabled` field +
  `update_weather` method + `Engine::new` signature + `spawn_rotation`/
  `emit_current_status_blocking` wiring, per "Re-grounded against the
  Engine" above.
- `src-tauri/src/status.rs` — `WeatherStatus { enabled: bool, current:
  Option<WeatherSummary> }` on `StatusState` (mirroring
  `FootballStatus`'s shape, `status.rs:28-34`); `WeatherSummary` struct
  (mirroring `LiveMatchSummary`, `status.rs:36-43`); `snapshot()`
  (`status.rs:58-75`) gains the two new params.
- `src/useStatusState.ts` — the `StatusState`/`WeatherSummary` TS types
  (`useStatusState.ts:9-19`, hand-mirrored, same as `SettingsApp.tsx`'s
  `Config` interface found during the 039 review), the `FALLBACK_STATUS`
  object (`useStatusState.ts:30-35`), AND `isValidStatusState`
  (`useStatusState.ts:55+`) — this validator explicitly type-guards
  each field; a `weather` field the validator doesn't check means a
  malformed payload could still be treated as valid `StatusState`
  overall. Do not skip this file — it's easy to add the type and the
  fallback and miss the validator, since nothing will visibly break
  until a malformed weather payload actually arrives.
- `src/components/IdleView.tsx` — a weather chip, following the
  News/Football chip pattern at `IdleView.tsx:24-31` (dim when
  disabled, otherwise showing `current` if present).
- `src/settings/SettingsApp.tsx` — the config UI (toggle + location +
  poll interval), following the ESPN group's pattern
  (`SettingsApp.tsx:645-661`, established during the 039 review).
- `docs/TESTING_STRATEGY.md` §0, `docs/ARCHITECTURE.md` (new source +
  the ambient-vs-card design split).

**Out of scope**:
- Any `#[tauri::command]` addition or `capabilities/*.json` change
  (STOP condition, unchanged from the original filing).
- A keyed/OAuth weather provider's secret storage (out of scope unless
  the operator's provider choice requires it — if so, this plan's
  effort estimate is stale and needs revisiting before Step 1, not
  silently absorbed).
- Any change to the football/news pollers or the Topic/Recurring
  machinery (039's territory) — this plan's producer is independent.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust suite | `cd src-tauri && cargo test --locked` | all pass; recount against §0 |
| Gates | `cargo clippy --locked --all-targets -- -D warnings && cargo fmt --check` | exit 0 |
| Frontend tests | `npx vitest run` | all pass |
| Frontend gates | `npx biome ci . && npx tsc --noEmit && npx vite build` | exit 0 |

## Steps

### Step 0: provider decision gate

STOP here until the operator has confirmed the provider per "Provider
decision" above. Do not guess an API shape and build against it
speculatively — the poll/parse code in Step 3 is provider-specific.

### Step 1: `SourceKind::Weather` + config surface

Add the enum variant (`event.rs:72-77`) and the config fields
(`config.rs`, per Scope above), including `default_rotation_order`'s
new entry and `settings::validate`'s new range checks.

**Verify**: `cargo test --locked config:: event::` → all pass.

### Step 2: `Engine` ambient-weather plumbing

Per "Re-grounded against the Engine" above: add the `weather`/
`weather_enabled` field and `update_weather` method to `engine.rs`,
thread through `Engine::new`, `spawn_rotation`, and
`emit_current_status_blocking`. Add a test mirroring
`update_live_match_wakes_only_on_change` (`engine.rs:609-642`) for
`update_weather`.

**Verify**: `cargo test --locked engine::` → all pass, including the
new test.

### Step 3: `status.rs` + `weather_poller.rs`

Add `WeatherStatus`/`WeatherSummary` to `status.rs` and thread through
`snapshot()`. Build `weather_poller.rs` per the Scope description —
pure diff/parse function fixture-tested against a captured real
response (same discipline as the ESPN/RSS fixtures under
`src-tauri/tests/fixtures/`), `spawn_weather_poller` wired the same way
`spawn_espn_poller`/`spawn_rss_poller` are (`lib.rs:317-333`).

**Verify**: `cargo test --locked status:: weather_poller::` → all pass.

### Step 4: wire into `lib.rs`

Config-gate the spawn (`if weather_enabled { ... }`, matching the
`espn_enabled`/`rss_enabled` pattern at `lib.rs:317,326`).

**Verify**: `cargo test --locked` (full) → all pass; `cargo clippy
--locked --all-targets -- -D warnings && cargo fmt --check` → exit 0.

### Step 5: frontend

Add the TS types, fallback, validator branch, `IdleView.tsx` chip, and
`SettingsApp.tsx` config UI per Scope above.

**Verify**: `npx vitest run && npx tsc --noEmit && npx biome ci .` →
all pass/exit 0.

### Step 6: docs + status

Update `docs/TESTING_STRATEGY.md` §0 and `docs/ARCHITECTURE.md`. Flip
this plan's `plans/README.md` row to DONE.

**Verify**: `cargo test --locked 2>&1 | grep "test result"` and `npx
vitest run` totals match §0.

## Done criteria

| Check | Command | Expected |
|---|---|---|
| Rust tests | `cargo test --locked` (from `src-tauri/`) | all pass; §0 updated |
| Frontend tests | `npx vitest run` | all pass; §0 updated |
| Lint/format/build | `cargo clippy --locked --all-targets -D warnings && cargo fmt --check`; `npx biome ci . && npx tsc --noEmit && npx vite build` | exit 0 |
| Ambient chip | weather enabled → idle chip shows current conditions; disabled → no chip, byte-identical behavior | pass |
| Threshold alert | a threshold-crossing weather event enqueues a card via `accept`; plain conditions never do | pass |
| Ambient ≠ card | `rg -n "SourceKind::Weather" src-tauri/src/weather_poller.rs` shows it used ONLY on the `accept()` path, never on the `update_weather()` path (the ambient write takes a plain `WeatherSummary`, not an `Event`) | pass |
| Receive-only intact | `git diff -- src-tauri/capabilities/default.json src-tauri/capabilities/settings.json` | byte-identical |
| Validator complete | `rg -n "weather" src/useStatusState.ts` hits the type, the fallback, AND `isValidStatusState` — not just the first two | pass |

## STOP conditions

- Step 0's provider decision is unresolved → STOP, do not speculate.
- The chosen weather provider needs an API key or has terms
  incompatible with polling → STOP and surface the provider choice to
  the operator (this changes the effort estimate).
- Any change would touch `capabilities/default.json` or add a
  `#[tauri::command]` → STOP (receive-only / command-ACL guarantee).
- `StatusState::snapshot`'s new 6-arg signature trips
  `clippy::too_many_arguments` → STOP and report rather than adding an
  `#[allow(...)]` silently; consider whether ambient inputs should
  bundle into a small struct instead (a judgment call for whoever's
  executing, informed by how the codebase already resolved this for
  `spawn_espn_poller` in plan 037 — collapsing scattered params into
  one owned type, not an attribute).

## Notes

- Part A is done and needs no further action.
- Part B is independent of the sports-card work (037/039) — no queue
  changes; it only adds a producer + an Engine-owned ambient handle +
  a status field + a chip, following the football precedent throughout.
