# Plan 042: live-match scorecard presentation (bigger, persistent while playing)

> **Executor instructions**: This plan carries an **unresolved maintainer
> decision** (slot behavior, below) — do NOT execute until it's chosen.
> When it is, run every gate and update the `plans/README.md` row.
>
> **Coordination**: builds on **039** (the consolidated live-match card)
> and touches `poller.rs` (037) + the overlay frontend. Sequence AFTER
> 037 + 039. Do not edit `poller.rs` while 037 is in flight.

## Status

- **BLOCKED on a maintainer decision** (see Open decision) + gated on
  037/039. Filed 2026-07-19 from live-session feedback watching ENG–FRA:
  "why is the nudge that small? while a match runs it should be bigger,
  a scorecard — countries, score, time, yellow cards — even collapsed."
- **Priority**: P2 · **Effort**: M–L (depends on decision) · **Risk**: MED
  (challenges the single-slot rotation model).
- **Depends on**: 039 (hard — extends the live-match card), 037 (coord).

## Problem

The overlay is a rotating single-slot **notification strip**: every card
(goal, news, cmux) shares one small slot and auto-dismisses. The live
goal card is just another notification passing through — so while a match
is on, there's no persistent, glanceable scoreboard. 039 makes the match
*one* card that updates in place, but it stays the same small strip and
still rotates out. The operator wants, while a match is live, a **bigger
scorecard** — flags/countries, large score, clock, yellow/red counts —
readable **even when collapsed**, not a transient nudge.

The raw material already exists in the poller: `home_score`/`away_score`,
`display_clock`, and per-card red/yellow info (`poller.rs`). The gap is
the **presentation model**, not the data.

## Open decision (maintainer — REQUIRED before build)

How does the live scorecard behave in the single slot?
- **Option A — "match mode" (pin + enlarge)**: while a match is live, the
  scorecard pins the slot and the window grows to a scoreboard footprint;
  news/cmux queue behind it (or show in a secondary strip). Most
  scoreboard-like; biggest change to the rotation model.
- **Option B — richer rotating card**: stays a normal rotating card
  (shares the slot, still rotates), but its **collapsed** layout is
  redesigned into a fuller scorecard (flags, big score, clock, card
  counts). Smaller change; not always-on.
- **Option C — hybrid**: rotating by default, but a hotkey/priority
  "expand to match mode" pins it on demand.

(Recommend deciding via a short real-hardware look once 039 is running.)

## Scope (indicative — finalize after the decision + 039)

- Overlay frontend — a live-match scorecard layout (flags/country codes,
  large score, clock, yellow/red counts). Collapsed state carries the
  scoreboard; expanded keeps the manifest detail.
- `poller.rs` — surface yellow/red **counts** per side + clock into the
  live-match event/`SlotState` (today only the latest card's text is
  carried). Country/flag mapping from league + team.
- If Option A/C: overlay window sizing + slot-pinning logic
  (`queue.rs`/Engine + window positioning) — the heavy part.
- Tests: layout render tests; poller count-surfacing tests.

## STOP conditions

- Decision not made → STOP.
- Option A/C would touch `capabilities/default.json` or the receive-only
  guarantee → STOP.

## Notes

- Independent of event *coverage* (plan 043) — this is about how the
  score/clock/cards are *shown*, not about detecting more event types.
