# Plan 043: richer live-match event coverage (fouls, offside, disallowed goals, subs)

> **Executor instructions**: Follow this plan, run every gate, update the
> `plans/README.md` row when done.
>
> **Coordination**: touches `poller.rs` (037) — sequence AFTER 037, and
> ideally after/with 039. Do not edit `poller.rs` while 037 is in flight.

## Status

- **TODO** — filed 2026-07-19 from live-session feedback: "are we
  reporting fouls, free kicks? Saka scored a good goal but was offside and
  I didn't see anything."
- **Priority**: P3 · **Effort**: M–L · **Risk**: MED (heavier feed,
  more polling + parsing; rate-limit care).
- **Depends on**: 037 (coord), 039 (soft — same live-match surface).

## Problem

The poller reads ESPN's **scoreboard summary** endpoint and emits only
five event kinds: **Goal** (score change), **Yellow/Red card**,
**Kickoff**, **Half-time**, **Full-time**. Fouls, free kicks, offsides,
shots, substitutions, and **disallowed/VAR goals are not in that feed**,
so they're never surfaced. Concretely: an offside-disallowed goal means
the score never changes — from the scoreboard feed *nothing happened* —
so the operator correctly saw nothing.

## Approach

Getting these requires ESPN's richer **per-match play-by-play /
commentary / "summary" endpoint** (a different URL from the scoreboard),
which carries the timeline of key events. This is a real integration, not
a config tweak:

- New fetch path (per live match, not per league) against the play-by-play
  endpoint; capped via `net.rs` (plan 025); polite poll interval + only
  while a match is `in` play (don't poll finished/scheduled matches).
- Parse the event timeline; map event types → `MatchState`/`EventSignal`
  (goal, disallowed/VAR, offside, foul, free kick, sub, shot on target…).
  Decide which are **card-worthy** (interrupt) vs merely feed into the
  042 scorecard/consolidated card — don't spam a card per foul.
- Dedup against the scoreboard-derived goals so a real goal isn't
  double-reported by both feeds.
- Config: a `weather`-style opt-in (e.g. `espn_play_by_play` default
  `false`) — this is materially more polling, so it must be opt-in.

## Done criteria (finalize at build)

| Check | Command (from `src-tauri/`) | Expected |
|---|---|---|
| Rust tests | `cargo test --locked` | all pass; §0 updated |
| Lint/format | `cargo clippy --locked --all-targets -- -D warnings && cargo fmt --check` | exit 0 |
| Disallowed goal | test: a VAR/offside-disallowed event surfaces (or is deliberately filtered) per decision | pass |
| Opt-in off = no extra polling | test: `espn_play_by_play=false` hits only the scoreboard endpoint | pass |

## Open questions (resolve at build)

- Which event types are **card-worthy** vs scorecard-only? (foul-per-card
  would be noise — likely only goals, disallowed/VAR, red cards, penalties
  interrupt; the rest feed 042's scorecard.)
- Poll interval + rate limits for the heavier per-match feed.

## STOP conditions

- ESPN play-by-play endpoint requires auth / terms incompatible with
  polling → STOP and surface to the operator (as with 040's provider).
- Would touch `capabilities/default.json` → STOP.
