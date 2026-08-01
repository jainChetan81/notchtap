# 170 — Render Football's promoted match events through the shared notification template

- **Status**: APPROVED — operator authorized execution 2026-08-01 (score-row question below resolved: kept as an additive block)
- **Commit**: —
- **Severity**: N/A (design direction, not a bug)
- **Category**: UX / architecture — reconsiders a plan-042/084/151-era decision
- **Estimated scope**: large — `NotificationBody.tsx` (new `FootballHeroCard`), `live-scorecard.css` trim, `LiveMatchScorecard.test.tsx`/`StatusRailCard.test.tsx` rewrites, `StatusRailCard.tsx`'s call site. Frontend-only — no rust/`src-tauri` changes (see the Target correction below on why the wire doesn't support the prototype's fact-pill idea)
- **Depends on**: plan 168 (DONE) and plan 169 (DONE) — both landed on `agent-card-ui-unification` first, so the sibling-hero-component pattern and `.compact` overflow discipline are already proven on the simpler component
- **Executable spec**: `prototype/football-card.html`'s "⚠ proposal — the same events, through the unified shell" section for the overall shape (masthead/stamp/stripe + additive score-row) — **but not its per-event fact-pill data**, which this plan's Target section below corrects against the real wire contract

## Problem

Football's promoted (showing/expanded) card is `LiveMatchScorecard.tsx`,
its own bespoke layout entirely separate from `NotificationBody.tsx`:

```tsx
// src/components/LiveMatchScorecard.tsx:108-152 (current)
<div className="notif-block">
  <div className="sc-head">
    <span className="chip chip-league">{liveEspn.league}</span>
    <span className={`chip chip-live${pillVariant === "live" ? "" : ` ${pillVariant}`}`}>
      <span className="live-dot" />
      {pillLabel}
    </span>
    <span className="chip clock-pill">{liveEspn.clock}</span>
  </div>
  <div className="score-row">
    <div className="side"><Crest abbrev={liveEspn.homeAbbrev} path={liveEspn.homeCrest} /></div>
    <span className="score">
      <ScoreDigit value={liveEspn.homeScore} /><span className="dash">–</span><ScoreDigit value={liveEspn.awayScore} />
    </span>
    <div className="side"><Crest abbrev={liveEspn.awayAbbrev} path={liveEspn.awayCrest} /></div>
  </div>
  <div className={`event-line${eventPresentation?.tintClass ? ` ${eventPresentation.tintClass}` : ""}`}>
    {eventPresentation && <span className={eventPresentation.iconClass} />}
    {body}
  </div>
  {!cardsClean && <div className="cards-line">...</div>}
</div>
```

No masthead, no stamp, no accent-stripe/manifest/ttl-bar conventions —
this card genuinely does not share the vocabulary the rest of the app
speaks. Deliberately shipped this way (plan 084's football-specific
"structured ESPN meta wants structured layout" reasoning), but it's the
one origin the operator specifically called out as needing the most work
after seeing it next to the unified proposal.

