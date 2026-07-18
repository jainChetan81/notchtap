# Plan 031 (spike): Design the supersession engine's first producer — a live-match scoreboard card

> **Executor instructions**: This is a DESIGN SPIKE, not a build plan.
> The deliverable is a design document plus open questions for the
> maintainer — **zero production code changes**. Follow the steps, honor
> the STOP conditions, and when done update this plan's status row in
> `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat a58f115..HEAD -- src-tauri/src/queue.rs src-tauri/src/poller.rs CONTEXT.md`
> Drift doesn't block a spike — but read the drifted regions before
> quoting them.

## Status

- **Priority**: P2
- **Effort**: M (coarse — investigation + design doc, no build)
- **Risk**: LOW (docs only)
- **Depends on**: none
- **Category**: direction
- **Planned at**: commit `a58f115`, 2026-07-18

## Why this matters

The queue's Topic/supersession machinery is fully built and tested —
`supersede_if_topic_matches`, the capped Rotation top-up, Recurring
requeue — yet **every production producer sets `topic: None`** and no
producer emits `RotationSpec::Recurring`; the only constructions of
either live in `queue.rs` tests. The machinery's designed-for use case
is staring at it: the ESPN poller currently turns a goal-heavy match
into N separate one-shot cards marching through the single-slot queue.
One live scoreboard card keyed on the match's Topic would update in
place (Waiting or Visible) instead of stacking, exercising code that is
otherwise dead-in-production, on the app's marquee source. The prior
deep audit floated this and the maintainer deferred (not rejected) it.
This spike produces the concrete Topic-lifecycle design so the
maintainer can approve or kill it cheaply.

Important nuance the design must engage with honestly: the current
per-delta behavior was a *recorded choice*, not an accident —
`poller.rs`'s event constructor says the poller "doesn't need the
queue's topic supersession mechanism **in this pass**" (spec §3.4).
The spike's job is to lay out what changes when that pass ends, not to
pretend the current behavior is a bug.

## Current state (grounding — quote-verified at `a58f115`)

- The unused machinery, `src-tauri/src/queue.rs`:
  - `supersede_if_topic_matches` (line 153) — called from the enqueue
    path (line 117: `if self.supersede_if_topic_matches(&topic, &event, now)`);
    updates a Waiting or Visible item in place.
  - Capped extension on Visible supersede: `MIN_REMAINING_ON_SUPERSEDE_SECS = 2`
    (line 418) with the deficit top-up around lines 282-283.
  - Recurring requeue in `tick()` (~lines 200, 328).
  - Tests exercising all of it: the only `topic: Some(...)` /
    `Recurring` constructions in the tree (`queue.rs` tests ~451, 469,
    765).
- The vocabulary (`CONTEXT.md` — use these terms verbatim in the doc):
  - **Recurring** (line 49): "a Rotation kind that requeues to the back
    of its own Priority tier's Waiting line after its turn … bounded by
    supersession or the underlying state naturally ending, not a
    clock."
  - **Topic** (line 54): "the supersession identity carried by a
    Recurring Event … a fresh Event sharing a Topic updates the
    existing Notification in place — Waiting or Visible — rather than
    adding a new one; a Visible supersede can grant a small, capped
    Rotation extension if remaining time was already low, but never
    mutates when it was first promoted."
  - **Score Update** / **Match State** (lines 113-118).
