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
> Sequence AFTER 037 (DONE, merged `6b53c32`) + 039 (UNBLOCKED as of
> 2026-07-20 — 038 landed, 039 is code-grounded and ready but not yet
> executed; re-check `plans/README.md`'s row for 039 before starting,
> since this plan reads its live-match Topic/`Recurring` shape as
> already-landed reality). `git status` clean for `poller.rs` before
> starting either way.

## Status

- **BLOCKED on a maintainer decision** (see Open decision) + gated on
  039 landing (037 already has). Filed 2026-07-19 from live-session
  feedback watching ENG–FRA: "why is the nudge that small? while a
  match runs it should be bigger, a scorecard — countries, score, time,
  yellow cards — even collapsed."
- **Priority**: P2 · **Effort**: M–L (depends on decision — this pass
  narrows the range per option, see below) · **Risk**: MED (challenges
  the single-slot rotation model, for Option A/C only).
- **Depends on**: 039 (hard — extends the live-match card; UNBLOCKED,
  not yet executed), 037 (DONE).
- **Review-plan pass (2026-07-20)**: did not resolve the Option A/B/C
  decision. Grounded each option against live code instead: (1) found
  `EventMeta.details`/`subtitle` (plan 035's rich-relay wire fields)
  already carry arbitrary label/value cells end-to-end with ZERO
  schema/wire changes needed — this directly de-risks Option B's
  "smaller change" framing with evidence, not just intuition, but ALSO
  found those cells currently render **only when expanded**
  (`Manifest.tsx:38`, `{expanded && (...)}`) — so "readable even when
  collapsed" is real, uncosted frontend work regardless of which option
  is chosen; (2) found the overlay window's size is set once at
  startup, not dynamically resized (`lib.rs:526-560`'s `position_window`
  reads `outer_size()`, never sets it) — Option A's "window grows to a
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
(`MatchSnapshot`, `poller.rs:129-149`) but is NOT true for per-side
card counts (today's `cards: usize` on `MatchSnapshot` is a single
total across both teams and both colors — see "Per-side card counts"
below) — and none of this structured data reaches the wire today
regardless: `Event`'s payload is `{ title: String, body: String }`
only (`event.rs:117-120`); `SlotState::Showing` has no score/clock
fields (`event.rs:169-188`). The gap is real presentation-model work,
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

`position_window` (`lib.rs:526-560`) reads `window.outer_size()` to
compute cutout-anchored X position — it never calls a size-setting API.
Nothing else in `lib.rs` resizes the main overlay window at runtime
either (`rg -n "set_size\|inner_size" src-tauri/src/lib.rs` — the one
`inner_size` hit is the **settings** window's fixed
`480.0, 600.0` at creation, `lib.rs:637`, unrelated). This means
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

`MatchSnapshot.cards: usize` (`poller.rs:142`) is computed as a single
count across BOTH teams and BOTH colors (`poller.rs:214-222`,
`details.iter().filter(|d| ...text.contains("Card")).count()`) — there
is no home/away split today. The raw ESPN feed DOES carry the
attribution needed: every card (and goal) detail has a `"team": {"id":
"..."}` field (verified against the fixture — a Kai Havertz card
detail's `team.id` correctly matches Arsenal's competitor `team.id` in
the same fixture), but **neither `SbDetail` nor `SbTeam` currently
parses any team ID** (`SbTeam` only has `abbreviation: String`,
`poller.rs:74-78`; `SbDetail` has no `team` field at all,
`poller.rs:81-98`). Getting "yellow/red counts per side" requires:
add an `id: String` field to `SbTeam` (populated on both the
competitor-level and, via a new `team: Option<SbTeamRef>` field, the
detail-level), then cross-reference each card detail's team id against
the home/away competitor ids to bucket the count. This is small, but
it's new parsing work common to every option — not something any
option gets "for free" from data that already flows through.

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
  frontend layout plus the shared per-side-count parsing.
- **Option C — hybrid**: rotating by default, but a hotkey/priority
  "expand to match mode" pins it on demand. **Grounded cost (this
  pass)**: inherits Option A's window-resize work (still needed for the
  pinned "match mode" state) plus a new hotkey/priority trigger — the
  most total scope of the three, not a middle ground on cost even
  though it reads as one.

(Recommend deciding via a short real-hardware look once 039 is running,
per the original filing — this pass's grounding is meant to inform that
look, not replace it.)

## Scope (indicative — finalize after the decision + 039)

**Common to every option** (grounded this pass, not option-dependent):
- `src-tauri/src/poller.rs` — `SbTeam`/`SbDetail` gain team-id parsing
  (see "Per-side card counts" above); `MatchSnapshot`/`MatchView` gain
  per-side yellow/red counts instead of (or alongside) the aggregate
  `cards: usize`; clock is already tracked (`display_clock`,
  `poller.rs:141`) and just needs threading into `meta.details`/
  `subtitle` on the live-match event (039's territory).
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
  math re-run after resize (`lib.rs:526-560`) — genuinely new
  capability, see "Grounded findings" above.
- Slot-pinning logic — how a pinned match-mode card interacts with
  `queue.rs`/`Engine`'s rotation (`apply`/`accept`) and where
  news/cmux go while pinned. This is a real Engine-adjacent design
  question, not just UI — read `engine.rs`'s current `apply`/`accept`
  shape before proposing a pin mechanism, since "the Engine is the one
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
- 039 hasn't landed → STOP (this plan reads the live-match `Recurring`
  event shape as already-existing; building against it before 039 lands
  means building against code that doesn't exist yet).

## Notes

- Independent of event *coverage* (plan 043) — this is about how the
  score/clock/cards are *shown*, not about detecting more event types.
- The per-side card-count parsing work (see "Grounded findings" above)
  is small enough that it could reasonably land as its own tiny
  preparatory plan ahead of whichever option is chosen, the same way
  038 landed ahead of 039 — flag this to the maintainer as an option,
  not a requirement; this plan doesn't mandate splitting it out.