**Score-row question — resolved 2026-08-01.** The prototype's proposal
section originally dropped the score-row (crests + big rolling score
digits) entirely, folding the score into the title string instead ("GOAL!
Harbor City 2–1 Rivertown") — a real information-density loss versus
what's shipped today, and very likely *why* football read as needing more
work than Agent Board did. Operator confirmed: keep the score-row as an
additive flavor block (same pattern as Agent Board's queue-rows in plan
169). `prototype/football-card.html`'s proposal section was updated to
match — titles shortened to the event only ("Goal — J. Marquez", not the
score string), with the real `.sc-head`/`.score-row`/`.cards-line` markup
ported verbatim underneath as an additive block. The Target below reflects
this resolved shape; there is no remaining open question blocking
implementation.

## Target

**Correction (2026-08-01), found while grounding this plan against the
real wire contract before dispatch; RE-CORRECTED (2026-08-01, second
pass) after a dispatched executor caught an error in the first
correction — see below.** The prototype's per-event facts column
(scorer/assist/booking pills) is aspirational, not achievable as
written: there is no structured scorer/assist/booking data anywhere on
the wire, only the one flat `body` string (`EventPayload.body`, e.g.
`"Goal — K. Havertz 78'"`, `candidate.text` verbatim from ESPN).
Parsing that string to fabricate a scorer/assist split would be exactly
the text-sniffing `lib/presentation.ts`'s own `SIGNAL_STAMPS` doc
already rejects ("never derived from parsing title/body text") — not a
shortcut worth taking. The richer fact-pill treatment stays a
documented follow-up (a separate plan extending `EventPayload`/
`poller.rs` to carry structured scorer/assist fields), not part of this
plan — **no fact pills in `FootballHeroCard`, full stop.**

The FIRST correction pass claimed `slot.subtitle`/`slot.details` are
*always* empty for football, citing `make_event`/`make_rich_event`
constructing `meta: EventMeta::default()`. That was wrong for exactly
the path this plan touches. A dispatched executor caught it (correctly
stopped rather than proceeding past a plan/reality mismatch — see
`plans/README.md`'s note on this plan's dispatch history) and it was
verified directly against `poller.rs` before writing this paragraph:
`make_event`/`make_rich_event` do construct with a default `meta`, but
their caller, `diff_match` (`poller.rs:589-624`), OVERWRITES it
afterward — `event.meta = meta.clone()` — whenever `topic.is_some()`
(i.e. whenever `espn_live_card` is on, the SAME condition that makes
`isLiveCard`/`liveEspn` true on the frontend and routes a card through
`FootballHeroCard` at all). That overwritten `meta.details` carries two
possible `DetailItem`s: `{label: "Clock", value: <display_clock>}`
always, and `{label: "Cards", value: "<away> <y>Y<r>R · <home> <y>Y<r>R"}`
when either side has a card. `subtitle` genuinely does stay `None` in
every case (`EventMeta::default()`'s value, never overwritten to
`Some(...)` anywhere in `diff_match`) — that half of the original claim
held.

This does NOT change the plan's deliverable, only the reasoning: both
`details` entries this path can carry (Clock, aggregate Cards tally)
duplicate information the kept additive score-row already shows
verbatim (the `clock-pill` chip; the `cards-line` block) — there is
still no scorer/assist/booking-level data that a fact pill could show
which isn't already on the card. **`FootballHeroCard` still renders NO
fact pills** — simply because rendering `slot.details` through
`renderFactPills` here would be pure duplication of the score-row, not
because the field happens to be empty (it usually is not, on a
live-card-flagged match). Do not wire `liveVisibleDetails`/
`slot.details` into `FootballHeroCard` at all.

Also corrected: `slot.title` for football is `matchup()`'s output
(`poller.rs:369-376`, e.g. `"UCL: ARS 1–1 PSG"`) — already redundant
with the score-row's own crests+digits — so it is NOT promoted to
`.title.headline`. `slot.body` (the flat event string) is: it is the
one piece of information that changes card-to-card and is not otherwise
visible anywhere in the kept score-row.

Masthead (plain dot, same as the news/generic branches — no
agent-style pulse/large variant — + `football` kicker) + a real
per-signal stamp, wired through the EXISTING `<Stamp priority={slot.priority}
signal={slot.signal} eventType={slot.eventType} />` (no new stamp table:
`stampFor` already returns `SIGNAL_STAMPS[signal]` — Card/Off/Foul/
Offside/VAR/Sub/Break/Final — for every real football `EventSignal`,
short-circuiting before `eventType` is even consulted, since football
never carries `signal: "generic"`) + the priority accent stripe + a
title that IS `slot.body` verbatim (e.g. "Goal — K. Havertz 78'",
"Half-time", "Full-time") + no subtitle, no `.notif-body`, no fact pills
(no data source for any of the three, see correction above) — from the
shared template, exactly as plan 169's Agent Board treatment does it.
The score-row (league chip, live/break/final chip, clock, crests,
rolling score digits) is kept, ported close to verbatim from the
current component, as an ADDITIVE flavor block — matching plan 169's
own precedent for content that doesn't fit the generic template's
shape. Cards-line (disciplinary tally) stays as a second, smaller
additive line below the score-row when `!cardsClean`, same as today.

Also dropped, as a documented simplification: the current component's
`.event-line` icon+tint (`ev-ico goal`/`tint-goal` etc, `eventPresentation`
from `lib/presentation.ts`) does not carry over onto `.title.headline`.
`.event-line` was a flex row built for an icon beside text;
`.title.headline` is a line-clamped text block with no equivalent icon
slot, and forcing the two together risks a real layout bug for a purely
cosmetic loss — `slot.body`'s own text already names the event
("Goal —"/"Penalty - Scored —"/"Own Goal —"), and the celebration
(burst/ring/strobe on `.card-assembly`) still fires distinctly per
family regardless. `eventPresentation`/`footballEventKindFor` stay
exactly as they are — still needed for `StatusRailCard.tsx`'s
celebration-class selection, just no longer for an icon inside the
content template.

Goal-family events (`goal`, `penalty_scored`) keep the real
`goal-overshoot`/`goal-burst`/`goal-ring` celebration (`choreography.css`,
unchanged) — this is a shell-level effect independent of whichever
content template sits underneath it, already confirmed working
end-to-end in the prototype. `own_goal` stays quiet (no celebration),
matching the current component's own distinction. `red_card` keeps its
existing `steps(1,end)` strobe (`.pulse-red`) — this plan does not touch
that celebration.

Per-event stamp mapping (already correct — `stampFor`/`SIGNAL_STAMPS`,
`lib/presentation.ts:28-39`, unchanged by this plan):

| event | `EventSignal` | stamp (`stampFor`) |
|---|---|---|
| `goal` / `penalty_scored` / `own_goal` | `goal` | Live |
| `yellow_card` | `yellow_card` | Card |
| `red_card` | `red_card` | Off |
| `foul` | `foul` | Foul |
| `offside` | `offside` | Offside |
| `var_check` | `var_check` | VAR |
| `substitution` | `substitution` | Sub |
| `halftime` | `halftime` | Break |
| `fulltime` | `fulltime` | Final |

The idle hover-peek scorecard (`IdleHoverPeek.tsx`'s
`ScorecardRevealContent`, `.idle-reveal-scorecard`) is **out of scope** —
it already renders through the shared shell's idle-peek mechanism (same
one weather uses), is not part of `LiveMatchScorecard.tsx`, and the
operator has given no indication they want it changed.

