# Plan 041: ESPN event-card copy — name the event, not just scorer+minute

> **Executor instructions**: Follow this plan, run every gate, update the
> `plans/README.md` row when done.
>
> **Drift/coordination check (run first)**: this touches `poller.rs`,
> which **037 (the Engine)** migrates and **039 (live card)** reworks.
> If 037 is IN PROGRESS / not merged, STOP and wait — do not edit
> `poller.rs` under an in-flight refactor. Cleanest sequencing: land this
> **after 037**, ideally folded into or right after **039** (which already
> rebuilds the ESPN event-construction path).

## Status

- **TODO** — filed 2026-07-19 from operator feedback watching ENG–FRA
  (World Cup) live on the new build.
- **Priority**: P3 · **Effort**: S · **Risk**: LOW (copy/formatting only).
- **Depends on**: 037 (coordination — same file), 039 (soft — same path).

## Problem

A live goal card renders title `fifa.world: ENG 1–0 FRA`, body
`D. Rice 3'`. The scoreline moving to 1–0 implies a goal, but the body
never says **what** the event was — operator "had to check what Declan
Rice did there." Nothing distinguishes a goal from a penalty, own-goal,
red card, or other `MatchState` — they'd all read as `<name> <minute>'`.

## Scope

- `src-tauri/src/poller.rs` — the `MatchState` → event title/body
  formatting only. Prefix/label the body by event type so the card is
  self-describing at a glance, e.g.:
  - goal → `⚽ Goal — D. Rice 3'`
  - penalty → `⚽ Penalty — … `
  - own goal → `⚽ Own goal — …`
  - red card → `🟥 Red card — …`
  - keep kickoff / half-time / full-time as-is (already self-describing).
- Keep it terse (one line, still fits the collapsed card); no wire/schema
  change, no `SlotState` change — this is producer-side copy.
- Tests: extend the poller's `MatchState` formatting tests to assert the
  event label appears per type.

## Done criteria

| Check | Command (from `src-tauri/`) | Expected |
|---|---|---|
| Rust tests | `cargo test --locked` | all pass; §0 updated if tests added |
| Lint/format | `cargo clippy --locked --all-targets -- -D warnings && cargo fmt --check` | exit 0 |
| Copy present | poller test asserts a goal event body contains "Goal" | pass |

## Notes

- Independent of the consolidation work — this improves *today's* burst
  cards too, not just the future single card.
- If 039 lands first, do this in the same `poller.rs` pass rather than a
  second round-trip through the file.
