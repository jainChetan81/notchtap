# Plan 057: SPIKE — evaluate a paid sports-data API (e.g. Sportmonks) as an ESPN alternative

## Status

- **Priority**: P3
- **Effort**: M (evaluation) / L (migration, if approved)
- **Risk**: LOW (evaluation) — a design doc, no source changes
- **Depends on**: none (informs 043, which is currently gated on whether
  ESPN's endpoint has play-by-play data at all)
- **Category**: direction / dependency
- **Planned at**: commit `f58ced2`, 2026-07-19 — filed from an operator
  question during a live UI walkthrough.

## Why this matters

`poller.rs:712` hits `https://site.api.espn.com/apis/site/v2/sports/soccer/{league}/scoreboard`
— ESPN's public **undocumented** endpoint. This is called out explicitly,
twice, in the repo's own docs as a risk: `docs/IMPLEMENTATION_PLAN.md:134`
("undocumented public endpoint, best-effort — no sla, no notice before…")
and `docs/archive/BLIND_REVIEW.md:70` ("ESPN API is undocumented public,
best-effort. Poller must fail gracefully"). Plan 043's own Step 0 (filed
2026-07-20) independently hit this exact limitation trying to extend the
same endpoint for richer event data — it fetched two real match
summaries and found no play-by-play/commentary key in either response,
which is *why* 043 is gated rather than built.

**Nobody has evaluated a paid alternative before this.** A repo-wide
grep for `sportmonk`, `api-football`, and "paid api" across `code`,
`docs/`, and `plans/` returns zero hits.

## Current state

- ESPN poller: `src-tauri/src/poller.rs` — no API key, no rate-limit
  contract (best-effort), no SLA.
- Cost so far: $0, but bounded by "whatever ESPN feels like providing,
  whenever they feel like changing it."
- Known gap this would solve: plan 043 (richer event coverage — fouls,
  offside, disallowed goals, subs) is blocked specifically because
  ESPN's endpoint doesn't expose that data reliably.

## What a paid alternative would need to prove out

1. **Actual data richness** — does it have real play-by-play/commentary
   with structured event types (not just score updates)? This directly
   unblocks 043 if yes.
2. **Documented, versioned API** — contrasted with ESPN's "could change
   without notice" status; a real SLA and changelog is the whole point
   of paying.
3. **Cost model at this app's actual usage** — current `espn_poll_secs`
   default and league count (`espn_leagues`, `config.rs`) sets the
   request-volume baseline; check against Sportmonks' (or a comparable
   provider's) plan tiers and rate limits at that volume.
4. **Auth/key handling** — this app already has a secrets pattern
   (`~/.config/notchtap/`, mode `0600`, the OpenRouter/Telegram
   precedent) — a new provider key would follow that, not invent a new
   mechanism.
5. **Migration blast radius** — `poller.rs`'s `make_event`/`diff_scoreboard`
   functions are ESPN-response-shaped today; a provider swap touches all
   of them, plus the fixture-based test suite (`poller.rs` tests read
   captured ESPN JSON fixtures — a new provider needs its own fixture
   set).

## Decision needed (operator)

- Is this worth spending money on, given ESPN has worked so far (no
  actual outage/breakage reported in this repo's history)?
- If yes: which provider — Sportmonks was the one named, but API-Football
  (RapidAPI) and others exist; a real evaluation should compare at least
  two on price/coverage/data richness before picking one.
- Scope: full ESPN replacement, or ESPN stays primary with a paid API
  only for the specific data ESPN lacks (richer events)?

## Recommendation

Don't commit to a provider without a real trial-account comparison —
this needs actual API responses pulled and inspected (similar to how
043's Step 0 pulled real ESPN responses), not a decision made from
marketing pages. Worth doing specifically *if* 043 stays blocked on
ESPN's data gap — that's the concrete, evidenced reason to pay for
something better, versus a speculative "paid is probably better" swap.

## Maintenance notes

- If pursued, this should land as a `docs/design/` spike doc (following
  the pattern of plans 030/031/049-053) before any `poller.rs` change —
  same rigor as every other sourced-data decision in this repo.