## Repo conventions to follow

- Same sibling-component pattern plan 169 established with `AgentHeroCard`
  (`NotificationBody.tsx`) — add a new exported `FootballHeroCard` in the
  SAME file, called directly from `StatusRailCard.tsx` in place of
  `<LiveMatchScorecard>`, rather than folding football into
  `NotificationBody`'s own function body. This is a hard requirement, not
  a style preference: `NotificationBody`'s function unconditionally
  renders `<Manifest>` and `<TtlBar>` after `.compact` (outside the
  `news ? : ` branch), and football must render NEITHER — that's an
  existing, intentionally locked design decision (`LiveMatchScorecard.tsx`'s
  own header comment: "no TtlBar... no generic Stamp"; test-enforced by
  `StatusRailCard.test.tsx`'s "renders no TtlBar and no Manifest on the
  live card" and "renders no ttl-bar (queue segments included) on the
  live card" — both must keep passing). Routing through `NotificationBody`
  itself would require new conditional branching to suppress those two
  children specifically for football, which is more invasive than adding
  one more sibling component next to `AgentHeroCard`. Some JSX duplication
  between the two hero-card components (masthead-row/title/accent-stripe
  shape) is expected and fine — `AgentHeroCard` takes agent-specific
  primitive props (`dotKey`/`pulse`/`factsDanger`) that don't apply here,
  and `FootballHeroCard` needs its own additive score-block slot that
  `AgentHeroCard` doesn't; forcing one shared component to cover both
  would need more conditional plumbing than the two staying separate.
- `Crest`/`ScoreDigit` (existing sub-components, `LiveMatchScorecard.tsx`)
  port unchanged into the new additive score-row block — this plan
  restructures where they render, not their own implementation.
- Reuse the exact `SIGNAL_STAMPS`/`stampFor`/`eventPresentation`/
  `footballEventKindFor`/`livePillVariantFor` lookups (`lib/presentation.ts`)
  already driving the current component, unchanged — this plan changes
  rendering, not event-classification logic.
- Same fixed-size discipline as plan 169: `.compact` already carries
  `overflow: hidden` (plan 169, `masthead-content.css`) — this plan does
  not touch that rule again. The score-row additive block needs its OWN
  equivalent discipline (it wasn't part of plan 169's precedent, and this
  plan renders NO fact pills at all per the Target correction above) —
  cap it to not push the card past its state's fixed width either; the
  current component already fits within the fixed widths today, so this
  is a "don't regress it" check against the longest realistic team
  abbreviation/league name, not new layout work.

## Boundaries

- Build the score-row as a kept, additive block (resolved above) — do NOT
  build the prototype's earlier score-less/title-only version, that
  draft is superseded.
- Do NOT touch `IdleHoverPeek.tsx`/the idle hover-peek scorecard — out of
  scope, see Target.
- Do NOT touch the goal/red-card celebration keyframes or triggers
  (`choreography.css`'s `pulse-goal`/`pulse-red` classes, or whatever sets
  them in `StatusRailCard.tsx`) — this plan only changes the content
  template underneath them.
- Do NOT remove `live-scorecard.css`'s current rules in the same commit
  that adds the new template path — land new rendering first, confirm
  live, THEN trim dead CSS as a follow-up (same reasoning as plan 169's
  identical boundary).
- Do NOT touch `src-tauri/`/any rust file, and do NOT add fact pills to
  `FootballHeroCard` by parsing `slot.body`'s text to fabricate
  scorer/assist/booking data — see Target's correction. If real
  structured per-event data is wanted later, that's `EventPayload`/
  `poller.rs` wire work and belongs in its own plan, not this one.
- If `poller.rs`'s `diff_match`/`make_event`/`make_rich_event` no longer
  match the description in Target's correction above (real drift since
  this plan was written this time, not the first correction's own
  error — e.g. `subtitle` starts being populated, or `details` starts
  carrying something beyond Clock/Cards), STOP and report back instead
  of improvising a fact-pill design on the fly.

