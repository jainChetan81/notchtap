# Plan 042: live-match scorecard presentation (bigger, persistent while playing)

> **Executor instructions**: This plan carries an **unresolved maintainer
> decision** (slot behavior, below) — do NOT execute until it's chosen.
> This review-plan pass did NOT make that decision (it's explicitly the
> maintainer's call) — it grounded each option's real cost against live
> code so the decision can be made with evidence instead of estimates.
> When it is chosen, run every gate and update the `plans/README.md` row.
>
> **Coordination**: builds on **039** (the consolidated live-match card)
> and touches `poller.rs` (037's territory) + the overlay frontend.
> Sequence AFTER 037 (DONE, merged `6b53c32`) + **039 (DONE 2026-07-19,
> confirmed via `plans/README.md`'s row and directly against landed
> `poller.rs`)** — its live-match Topic/`Recurring` shape is now real,
> landed code, not a forward reference; this review-plan pass
> (2026-07-20) re-verified every citation below against it and found
> real drift (see "Re-verified after 039 landed" below) beyond what a
> prior pass caught. `git status` clean for `poller.rs` before starting.

## Status

- **BLOCKED on a maintainer decision only** (see Open decision) — 039 is
  no longer a blocker, it landed 2026-07-19. Filed 2026-07-19 from
  live-session feedback watching ENG–FRA: "why is the nudge that small?
  while a match runs it should be bigger, a scorecard — countries,
  score, time, yellow cards — even collapsed."
- **Priority**: P2 · **Effort**: M–L (depends on decision — this pass
  narrows the range per option, see below) · **Risk**: MED (challenges
  the single-slot rotation model, for Option A/C only).
- **Depends on**: 039 (hard — extends the live-match card; **DONE**),
  037 (DONE). Nothing left to wait on except the maintainer's pick.
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

## Open decision (maintainer — REQUIRED before build)

How does the live scorecard behave in the single slot?
- **Option A — "match mode" (pin + enlarge)**: while a match is live, the
  scorecard pins the slot and the window grows to a scoreboard footprint;
  news/cmux queue behind it (or show in a secondary strip). Most
  scoreboard-like; biggest change to the rotation model. **Grounded cost
  (this pass)**: genuinely new window-resize + re-anchor capability (see
  above) on top of the shared collapsed-rendering and per-side-count
  work every option needs.
- **Option B — richer rotating card**: stays a normal rotating card
  (shares the slot, still rotates), but its **collapsed** layout is
  redesigned into a fuller scorecard (flags, big score, clock, card
  counts). Smaller change; not always-on. **Grounded cost (this pass)**:
  no window/rotation-model changes, no new wire fields (reuses
  `details`/`subtitle`) — the bulk of the work is a new collapsed-state
  frontend layout plus the shared per-side-count parsing, plus the
  `make_event` bundling wrinkle from "Re-verified after 039 landed"
  above (small, internal, disclosed — not a wire change, but not truly
  zero-touch on `poller.rs` either).
- **Option C — hybrid**: rotating by default, but a hotkey/priority
  "expand to match mode" pins it on demand. **Grounded cost (this
  pass)**: inherits Option A's window-resize work (still needed for the
  pinned "match mode" state) plus a new hotkey/priority trigger — the
  most total scope of the three, not a middle ground on cost even
  though it reads as one.

(Recommend deciding via a short real-hardware look, per the original
filing — this pass's grounding is meant to inform that look, not
replace it. 039 is landed now, so that look can happen whenever the
maintainer is ready, not gated on 039 shipping first.)

## Scope (indicative — finalize after the decision; 039 already landed)

**Common to every option** (grounded this pass, not option-dependent):
- `src-tauri/src/poller.rs` — `SbTeam`/`SbDetail` gain team-id parsing
  (see "Per-side card counts" above); `MatchSnapshot`/`MatchView` gain
  per-side yellow/red counts instead of (or alongside) the aggregate
  `cards: usize`; clock is already tracked (`display_clock`,
  `poller.rs:146` at last check) and needs threading into
  `meta.details`/`subtitle` on the live-match event — NOT a one-line
  addition, see "Re-verified after 039 landed" above for why
  `make_event`'s existing 7-arg ceiling makes this a small bundling
  task, not a trivial param add.
- Country/flag mapping from league + team — no existing code for this;
  new, small, presentation-only lookup.
- Populate the live-match `Recurring` event's `meta.details`/`subtitle`
  with the score/clock/card cells (reusing plan 035's wire mechanism,
  see "Grounded findings" above) — no new `Event`/`SlotState` field.
- Overlay frontend — a new collapsed-state layout that reads
  `subtitle`/`details` (today gated to expanded-only,
  `Manifest.tsx:38`) for the live-match card specifically. Flags/large
  score/clock/card-count rendering.
- Tests: poller per-side-count tests (fixture-driven, mirroring
  `poller.rs`'s existing card-count tests); frontend collapsed-layout
  render tests.

**Option A/C only**:
- Overlay window dynamic resize + `position_window`'s cutout-anchor
  math re-run after resize (`lib.rs:528-562` at last check — locate
  with `rg -n "fn position_window"`) — genuinely new capability, see
  "Grounded findings" above.
- Slot-pinning logic — how a pinned match-mode card interacts with the
  `Engine`'s rotation and where news/cmux go while pinned. This is a
  real Engine-adjacent design question, not just UI — read `engine.rs`'s
  current `apply`/`apply_blocking`/`accept`/`update_live_match` shapes
  before proposing a pin mechanism (all four exist and are the real
  candidates — see "Re-verified after 039 landed" above, which found
  `apply` specifically is a currently-unused, pre-built seam for exactly
  this kind of new async mutation site), since "the Engine is the one
  module through which every Slot mutation flows" (its own doc comment,
  `engine.rs:1-8`) means a pin that bypasses normal rotation needs to
  either go through the Engine too or explain why it's a deliberate
  exception.
- Option C additionally: a hotkey/priority trigger — follow the
  existing hotkey pattern (`lib.rs`'s `EXPAND_TOGGLE_SHORTCUT` and
  friends, `toggle_manual_expand`) as the exemplar shape, not a new
  mechanism.

## STOP conditions

- Decision not made → STOP. Do not default to any option or start
  building a "safe subset" without the maintainer's pick — the three
  options genuinely diverge in `poller.rs`/frontend/Engine surface, per
  the grounded costs above, not just in visual polish.
- Option A/C would touch `capabilities/default.json` or the
  receive-only guarantee → STOP.
- The `poller.rs`/`lib.rs`/`engine.rs` citations in this plan don't
  match live code when you check them → STOP and reconcile before
  building, don't guess. 039 is landed (confirmed, not a live
  blocker), but this plan's citations have already drifted once since
  039 shipped (poller.rs lines by 5-19 lines, see "Re-verified after
  039 landed" above) — more time passing means more drift is likely,
  not a reason to skip the check.

## Notes

- Independent of event *coverage* (plan 043) — this is about how the
  score/clock/cards are *shown*, not about detecting more event types.
- The per-side card-count parsing work (see "Grounded findings" above)
  is small enough that it could reasonably land as its own tiny
  preparatory plan ahead of whichever option is chosen, the same way
  038 landed ahead of 039 — flag this to the maintainer as an option,
  not a requirement; this plan doesn't mandate splitting it out.
