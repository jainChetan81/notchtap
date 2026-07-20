# Plan 043: richer live-match event coverage (fouls, offside, disallowed goals, subs)

> **Executor instructions**: Follow this plan, run every gate, update the
> `plans/README.md` row when done. **Read "Step 0: CONFIRMED against a
> genuinely live match (2026-07-20)" below FIRST** — Step 0's gate has
> been satisfied with real evidence; you do not need to re-run it. But
> that same evidence surfaced a new requirement (`summary` is flaky —
> see below) that changes the "Approach" section below the original
> filing didn't have — read that too before writing fetch/parse code.
>
> **Coordination**: touches `poller.rs`, which 037 (the Engine) has
> already migrated (DONE, merged `6b53c32` — no coordination needed
> there anymore) and which 039 (the live-match card) will also touch —
> 039 is now unblocked (038 landed) but not yet executed; check
> `plans/README.md`'s row for 039 before starting, since this plan's
> "feed into 042's scorecard" framing assumes 039's `Recurring`
> live-match event shape exists.

## Status

- **TODO — Step 0 CONFIRMED, ready for Step 1.** Filed 2026-07-19 from
  live-session feedback: "are we reporting fouls, free kicks? Saka
  scored a good goal but was offside and I didn't see anything."
- **Priority**: P3 · **Effort**: M–L (no longer contingent on Step 0 —
  the data exists; the fallback-chain requirement below adds real but
  bounded scope, not open-ended risk) · **Risk**: MED (heavier feed,
  more polling + parsing, rate-limit care, **plus** the `summary`
  endpoint's confirmed flakiness means the fetch path needs a fallback
  chain, not a single request — see "Approach" below).
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
- **Step 0 result (2026-07-20 night, confirmed 2026-07-20)**: the
  question this plan was blocked on is now answered — see "Step 0:
  CONFIRMED against a genuinely live match" below for the full
  evidence. Short version: `commentary`/`keyEvents` both exist and grow
  monotonically through a live match (confirmed independently by two
  systems across 6+ polls of the same real match, FIFA World Cup Final,
  ESPN event `760517`), but the `summary` endpoint that carries them
  returned empty/404 on 2 of those polls even while the match was live
  — so the fetch path needs a fallback chain (core API
  `/competitions/{id}/plays`, then HTML scrape), not a single endpoint
  with retries. This is now folded into "Approach" below.

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

## Step 0: CONFIRMED against a genuinely live match (2026-07-20)

The "Step 0" gate below (filed by the review-plan pass above) has been
satisfied. During the actual FIFA World Cup Final (ESPN event
`760517`, league slug `fifa.world`), the `summary?event={id}` endpoint
was polled repeatedly across the full match by two independent
systems — full raw findings live in
`research/043-worldcup-final-verification/` (six `kimi-*` files plus
three `hermes-*` files; the one bad run,
`kimi-worldcup-final-api-UNRELIABLE-discard.md`, is flagged in its own
filename and excluded from the conclusions below).