## Steps

1. Read `src/components/LiveMatchScorecard.test.tsx` in full (small,
   ~206 lines) and the `describe("live-match football scorecard (plan
   084)", ...)` block in `src/components/StatusRailCard.test.tsx`
   (~1626-1870) — know what breaks before touching the component. Note
   `liveSlot()`'s definition and the `ESPN_BASE` fixture near the top of
   that file; both are reused by the new tests.
2. In `src/components/NotificationBody.tsx`, add a new exported
   `FootballHeroCard` component, alongside (not replacing) `AgentHeroCard`.
   Props: `title: string` (pass `slot.body` verbatim), `priority: Priority`,
   `signal: EventSignal`, `eventType: EventType` (pass `slot.priority`/
   `slot.signal`/`slot.eventType` straight through from the call site —
   no new computation), plus the score-row's own data (league, clock,
   live-pill variant + label, home/away abbrev + crest path + score,
   cards clean flag + per-side card counts). Body:
   ```tsx
   <div className="compact">
     <div className="copy">
       <div className="masthead-row">
         <div className="masthead">
           <span className="dot" />
           {GENERIC_MASTHEAD_KICKER.football}
         </div>
         <Stamp priority={priority} signal={signal} eventType={eventType} />
       </div>
       <div className="title headline">{title}</div>
       {/* additive score-row block next, Step 3 */}
     </div>
   </div>
   ```
   No subtitle, no `.notif-body`, no fact pills — see Target's correction
   for why (no wire data backs any of the three for football today).
3. Port `Crest`/`ScoreDigit` (currently in `LiveMatchScorecard.tsx`) and
   the `.sc-head`/`.score-row`/`.cards-line` JSX verbatim into
   `FootballHeroCard`, as a sibling block after `.title.headline` inside
   `.copy` (same DOM position the prototype's `scoreBlockHtml()` uses —
   see `prototype/football-card.html:1130-1145` for the exact shape to
   match, translated from that file's static-HTML-string form back into
   JSX). Whether `Crest`/`ScoreDigit` move into `NotificationBody.tsx` or
   stay in `LiveMatchScorecard.tsx` and get imported is an implementation
   choice — either is fine as long as there is exactly one definition of
   each, not a duplicate.
