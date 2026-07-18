# Plan 040: weather source + idle-rail ambient presence (incl. football chip)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report. When done,
> update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**:
> `git diff --stat 55d799e..HEAD -- src-tauri/src/status.rs src-tauri/src/poller.rs src-tauri/src/rss_poller.rs src-tauri/src/settings.rs src/components/IdleView.tsx src/useStatusState.ts`
> Re-verify the excerpts below against any drift before starting.

## Status

- **Part A DONE, Part B TODO** — filed 2026-07-19 from a live-session
  product discussion (operator wants weather; operator noticed football
  has no idle presence). **Part A landed the same session**: `IdleView.tsx`
  now renders a bright `Football` chip when `football.enabled && !live`
  (and dim `Football off` when disabled), upgrading to the live green-dot
  chip at kickoff — mirrors the News chip. Frontend-only, applied via HMR
  without restarting the poller; 2 new IdleView tests (rail 4→6, vitest
  105→107). Part B (weather source) remains TODO.
- **Priority**: P2
- **Effort**: Part A (football chip) S · Part B (weather) M
- **Risk**: LOW — additive. Part B is a new poller/source in the
  established ESPN/RSS mould; no changes to the queue/Engine.
- **Depends on**: none (independent of 035/037/039). Part A is a
  standalone quick win; Part B builds on the same status/idle surface.

## Motivation

Two gaps surfaced watching the app during a live match:

1. **Football has no idle presence.** `IdleView.tsx` renders the News
   chip **always** (`"News"` / `"News paused"`) but the football chip
   only when a match is *in play* (`live !== null`,
   `IdleView.tsx:19-24`). With ESPN enabled and 7 leagues polling but
   nothing kicked off yet, the rail shows a News chip and **nothing for
   football** — so "is football even on?" is invisible until kickoff.
   `status.rs`'s `FootballStatus { enabled, live }` already carries
   `enabled`; the frontend just ignores it when `live` is null.
2. **No weather.** A glanceable weather readout (and rain/severe alerts)
   is high-value ambient info the notch/HUD is well suited to, and there
   is zero weather code in the tree today.

## Part A — football presence chip (S, do first, independent)

When `football.enabled` is true but `football.live` is null, render a
**dim** football chip (mirroring `"News paused"`'s dim treatment) — e.g.
`"⚽ Watching N leagues"` or, if a fixture is scheduled today, the next
kickoff. Minimal version: a dim `"Football"` chip whenever
`football.enabled`, upgrading to the existing green-dot live chip when
`live !== null`.

- **Where**: `src/components/IdleView.tsx` only — the data
  (`football.enabled`) is already in `StatusState`; no rust change needed
  for the minimal version. (Surfacing "next kickoff" *would* need a
  `next: Option<...>` on `FootballStatus` in `status.rs` + poller — treat
  that as an optional stretch, not the core.)
- **Tests**: extend `IdleView.test.tsx` — the existing `LIVE_EVENING`
  and all-clear fixtures; add an "enabled, nothing live" case asserting a
  dim football chip renders (today it renders none).

## Part B — weather source (M)

**Design decision (operator, 2026-07-19): two modes, not spammy cards.**
- **Ambient chip** (idle rail): current conditions + temp, always shown
  when weather is enabled — the same "presence" treatment Part A gives
  football. This is the default, non-interrupting surface.
- **Threshold alerts** (real cards, `high`/`normal`): only for events
  that earn an interruption — "rain in ~15 min", severe-weather warnings,
  a configured temp threshold crossed. Plain conditions never become a
  card.

Architecture — mirror the existing poller pattern (do NOT invent a new
one):
- **`src-tauri/src/weather_poller.rs`** — new module in the shape of
  `rss_poller.rs`/`poller.rs`: shared capped fetch (`net.rs`, plan 025),
  client posture, poll loop, diff → emit. A `Weather` `SourceKind`
  (`event.rs`) for card origin + rotation-order.
- **Provider**: a keyless/low-friction API (e.g. Open-Meteo — no key,
  lat/lon in, JSON out); confirm terms at build time. Cap the body via
  `read_body_capped`.
- **`status.rs`**: add `WeatherStatus { enabled, current:
  Option<WeatherSummary> }` to `StatusState` beside `football`/`news`;
  extend `snapshot()`/`status_state_if_changed` + the change-guard tests.
- **`IdleView.tsx`** / `useStatusState.ts`: a weather chip (temp +
  condition glyph); validate the new field.
- **Config** (`settings.rs` + `Config`): `weather_enabled: bool`
  (default `false`), `weather_lat`/`weather_lon` (or a location string),
  `weather_poll_secs` (default e.g. 900), `weather_alert_*` thresholds,
  `weather_priority`. Add to `rotation_order` vocabulary. Settings UI
  toggle + location field. **No new invoke command** → `build.rs`
  command ACL and `capabilities/default.json` stay untouched.

## Done criteria

| Check | Command | Expected |
|---|---|---|
| Rust tests | `cargo test --locked` (from `src-tauri/`) | all pass; §0 updated |
| Frontend tests | `npx vitest run` | all pass; §0 updated |
| Lint/format/build | `cargo clippy --locked --all-targets -D warnings && cargo fmt --check`; `npx biome ci . && npx tsc --noEmit && npx vite build` | exit 0 |
| Part A | IdleView test: `enabled && !live` renders a dim football chip | pass |
| Part B ambient | weather enabled → idle chip shows temp/condition; disabled → no chip, byte-identical behavior | pass |
| Part B alert | a threshold-crossing weather event enqueues a card; plain conditions do NOT | pass |
| Receive-only intact | `git diff -- src-tauri/capabilities/default.json` | byte-identical |

## STOP conditions

- Any change would touch `capabilities/default.json` or add a
  `#[tauri::command]` → STOP (receive-only / command-ACL guarantee,
  CLAUDE.md).
- The chosen weather provider needs an API key or has terms incompatible
  with polling → STOP and surface the provider choice to the operator.

## Notes

- Part A is a genuine 20-minute quick win and can ship on its own.
- Part B is independent of the sports-card work (037/039) — no queue or
  Engine changes; it only adds a producer + a status field + a chip.
