# Plan 084: Football card display — sticky scorecard with event-driven expansion

> **Executor instructions**: This is a build plan for the DISPLAY half
> of the football card, implementing `prototype/football-card.html`
> exactly as operator-locked 2026-07-20 (sticky medium-priority presence
> ~T-30 → full-time; resting = idle rail green dot; hover = compact
> scorecard, NO full-expand; expansion is EVENT-driven — goal/penalty/
> yellow/red celebrate, own-goal updates the score with NO celebration,
> foul/offside/VAR/sub open quietly;
> preemptible by higher tiers, returns after; paused football → ambient
> weather fallback; no queue track). It consumes the wire contract plan
> 083 ships (structured espn meta, crest URLs, richer event signals) —
> do not start before 083 lands, and check 083's Maintenance notes for
> any wire-shape divergence from its Step-1 sketch before coding against
> it. The preview-overlay.css mirror law applies: every CSS change lands
> in `src/settings/preview-overlay.css` in the SAME commit. Hover
> behaviors in the prototype are contextual only — the hover TRIGGER is
> plan 086's gated work; this plan builds the card's states and renders,
> not the cursor machinery. When done, update the status row for this
> plan in `plans/README.md` — unless a reviewer dispatched you and told
> you they maintain the index.
>
> **Drift check (run first)**: `git diff --stat 3de785a..HEAD -- src/components/StatusRailCard.tsx src/lib/presentation.ts src/styles.css src/settings/preview-overlay.css src/useSlotState.ts prototype/football-card.html`
> Expect `StatusRailCard.tsx`/`styles.css`/`preview-overlay.css` to
> differ from 3de785a by exactly 081's and/or 082's diff (081 mounts
> its TTL bar in the card and adds `.ttl-*` CSS; 082 adds `.wx-*` CSS),
> and `useSlotState.ts` by 081's and/or 083's — anything MORE is drift
> to reconcile. Any diff in
> `prototype/football-card.html` is a STOP condition. Baseline
> `3de785a` already INCLUDES plan 080 (merged 2026-07-21 as `d21d689`)
> — the StatusRailCard.tsx refs below are post-080. (Baseline history:
> `9a954b0` → `4fb3af9` for 063's merge — celebration/keyframe refs
> shifted NON-uniformly, +9 early and +24 by the live-dot block — then
> → `3de785a` for 080's merge. All refreshed individually and
> re-verified by direct read.)

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED — adds a second full card layout to `StatusRailCard.tsx`
  (until now: one compact layout + content branches) and new event
  signals through the exhaustive presentation tables; celebrations must
  not regress the shipped goal/red pulses
- **Depends on**: 083 (backend wire contract — structured meta, crests,
  rich-event signals), 080 (news card landed; the card CSS this plan
  extends is post-080). Hover-reveal of the compact scorecard from the
  rail is gated on 086's outcome like every other hover interaction.
- **Category**: direction (locked 2026-07-20 — 079 items 3/4/5/6
  display + 043 looped in) → build
- **Planned at**: commit `9a954b0`, 2026-07-20 (reviewed same date:
  drift baseline corrected, own-goal header contradiction fixed,
  pulse-gating/SOON-source/compact-hint gaps pinned). **Review-plan
  pass 2 (2026-07-21, against `4fb3af9`)**: component/prototype
  citations re-verified exact (StatusRailCard.tsx :17-20/:28-50/:47/
  :57-67, presentation.ts :22-54/:28,
  useSlotState.ts :4-13, useStatusState.ts :106-116, poller.rs
  :373-436/:385-388/:481-487/:504-506, football-card.html :46-68/
  :74-92/:123-129); eight styles.css refs refreshed for 063's
  non-uniform shift (goal-overshoot `:142`→`:151`, goal-burst
  `:157`→`:166`, goal-ring `:176`→`:185`, ripple-out `:222`→`:231`,
  red-alert `:245`→`:254`, pulse machinery `~:76-256`→`~:99-257`,
  live-dot-pulse `:601`→`:622-625`, reduced-motion precedents
  `:233-237`/`:260-267`→`:242-246`/`:269-276`). Drift baseline
  re-stamped to `4fb3af9`, then again to `3de785a` when plan 080 merged
  mid-review (StatusRailCard.tsx refs refreshed to post-080: branches
  `:101-144`→`:104-175`, detail cells `:135-142`→`:141-148`,
  compact-hint `:151-155`→`:157-161`).

