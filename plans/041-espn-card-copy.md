# Plan 041: ESPN event-card copy — name the event, not just scorer+minute

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report — do not
> improvise.
>
> **Drift/coordination check (run first)**: this touches `poller.rs`,
> which **037 (the Engine)** already migrated (merged `6b53c32` —
> nothing left to wait on there: 037 didn't touch event-body
> construction, only the ingest/wake/emit plumbing around it) and which
> **039 (live card)** will also touch, but 039 is currently BLOCKED on
> **038** and not in flight. `git status` clean for
> `src-tauri/src/poller.rs` — if dirty, STOP and coordinate rather than
> layering on top. If 039 lands first, do this plan's `detail_type.text`
> extraction (Step 1) in the same pass rather than a second round-trip
> through `make_event`/`diff_scoreboard` — the two plans touch adjacent
> but non-overlapping code (039 adds `topic`/`rotation` args, this plan
> changes only the `body` string), so ordering either way is safe, just
> wasteful to split into two edits of the same functions.

## Status

- **TODO** — filed 2026-07-19 from operator feedback watching ENG–FRA
  (World Cup) live on the new build.
- **Priority**: P3 · **Effort**: S · **Risk**: LOW (copy/formatting only,
  no `EventSignal`/wire/schema change — verified below).
- **Depends on**: none (037 already landed; 039 coordination is
  same-file-touch only, not a hard dependency — see gate above).
