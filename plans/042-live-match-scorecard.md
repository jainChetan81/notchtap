# Plan 042: live-match scorecard presentation (richer collapsed card, Option B)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving
> on. If anything in "STOP conditions" occurs, stop and report — do not
> improvise. When done, update this plan's status row in
> `plans/README.md`.
>
> **Coordination**: builds on **039** (the consolidated live-match card,
> DONE, merged) and touches `poller.rs` (037's territory, DONE) + the
> overlay frontend. `git status` clean for `poller.rs`/`src/components/`
> before starting.
>
> **Drift check (run first)**: `git diff --stat 088531d..HEAD --
> src-tauri/src/poller.rs src-tauri/src/event.rs src-tauri/src/lib.rs
> src-tauri/src/engine.rs src/components/Manifest.tsx
> src/components/StatusRailCard.tsx`. `088531d` is this pass's baseline
> (2026-07-20) — every citation was re-verified against live code at
> that commit. This plan's citations have already drifted once before
> (5-19 lines, when 039/041 landed between two prior review-plan
> passes) — re-verify with `rg`, don't trust line numbers blindly.

## Status

- **UNBLOCKED — ready to execute.** Filed 2026-07-19 from live-session
  feedback watching ENG–FRA: "why is the nudge that small? while a
  match runs it should be bigger, a scorecard — countries, score, time,
  yellow cards — even collapsed."
- **Priority**: P2 · **Effort**: S–M (narrowed from M–L once Option B
  was picked — no window resize, no Engine pin mechanism, no hotkey
  trigger) · **Risk**: LOW (Option B never touches the rotation model;
  the MED risk the original filing carried was specific to Option A/C,
  which are no longer in scope).
- **Depends on**: 039 (DONE), 037 (DONE). Nothing left to wait on.
- **Decision (`/grill-me`, 2026-07-20)**: **Option B — never pin.** The
  scorecard stays a normal rotating card; the existing priority system
  (football defaults to `High`) already keeps it well-represented in
  rotation without a bespoke pin mechanism duplicating that job. Three
  follow-on shape decisions resolved in the same session — see
  "Decision" below for the full record with reasoning.
- **Review-plan pass 1 (2026-07-20)**: did not resolve the Option A/B/C
  decision. Grounded each option against live code instead: (1) found
  `EventMeta.details`/`subtitle` (plan 035's rich-relay wire fields)
  already carry arbitrary label/value cells end-to-end with ZERO
  schema/wire changes needed — this directly de-risks Option B's
  "smaller change" framing with evidence, not just intuition, but ALSO
  found those cells currently render **only when expanded**
  (`Manifest.tsx:38`, `{expanded && (...)}`) — so "readable even when
  collapsed" is real, uncosted frontend work regardless of which option
  is chosen; (2) found the overlay window's size is set once at
  startup, not dynamically resized (`position_window` reads
  `outer_size()`, never sets it) — Option A's "window grows to a
  scoreboard footprint" needs genuinely new resize+reposition
  capability, not a tweak to existing logic, which the original filing's
  "the heavy part" undersold; (3) found the "yellow/red counts per
  side" requirement needs data that doesn't exist in the parsed structs
  yet on ANY option — the raw ESPN feed carries a `team.id` on every
  card detail (verified against the fixture: Havertz's card correctly
  correlates to Arsenal's competitor `team.id`), but neither `SbDetail`
  nor `SbTeam` currently parses team IDs, so per-side attribution is a
  real (if small) new parsing task common to every option, not
  automatically implied by "surface" existing data.
- **Review-plan pass 2 (2026-07-20)**, run after 039 actually landed
  and a fresh-context subagent cold-read this file: found the
  "039 not yet executed" framing above was stale in three places (this
  header, this Status block, a STOP condition) — 039 landed the same
  day this plan's first pass was written, and every `poller.rs` line
  citation had drifted by 5-19 lines as a direct result (039 added
  `CardTopic`/`card_topic`/the `card` param, 041 added the `own_goal`
  field, both landing between this file's two passes) — see
  "Re-verified after 039 landed" below for the corrected citations and
  one real technical gap the first pass missed entirely:
  `make_event`'s signature is already AT clippy's 7-arg ceiling (by its
  own doc comment, deliberately engineered around by 039), so Scope's
  "clock just needs threading into `meta.details`" undersold what that
  threading actually requires.
- **`/grill-me` (2026-07-20)**: resolved the Option A/B/C decision
  (Option B — never pin) plus three follow-on shape questions (flags,
  per-side card split, redesign scope) — see "Decision" below. Rewrote
  Scope around Option B only (Option A/C material moved to a "Rejected
  alternatives" note for the record, not deleted) and added concrete
  numbered Steps, which this plan never had before.

## Problem

The overlay is a rotating single-slot **notification strip**: every card
(goal, news, cmux) shares one small slot and auto-dismisses. The live
goal card is just another notification passing through — so while a match
is on, there's no persistent, glanceable scoreboard. 039 makes the match
*one* card that updates in place, but it stays the same small strip and
still rotates out. The operator wants, while a match is live, a **bigger
scorecard** — flags/countries, large score, clock, yellow/red counts —
readable **even when collapsed**, not a transient nudge.

**Corrected this review-plan pass**: "the raw material already exists in
the poller" is true for `home_score`/`away_score`/`display_clock`
(`MatchSnapshot`, `poller.rs:134-154` at last check) but is NOT true
for per-side card counts (today's `cards: usize` on `MatchSnapshot` is
a single total across both teams and both colors — see "Per-side card
counts" below) — and none of this structured data reaches the wire
today regardless: `Event`'s payload is `{ title: String, body: String }`
only (`event.rs:117-120`); `SlotState::Showing` has no score/clock
fields (`event.rs:169-194`). The gap is real presentation-model work,
but it's not purely a frontend gap either — some of it is a wire-payload
gap the original filing's framing ("the gap is the presentation model,
not the data") slightly understates.

## Grounded findings (this review-plan pass — inform the decision, don't make it)

### The `details`/`subtitle` wire mechanism already exists (de-risks Option B)

Plan 035 already wired `EventMeta.subtitle: Option<String>` and
`EventMeta.details: Vec<DetailItem>` (`{label, value}` cells,
server-capped) end-to-end: any producer (not just `/notify`) can
populate them on an `Event`, and `StatusRailCard.tsx` →
`Manifest.tsx` already renders `subtitle` as its own cell and one cell
per `details` pair. **This means a scorecard's score/clock/card-count
readout could ride this exact mechanism with zero new wire fields** —
the espn poller would populate `meta.details` with cells like
`{label: "Score", value: "PSG 1–1 ARS"}`, `{label: "Clock", value:
"87'"}`, `{label: "Cards", value: "2Y 0R"}` on the live-match
`Recurring` event (039's territory) instead of inventing new
`SlotState` fields.

**But this only covers the EXPANDED state today.** `Manifest.tsx:38`
gates the whole cell block on `{expanded && (...)}` — collapsed cards
show only `title`/`body`. The operator's ask is specifically "readable
even when collapsed," so **new frontend work is required regardless of
which option is picked**: either a new collapsed-state layout that also
reads `details`/`subtitle` (Option B's actual scope), or a distinct
"match mode" component (Option A's scope). Don't read "the wire
mechanism exists" as "collapsed rendering exists" — it doesn't.

### The overlay window does not resize dynamically today (Option A's real cost)

`position_window` (`lib.rs:528-562` at last check — locate with
`rg -n "fn position_window"`) reads `window.outer_size()` to
compute cutout-anchored X position — it never calls a size-setting API.
Nothing else in `lib.rs` resizes the main overlay window at runtime
either (`rg -n "set_size\|inner_size" src-tauri/src/lib.rs` — the one
`inner_size` hit is the **settings** window's fixed
`480.0, 600.0` at creation, `lib.rs:639` at last check, unrelated). This means
Option A's "the window grows to a scoreboard footprint" requires:
1. A runtime window-resize call (new capability, not present anywhere
   in the overlay's code today).
2. Re-running `position_window`'s cutout-anchoring math after every
   resize, since it depends on `win_width` to compute X — a wider
   window needs to re-center or re-anchor, not just grow from a fixed
   top-left.
3. Deciding what happens to `news`/`cmux` cards while match-mode is
   pinned (the plan's own "queue behind it (or show in a secondary
   strip)" is still an open sub-question, not just an implementation
   detail).

This is consistent with the original filing's "biggest change to the
rotation model" framing for Option A, but "the heavy part" undersold
just how much of this is new capability vs. adapting existing code —
there is no existing dynamic-resize path to adapt.

### Per-side card counts need new parsing, on every option (not just "surfacing")

`MatchSnapshot.cards: usize` (`poller.rs:147` at last check — see
"Re-verified after 039 landed" below for why line numbers throughout
this section are indicative, re-locate everything with `rg`) is
computed as a single count across BOTH teams and BOTH colors
(`poller.rs:232-241` at last check,
`details.iter().filter(|d| ...text.contains("Card")).count()`) — there
is no home/away split today. The raw ESPN feed DOES carry the
attribution needed: every card (and goal) detail has a `"team": {"id":
"..."}` field (verified against the fixture — a Kai Havertz card
detail's `team.id` correctly matches Arsenal's competitor `team.id` in
the same fixture), but **neither `SbDetail` nor `SbTeam` currently
parses any team ID** (`SbTeam` only has `abbreviation: String`,
`poller.rs:75-78` at last check; `SbDetail` has no `team` field at all,
`poller.rs:81-103` at last check — it does now have `own_goal`/
`red_card` structural booleans from plans 041/pre-existing, following
that exact pattern for `team.id` is the right shape to copy). Getting
"yellow/red counts per side" requires:
add an `id: String` field to `SbTeam` (populated on both the
competitor-level and, via a new `team: Option<SbTeamRef>` field, the
detail-level), then cross-reference each card detail's team id against
the home/away competitor ids to bucket the count. This is small, but
it's new parsing work common to every option — not something any
option gets "for free" from data that already flows through.

## Re-verified after 039 landed (review-plan pass 2, 2026-07-20)

039 landed the same day this plan's first pass was written, and 041
landed shortly after — both touched `poller.rs` directly, drifting
every citation above by 5-19 lines. Two things worth knowing beyond
the corrected line numbers (already applied above):

**`make_event` is already at clippy's 7-arg ceiling — the "clock just
needs threading into `meta.details`" line in Scope undersold this.**
Read the function directly (`poller.rs:351-359` at last check,
`rg -n "fn make_event"`): `event_type, title, body, ttl_secs, signal,
priority, card` — exactly 7 params. Its own doc comment on the
sibling `CardTopic` enum (`poller.rs:322-324`) says outright: "bundled
into a single parameter so `make_event` stays under clippy's 7-arg
`too_many_arguments` threshold." Whichever option is chosen, threading
`meta.details`/`subtitle` through this function is NOT a one-line param
add — it needs the same bundling treatment `CardTopic` already got
(fold the detail cells into a struct, or build `meta` at the
`diff_scoreboard` call sites and attach it to the `Event` after
`make_event` returns rather than passing it through the function at
all). This changes Option B's "no new wire fields... smaller change"
framing only slightly (still true, no *wire* change) but the *internal*
signature work is a real, disclosed wrinkle now, not an undocumented
surprise for whoever builds it.

**`Engine::apply`/`Engine::accept` do both exist** (`engine.rs:86,149`
— re-confirmed directly, contradicting an earlier draft of this pass
that mis-grepped and claimed otherwise). One more fact worth carrying
into the Option A/C design question: `apply` (`engine.rs:86`) currently
carries `#[allow(dead_code)]` with its own comment — "Today every async
mutation is an ingest (`accept`); `apply` is the door the next async
mutation site must walk through" — meaning it is a pre-built, currently
-unused seam for exactly the kind of new async mutation a pin mechanism
would need, not something to invent from scratch. `update_live_match`
(`engine.rs:186`) is the narrower precedent for a NON-queue-mutating
poller write (its own doc comment explicitly contrasts it with
`apply`/`accept`: "not `apply`/`accept`: it never touches the queue").
Whichever of the three fits a pin mechanism better is a real design
call for whoever builds Option A/C — but the seam already exists either
way; it isn't a gap.

## Decision (`/grill-me`, 2026-07-20 — RESOLVED)

Four decisions, in dependency order:

1. **Option B — never pin.** The scorecard behaves like every other
   card: it shares the single slot and rotates normally. No window
   resize, no Engine-level pin mechanism, no hotkey trigger. Reasoning:
   rereading the original complaint — "it should be bigger... readable
   even when collapsed" — that's about the card's own layout richness,
   not about whether it stays glued to the slot. The existing priority
   system already gives football events precedence in rotation
   (`espn_priority` defaults to `High`, `config.rs`), so a bespoke pin
   mechanism would mostly duplicate a job the app already does. Cost
   and risk both drop sharply versus Option A/C: no dynamic window
   resize (a genuinely new capability, see "Grounded findings" above),
   no Engine-adjacent pin design question, no new hotkey.
2. **No flags/country icons.** Team abbreviations are already parsed
   and already shown in the card title (`matchup()`,
   `poller.rs:308-320` at last check — e.g. `"UCL: ARS 1–1 PSG"`);
   render them bigger/more prominent in the new layout instead of
   adding flag glyphs. Reasoning: this codebase has zero emoji/icon
   glyphs anywhere (confirmed during the 041 review) — flags would be
   the first. They'd also only make sense for international matches
   (the triggering ENG–FRA example); most of what's actually polled is
   club football (UCL, EPL, La Liga, MLS), which has no national flag
   to show at all, only club crests the ESPN feed doesn't carry.
3. **Cards split per side** (`ARS 1Y · PSG 0Y`), not a bare aggregate
   total. The aggregate (`MatchSnapshot.cards: usize`) already exists
   with zero new parsing; a per-side split needs new `team.id` parsing
   on `SbTeam`/`SbDetail` (see "Per-side card counts" above). Worth the
   small added cost: "yellow cards" was specifically named in the
   original complaint, and a bare total ("2 cards") doesn't tell you
   who's picking them up.
4. **Minimal layout tweak, not a full scoreboard redesign.** Keep
   today's `title`/`body` largely as-is (the title already carries the
   matchup), add compact new lines for clock + per-side cards. A full
   scoreboard-style component (team names either side, big centered
   score) is explicitly deferred — the operator wants to see the
   minimal version live (or review a prototype) before committing to a
   bigger frontend build. **Do not build the full redesign as part of
   this plan** — that's a distinct, later plan once the minimal version
   has been lived with.

## Scope

**Grounded this pass**: title already carries the score (`matchup()`,
`poller.rs:308-320`, e.g. `"UCL: ARS 1–1 PSG"`), so the only genuinely
new information the collapsed view needs is **Clock** and **per-side
Cards** — not a third "Score" cell. `SlotState` has no `topic`/
`rotation` field (`event.rs:169-194`), so the frontend can't directly
tell "this is the live-match card" from a one-shot card — but it
doesn't need to: the presence of a populated `details` array on a
`score_update` card is naturally that signal, since only this plan's
producer path will ever populate it.

**In scope**:
- `src-tauri/src/poller.rs`:
  - Add `id: String` to `SbTeam` (`poller.rs:75-78` at last check) and
    a new `team: Option<SbTeamRef>` field to `SbDetail`
    (`poller.rs:81-103` at last check) — mirrors exactly how
    `own_goal`/`red_card` already read structural ESPN fields instead
    of guessing from text (`poller.rs:94-98`). `SbTeamRef` is a new,
    minimal struct: just `{ id: String }`, same shape as the existing
    `SbTeam`/`SbAthlete` "just the fields we use" pattern.
  - `MatchSnapshot` (`poller.rs:134-154`) gains `home_cards: (u32, u32)`
    /`away_cards: (u32, u32)` (yellow, red) — or two pairs, whichever
    reads cleaner in review — replacing the single aggregate `cards:
    usize` (`poller.rs:147`) at the point of use; cross-reference each
    card detail's `team.id` against the competitor-level `team.id`
    (also newly parsed) to bucket yellow/red per side. The existing
    aggregate-count closure (`poller.rs:232-241` at last check) is
    the thing to replace, not add alongside — don't leave two card-
    counting code paths that can drift apart.
  - `make_event` (`poller.rs:351-359`) is already at clippy's 7-arg
    ceiling (its sibling `CardTopic` enum's own doc comment says so
    explicitly, `poller.rs:322-324`) — do NOT add an 8th param. Build
    `meta: EventMeta { details: vec![...], ..EventMeta::default() }`
    at the `diff_scoreboard` call sites instead and attach it to the
    `Event` `make_event` returns (`event.meta = meta;` after the call),
    the same "build outside, attach after" shape already flagged in
    "Re-verified after 039 landed" above.
  - Populate `meta.details` with exactly two cells, only on the
    `CardTopic::Live` and `CardTopic::FullTime` branches (never
    `CardTopic::Off` — flag-disabled behavior must stay byte-identical,
    per the existing regression-pin tests): `{label: "Clock", value:
    display_clock}` always; `{label: "Cards", value: "<away_abbrev>
    <awayY>Y<awayR>R · <home_abbrev> <homeY>Y<homeR>R"}` — but OMIT the
    Cards cell entirely when both sides have zero cards, to avoid
    showing "0Y 0R · 0Y 0R" clutter on a clean match. No `subtitle` —
    unused by this feature, leave it `None`.
- Frontend, `src/components/StatusRailCard.tsx`: the collapsed
  (non-expanded) `.compact` branch (`:138-143` at last check) currently
  renders only `title`/`body` for non-news cards — `slot.details`/
  `slot.subtitle` are passed to `<Manifest>` (`:149-158`) but NEVER
  read in the compact view itself. Add a new conditional block, inside
  the same non-news branch, rendering one line per `details` entry
  (label: value) directly below `body` — gated on `slot.details.length
  > 0` (naturally true only for a live-match card with `espn_live_card`
  on, per the poller-side gating above; false for every other card
  today, so this is additive, not a behavior change for existing
  cards). This is the "minimal tweak" — reuse `Manifest.tsx`'s own
  `detail-label`/`detail-value` CSS classes for visual consistency with
  the expanded view rather than inventing new styling.
- Tests: poller per-side-card-count tests (fixture-driven, mirroring
  the existing card-count tests, using the UCL fixture's real cards);
  poller tests confirming `meta.details` is populated on Live/FullTime
  and absent on Off (the flag-off regression pin — must stay
  byte-identical to pre-existing behavior); frontend render test for
  the new compact-view details line, following `StatusRailCard.test.tsx`'s
  existing pattern.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust suite | `cd src-tauri && cargo test --locked` | all pass; recount against §0 |
| Gates | `cargo clippy --locked --all-targets -- -D warnings && cargo fmt --check` | exit 0 |
| Frontend tests | `npx vitest run` | all pass |
| Frontend gates | `npx biome ci . && npx tsc --noEmit && npx vite build` | exit 0 |

## Steps

### Step 1: per-side card parsing

Add `SbTeamRef { id: String }`, the new `team: Option<SbTeamRef>` field
on `SbDetail`, and `id: String` on `SbTeam` (Scope above for exact
locations). Thread `team.id` through `view()`'s card-counting closure
to replace the aggregate `cards: usize` with per-side yellow/red
counts on `MatchSnapshot`. Add a fixture-driven test asserting the UCL
fixture's real cards attribute to the correct side (cross-check: which
team scored/committed each card in the raw fixture JSON, assert the
count lands on that side, not the other).

**Verify**: `cargo test --locked poller::` → all pass, including the
new per-side test.

### Step 2: `meta.details` on the live-match event

Build `meta` at the `diff_scoreboard` call sites and attach it after
`make_event` returns (Scope above — do not add an 8th param to
`make_event`). Populate the two cells (`Clock`, `Cards`) on
`CardTopic::Live`/`CardTopic::FullTime` only; leave `meta` at
`EventMeta::default()` on `CardTopic::Off`, unchanged from today. Add
tests: (a) flag-off regression pin — `meta.details` stays empty,
byte-identical to pre-039 behavior; (b) flag-on, mid-match — `Clock`
and `Cards` cells present with correct per-side values; (c) flag-on,
zero cards so far — `Cards` cell absent, `Clock` still present.

**Verify**: `cargo test --locked poller::` → all pass, including the
three new cases; `cargo test --locked` (full) → all pass, totals match
`docs/TESTING_STRATEGY.md` §0.

### Step 3: frontend — read `details` in the collapsed view

In `StatusRailCard.tsx`'s non-news compact branch, add the new
conditional block per Scope above — one line per `details` entry,
reusing `Manifest.tsx`'s `detail-label`/`detail-value` classes, gated
on `slot.details.length > 0`. Add a render test in
`StatusRailCard.test.tsx` covering: a card with `details` populated
shows the new lines in the COLLAPSED state (not just expanded); a card
with empty `details` (every other card type today) renders unchanged
from current behavior.

**Verify**: `npx vitest run && npx tsc --noEmit && npx biome ci .` →
all pass/exit 0.

### Step 4: docs + status

Update `docs/TESTING_STRATEGY.md` §0 for the new rust + frontend test
counts. Flip this plan's `plans/README.md` row to DONE.

**Verify**: `cargo test --locked 2>&1 | grep "test result"` and `npx
vitest run` totals match §0.

## Done criteria

- [ ] `cd src-tauri && cargo test --locked` exits 0; totals match §0
- [ ] `cargo clippy --locked --all-targets -- -D warnings && cargo fmt --check` exit 0
- [ ] `npx vitest run && npx tsc --noEmit && npx biome ci .` all pass/exit 0
- [ ] Flag-off regression: `meta.details` empty when `espn_live_card=false` — byte-identical to pre-039 behavior
- [ ] A live match's collapsed card shows Clock + per-side Cards (when any exist) without needing to expand
- [ ] `git diff -- src-tauri/capabilities/default.json src-tauri/capabilities/settings.json` byte-identical (Option B touches neither — confirms no scope creep toward A/C)
- [ ] `docs/TESTING_STRATEGY.md` §0 updated
- [ ] `plans/README.md` status row updated

## STOP conditions

- Any change would touch `capabilities/default.json`, add a
  `#[tauri::command]`, resize the overlay window, or add a hotkey →
  STOP and report. None of these belong to Option B — if a step seems
  to need one, that's a sign of scope creep toward Option A/C, not
  something to build around.
- The `poller.rs`/`event.rs`/frontend citations in this plan don't
  match live code when you check them → STOP and reconcile before
  building, don't guess. This plan's citations have already drifted
  once before (5-19 lines, when 039/041 landed between two earlier
  review-plan passes) — re-verify with `rg`.
- The per-side card count doesn't match the raw fixture JSON on manual
  inspection → STOP; a `team.id` cross-reference bug here would silently
  misattribute cards to the wrong team, which is worse than the
  aggregate count this replaces.

## Rejected alternatives (Options A and C — kept for the record, not deleted)

The full grounded-cost analysis for Options A ("match mode," pin +
enlarge the window) and C (hybrid, hotkey-pinned) from the review-plan
pass is preserved above in "Grounded findings" and "Re-verified after
039 landed" — that reasoning isn't reproduced here. Short version of
why they were rejected in favor of Option B: both require genuinely
new window-resize/re-anchor capability that doesn't exist anywhere in
the codebase today, plus (Option A) an Engine-adjacent pin-mechanism
design question about where news/cmux go while pinned, plus (Option C)
a new hotkey trigger on top of all of Option A's cost. Option B answers
the operator's actual complaint (bigger, readable-when-collapsed
scorecard) at a fraction of the engineering risk, by reusing the
existing priority system instead of building a parallel pin mechanism.
If, after living with Option B, key moments are still being missed
because the card rotates away at the wrong second, that's a concrete,
evidence-backed case for revisiting Option C later — a better position
to make that call from than committing to it speculatively now.

## Notes

- Independent of event *coverage* (plan 043) — this is about how the
  score/clock/cards are *shown*, not about detecting more event types.
- **Deferred, explicitly not this plan** (per the `/grill-me` decision
  above): a full scoreboard-style visual redesign (team names either
  side, big centered score) and flag/country icons. Both are real
  follow-up candidates once the operator has watched the minimal
  version live or reviewed a prototype — this plan intentionally ships
  the smaller, lower-risk version first.