## Why this matters

The football mockup is the most-iterated surface of the 079 redesign
(v3, all 9 event states + 3 match states + the full interaction arc
mocked) and the operator locked its model explicitly: football is a
STICKY medium-priority resident from ~T-30 to full-time, NOT a burst of
one-shot cards. Today's live-match card (plans 039-042, opt-in) already
collapses a match into one updating Topic card, but renders it through
the generic compact layout — pre-joined title string, detail cells —
with no score prominence, no crests, and no event-specific presentation.
This plan gives the locked design its render: the compact scorecard
(league chip, state pill, clock pill, crest–score–crest, event line
with CSS-shape icons, per-side cards line) plus the celebration/quiet
expansion split, on top of 083's structured wire.

## Current state

- `src/components/StatusRailCard.tsx` — one card shell (`.rail-card` +
  priority/idle/expanded/pulse classes, `:57-67`), content branches:
  idle vs showing, and within showing: news vs generic (`:104-175`,
  post-080). Plan 042's collapsed detail cells render at `:141-148`. The pulse
  celebration (`:28-50`; the `PULSE_END_ANIMATION` constant is DEFINED
  at `:17-20` and used at `:47`): `pulse-goal`/`pulse-red` classes keyed on
  `[currentId, currentSignal]`, cleared on `animationend` via
  `PULSE_END_ANIMATION` — goal/red bursts are pure CSS keyed off the
  signal, never priority (the documented acceptance criterion at
  `:30-34`).
- `src/styles.css` — shipped celebration CSS this plan echoes:
  `goal-overshoot` (:151), `goal-burst` (:166), `goal-ring` (:185),
  `ripple-out` (:231), `red-alert` (:254) — the `.rail-card.pulse-goal`
  /`.pulse-red` machinery spans ~:99-257. Green `#7fe08d` is RESERVED
  for live-now (locked decision — `.src-chip.live`, pulsing live-dot,
  `live-dot-pulse` :622-625).
- `src/lib/presentation.ts:22-54` — `SIGNAL_STAMPS` (goal/kickoff→Live,
  halftime→Break, yellow_card→Card, fulltime→Final, red_card→Off) and
  `stampFor` — exhaustive switches with `assertNever`; if 083 added
  foul/offside/var/sub signals these tables failed to compile until
  extended (the seam working as designed).
- `src/useSlotState.ts:4-13` — `EVENT_SIGNALS` validator list, mirroring
  the rust enum; post-083 it carries any new signals.
- Backend contract (083, verify against its final shape): structured
  espn meta on `SlotState::Showing` (league, home/away abbrev, home/away
  score, clock, per-side cards tuples, optional crest URLs — `None` =
  text-abbrev fallback); richer informational events (foul/offside/VAR/
  sub) arriving as Football events through the same Topic; sticky
  presence via the EXISTING Topic supersession machinery
  (`poller.rs:373-436`, `CardTopic::Live` = `Recurring`,
  `CardTopic::FullTime` = same-Topic `OneShot` that retires the card
  through the ordinary rotate-out path).
