# Plan 039: ESPN live-match scoreboard card (opt-in, single-match)

> **Executor instructions**: This plan is **design-complete but not
> code-grounded**. Do NOT execute it directly from the excerpts here —
> its entire spine (poller → enqueue → `Recurring` `tick()` rotation) is
> the exact code that plan **037 (the Engine)** rewrites. Follow the
> dependency gate, then the mandatory review-plan pass, before writing
> any code.
>
> **Dependency gate (hard — STOP if unmet)**:
> 1. `plans/README.md` rows for **037** and **038** read DONE.
>    - 037 must land first so the card's enqueue/rotation wiring is built
>      **on the Engine**, not on the pre-Engine queue it replaces
>      (maintainer decision 2026-07-19, 031 §10: "hold B until 037
>      lands, build clean on the Engine").
>    - 038 must land first so a `Recurring` card doesn't corrupt the 033
>      queue-slider counter (031 §10 Q4).
> 2. `git status` clean for `poller.rs`, `event.rs`, `settings.rs`,
>    `queue.rs`/the Engine module — STOP if any is dirty.
>
> **Mandatory review-plan pass (run after the gate, before Step 1)**:
> `/improve review-plan` this file against settled master. The governing
> design is `docs/design/scoreboard-topic-card.md` — re-ground every
> "where" below against the **Engine's** `apply`/`accept`/rotation
> surface (§7 of the design doc: "if 037 lands first, re-ground the
> enqueue/tick paths against the Engine module"). Only then write code.

## Status

- **BLOCKED — gated on 037 + 038.** Design decisions locked; code
  grounding deferred to the review-plan pass above. This file exists so
  the feature is execute-ready the moment its dependencies land.
- **Priority**: P2
- **Effort**: M
- **Risk**: MEDIUM — first production user of the queue's
  Topic/supersession/`Recurring` machinery (built + tested, zero
  producers today). Opt-in default-off contains the blast radius.
- **Depends on**: **037 (hard)**, **038 (hard)**. Soft: inherits the
  Topic-identity + rotation-kind from the 031 design doc verbatim.

## Governing design

`docs/design/scoreboard-topic-card.md` (plan 031 spike, reviewed
APPROVE). This plan is the *build* of that doc; it does not re-decide
anything §§1–7/9 settled. The §10 maintainer decisions (2026-07-19) that
shape scope:

1. **Build it, opt-in.** New config flag `espn_live_card`, default
   `false`. Today's burst-of-one-shot-cards stays the default; the flag
   flips one live match into a single updating card. Zero behavior change
   when off.
2. **Defer multi-match.** Scope is single-match correctness. Multiple
   concurrent live matches each get their own `Recurring` Topic and share
   the tier via rotation-order/FIFO exactly as today's multi-match burst
   already does — no special arbitration in this plan.
3. **Reuse `ttl_secs`.** The live card's `display_secs` = the existing
   `espn_ttl_secs` (default 8s). No new dwell knob; add `live_card_secs`
   only in a later plan if 8s proves wrong on hardware.
4. **Counter fix is separate (038), lands first** — not bundled here.

## Design spine (from the doc — re-verify targets in the review-plan pass)

- **Topic identity**: `espn:{league}:{match_id}` — every event for one
  match shares it, so kickoff/goal/card/half-time/full-time supersede
  each other in the single Slot instead of queueing as separate items.
- **Rotation kind**: `RotationSpec::Recurring { display_secs: <ttl> }`
  for the live match while it is in play; the **full-time** `MatchState`
  is emitted `OneShot` on the **same Topic**, so the existing
  supersede-then-rotate-out path retires the card automatically at
  match end — no bespoke teardown.
- **Producer**: `poller.rs`'s ESPN diff is the only changed producer.
  When `espn_live_card` is on, its per-match events carry
  `topic: Some("espn:{league}:{match_id}")` and the `Recurring`/`OneShot`
  rotation above, instead of today's `topic: None` one-shots.
- **Render**: reuse the existing card rendering — no new render path
  (the card shows the latest superseding `MatchState`).
- **Connector semantics**: verify (design §5) that Telegram/cmux fan-out
  still receives every delta — `enqueue_and_fan_out` clones the event
  before `enqueue()` and offers regardless of supersession, so each goal
  still relays even though the overlay shows one consolidated card.

## Config surface

- `Config`: add `espn_live_card: bool` (default `false`) alongside the
  existing ESPN poller settings (`settings.rs` / config struct + its
  `Default`). Wire it through `get_config`/`get_default_config` — no new
  invoke command, so `build.rs`'s command ACL is untouched.
- Settings UI: a toggle in the ESPN/source section. Follows the existing
  appearance/rotation control patterns; no new capability.

## Done criteria (to be finalized in the review-plan pass)

| Check | Command | Expected |
|---|---|---|
| Rust tests | `cargo test --locked` (from `src-tauri/`) | all pass; §0 updated |
| Frontend tests | `npx vitest run` | all pass; §0 updated |
| Lint/format | `cargo clippy --locked --all-targets -D warnings && cargo fmt --check`; `npx biome ci . && npx tsc --noEmit && npx vite build` | exit 0 |
| Flag off = no change | test: ESPN diff with `espn_live_card=false` emits `topic: None` one-shots (today's behavior, byte-identical) | pass |
| Flag on = one card | test: a match's kickoff→goal→goal→full-time collapses to one superseding Topic; full-time (`OneShot`) retires it | pass |
| Connector deltas | test: every per-goal event still reaches the fan-out path with the flag on | pass |
| `capabilities/default.json` | `git diff` | byte-identical (receive-only guarantee intact) |

## Scope (indicative — ground against the Engine before editing)

- `src-tauri/src/poller.rs` — ESPN diff: build Topic + rotation per the
  spine above, gated on `espn_live_card`.
- `src-tauri/src/settings.rs` / config struct — `espn_live_card` flag +
  `Default` + config plumbing.
- The **Engine module** (post-037) — confirm the `apply`/`accept`
  enqueue path carries Topic + `Recurring` correctly; the machinery
  exists, this is its first real caller.
- `src/settings/SettingsApp.tsx` — the toggle.
- Docs: `docs/ARCHITECTURE.md` (record the first Topic/Recurring
  producer + the opt-in default-off decision), `docs/V3_6_TECHNICAL_SPEC.md`
  (config field), `docs/TESTING_STRATEGY.md` §0 counts.

## STOP conditions

- 037 or 038 not DONE → STOP (dependency gate).
- The review-plan pass finds the Engine's enqueue/rotation surface
  doesn't cleanly accept a `Recurring` Topic from a producer → STOP and
  report; that is a design-doc §7 re-grounding question for the
  maintainer, not something to improvise around.
- Any change would touch `capabilities/default.json` or add a
  `#[tauri::command]` → STOP (receive-only / command-ACL guarantee).
