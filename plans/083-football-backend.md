# Plan 083: Football backend — crest fetch/cache, structured wire fields, richer ESPN events (summary→plays fallback)

> **Executor instructions**: This is a build plan for the BACKEND half of
> the football card — plan 079 items 3 (crests), 4 (structured wire
> shape, decided STRUCTURED not pre-joined), and 6a (richer ESPN match
> events with a mandatory fallback chain). It gates plan 084 (the
> display half): 084 consumes exactly what this plan puts on the wire,
> so land this first. Three workstreams (a/b/c below) — they share only
> `poller.rs` surface area; land them as separate commits in the order
> written (b before c, since c's dedup depends on b's structured
> identity). The crest workstream has a hard legal/scope rule from the
> sourcing research: crest PNGs are runtime-cached by rust, NEVER
> committed to git. Item 6a's fallback chain is not optional — the
> `summary` endpoint 404'd/emptied on 2 of 6 live polls during
> verification. When done, update the status row for this plan in
> `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 4fb3af9..HEAD -- src-tauri/src/poller.rs src-tauri/src/event.rs src-tauri/src/config.rs src-tauri/src/settings.rs src-tauri/src/queue.rs src-tauri/src/net.rs src-tauri/src/lib.rs src-tauri/capabilities/default.json src-tauri/tauri.conf.json src/useSlotState.ts`
> Any diff means line refs below have shifted — re-read before editing.
> Plan 064 already landed (`apply_fresh_content` copies `meta` on
> supersede — this plan's new meta fields ride Topic supersession
> correctly BECAUSE of that fix; if you find it reverted, STOP).
> (Baseline re-stamped `9a954b0` → `4fb3af9` on 2026-07-21 when plan 063
> merged, and `src-tauri/src/lib.rs` ADDED to the path list — 063's
> +41-line lib.rs block shifted the full SlotState test literal this
> plan edits to `lib.rs:1057`. No other 083-scoped file changed in
> 063's merge; every other citation re-verified unchanged by direct
> read.)

## Status

- **Priority**: P2
- **Effort**: M–L (three workstreams; 6a is the L half — new endpoint,
  fallback chain, dedup)
- **Risk**: MED — all additive/opt-in, but touches `poller.rs`'s
  diff/emission heart and adds a second ESPN endpoint with its own
  failure modes; the crest cache is this repo's first binary-asset
  cache
- **Depends on**: none structurally. Gates 084 (football card display).
  Plan 064 (meta-carrying supersede) is a DONE prerequisite — its
  behavior is load-bearing here.
- **Category**: direction (locked 2026-07-20 — 079 items 3/4/15 + 6a
  confirmed executable independently) → build
- **Planned at**: commit `9a954b0`, 2026-07-20 (reviewed same date:
  drift baseline corrected + `queue.rs`/`net.rs` added to the check,
  asset-protocol route corrected to tauri.conf.json-only, CSP step
  added, endpoint URLs/pagination inlined, biome gate added).
  **Review-plan pass 2 (2026-07-21, against `4fb3af9`)**: every rust
  citation re-verified exact (poller.rs :75-83/:144-179/:348-357/
  :359-371/:373-436/:443-460/:481-487/:497-526/:711, event.rs
  :150-162/:183-206/:320/:376, queue.rs :496/:524-530 — plan 064's
  meta copy live at :529, config.rs :367-370, settings.rs :505-509,
  notifier.rs :163, tauri.conf.json :32-33, capabilities/default.json's
  two event permissions, SettingsApp.tsx :715/:769-770); one stale ref
  fixed (`lib.rs:1016` → `:1057`, shifted by 063) and lib.rs added to
  the drift paths. One precision gap pinned in Step 1: the flag-off
  "byte-identical" promise needs `skip_serializing_if` on the new
  field, or every payload gains an `"espn": null` key. Drift baseline
  re-stamped to `4fb3af9`.

## Why this matters