- **Review-plan pass (2026-07-19)**: the original filing assumed the
  fix was uniformly mechanical across goal/penalty/own-goal/red-card.
  Reading the actual parsing code found it isn't: cards are **already**
  self-describing (`last_card` already prefixes with ESPN's own
  `detail_type.text`, e.g. "Yellow Card — B. Saka 54'") — only the
  scoring-play path (`last_scoring_play`) discards that same field.
  The real fix is narrower and more specific than the original Scope
  implied; this pass also found real fixture text ("Goal", "Penalty -
  Scored") to test against, found zero fixture coverage for "own goal"
  (with a precedent for synthesizing it), corrected `MatchState` →
  `ScoreUpdate` in the Scope description (goals are `ScoreUpdate`, not
  `MatchState`), and flagged the plan's emoji proposal as a style
  choice with zero precedent elsewhere in the codebase, not a given.

## Problem

A live goal card renders title `fifa.world: ENG 1–0 FRA`, body
`D. Rice 3'`. The scoreline moving to 1–0 implies a goal, but the body
never says **what** the event was — operator "had to check what Declan
Rice did there." Nothing distinguishes a goal from a penalty or
own-goal — they'd all read as `<name> <minute>'`.

## Root cause (grounded, this review-plan pass)

Two sibling extraction blocks in `view()` (`poller.rs:180-266`) handle
this differently:

- **Cards already self-describe.** `last_card` (`poller.rs:229-247`)
  reads `d.detail_type.text` (ESPN's own play-type label — "Yellow
  Card", presumably "Red Card") and formats `"{kind} — {line}"`, e.g.
  `"Yellow Card — B. Saka 54'"` (`poller.rs:245`). This is already
  exactly the shape the operator wants for goals. **No card-side fix is
  needed for the "what happened" gap** — see "Deliberate scope
  narrowing" below for what IS still worth doing to cards.
- **Scoring plays throw the label away.** `last_scoring_play`
  (`poller.rs:223-228`) finds the matching detail the same way, but
  maps it through `detail_line()` (`poller.rs:166-178`) alone — which
  returns ONLY `"{athlete} {clock}"`, never touching
  `d.detail_type.text`. That field — which for a goal carries ESPN's
  own label ("Goal", "Penalty - Scored", presumably "Goal - Own Goal")
  — is read here (`d.detail_type` is in scope) and discarded. **This is
  the entire bug**: the data needed is already being parsed, just not
  kept.
- **Goals are `EventType::ScoreUpdate`, not `MatchState`.** The
  original filing's Scope header said "the `MatchState` → event
  title/body formatting" — that's the wrong `EventType` for the
  reported case. `diff_scoreboard` (`poller.rs:346-461`) emits goals as
  `EventType::ScoreUpdate` (`poller.rs:377`); only kickoff/half-time/
  full-time/card are `EventType::MatchState` (`poller.rs:388,398,408,425`).
  A fix aimed only at `MatchState` code would miss the actual complaint.

## Fixture evidence (this review-plan pass)

`rg -o '"text":"[^"]*"' src-tauri/tests/fixtures/*.json | sort -u`
against the five checked-in fixtures:

- `scoreboard-uefa.champions.json` contains real ESPN detail-type text:
  `"Goal"` (index 0 of the match's `details`) and `"Penalty - Scored"`
  (indices 3 and 8–14 — a penalty shootout). Both are real, unmodified
  ESPN label strings — no invented text needed for either — but the
  fixture's own scoring-play ordering means isolating the `"Goal"` case
  requires trimming the `details` list, not just diffing the fixture
  as-is; see Step 2 for the exact mechanics and why (`last_scoring_play`
  picks the *last* matching detail, which in the untouched fixture is
  a penalty, not the goal).
- **No fixture contains an own-goal or a real red card.** For red
  cards, `poller.rs:886-914`'s `red_card_emits_red_card_signal` already
  establishes the precedent for this exact situation: "none of the
  checked-in fixtures contain a real red card, so this synthesizes one
  by mutating a detail's structural booleans" — it clones the UCL
  fixture, sets `last_detail.red_card = true` and
  `last_detail.detail_type.text = "Red Card"` by hand. Step 2 follows
  this same pattern for an own-goal test case, using `"Goal - Own Goal"`
  as a documented best-guess for ESPN's real label (this plan's
  evidence stops at "no fixture has one" — it does not independently
  confirm ESPN's exact own-goal string, and Step 2 says explicitly not
  to spend effort trying to verify it further).

## Deliberate scope narrowing (this review-plan pass)

- **`EventSignal` does not change.** Every score change — goal,
  penalty, own-goal — already emits `EventSignal::Goal` unconditionally
  (`poller.rs:381`), and this plan does not add `Penalty`/`OwnGoal`
  variants (confirmed no such variants exist today —
  `event.rs:105-114` lists `Goal`/`RedCard`/`YellowCard`/`Kickoff`/
  `Halftime`/`Fulltime`/`Generic` only). The frontend's stamp lookup
  (`src/lib/presentation.ts:22-29`, `SIGNAL_STAMPS`) keys off
  `EventSignal` for a fixed generic corner-badge word ("Live" for any
  goal) — it does NOT render event-specific text, so this plan's body
  text is the only place the distinction will ever be visible, and
  leaving `EventSignal` alone is correct, not an oversight: the
  original filing's "no wire/schema change, no `SlotState` change"
  claim is confirmed true under this design.
- **The emoji prefixes (⚽/🟥) in the original filing have zero
  precedent anywhere in this codebase.** Checked: no emoji appears in
  any event body, the card labels ("Yellow Card — …"), or the
  frontend's stamp table (`presentation.ts`'s `SIGNAL_STAMPS` values
  are plain English: "Live", "Break", "Card", "Final", "Off"). This
  isn't a reason to reject emoji, but it IS a deliberate style choice
  the original filing made without checking, not an established
  convention to match. **Recommendation**: drop the emoji, match the
  existing card convention exactly (`"{Label} — {athlete} {clock}"`,
  no prefix glyph) — it's simpler, consistent with the one sibling case
  that already does this, and avoids introducing the first emoji in
  the codebase's card copy as a side effect of an unrelated bug fix. If
  the operator specifically wants the emoji, that's a one-line format-
  string change on top of Step 1 below — flag it as a question rather
  than assuming either way.

## Scope

**In scope**:
- `src-tauri/src/poller.rs`:
  - `last_scoring_play`'s construction (`poller.rs:223-228`) — change
    to capture `detail_type.text` alongside the athlete/clock line, the
    same way `last_card` already does (`poller.rs:235-247`). Extract a
    shared helper and call it from both `last_card` and
    `last_scoring_play`, so the two sites can't drift again — see
    Step 1 below for the exact helper shape (it needs one edge-case
    guard `last_card` doesn't, don't use a simpler sketch than the one
    in Step 1).
  - `MatchView`'s `last_scoring_play` field/usage
    (`poller.rs:159,223-228,263,372-375`) — the body source for the
    `ScoreUpdate` event (`poller.rs:372-383`) already reads
    `v.last_scoring_play` directly; once that field carries the label,
    no change is needed at the call site itself.
- `docs/TESTING_STRATEGY.md` §0 — bump the `poller` count for the new
  test(s).
- Tests: extend `mod tests` in `poller.rs` (`poller.rs:629+`) —
  see "Test plan" below.

**Out of scope**:
- `last_card`'s own formatting — already correct, do not touch (see
  "Root cause" above). Do not add an emoji prefix there either, per the
  scope-narrowing recommendation, unless the operator asks for it
  explicitly for both goal and card copy together.
- Kickoff/half-time/full-time bodies (`poller.rs:390,400,410`) — plain
  `"kickoff"`/`"half-time"`/`"full-time"` strings, already
  self-describing, unaffected by this plan.
- `EventSignal`, `EventType`, any wire/schema/`SlotState` field.
- Any frontend file — `stampFor`/`presentation.ts` needs no change
  (confirmed above).
- `src-tauri/src/rss_poller.rs` and any non-ESPN producer.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust suite | `cd src-tauri && cargo test --locked` | all pass; recount against `docs/TESTING_STRATEGY.md` §0 |
| Gates | `cargo clippy --locked --all-targets -- -D warnings && cargo fmt --check` | exit 0 |

## Git workflow

- Branch: `advisor/041-espn-card-copy` (or per operator dispatch).
- Commit style (from `git log`): lowercase `area: imperative summary`,
  e.g. `poller: label scoring plays with espn's own event-type text`.
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: carry the detail-type label into scoring-play bodies

In `poller.rs`, factor the "`{kind} — {line}`, or just `{kind}` if the
line is empty" formatting `last_card` already does (`poller.rs:235-247`)
into a small shared helper, and use it for BOTH `last_card` and
`last_scoring_play` (`poller.rs:223-228`) so a goal's body becomes
e.g. `"Goal — D. Rice 3'"` or `"Penalty - Scored — H. Kane 34'"` (ESPN's
own label text passed through verbatim — do not rewrite/shorten
"Penalty - Scored" unless you're deliberately choosing to, see the
emoji note above for the same kind of judgment call). Leave `last_card`
observably unchanged (it already produces this shape) — the point of
sharing the helper is to prevent future drift between the two sites,
not to change card behavior.

**Guard the case `last_card` can't hit but `last_scoring_play` can**:
`last_card`'s search already filters on `detail_type.text.contains("Card")`,
so `kind` is guaranteed non-empty there. `last_scoring_play`'s search
(`poller.rs:226`) only filters on `d.scoring_play` — `detail_type` is
`Option<SbDetailType>` (`poller.rs:83`), so a scoring-play detail
without a type label is possible in principle, even though neither
fixture exercises it. A naive `"{kind} — {line}"` helper would produce
a stray leading `"— D. Rice 3'"` in that case — worse than today's
plain `"D. Rice 3'"`. Write the helper so an empty `kind` falls back to
just `line` (not the reverse — `line` is never empty when a detail has
an athlete):

```rust
fn labeled_detail_line(kind: &str, d: &SbDetail) -> String {
    let line = detail_line(d);
    match (kind.is_empty(), line.is_empty()) {
        (true, _) => line,
        (false, true) => kind.to_string(),
        (false, false) => format!("{kind} — {line}"),
    }
}
```

**Verify**: `cargo build --locked` (from `src-tauri/`) → exit 0.

### Step 2: tests

Add to `poller.rs`'s `mod tests` (`poller.rs:629+`). **Read this whole
step before writing any test** — the UCL fixture's own match is already
`post`/final on first sighting (confirmed by the existing
`first_sighting_is_silent_and_final_matches_are_not_tracked` test,
`poller.rs:717-723`: `baseline(UCL)` returns an EMPTY snapshot for this
reason), so `baseline(UCL)` cannot be used as a starting point the way
`baseline(USA)` is used elsewhere — diffing against an empty snapshot
takes the `None` branch in `diff_scoreboard`'s match (`poller.rs:360-367`)
and emits nothing at all. The two existing UCL-fixture tests
(`new_card_emits_match_state_with_detail`, `poller.rs:865-884`, and
`red_card_emits_red_card_signal`, `poller.rs:886-914`) both work around
this the same way: hand-build a synthetic `prev: Snapshot` from
`view(&sb.events[0]).snap` with `state` forced to `"in"` (and whatever
field needs decrementing so the diff detects a change), then diff a
separately-parsed `live` copy (also `state` forced to `"in"`) against
it. Follow that exact pattern here, not `baseline`.

**Also read this before writing test 1**: the UCL fixture's `details`
array has scoring-play entries at multiple indices — index 0 is
`"Goal"`, and indices 3 and 8–14 are all `"Penalty - Scored"` (a
penalty shootout — verified by reading the fixture directly).
`last_scoring_play` (`poller.rs:223-228`) is
`details.iter().rev().find(|d| d.scoring_play)` — the LAST matching
entry in array order wins. **Diffing the untouched fixture yields
`"Penalty - Scored"`, not `"Goal"`** — the reverse of what you might
expect from "the fixture has a Goal detail." Test 2 (Penalty) can use
the fixture as-is; test 1 (Goal) must truncate `details` down to just
the index-0 entry first, or `.rev().find()` will still land on a later
penalty.

1. **Goal** — isolate the one `"Goal"` detail:
   ```rust
   let sb = parse_scoreboard(UCL).unwrap();
   let mut v_snap = view(&sb.events[0]).snap;
   v_snap.state = "in".to_string();
   v_snap.home_score -= 1; // "one goal ago"
   let mut snap = Snapshot::new();
   snap.insert(sb.events[0].id.clone(), v_snap);

   let mut live = parse_scoreboard(UCL).unwrap();
   live.events[0].status.status_type.state = "in".to_string();
   live.events[0].competitions[0].details.truncate(1); // keep only the "Goal" entry (index 0)

   let (events, _) = diff_scoreboard(&snap, &live, 8, "uefa.champions", Priority::High);
   assert_eq!(events.len(), 1);
   assert!(matches!(events[0].event_type, EventType::ScoreUpdate));
   assert!(events[0].payload.body.starts_with("Goal — "));
   ```
   (`home_score` here is PSG's, per the fixture — either team's score
   works for triggering the diff; the specific team doesn't matter for
   this test.)
2. **Penalty** — same shape, but do NOT truncate `details` (the last
   scoring-play entry in the untouched fixture already IS
   `"Penalty - Scored"`, so this is what you get for free):
   ```rust
   let sb = parse_scoreboard(UCL).unwrap();
   let mut v_snap = view(&sb.events[0]).snap;
   v_snap.state = "in".to_string();
   v_snap.home_score -= 1;
   let mut snap = Snapshot::new();
   snap.insert(sb.events[0].id.clone(), v_snap);

   let mut live = parse_scoreboard(UCL).unwrap();
   live.events[0].status.status_type.state = "in".to_string();

   let (events, _) = diff_scoreboard(&snap, &live, 8, "uefa.champions", Priority::High);
   assert_eq!(events.len(), 1);
   assert!(events[0].payload.body.starts_with("Penalty - Scored — "));
   ```
3. **Own goal** — synthesized (no fixture has one), following
   `red_card_emits_red_card_signal`'s exact technique of mutating one
   detail's structural fields by hand: same `prev`-construction as
   above, then on `live`, pick `details.last_mut()` (or any single
   entry — index doesn't matter once you're overwriting it) and set
   `scoring_play = true` and `detail_type` text to
   `"Goal - Own Goal"`. **This exact string is a documented best-guess,
   not a confirmed ESPN value** (no source available to this plan
   confirms it) — use it as-is, do not spend effort trying to verify it
   externally, and note the assumption in the commit message. This is
   the expected, planned outcome for this test, not a fallback for a
   failure case.
   ```rust
   let sb = parse_scoreboard(UCL).unwrap();
   let mut v_snap = view(&sb.events[0]).snap;
   v_snap.state = "in".to_string();
   v_snap.home_score -= 1;
   let mut snap = Snapshot::new();
   snap.insert(sb.events[0].id.clone(), v_snap);

   let mut live = parse_scoreboard(UCL).unwrap();
   live.events[0].status.status_type.state = "in".to_string();
   let last_detail = live.events[0].competitions[0]
       .details
       .last_mut()
       .expect("fixture has at least one detail");
   last_detail.scoring_play = true;
   if let Some(t) = last_detail.detail_type.as_mut() {
       t.text = "Goal - Own Goal".to_string();
   }

   let (events, _) = diff_scoreboard(&snap, &live, 8, "uefa.champions", Priority::High);
   assert_eq!(events.len(), 1);
   assert!(events[0].payload.body.starts_with("Goal - Own Goal — "));
   ```

**Verify**: `cargo test --locked poller::` → all pass, including the
three new cases; `cargo test --locked` (full) → all pass, totals match
the updated `docs/TESTING_STRATEGY.md` §0.

### Step 3: docs + status

Bump `docs/TESTING_STRATEGY.md` §0's `poller` count. Flip this plan's
`plans/README.md` row to DONE.

**Verify**: `cargo test --locked 2>&1 | grep "test result"` totals
match §0 exactly.

## Test plan

- New: the three cases in Step 2 (real-fixture goal, real-fixture
  penalty, synthesized own-goal).
- Unchanged: every existing card/kickoff/half-time/full-time test
  (`last_card`'s shape doesn't move) and `red_card_emits_red_card_signal`
  itself (the shared helper this plan introduces must produce the same
  output for that test's inputs — if it doesn't, the refactor changed
  card behavior by accident, which is out of scope). Specifically watch
  `score_delta_emits_one_score_update_and_nothing_for_unchanged`
  (`poller.rs:767-777`): it asserts `body == "goal"` (lowercase, the
  `diff_scoreboard`-level fallback for when the USA fixture's feed has
  no scoring-play detail at all, so `last_scoring_play` is `None`) —
  this plan's change only affects what `last_scoring_play` contains
  when a detail IS found; the `None` fallback path and its exact string
  are untouched. This test failing is the clearest signal the fallback
  path was accidentally touched.

## Done criteria

- [ ] `cd src-tauri && cargo test --locked` exits 0; totals match §0
- [ ] `cargo clippy --locked --all-targets -- -D warnings && cargo fmt --check` exit 0
- [ ] A goal event's body contains ESPN's own scoring-play label
      (`"Goal"`/`"Penalty - Scored"`/etc.), not just `"{athlete} {clock}"`
- [ ] `last_card`'s existing tests are unchanged and still pass (the
      shared-helper refactor is behavior-preserving for cards)
- [ ] No `EventSignal`, `EventType`, or `SlotState` field changed —
      `git diff -- src-tauri/src/event.rs` is empty
- [ ] `docs/TESTING_STRATEGY.md` §0 updated
- [ ] `plans/README.md` status row updated

## STOP conditions

(Note: the own-goal label is deliberately NOT a STOP condition — Step
2.3 already tells you exactly what string to use and why; use it
without further verification.)

- `poller.rs` dirty in the working tree at start → STOP and coordinate.
- The shared-helper refactor changes `red_card_emits_red_card_signal`'s
  or any other existing card test's expected output → STOP; that means
  the refactor isn't behavior-preserving for cards and needs rethinking,
  not a test-value update (the point of sharing the helper is that
  cards don't change).

## Maintenance notes

- ESPN's own `detail_type.text` is now the source of truth for
  scoring-play labels, matching how cards already work — a future new
  ESPN play-type label (if ESPN ever adds one) will "just work" without
  a code change, since nothing here matches on a closed set of known
  strings. If a future card wants a closed-set classification instead
  (e.g. to drive a new `EventSignal` variant), that's a bigger, separate
  change — this plan deliberately doesn't do that (see "Deliberate
  scope narrowing").
- If 039 lands after this plan, its `make_event`/`diff_scoreboard`
  signature changes (adding `topic`/rotation args) are orthogonal to
  the `body` string this plan touches — expect a clean textual merge,
  not a semantic conflict.