4. Delete `LiveMatchScorecard.tsx` once `FootballHeroCard` fully replaces
   its rendering (its content is now `FootballHeroCard` — don't leave the
   old component as unused dead code). Update
   `src/components/StatusRailCard.tsx`'s call site (currently lines
   1066-1086: `isLiveCard && liveEspn !== undefined ? <LiveMatchScorecard
   .../> : <NotificationBody ... />`) to call `<FootballHeroCard>` instead
   of `<LiveMatchScorecard>` in that branch, passing `slot.body` as
   `title` and the existing `liveEspn`/`pillVariant`/`pillLabel`/
   `cardsClean` values (already computed at lines 620-630, unchanged) plus
   `slot.priority`/`slot.signal`/`slot.eventType` for the new Stamp props.
   Delete line 622's `const eventPresentation = footballKind ?
   eventKindPresentationFor(footballKind) : null;` too — confirmed (grep
   the file) its ONLY use is the removed `eventPresentation` prop on
   `<LiveMatchScorecard>`; it is NOT the same computation that drives the
   `.card-assembly` celebration class (that's `setLiveCelebration` at line
   333, a separate call to `eventKindPresentationFor` feeding React state,
   untouched by this step) — removing line 622 does not affect
   celebrations. Leave `footballKind` itself (line 621) alone; line 333
   still needs it. `eventKindPresentationFor`/`EventKindPresentation`
   stay imported for line 333's use even after line 622 is deleted — don't
   remove the import.
5. Delete `src/components/LiveMatchScorecard.test.tsx` (its coverage
   moves with the component) and add an equivalent direct-render test
   file for `FootballHeroCard` covering the same odometer/chip-morph
   behavior (the CSS-string assertions against `.score-digit`/`.chip-live`
   selectors carry over unchanged — those rules aren't moving).
6. Rewrite the `describe("live-match football scorecard (plan 084)", ...)`
   block in `StatusRailCard.test.tsx`: swap `.notif-block`/`.event-line`
   assertions for `.title.headline`/`.stamp` assertions (title text ==
   the `body` passed to `liveSlot()`, stamp text == the expected
   `SIGNAL_STAMPS` word per signal), keep the `.chip-league`/`.chip-live`/
   `.clock-pill`/`.crest`/`.score`/`.cards-line` assertions as-is (that
   markup doesn't move), and keep the "renders no TtlBar and no Manifest"
   /"renders no ttl-bar" tests passing unchanged (still true —
   `FootballHeroCard` renders neither, same as `AgentHeroCard`).
7. Trim `live-scorecard.css`'s now-dead masthead-equivalent rules (if any
   — most of that file is the score-row/crest/digit styling, which stays
   unchanged) as a separate follow-up commit, only after steps 1-6 are
   confirmed passing.

## Verification

- **Mechanical**: `npx vitest run` clean. `npx tsc --noEmit` clean. `npx
  biome ci .` clean.
- **Visual, real app**: trigger each of the 11 event types (test fixtures
  or a live match if one's running) and compare against
  `prototype/football-card.html`'s proposal section — masthead, real
  stamp word, accent stripe, and the score-row's crests/digits should all
  be present and legible.
- **Celebration check**: trigger a `goal` and a `red_card` event
  specifically; confirm the existing celebration (burst/ring for goal,
  strobe for red card) still plays correctly on top of the new content
  template, unchanged from before this plan.
- **Overflow check**: same discipline as plan 169 — longest realistic
  team names/player names, confirm no card ever exceeds its state's fixed
  width/height in real DevTools.
- **Done when**: all 11 event types render through the shared template
  with the score-row kept as an additive block, both celebrations still
  fire correctly, and a side-by-side against
  `prototype/football-card.html`'s proposal section shows no unintended
  information loss versus what's shipped today.