- Prototype (`prototype/football-card.html`): compact scorecard —
  `.sc-head` = `.league-chip` + `.live-pill` (variants live / `.break`
  amber / `.final` dim / `.soon` pre-match, `:48-56`) + `.clock-pill`
  (`:57`); `.score-row` = side(`.crest` 30px circle + `.side-name`) —
  `.score` (800 24px mono with dim `.dash`, `.score.vs` pre-match) —
  side (`:59-65`); `.event-line` with `.ev-ico` CSS-shape icons
  (`:67,74-87`: goal filled green ball, pen green ring, og hollow grey
  ball, yc/rc amber/coral slips, foul triangle, off flag, var monitor,
  sub up/down triangles) and tint classes `.tint-goal`/`.tint-yc`/
  `.tint-rc` (`:90-92`); `.cards-line` per-side tallies (`:68`).
  Celebrations (`:123-129`): `.cele-goal` = green `goal-glow` +
  `cele-ring`; `.cele-yc` = amber `cele-ring`; `.cele-rc` =
  `red-strobe` — deliberately echoing the shipped pulse-goal/pulse-red.
  Match states (:309-359): pre-match SOON + `vs` + kickoff line,
  half-time BREAK + HT, full-time FINAL + FT (result shows once as a
  normal rotation item, then retires). Interaction model (:361-431):
  sticky ~T-30 → FT, hover reveals compact only, event forces the card
  open ~4s then settles to rail, higher-priority preempts and football
  returns, paused → weather peek.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Frontend unit tests | `npx vitest run` | all pass |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Lint + format gate | `npx biome ci .` | exit 0 |
| Frontend build | `npx vite build` | exit 0 |

## Scope

**In scope**:
- `src/components/StatusRailCard.tsx` — a live-match (football) showing
  branch rendering the compact scorecard from 083's structured meta;
  crest `<img>` with text-abbrev fallback; event line + icon + tint;
  per-side cards line; celebration classes for goal/pen/og/yellow/red;
  quiet presentation for foul/offside/VAR/sub; NO `Track` (queue
  slider) for the recurring live card.
- `src/styles.css` — the scorecard + icon + celebration CSS (prototype
  classes adapted to the rail-card), mirrored in
  `src/settings/preview-overlay.css` SAME commit.
- `src/lib/presentation.ts` — extend the exhaustive tables for any new
  083 signals (stamps for foul/offside/var/sub); a small
  event-kind → (icon class, tint class, celebration class) table in the
  same data-table spirit.
- Vitest coverage per the Test plan.
- Preemption/paused behaviors INSOFAR as they render: preempted = the
  slot simply shows the preempting item (existing machinery — nothing
  to build); paused football = the ambient rail falls back to weather
  (verify it already does via the status-state gates —
  `statusRailActive` in `src/useStatusState.ts:106-116`; build only if
  a gap exists).

**Out of scope**:
- The hover TRIGGER and any cursor machinery (plan 086-gated): the
  rail→compact hover reveal, hover weather peek. This plan builds the
  states; 086 decides how pointer input reaches them.
