# Plan 079: Full overlay-card visual revamp — consolidated decision session (supersedes 043, 054, 055, 056, 060)

> **Executor instructions**: This is a pure decision-gathering plan — no
> code changes described anywhere in this file, **except** the confirmed
> backend data-fetch work inherited from plan 043 (see item 6a below),
> which is ready to execute independently of the visual decisions. Do not
> execute any *display* work here until every item in "Decision needed"
> below has been resolved by the operator in a dedicated `/grilling`
> session against this file. This plan supersedes plans 043, 054, 055,
> 056, and 060 — do not execute those files separately; their content is
> folded into the topic list below.

## Status

- **Priority**: P2 (matches the higher-priority items it absorbs; the
  overlay's visual language is seen constantly, unlike a one-off spike)
- **Effort**: L (decision session) — larger than any single folded plan;
  M–L per surface once decisions land (build effort tracked per-surface
  once this session produces a locked decision record, same pattern as
  plan 063's `/grilling` session today)
- **Risk**: LOW-MED — visual-only, but touches every card a user sees
  constantly (idle rail, live-match card, news card, cmux relay)
- **Depends on**: 042 (DONE — current collapsed scorecard baseline),
  063 (ships separately and first — this plan's "before" baseline is
  063's shipped result, not a redo of its width/wrap mechanics)
- **Soft-coordinates with**: 059 (notification history — a privacy/
  retention decision, not a visual one; if it's ever built, its browse
  view would want this plan's visual language)
- **Category**: direction

## Why this matters

Filed 2026-07-20 from a `/grilling` session on plan 063 (idle-rail
notch-overlap bug) that organically expanded into a full redesign
conversation once the operator shared a reference image (bold
typography, team badges, a match/weather panel, an action-button row).
Rather than let that conversation's decisions live only in a chat
transcript, or re-litigate the same "how should notchtap's overlay
actually look" question separately across plans 054 (app icon), 055
(pause control), 056 (scorecard visual — the closest existing match to
today's discussion), and 060 (HUD-mode visual merge), this plan
consolidates all of them into one place. One dedicated session should
answer the full topic list below once, covering every surface: the idle
clock/status rail, the live-match scorecard, news cards, weather
(today's chip plus a possible future ambient background), cmux-relayed
cards, and the app icon.

**Explicitly out of scope — already decided elsewhere**: plan 063's
notch-mode width-cap/wrap mechanics were fully grilled to a locked
decision the same day this plan was filed, and ship as their own narrow
bug fix using today's *existing* visual style (no restyle). This plan's
"current state" baseline, once 063 lands, is 063's shipped result — this
plan does not reopen 063's already-settled width/wrap/floor numbers.

## Session-0 record — decisions already locked (2026-07-20, from the 063 `/grilling` session that produced this plan)

These came up during today's conversation and don't need to be re-asked:

- The overlay frontend is **receive-only** — no clickable buttons on the
  card in any redesign (`docs/ARCHITECTURE.md`'s receive-only law,
  reconfirmed today against the reference image's button row). Any
  interactivity (including 055's pause-visibility question) must be
  decided as an explicit, deliberate exception, not a quiet one.
- Real team crest **images** are not currently fetchable by the frontend
  (no frontend network access, `docs/ARCHITECTURE.md`) and would need
  rust-side fetch-and-embed plumbing plus a licensing question about
  real club logos — not resolved today, listed as an open item below.
- `SbTeam` (`src-tauri/src/poller.rs:76-82`) already parses `id` +
  `abbreviation`; `matchup()` (`poller.rs:358-370`) already computes
  structured away/home abbrev+score internally before joining them into
  one string. Sending those fields separately instead of pre-joined is
  the only "new" backend fact-gathering this topic needs — the
  computation already exists.
- `default_weather_poll_secs()` is 900s / 15 min
  (`src-tauri/src/config.rs:225-227`) — current weather condition data
  is already this fresh; only condition-based *background art* is
  missing, not the underlying data.
- The green accent (`#7fe08d`) is reserved for "something is live right
  now" (`.src-chip.live`, the pulsing live-dot) — any typography refresh
  should keep that meaning exclusive, not spend it on the plain clock.
- **Inherited from plan 043 (richer live-match event coverage)**: Step 0
  is CONFIRMED — independent overnight research (Hermes + Kimi,
  cross-checked, raw evidence in
  `research/043-worldcup-final-verification/`) polled ESPN's real World
  Cup Final (event `760517`) across the whole match and confirmed its
  `summary` endpoint's `commentary`/`keyEvents` fields both exist and
  grow monotonically (9→115→144 and 1→29→41 entries across
  9'/90'+8'/108'). A real gap found beyond the original ask: `summary`
  returned empty/404 on 2 of 6 polls even while the match was live, so
  the fetch needs a fallback chain (core API
  `/competitions/{id}/plays`, confirmed working when `summary` wasn't)
  rather than a single endpoint with retries. This backend fetch work is
  **ready to execute independently** of any display decision below — see
  item 6a.

## Decision needed (operator) — full topic list for the dedicated session

1. **Card shape/positioning**: keep today's flush-hanging, rounded-
   bottom-only shape (`position_window`'s deliberate `y: 0.0` anchor,
   `lib.rs`) or switch to floating-with-a-gap, fully-rounded (the
   reference image) — the bigger of the two, since it touches every
   card state's shared `.rail-card` base class and the one window-
   positioning line the code comments call out as deliberate.
2. Idle clock/day-row typography scale (bold time, day-of-week
   treatment).
3. **Team identification** across every surface that shows a team: text
   abbreviation badges (today's session direction), flags (plan 056's
   original proposal), real crest images (needs new rust plumbing +
   licensing call), or keep today's plain text. One decision, applied
   everywhere teams appear.
4. Live-match wire shape: confirm structured fields (league, home/away
   abbrev, home/away score, minute) over one joined string, once 043's
   real data shape is available to design against.
5. Score prominence — a bigger, visually distinct score element vs.
   today's inline title text.
6. Event icon set for scoring plays / cards / fouls / offside — the
   *display* half of what plan 043 fed into this file; without richer
   event data, this stays limited to goal/card/penalty/own-goal (today's
   `poller.rs` extraction).
   - **6a. The backend half is not a design decision** — the fetch/parse
     work described in the Session-0 record above (fallback chain
     against ESPN's `summary`/`plays` endpoints) is confirmed and can be
     executed independently, any time, ahead of or in parallel with this
     session's display decisions. Whoever picks this up should treat 6a
     as its own execution step, not wait on items 1-14.
7. **News card** presentation — any restyle beyond today's
   masthead/category-pill treatment (flagged in-scope per the operator's
   "everything UI-related" direction).
8. **Cmux-relayed** notification presentation — any surface-specific
   visual treatment.
9. **Weather**: today's chip styling, plus condition-based *background*
   art (starfield for clear night, rain, snow, hot/cold, etc.) —
   deliberately deferred out of plan 063 today; this is the open-ended
   one (condition bucket list, day/night variants, static vs. animated,
   asset sourcing) and may itself need splitting into its own follow-up
   spike rather than a single-sitting answer.
10. Bottom-row / status-pill final shape — today's direction (inert
    pills carrying real existing data: News/Weather/Queued, restyled
    bigger/bolder) applied consistently across every card state, not
    just idle.
11. **In-card pause discoverability** (plan 055) — pause already exists
    (hotkey + tray + boot checkbox); does it get a visible *static*
    indicator (not a button, per the receive-only law above), and if so
    what does it look like.
12. **App icon/branding** (plan 054) — `src-tauri/icons/` is still the
    Tauri scaffold default; needs a visual-identity direction before any
    artwork is generated. Different surface (the app icon, not the
    in-app card) but folded in for one-stop visual-identity discussion.
13. **Asset sourcing / licensing**, resolved once for every surface that
    wants non-text art: real club crests/flags, weather-condition art,
    app icon artwork — where do assets come from, and is using real
    team branding acceptable scope for this app at all (worth a direct
    answer, not an assumption).
14. Staleness/age indicators — confirm whether today's shipped
    `ageLabel`/pill treatment (plan 032) extends into the new visual
    language as-is or needs its own restyle pass.

## Recommendation

Run one dedicated `/grilling` session against this file covering all 14
topics above, in the same style as today's plan 063 session (one
decision at a time, recommendation-first, facts looked up rather than
asked). Produce a single locked decision record before any execution
plan gets written — do not start building any one surface (e.g. just the
scorecard) ahead of the others, since several topics (team ID, asset
sourcing, bottom-row shape) are shared decisions that apply across
multiple cards and would need re-litigating per-surface otherwise.

## Maintenance notes

- Plans 043, 054, 055, 056, 060 are marked `SUPERSEDED by 079` in
  `plans/README.md` — their content is fully represented in this file
  (043's confirmed backend research lives in the Session-0 record and
  item 6a above); do not execute those files separately.
- Plan 059 is **not** superseded — persisting/browsing notification
  history is a privacy/retention decision first; only its eventual
  browse-view styling would draw on this plan.
- Plan 077 (Settings-window log panel) is deliberately **not** part of
  this consolidation — it's a different surface (the capability-gated
  Settings window, not the receive-only overlay card) with its own
  visual context.
- Plan 065 (hardcoded BrightData API token) is unrelated to this plan
  entirely — a live credential exposure needing operator rotation, not a
  design decision. Flagged here only so it doesn't get lost in the
  shuffle of "combine everything."
