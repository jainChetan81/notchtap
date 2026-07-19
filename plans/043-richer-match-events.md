# Plan 043: richer live-match event coverage (fouls, offside, disallowed goals, subs)

> **Executor instructions**: Follow this plan, run every gate, update the
> `plans/README.md` row when done. **Read "Verified against live ESPN
> endpoints" below FIRST** — this review-plan pass found real evidence
> that the plan's core assumption (the `summary` endpoint carries a
> play-by-play timeline) may not hold, and Step 0 below is a hard
> verification gate on a genuinely live match before any other work.
>
> **Coordination**: touches `poller.rs`, which 037 (the Engine) has
> already migrated (DONE, merged `6b53c32` — no coordination needed
> there anymore) and which 039 (the live-match card) will also touch —
> 039 is now unblocked (038 landed) but not yet executed; check
> `plans/README.md`'s row for 039 before starting, since this plan's
> "feed into 042's scorecard" framing assumes 039's `Recurring`
> live-match event shape exists.

## Status

- **TODO, but see Step 0** — filed 2026-07-19 from live-session
  feedback: "are we reporting fouls, free kicks? Saka scored a good
  goal but was offside and I didn't see anything."
- **Priority**: P3 · **Effort**: M–L, contingent on Step 0's finding (see
  below — could be materially larger, or this endpoint could turn out
  not to work at all, which changes the effort question entirely) ·
  **Risk**: MED (heavier feed, more polling + parsing; rate-limit care)
  **plus** a real feasibility risk this pass surfaced (see below) that
  the original filing didn't have evidence for either way.
- **Depends on**: 037 (DONE — no longer a coordination concern), 039
  (soft — same live-match surface; unblocked, not yet executed).
- **Review-plan pass (2026-07-20)**: fetched ESPN's actual `summary`
  endpoint for two real matches (details below) to check whether it
  contains the play-by-play/commentary timeline this plan's entire
  approach depends on. **It did not, in either sample.** This doesn't
  disprove the plan's premise (neither sample was a live, in-progress
  match — the one case this plan actually cares about — and commentary
  may only populate during live play), but it's real counter-evidence
  the original filing didn't have, and it changes Step 1 from "build
  the fetch path" to "confirm the data exists on a live match, THEN
  build the fetch path." Also corrected the 037/039 coordination
  language (037 landed; 039 is unblocked, not still gated) and updated
  the config-opt-in pattern reference now that 040 (weather) has
  established a second real precedent for a `*_enabled`-style flag
  alongside `espn_enabled`/`rss_enabled`.

## Problem