- Backend changes (083's whole scope); sticky-window timing changes
  (~T-30 activation is the poller's existing live-match detection; if
  real T-30 pre-match presence needs poller changes beyond what 083
  shipped, STOP and note it rather than expanding scope).
- The generic card / news card / manifest layouts (080 shipped news;
  generic restyle is a future 079-item-19 plan).
- Full-time retirement mechanics — already owned by
  `CardTopic::FullTime` (`poller.rs:385-388`); this plan only renders
  the FINAL state while it's visible.

## Steps

### Step 1: Event-kind table + signal stamps

In `src/lib/presentation.ts`, extend `SIGNAL_STAMPS` for 083's new
signals (foul/offside/var/sub → quiet informational stamps — pick
terse words in the existing style, e.g. foul→"Foul", offside→"Flag"
(NOT "Off" — that stamp is already red_card's, `presentation.ts:28`),
var→"Check", sub→"Sub"; the stamps are the small status words, the
event line carries the detail). Add the event-kind table mapping each
football signal to its presentation triple:
`{ iconClass, tintClass: string | null, celebration: "goal" | "yc" | "rc" | null }`
— goal + penalty-scored + (celebration "goal"), own-goal (no
celebration, hollow icon), yellow ("yc"), red ("rc"),
foul/offside/var/sub (null/null). Exhaustive with `assertNever`, same
discipline as the existing tables.

**Verify**: `npx tsc --noEmit` → exit 0; `npx vitest run` → all pass
(existing stampFor tests may need the new arms — update the tables'
tests, don't weaken them).

### Step 2: The live-match branch in StatusRailCard

Add the branch: `showing` + event is a football Topic card with
structured espn meta (detect via the espn meta block's presence, not
string sniffing — 083 put it on the wire for exactly this). Render the
prototype's compact scorecard: `.sc-head` (league chip from
`meta.league`; state pill derived from the existing signal/match-state
— Live/Break/Final mapping three of the prototype's four `.live-pill`
variants; clock pill from `meta.clock`). SOON variant caveat: NO
pre-match signal or meta field exists on the wire today (kickoff→Live,
and the poller's first sighting of a match is a silent baseline,
`poller.rs:481-487`) — render SOON only if 083 shipped an explicit
pre-match state; otherwise omit the variant and note the gap in the
completion report (the STOP below covers the deeper poller gap).
`.score-row` with crest
`<img src={crestUrl}>` on each side falling back to the text-abbrev
`.crest` circle when the URL is `None` (onerror → swap to fallback
too, defense in depth); `.event-line` with the Step-1 icon + tint for
the latest event (the event text continues to come from the payload
body, labeled by ESPN's own text as today — plan 041); `.cards-line`
from the per-side tuples, omitted on a clean match (same rule as
plan 042's cell omission, `poller.rs:504-506`). NO `Track` for this
card — the queue slider is meaningless for a recurring presence
(prototype lock: "no queue track"); keep it on every other card type.
  Also omit the `⌃⇧N more` compact-hint (`StatusRailCard.tsx:157-161`)
  on this branch — it advertises a full-expand this card doesn't have.
NO full-expand: this branch does not render the `Manifest` expand path
(the card's expansion is event-driven presence, not the ⌃⇧N manifest —
but ⌃⇧N expand-toggle must not break; if the slot's `expanded` flag
arrives true on a live card, render the same compact scorecard, and
note the choice in a code comment AND in this plan's Maintenance notes
when you report completion).

**Verify**: `npx vitest run` → all pass; `npx tsc --noEmit` → exit 0.

### Step 3: Celebrations on the football card

Reuse the shipped pulse discipline (`StatusRailCard.tsx:28-50`):
celebration classes keyed on `[currentId, currentSignal]`, cleared on
`animationend` via the ending keyframe's name, never keyed on priority.
CRITICAL gating: the existing pulse `useEffect` fires `pulse-goal`/
`pulse-red` on signal alone regardless of branch — gate it on
`!isLiveCard` so a live-branch goal plays ONLY `cele-goal`, never both
celebrations stacked (the existing pulses stay for non-live-card
football events and the flag-off path).
Add `cele-goal`/`cele-yc`/`cele-rc` CSS (prototype `:123-129` adapted):
green glow + ring for goal/penalty-scored, amber ring for yellow,
coral strobe for red — visually echoing `pulse-goal`/`pulse-red`
(:99-257) rather than replacing them. Own-goal updates
the score with NO celebration (prototype lock). Each new keyframe gets
a `prefers-reduced-motion` override, matching the file's existing
precedents at `styles.css:242-246` and `:269-276`.
Foul/offside/VAR/sub render the quiet event line only.

**Verify**: `npx vitest run` → all pass (new assertions per Test plan);
the existing pulse tests pass UNCHANGED.

### Step 4: styles.css + preview-overlay.css mirror (same commit)

All scorecard/icon/celebration CSS into `src/styles.css`; mirror every
rule in `src/settings/preview-overlay.css` under `.appearance-preview`
in the same commit. Class names follow the prototype's (`sc-head`,
`live-pill`, `clock-pill`, `score-row`, `crest`, `event-line`,
`ev-ico`, `cards-line`, `cele-*`) so future cross-referencing against
`prototype/football-card.html` stays greppable.

**Verify**: `npx vite build` → exit 0; mirror grep —
`grep -c 'sc-head\|live-pill\|clock-pill\|score-row\|ev-ico\|cele-goal\|cele-yc\|cele-rc\|cards-line' src/settings/preview-overlay.css` ≥ 8.

### Step 5: Full gate

**Verify**: every command in the Commands table exits 0;
`git diff --stat` touches only the four frontend files + tests.

## Test plan

- **Vitest — per event state** (the prototype's 9): goal (green tint +
  cele-goal class + score), penalty-scored (same celebration family),
  own-goal (score updated, NO celebration class), yellow (amber tint +
  cele-yc + cards line ticks), red (coral tint + cele-rc), foul/
  offside/VAR/sub (quiet event line, correct icon class, no tint, no
  celebration).
- **Vitest — match states**: pre-match SOON pill + `vs` score;
  half-time BREAK + HT; full-time FINAL + FT.
- **Vitest — no-track-on-recurring**: the live football card renders
  NO `Track`/queue-slider element; a generic card in the same batch
  still renders it (regression pin).
- **Vitest — crest fallback**: crest URL present → `<img>`; `None` →
  text-abbrev circle with the abbreviation.
- **Update minimally**: existing StatusRailCard assertions repointed
  only where the new branch genuinely changes structure; the
  pulse-goal/pulse-red and no-remount (plan 078) tests pass unchanged.
- **Manual-only** (operator, TESTING_STRATEGY §5): a real live match
  with flags on — sticky presence, preemption by a High cmux alert and
  return, paused→weather fallback, celebration look against the
  prototype. The hover reveal from the rail is 086-gated and NOT part
  of the acceptance here.

## Done criteria

- [ ] Compact scorecard renders from structured meta: league chip, state pill (4 variants), clock pill, crest–score–crest, event line + CSS-shape icon, per-side cards line
- [ ] Celebrations: goal/pen green, yellow amber, red coral — keyed on id+signal, animationend-cleared, reduced-motion overrides present; own-goal and the four informational events quiet
- [ ] No queue `Track` on the recurring live card; present on other cards (pinned by test)
- [ ] `presentation.ts` tables extended exhaustively for all 083 signals (tsc proves it)
- [ ] Every CSS rule mirrored in `src/settings/preview-overlay.css`, same commit
- [ ] `npx vitest run`, `npx tsc --noEmit`, `npx biome ci .`, `npx vite build` all exit 0; no rust diff
- [ ] `plans/079-checklist.html` and `plans/frontend-ui-consolidated.html` statuses updated (football card → shipped, hover caveat noted); `plans/README.md` row for 084 updated

## STOP conditions

- **Prototype drift**: `prototype/football-card.html` differs from this
  plan's description (drift-check non-empty) — the model was locked
  against this exact mockup; stop and re-confirm.
- **Mirror-law risk**: a scorecard/celebration rule can't be scoped
  into `preview-overlay.css` — stop; do not land unmirrored CSS.
- 083's shipped wire shape diverges from what this plan assumes (check
  its Maintenance notes / the actual `EspnMeta` before Step 2) — if the
  divergence is more than field renames, stop and reconcile the two
  plans rather than coding against a guessed contract.
- The sticky/preempt/pause behaviors turn out to need poller or queue
  changes (e.g. real T-30 pre-match presence isn't producible by what
  083 + the existing live detection emit) — stop and file the backend
  gap as its own plan; do not expand this one.
- The new branch starts regressing the generic/news render paths
  (existing tests failing beyond structure-repointing) — the branch is
  too invasive; stop and re-shape it.

## Maintenance notes

- Update `plans/079-checklist.html` and
  `plans/frontend-ui-consolidated.html` (football card locked-decision
  entry → shipped; its "hover reveals compact" caveat stays marked
  086-gated until the trigger exists).
- The event-kind → icon/tint/celebration table is THE place future
  event types register (e.g. if 043's feed later adds disallowed-goal
  or shootout events) — one row, exhaustive-compiled.
- Crest `<img>` src comes from 083's serving route (asset protocol or
  `crest://`); if 083 changed routes late, this plan's Step 2 inherits
  whichever shipped — the render code should treat the URL as opaque.
- When 086 unblocks hover, the rail→compact reveal wires onto this
  branch's resting state; keep the branch's mounted-when-showing
  structure compatible with a future always-mounted-but-collapsed
  treatment (plan 078's manifest-wrap precedent).