1. **The key names are `commentary` and `keyEvents`** (not `plays` or
   `playByPlay` at the `summary` top level — those don't exist there).
   Confirmed on 4 independent polls spanning the match:

   | Poll (match clock) | `commentary` count | `keyEvents` count |
   |---|---|---|
   | 9' (1st half) | 9 | 1 |
   | 90'+8' (2nd half stoppage) | 115 | 29 |
   | 108' (overtime) | 144 | 41 |

   Both arrays grow monotonically as the match progresses — this is a
   live, updating timeline, not a static snapshot.

2. **`commentary` entry shape**: `sequence`, `time` (`{value: seconds,
   displayValue: "X'"}`), `text`, plus an embedded `play` object with
   `id`, `type` (foul, shot-on-target, kickoff, yellow-card, etc.),
   `period`, `clock`, `team`, `participants`, `fieldPositionX/Y` +
   `fieldPosition2X/Y` (pitch coordinates), `wallclock` (ISO
   timestamp), `source`. This is the target shape for Step 1's parser
   — richer than the scoreboard's existing `SbDetail`, with per-event
   type text ready to use directly (e.g. "Foul by Pedro Porro (Spain)
   on Nico González", "Shot On Target — Lamine Yamal (Spain) saved by
   Emiliano Martínez").

3. **`keyEvents` entry shape**: `id`, `type`, `text`, `period`, `clock`,
   `scoringPlay` (bool), `wallclock`, `shootout` (bool), `source`. This
   is the filtered/significant-events-only view — likely the better
   source for "card-worthy" event selection (open question below),
   with `commentary` feeding the fuller 042 scorecard detail view.

4. **New finding beyond what Step 0 asked for: `summary` is flaky even
   mid-match.** Of 6 polls against the same live match, 2 returned
   empty `{}` or 404 on every parameter variant tried (one at 39' into
   the first half, one at half-time) — not because the match wasn't
   live, but as an apparent transient API gap. Two working fallbacks
   were confirmed in the same research pass:
   - Core API `/competitions/{id}/plays` (paginated, 25/page — 625
     plays available by 39') — same event data, different endpoint,
     worked when `summary` didn't.
   - HTML-scraping `https://www.espn.com/soccer/match/_/gameId/{id}`
     as a last resort — worked when both API paths failed (confirmed
     at the half-time poll).

   This means Step 1's fetch path cannot be "hit `summary`, retry on
   failure" — it needs to fall through to `/competitions/{id}/plays`
   on an empty/error response. (HTML scraping is a heavier, more
   fragile last resort; whether to build it or just tolerate an
   occasional missed poll is an open question for whoever builds this,
   not decided here.)

**The original Step 0 STOP condition ("no play-by-play found on a live
match → STOP and report") does not fire.** The data is there. Proceed
to Step 1 with the shapes and fallback-chain requirement above.

## Approach

Getting these requires ESPN's richer **per-match play-by-play /
commentary / "summary" endpoint** (a different URL from the scoreboard,
confirmed above), which **is now confirmed** to carry the timeline of
key events while a match is live — see "Step 0: CONFIRMED" above for
the verified `commentary`/`keyEvents` shapes. This is a real
integration, not a config tweak:

- New fetch path (per live match, not per league) against the play-by-play
  endpoint; capped via `net.rs` (plan 025 — reuse `build_poll_client()`/
  `read_body_capped()` as-is, same as every other poller); polite poll
  interval + only while a match is `in` play (don't poll finished/
  scheduled matches). **Must fall through to core API
  `/competitions/{id}/plays` when `summary` returns empty/error** — see
  "Step 0: CONFIRMED" point 4 above; this isn't optional hardening,
  `summary` was observed failing on a genuinely live match in the
  Step 0 evidence, not just a hypothetical.
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

## Step 0 original instructions (SATISFIED — kept for reference only)

**Status: done.** See "Step 0: CONFIRMED against a genuinely live
match (2026-07-20)" above for the actual result — key names, shapes,
growth counts, and the fallback-chain finding. The instructions below
are kept only so a future reader can see what was being verified; do
not re-run this check before starting Step 1.

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

## Done criteria (finalize at build)

| Check | Command (from `src-tauri/`) | Expected |
|---|---|---|
| Rust tests | `cargo test --locked` | all pass; §0 updated |
| Lint/format | `cargo clippy --locked --all-targets -- -D warnings && cargo fmt --check` | exit 0 |
| Disallowed goal | test: a VAR/offside-disallowed event surfaces (or is deliberately filtered) per decision | pass |
| Opt-in off = no extra polling | test: `espn_play_by_play=false` hits only the scoreboard endpoint, never the summary one | pass |
| Dedup | test: a real goal reported by both the scoreboard and play-by-play feeds produces exactly one card, not two | pass |
| Fallback chain | test: a `summary` response of empty `{}`/error causes a fetch against `/competitions/{id}/plays` instead of silently dropping that poll — see "Step 0: CONFIRMED" point 4 | pass |

## Open questions (resolve at build)

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

- ~~Step 0 finds no play-by-play/commentary data on a genuinely live
  match~~ — **resolved, does not apply.** Step 0 confirmed the data
  exists (see above); kept struck-through rather than deleted so a
  reader knows this was a real gate that got cleared, not an
  oversight.
- The fallback chain (`summary` → core API `/competitions/{id}/plays`)
  both fail on the same poll during a live match → STOP and report;
  this would mean the live-match data path is unavailable for reasons
  beyond the flakiness Step 0 already characterized, and needs a fresh
  decision rather than a third fallback improvised on the spot.
- ESPN play-by-play endpoint requires auth / terms incompatible with
  polling → STOP and surface to the operator (as with 040's provider).
- Would touch `capabilities/default.json` → STOP.