Three locked decisions from plan 079 need backend truth before any
football-card display work is honest: (3) real club crests — ESPN's
already-fetched scoreboard response carries a direct `team.logo` URL
per team, so the discovery cost is zero and only fetch/cache/serve
plumbing remains; (4) the live-match wire shape is STRUCTURED fields
(league, home/away abbrev, home/away score, clock, per-side cards) —
`matchup()` (`poller.rs:359-371`) currently pre-joins all of it into
one display string, which is why the card can't lay out a
crest–score–crest row; (6a) the confirmed-executable richer-events
fetch — independent overnight research (raw evidence in
`research/043-worldcup-final-verification/`, cross-checked Hermes +
Kimi runs) verified ESPN's `summary` endpoint `commentary`/`keyEvents`
fields exist and grow monotonically across a real World Cup Final
(event `760517`: 9→115→144 commentary and 1→29→41 keyEvents entries at
9'/90'+8'/108'), AND that `summary` returned empty/404 on 2 of 6 polls
even while live — hence a fallback chain to the core API
`/competitions/{id}/plays` (confirmed working when `summary` wasn't),
not a single endpoint with retries. (Caution: the file
`kimi-worldcup-final-api-UNRELIABLE-discard.md` in that directory is a
fabricated run kept as a cautionary artifact — never cite it.)

## Current state

- `src-tauri/src/poller.rs:75-83` — `SbTeam` parses `id` +
  `abbreviation` only; NO `logo` field yet. ESPN's team object also
  carries `logo` (a CDN URL of the form
  `a.espncdn.com/i/teamlogos/soccer/500/{id}.png`) — verified present
  in the checked-in fixtures (`src-tauri/tests/fixtures/scoreboard-esp.1.json`,
  per 079's Session-0 record).
- `src-tauri/src/poller.rs:144-179` — `MatchSnapshot` already parses
  everything item 4 needs: `home_abbrev`, `away_abbrev`, `home_score`,
  `away_score`, `state`, `status_name`, `display_clock`,
  `home_cards`/`away_cards` ((yellow, red) tuples). `matchup()`
  (`poller.rs:359-371`) joins them into the display string;
  `league_label()` (`poller.rs:348-357`) maps league slugs to
  EPL/UCL/La Liga.
- `src-tauri/src/event.rs:150-162` — `EventMeta` (source/category/
  published_at_ms/link/subtitle/details), mirrored onto
  `SlotState::Showing` (`event.rs:183-206`) and validated field-by-field
  frontend-side (`src/useSlotState.ts:77-127`). Plan 042's
  Clock/Cards cells ride `meta.details` built once per match at
  `poller.rs:497-526`.
- `src-tauri/src/queue.rs:524-530` — `apply_fresh_content` copies
  `meta` on supersede (plan 064's fix) — new structured meta therefore
  survives Topic supersession, which the sticky live card depends on.
- Config/flags precedent: `espn_live_card` (default `false`,
  `config.rs`; settings toggle in `src/settings/SettingsApp.tsx`'s
  Football section — the section function starts at
  `SettingsApp.tsx:715`, the `espn_live_card` `ToggleControl` to copy
  is at `:764-771`) — item 6a's flag mirrors this exactly.
- Crest-serving constraint: the overlay webview has NO network access
  and `src-tauri/capabilities/default.json` holds only
  `core:event:allow-listen`/`allow-unlisten`. AGENTS.md's v5 amendment
  pins `default.json` against invoke-command creep ("must never
  change"). NOTE (reviewer-verified): tauri v2 has NO asset-protocol
  capability permission — asset access is governed solely by
  `app.security.assetProtocol` in `tauri.conf.json`, so route (i)
  leaves `default.json` byte-identical. Also note `tauri.conf.json:32`
  sets `csp: default-src 'self'` (and `devCsp` at :33) with no
  `img-src` for `asset:`/`crest:` — CSP is part of the serving work
  (Step 2). Config-dir precedent for the cache location:
  the literal `~/.config/notchtap/` path is built at
  `config.rs:367-370` (`Config::dir_from_home`), reached via
  `settings.rs:505-509` (`notchtap_config_dir`); `notifier.rs:163`.
- ESPN fetch posture: `reqwest` client with the hardened poller
  posture (UA, redirect limit, timeout, byte cap — plan 010/025;
  `poller.rs:711` `fetch_league`, shared helpers in `net.rs`). The
  poller's diff heart is pure and fixture-tested
  (TESTING_STRATEGY.md §4.7) — keep it that way.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `cd src-tauri && cargo test --locked` | all pass |
| Clippy | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Rust format | `cd src-tauri && cargo fmt --check` | exit 0 |
| Frontend unit tests (wire validator) | `npx vitest run` | all pass |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Lint + format gate (SettingsApp.tsx is edited) | `npx biome ci .` | exit 0 |

## Scope

**In scope**:
- (a) Crests: `SbTeam.logo` parsing; a rust-side crest fetch+cache
  under the notchtap config dir; serving cached PNGs to the webview
  (Step 2's two routes); text-abbrev fallback when no cached crest
  exists. **NEVER commit a crest PNG to git** — trademarked art;
  runtime-caching a feed-provided URL is the materially lower-risk
  posture (`research/2026-07-20-icon-artwork-sourcing.md`).
- (b) Structured wire fields: extend the espn slot payload's meta with
  league, home/away abbrev, home/away score, clock, and per-side cards
  tuples, from `MatchSnapshot`'s existing fields; mirror onto
  `SlotState::Showing` + the TS type + `isValidSlotState`.
- (c) Item 6a: ESPN `summary` endpoint `commentary`/`keyEvents`
  polling for live matches with fallback to the core API
  `/competitions/{id}/plays`; map the richer event types (foul,
  offside, VAR, substitution — goal/penalty/own-goal/yellow/red already
  exist); dedup against the scoreboard feed; opt-in config flag
  mirroring `espn_live_card`.
- Tests per the Test plan below (fixture-first, no live network).

**Out of scope**:
- ALL display work — the card layout, event icons, celebrations (plan
  084). This plan ends at the wire; 084 renders it.
- Changing existing event semantics: score/state/card emission rules
  (`poller.rs:443-460`'s documented rules) stay byte-identical with
  both new flags off.
- Telegram/connector fan-out policy for the new 6a events (default:
  they flow through `Engine::accept` like every other Football event;
  if that proves noisy in practice it's a follow-up decision, not this
  plan's).
- Now-playing (079 item 16) and any non-ESPN provider (item 15 locked:
  ESPN stays).

## Steps

### Step 1 (workstream b): Structured espn meta on the wire

Add an espn-only structured block to `EventMeta` — prefer a single
optional sub-struct over seven flat fields (keeps `EventMeta`'s
plan-035 flat-field shape clean; serde `default` keeps every other
source's payload byte-identical — **but only with
`#[serde(skip_serializing_if = "Option::is_none")]` on the new field,
pinned at review-plan pass 2**: this struct's house style serializes
`None` Options as explicit `null` (pinned by
`slot_state_showing_without_metadata_serializes_null_fields`,
event.rs:375), so without the skip EVERY payload — manual, cmux, news,
flag-off espn — gains an `"espn": null` key and the flag-off
byte-identical pin below fails at the JSON level. Apply the skip in
BOTH places the field lands (`EventMeta` and the `SlotState::Showing`
mirror); it deviates from the house style for exactly one field,
deliberately, so "byte-identical" stays literally testable):

```rust
/// plan 083: structured live-match fields (079 item 4 — decided
/// STRUCTURED, not pre-joined). Present only on Football events when
/// the live-card flag is on; everything else leaves it `None` and the
/// wire is unchanged. Display-only, like the rest of `EventMeta`.
pub struct EspnMeta {
    pub league: String,        // friendly label already ("EPL"/"UCL"/"La Liga")
    pub home_abbrev: String,
    pub away_abbrev: String,
    pub home_score: u32,
    pub away_score: u32,
    pub clock: String,         // display_clock, e.g. "78'"
    pub home_cards: (u32, u32), // (yellow, red)
    pub away_cards: (u32, u32),
}
```

Populate it in `poller.rs` at the same place plan 042 builds the
Clock/Cards `details` (`poller.rs:497-526`) — same `topic.is_some()`
gate, so flag-off payloads stay byte-identical (pin that with a test).
Mirror onto `SlotState::Showing` in `event.rs` (camelCase via the
existing `rename_all_fields`, same `skip_serializing_if` as above),
and copy the block in at the Showing
construction site `queue.rs:496` (`current_slot_state`) plus the test
constructors (`event.rs:320`, `event.rs:376`, `lib.rs:1057`) — the
compiler will name them all via missing-field errors if you miss one.
Then the TS side: `useSlotState.ts`
type + `isValidSlotState` grows an optional nested-object check (absent
or valid — a malformed espn block must fall back like every other
field, the discipline stated at `useSlotState.ts:72-103`). The pre-joined
`matchup()` string stays as `title` (it remains the compact title for
non-live presentation); the structured block is ADDITIVE.

**Verify**: `cd src-tauri && cargo test --locked` → all pass;
`npx vitest run` → all pass; `npx tsc --noEmit` → exit 0.

### Step 2 (workstream a): Crest parse, cache, serve

1. Parse: `SbTeam` gains `#[serde(default)] pub logo: Option<String>`
   (`poller.rs:75-83` — one field; ESPN already sends it).
2. Cache: a small `crests.rs` (or a `poller.rs` submodule — match repo
   layout judgment): given team id + logo URL, fetch the PNG with the
   poller's hardened reqwest posture and store it under the config dir
   (`~/.config/notchtap/crests/{league-slug-safe-id}.png` — config-path
   conventions per `settings.rs:505-506`). Cache policy: fetch only on
   cache miss (the same team's logo is never refetched within a run,
   and a tiny on-disk cache survives restarts); one in-flight fetch per
   team; failures are silent-with-fallback (text abbrev), never
   poller-fatal. Bound the fetch (byte cap — crests are small PNGs;
   256 KiB is generous) and the cache (per-league team counts are
   bounded by ESPN's feed; no eviction needed for v1 — note it).
3. Serve to the webview — two routes, pick (i) unless it hits the
   STOP condition: (i) tauri asset protocol — enable
   `app.security.assetProtocol` in `tauri.conf.json` with `enable:
   true` and `scope` covering ONLY the crests dir; the frontend reads
   via `convertFileSrc`. REVIEWER CORRECTION: tauri v2 has no
   asset-protocol capability permission — `capabilities/default.json`
   stays byte-identical on this route (which also moots the AGENTS.md
   "must never change" tension; say so in the completion report).
   CSP (load-bearing, silent-failure-prone): `tauri.conf.json:32`'s
   `csp` and :33's `devCsp` are `default-src 'self'` — extend BOTH
   with `img-src` allowing `asset: http://asset.localhost` (route i)
   or `crest:` (route ii), or the crests render as broken images with
   NO automated test catching it (only the manual live-match check
   would). (ii)
   Fallback if (i) is rejected: a custom `crest://` URI scheme protocol
   registered rust-side (`register_uri_scheme_protocol`) serving cached
   files — zero capability changes, `default.json` byte-identical,
   more rust code (and the same `img-src` CSP addition for `crest:`).
4. Wire: carry the crest availability on the structured meta from Step
   1 — e.g. `home_crest: Option<String>` / `away_crest` holding the
   servable URL (asset-protocol URL or `crest://` URL) when cached,
   `None` otherwise; the frontend renders the text-abbrev fallback on
   `None` (that fallback is 084's render work; this plan ends at the
   URL being present/absent honestly). On a cache hit arriving AFTER a
   card is already showing, the next poll's supersede carries it
   forward naturally (plan 064's meta-copy) — no special path.

**Verify**: `cd src-tauri && cargo test --locked` → all pass (new
tests: logo URL parses from a checked-in fixture; cache-miss → fetch
scheduled, cache-hit → none, via a test-local temp dir; fetch failure →
`None`, no panic); `cargo clippy --locked --all-targets -- -D warnings`
→ exit 0; `cargo fmt --check` → exit 0; `git status` shows NO new/
untracked `*.png` (tracked PNGs under `src-tauri/icons/` legitimately
exist — the bar is no NEW ones).

### Step 3 (workstream c, part 1): `summary` fetch + richer event parse

Opt-in flag first: `espn_rich_events` (default `false`), mirroring
`espn_live_card`'s full path — `config.rs` field + default-fn +
parse-heal awareness, `settings.rs` `validate()` if range-like (it's a
bool — follow the bool precedent, likely no validation), Settings
Football-section toggle in `src/settings/SettingsApp.tsx` following the
`espn_live_card` toggle at `:764-771`'s exact shape. Then the fetch:
for each match currently tracked live (only when the flag is on), poll
ESPN's `summary` endpoint —
`https://site.api.espn.com/apis/site/v2/sports/soccer/{league}/summary?event={event_id}`
(same host family as the existing scoreboard URL at `poller.rs:712`;
verified live in the 043 research) — and parse `commentary`/`keyEvents`
into a typed
event stream — map at minimum: foul, offside, VAR check, substitution
(the four locked informational types; goal/penalty/own-goal/yellow/red
already flow from the scoreboard feed — do NOT re-emit those from
summary). Endpoint shape and field evidence:
`research/043-worldcup-final-verification/` (hermes + kimi findings
files — read the findings, NOT the UNRELIABLE-discard file). Keep the
parse pure and fixture-testable: capture/redact a small `summary` JSON
fixture from the research evidence (or synthesize one matching the
documented shape with a comment naming its provenance), drop it in
`src-tauri/tests/fixtures/`.

**Verify**: `cd src-tauri && cargo test --locked` → all pass (parse
tests on the fixture: each of the four event types extracted,
monotonic-growth handling, unknown event types skipped not fatal).

### Step 4 (workstream c, part 2): Fallback chain + dedup

Fallback is mandatory, not a retry: when `summary` errors, 404s, or
returns empty for a match known-live, fall back to the core API —
`https://sports.core.api.espn.com/v2/sports/soccer/leagues/{league}/events/{event_id}/competitions/{competition_id}/plays`
(confirmed working during the live verification when `summary`
wasn't). PAGINATION (pinned): the feed is paginated at 25 plays/page
(625 plays = 25 pages by the 39th minute in the 043 evidence, and
`?limit=300` returned an internal error) — fetch the NEWEST page only
each poll and let the seen-set dedup absorb the overlap; do not
backfill pages. Chain: `summary` → `plays` →
give-up-this-poll (silent, next poll retries — same posture as the
scoreboard's absent-poll carry-forward, `poller.rs:455-460`). Dedup vs
the scoreboard feed: the scoreboard already emits
goal/penalty/own-goal/yellow/red/kickoff/halftime/fulltime — the rich
feed must NEVER re-emit a scored-goal/card event the scoreboard path
owns. Dedup key on (match id, event type, clock, athlete where
present); keep a small per-match seen-set carried in the snapshot
(mirror the `missed_polls` carry-forward pattern — the field lives at
`poller.rs:164-169`, its reset/evict logic in `diff_scoreboard`), and
drop any summary/plays event whose type is scoreboard-owned outright
(they're redundant by construction, not just by key-collision). Emitted events
route through `Engine::accept` like all Football events, as
one-shots with the football TTL — the sticky-card supersession is the
existing Topic machinery's job (`poller.rs:373-436`), not new code
here. Signal mapping: extend `EventSignal` ONLY if 084's icons need a
new variant (foul/offside/var/sub likely do) — that's a wire-enum
addition; keep it serde-snake_case and update the TS mirrors
(`src/useSlotState.ts:4-13`, `src/lib/presentation.ts`'s exhaustive
tables will
fail to compile until updated — good, that's the seam working).

**Verify**: `cd src-tauri && cargo test --locked` → all pass (new
tests: fallback triggers on summary-404 and on summary-empty; plays
used when summary fails; dedup drops a summary "goal" the scoreboard
already emitted; flag-off → zero behavior change, byte-identical
snapshots/events); `cargo clippy --locked --all-targets -- -D warnings`
→ exit 0; `cargo fmt --check` → exit 0.

### Step 5: Full gate

**Verify**: every command in the Commands table exits 0;
`git status`/`git diff --stat` shows no binary assets, no prototype
edits, no display-layer (`src/components/StatusRailCard.tsx`/`src/styles.css`) changes.

## Test plan

Following `docs/TESTING_STRATEGY.md` §4.7 — the poller heart stays
pure and fixture-tested; no live network in tests:

- **Parse (cargo)**: `team.logo` extracted from the checked-in
  scoreboard fixture; `EspnMeta` populated correctly from a
  `MatchSnapshot`; summary/plays fixture parsing for foul/offside/VAR/
  sub; unknown event types skipped.
- **Fallback (cargo)**: summary-404 → plays consulted; summary-empty →
  plays consulted; both-fail → silent, carried state intact, next poll
  unaffected (mirror the absent-poll carry-forward tests' shape).
- **Dedup (cargo)**: a summary goal/card event never re-emits;
  informational events dedupe on re-poll (same key twice → one event).
- **Flag-off pins (cargo)**: with `espn_rich_events` false and
  `espn_live_card` false, events+snapshots byte-identical to pre-083
  (the plan-039 flag-off pin at `poller.rs` tests is the model).
- **Wire (vitest)**: `isValidSlotState` accepts a payload with a valid
  espn block / absent block, rejects a malformed one (falls back), per
  the existing validator test patterns.
- **Manual-only** (operator): with both flags on during a real live
  match — crests appear, richer events arrive, nothing double-fires.
  Also the one real-network path here (crest fetch), unverifiable in
  CI by design.

## Done criteria

- [ ] `EspnMeta` (or equivalent structured block) on the wire end-to-end: poller → EventMeta → SlotState → TS validator; flag-off payloads byte-identical (pinned by test)
- [ ] `SbTeam.logo` parsed; crest PNGs fetched+cached under the config dir, bounded, failure-safe; served to the webview via the chosen route with its justification written; text-abbrev fallback representable (`None` on the wire)
- [ ] No crest PNG committed anywhere in git (`git ls-files | grep -c png` unchanged for crest files)
- [ ] `espn_rich_events` flag wired config → settings → validate → toggle, mirroring `espn_live_card`
- [ ] summary→plays fallback chain implemented with both fallback triggers tested; dedup vs scoreboard-owned events tested
- [ ] `cargo test --locked`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo fmt --check`, `npx vitest run`, `npx tsc --noEmit` all exit 0
- [ ] `plans/079-checklist.html` and `plans/frontend-ui-consolidated.html` statuses updated (items 3/4/6a backend → shipped); `plans/README.md` row for 083 updated

## STOP conditions

- The crest `<img>`s silently don't render after the serving route
  lands — that's the CSP (Step 2's `img-src` addition), not the cache
  or the fetch; nothing in the automated suite catches it, so check it
  first before debugging the pipeline.
- The asset-protocol route (Step 2 route i) requires a `default.json`
  change after all (it shouldn't — tauri v2 governs assets via
  `tauri.conf.json` alone), or `tauri.conf.json`
  asset scoping proves broader than the crests dir — stop and present
  routes (i) vs (ii) to the operator rather than choosing under a
  security-rule cloud.
- **Plan 064's meta-carrying supersede is missing or reverted** — the
  sticky card's fresh meta depends on it; do not work around it, stop.
- The `summary`/`plays` response shape in the research evidence
  contradicts what live polling shows during implementation (ESPN is
  undocumented/best-effort — the reason 043 needed a chain at all) —
  stop and capture fresh evidence rather than coding to stale docs.
- The richer event stream turns out to duplicate scoreboard-owned
  events in a way key-based dedup can't cleanly separate (e.g. clocks
  disagree between feeds) — stop and present the conflict; do NOT ship
  double-notifications.
- Any temptation to vendor a crest PNG into git "for the fallback" —
  the fallback is text abbreviations, full stop (trademark; the
  sourcing research is explicit).

## Maintenance notes

- Update `plans/079-checklist.html` and
  `plans/frontend-ui-consolidated.html`: items 3 (crests) and 4 (wire
  shape) move from open to shipped-backend; the football mockup's
  "Still open" list shrinks accordingly. Item 6's DISPLAY half stays
  with 084.
- The crest cache introduces the repo's first binary cache dir —
  document its location + lifecycle in `docs/ARCHITECTURE.md`'s
  config/logging paths section when it lands (one line, where
  `~/.config/notchtap/` is already documented).
- If `EventSignal` grew (Step 4), the pinned seam list grows with it:
  `useSlotState.ts`'s `EVENT_SIGNALS`, `presentation.ts`'s exhaustive
  tables, and any signal-count tests all changed in lockstep — future
  signal additions should copy this plan's checklist.
- 084 reads this plan's wire shape as its contract: if anything in
  `EspnMeta`'s final shape diverged from Step 1's sketch during
  implementation, update 084's Current-state section before dispatching
  it.