- The producer that would adopt it, `src-tauri/src/poller.rs`:
  - `make_event` (~line 284) hardcodes `topic: None` with the "in this
    pass" comment (lines 302-306) and `RotationSpec::OneShot`.
  - `diff_scoreboard` (line 337) — pure, well-tested delta engine; its
    emission rules are documented at lines 319-336 (silent first
    sighting, ScoreUpdate on score change, MatchState on
    kickoff/half-time/full-time + cards, absence-tolerant eviction).
    Each match has a stable id (`v.id`, ESPN's event id) — the natural
    Topic key.
  - Fan-out: `enqueue_and_fan_out` (~line 524) offers accepted events
    to every connector (telegram) — the doc must work out what
    supersession means for outbound (a superseded enqueue is not a
    normal accept; does telegram still get every delta? probably yes —
    connectors observe acceptance, `CONTEXT.md` Connector definition —
    but verify how the return value of `enqueue` interacts with the
    supersede path and say so explicitly).
- Adjacent behaviors the design must compose with (all shipped):
  - Auto-expand on High promotion (plan 008; espn default priority is
    High) — a Recurring card re-promoting each cycle would re-expand
    every turn; is that desirable?
  - Rotation-order tie-break by origin (v6) and per-source priority.
  - The deadline heartbeat (plan 015 / plan 036) — supersession of a
    Visible item can change its effective deadline via the extension;
    confirm the wake path covers it (the enqueue wake should).
  - `docs/V3_6_TECHNICAL_SPEC.md` §4 region documents supersede
    semantics decisions (e.g. "a priority-changing supersede moves the
    item to the back of its …" around line 112) — read and cite.

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Read-only exploration | `grep`, `Read` | — |
| Run the queue's supersession tests (understanding, not gating) | `cd src-tauri && cargo test --locked queue::` | pass |
| Confirm nothing changed | `git status` at the end | only the new doc + plans/README.md row |

## Scope

**In scope** (the only files you may create/modify):
- `docs/design/scoreboard-topic-card.md` (create; `docs/design/` may
  not exist yet — creating it is fine, or reuse it if plan 030 already
  did)
- `plans/README.md` (status row)

**Out of scope — hard rule for this spike**:
- ANY file under `src/`, `src-tauri/`, or config/build files. No
  prototype code in the repo; illustrative snippets live inside the
  doc.
- Rewriting `CONTEXT.md`/`ARCHITECTURE.md` — the doc *proposes* edits,
  it doesn't make them.

## Git workflow

- Docs-only commit `docs(design): scoreboard topic-card spike` in repo
  style. Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Trace both lifecycles end-to-end

(a) today's: `diff_scoreboard` → `make_event` (OneShot, no topic) →
`enqueue_and_fan_out` → queue/tiers → rotation-out; (b) the machinery's:
`enqueue` → `supersede_if_topic_matches` → in-place update / extension
→ Recurring requeue in `tick()`. Run the queue supersession tests and
read their assertions — they are the machinery's real spec.

### Step 2: Write the design doc

`docs/design/scoreboard-topic-card.md`, each section with a
**recommendation and at least one rejected alternative with reason**:

1. **Topic identity**: propose `espn:{league}:{match_id}` (or similar);
   collision/reuse behavior across days; what happens when a match id
   reappears after eviction.
2. **Which events carry the Topic**: ScoreUpdate and MatchState of the
   same match sharing one Topic (one card per match) vs score-only.
   What the card's payload becomes on each delta (title stays the
   matchup; body = latest delta? running scoreline?) — remember
   supersession *replaces* the notification content; a "2-1" card
   superseding "K. Havertz 6'" loses the scorer line — is that the
   right trade?
3. **Rotation kind**: `Recurring { display_secs }` for live matches
   (card cycles until full-time) vs OneShot-with-topic (card updates in
   place but still ages out). This is THE decision — a Recurring match
   card occupies a queue slot for 90+ minutes of cycling; with the
   single-slot overlay that changes the whole rail's feel. Model both
   against `CONTEXT.md`'s "bounded by supersession or the underlying
   state naturally ending" and define the ending (full-time →
   what removes the Recurring item? `diff_scoreboard` eviction alone
   doesn't dequeue — trace what would).
4. **Tier interplay**: espn priority default High + auto-expand on
   promotion (plan 008) + Recurring requeue = an expanded card
   re-promoting repeatedly; per-source priority (v6) mitigations.
5. **Connector semantics**: what telegram receives under supersession
   (every delta? only fresh cards?) — trace `enqueue_and_fan_out`'s
   accepted-set logic and state the answer with line refs.
6. **Config surface**: opt-in flag (`espn_live_card = true`?) vs
   replacing the current behavior outright; migration default.
7. **Phase/decision fit**: which `CONTEXT.md` terms need no change
   (ideally all — the machinery was designed for this), what
   `ARCHITECTURE.md` / `V3_6_TECHNICAL_SPEC.md` §3.4 amendments the
   build would need, and the "in this pass" comment's retirement.
8. **Test strategy**: `diff_scoreboard` stays pure — the topic/rotation
   assignment is a construction-rule change testable exactly like the
   existing `poller.rs` delta tests; queue behavior is already covered;
   name the new cross-boundary cases (supersede-while-visible from a
   real poll sequence fixture).
9. **Build estimate**: S/M/L with the file list the build would touch.
10. **Open questions for the maintainer** (e.g.: is one live card per
    match actually wanted during multi-match Saturdays, or is the
    current burst-of-deltas the preferred texture? does a Recurring
    card monopolizing rotation slots fight the news cards?).

### Step 3: Sanity-check citations

Every code claim gets a `file:line` valid at the commit read (stamped
at the top of the doc).

**Verify**: `git status` → only the design doc (+ plans/README.md row).

## Test plan

N/A — docs-only spike.

## Done criteria

- [ ] `docs/design/scoreboard-topic-card.md` exists, covers all 10
      sections, each with recommendation + rejected alternative
- [ ] The doc states the commit it was researched against
- [ ] The Recurring-vs-OneShot section explicitly models the
      single-slot occupancy consequence (section 3)
- [ ] No source-code changes (`git status` proof)
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- A production producer already sets `topic:`/`Recurring` (premise
  stale — re-grep first: `grep -rn "topic: Some\|Recurring" src-tauri/src --include=*.rs` outside `queue.rs` tests).
- You find a recorded *rejection* (not deferral) of this feature in
  `docs/` or `plans/README.md`.
- Understanding a lifecycle seems to require changing code to see what
  happens — write the uncertainty into the doc instead.

## Maintenance notes

- If approved, the build plan inherits this doc's Topic identity and
  rotation-kind decision verbatim; if rejected, record it in
  `plans/README.md`'s rejected list with the reason.
- This doc and plan 030's enrichment doc both touch `EventMeta`/card
  presentation ambitions — whoever builds second should re-read the
  first's decisions.