The poller reads ESPN's **scoreboard** endpoint and emits only five
event kinds: **Goal** (score change), **Yellow/Red card**, **Kickoff**,
**Half-time**, **Full-time** (verified this pass: exactly five
`make_event` call sites in `diff_scoreboard`, `poller.rs:376,387,397,407,424`
— matches the original filing's count exactly). Fouls, free kicks,
offsides, shots, substitutions, and **disallowed/VAR goals are not in
that feed**, so they're never surfaced. Concretely: an offside-disallowed
goal means the score never changes — from the scoreboard feed *nothing
happened* — so the operator correctly saw nothing.

## Verified against live ESPN endpoints (this review-plan pass)

The original filing's "Approach" names "ESPN's richer per-match
play-by-play / commentary / 'summary' endpoint" as the source for this
data, without independent confirmation that it actually carries a
timeline. This pass fetched it for real:

1. **The endpoint exists and needs no key**: `GET
   https://site.api.espn.com/apis/site/v2/sports/soccer/{league}/summary?event={id}`
   (a real, distinct URL from today's `.../scoreboard`, matching the
   `net.rs` client posture this codebase already uses — keyless,
   `build_poll_client()`-compatible). Fetched for
   `uefa.champions/summary?event=401862897` (the same match the
   `scoreboard-uefa.champions.json` fixture captures, now finished) —
   the response's top-level keys are `boxscore`, `format`, `gameInfo`,
   `lastFiveGames`, `headToHeadGames`, `leaders`, `broadcasts`,
   `pickcenter`, `odds`, `rosters`. **No `commentary`, `keyEvents`, or
   any play-by-play timeline array present.**
2. Fetched a second time against a real *upcoming* EPL match
   (`eng.1/summary?event=401879301`, Coventry City at Arsenal,
   `status.type.state = "pre"` at fetch time) — top-level keys:
   `boxscore`, `format`, `gameInfo`, `lastFiveGames`, `headToHeadGames`,
   `broadcasts`, `pickcenter`, `odds`, `hasOdds`, `rosters`, `news`.
   Again, **no `commentary` or `keyEvents` key anywhere in the
   response.**
3. **What this does and doesn't prove**: neither sample was a match
   `in` play — no EPL match was live at fetch time, and the UCL match
   is long finished. It's plausible ESPN only populates a
   commentary/play-by-play array WHILE a match is actually live, and
   that a finished match's summary drops it, or a scheduled match's
   summary never had it to begin with. **This pass could not confirm
   or rule out that possibility** — it needs a genuinely live match to
   test, and none was available. What IS confirmed: the endpoint is
   real, reachable, free, and its schema (at least for non-live
   matches) does not contain the field this plan's whole approach
   depends on. Treat "the data is there during live play" as an
   unverified assumption, not a confirmed fact, until Step 0 checks it
   for real.

## Approach

Getting these requires ESPN's richer **per-match play-by-play /
commentary / "summary" endpoint** (a different URL from the scoreboard,
confirmed above), which is EXPECTED to carry the timeline of key events
while a match is live — **not yet confirmed, see Step 0**. This is a
real integration, not a config tweak:

- New fetch path (per live match, not per league) against the play-by-play
  endpoint; capped via `net.rs` (plan 025 — reuse `build_poll_client()`/
  `read_body_capped()` as-is, same as every other poller); polite poll
  interval + only while a match is `in` play (don't poll finished/
  scheduled matches).
- Parse the event timeline; map event types → `MatchState`/`EventSignal`
  (goal, disallowed/VAR, offside, foul, free kick, sub, shot on target…).
  Decide which are **card-worthy** (interrupt) vs merely feed into the
  042 scorecard/consolidated card — don't spam a card per foul.
- Dedup against the scoreboard-derived goals so a real goal isn't
  double-reported by both feeds.
- Config: an opt-in flag following the now-twice-established pattern
  (`espn_enabled`/`rss_enabled`, and as of plan 040 `weather_enabled` —
  all default `false` or gated the same way; `config.rs`'s
  `default_*` fn convention, e.g. `espn_play_by_play: bool` default
  `false`) — this is materially more polling, so it must be opt-in.

## Step 0: confirm the data exists on a real live match (REQUIRED before Step 1)

Do not write any fetch/parse code before this. During an actual live
match (any league), fetch
`https://site.api.espn.com/apis/site/v2/sports/soccer/{league}/summary?event={id}`
for that match's event id (get the id from the scoreboard endpoint's
response for a match with `status.type.state == "in"`) and inspect the
raw JSON for a play-by-play/commentary array. If found, note its exact
key name and one sample entry's shape (what fields it has — likely
something like `type`, `text`, `clock`, `team`, similar to the
scoreboard's own `SbDetail` shape) — this becomes the actual target
struct for Step 1's parser, replacing every "presumably" in this
plan's Approach with a verified shape. **If no such array is found on
a genuinely live match either**, STOP and report to the operator: this
plan's entire premise doesn't hold against ESPN's free API as currently
understood, and either a different ESPN endpoint/query param, a
different provider entirely, or dropping this plan is the real decision
— don't improvise a fallback data source without that conversation.

## Done criteria (finalize at build, after Step 0)

| Check | Command (from `src-tauri/`) | Expected |
|---|---|---|
| Rust tests | `cargo test --locked` | all pass; §0 updated |
| Lint/format | `cargo clippy --locked --all-targets -- -D warnings && cargo fmt --check` | exit 0 |
| Disallowed goal | test: a VAR/offside-disallowed event surfaces (or is deliberately filtered) per decision | pass |
| Opt-in off = no extra polling | test: `espn_play_by_play=false` hits only the scoreboard endpoint, never the summary one | pass |
| Dedup | test: a real goal reported by both the scoreboard and play-by-play feeds produces exactly one card, not two | pass |

## Open questions (resolve at build, after Step 0 confirms feasibility)

- Which event types are **card-worthy** vs scorecard-only? (foul-per-card
  would be noise — likely only goals, disallowed/VAR, red cards, penalties
  interrupt; the rest feed 042's scorecard.)
- Poll interval + rate limits for the heavier per-match feed — this
  pass's fetches were single manual requests, not a load test; ESPN's
  actual rate-limit behavior under sustained polling is still unknown
  and should inform the poll interval choice, not just "polite" as a
  vibe.
- Exact dedup key against the scoreboard feed's goal events — likely
  needs a shared match-minute + scorer correlation, not just "another
  goal happened," since either feed could report first depending on
  timing.

## STOP conditions

- Step 0 finds no play-by-play/commentary data on a genuinely live
  match → STOP and report to the operator (see Step 0) — this
  supersedes and sharpens the original filing's more generic "endpoint
  requires auth" STOP condition below; a missing-key-or-terms problem
  and a missing-data problem are different failures needing different
  responses, don't conflate them in the report.
- ESPN play-by-play endpoint requires auth / terms incompatible with
  polling → STOP and surface to the operator (as with 040's provider).
- Would touch `capabilities/default.json` → STOP.
