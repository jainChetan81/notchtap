# Plan 044: a same-poll card must not un-retire a just-finished live-match card

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to
> the next step. If anything in "STOP conditions" occurs, stop and
> report — do not improvise. When done, update this plan's status row in
> `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat f2cbae6..HEAD -- src-tauri/src/poller.rs`
> If `poller.rs` changed since this plan was written, compare the
> "Current state" excerpts below against the live code before proceeding
> — this plan's fix is a one-line condition change at a specific site,
> and it depends on the exact emission order still holding.

## Status

- **Priority**: P1
- **Effort**: S
- **Risk**: LOW — a one-line, narrowly-scoped condition change inside an
  existing `if` block; no signature or type changes.
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `f2cbae6`, 2026-07-19
- **Review-plan pass (2026-07-20)**, run at HEAD `28c3d27` (zero drift on
  `poller.rs` since `f2cbae6` — `git diff --stat f2cbae6..HEAD --
  src-tauri/src/poller.rs` empty): independently re-verified every claim
  in this plan against live code (own read, plus a fresh-context
  subagent cold-read), specifically checking the two things most likely
  to be wrong in a plan like this — (1) that `queue.rs`'s Topic
  supersession really is an unconditional last-applied-wins with no
  priority/timestamp tiebreak (`apply_fresh_content`,
  `existing.rotation = fresh.rotation;` unconditional — confirmed), and
  (2) that the new test in Step 2 actually compiles and reproduces the
  bug by hand-tracing every field/method it touches against live struct
  definitions (confirmed: `home_cards.0 -= 1` on the UCL fixture's real
  `(2, 0)` doesn't underflow, `final_now && old.state != "post"` and the
  card condition both evaluate true pre-fix as claimed, exactly one
  event survives post-fix with the Cards cell intact in `meta.details`).
  Also independently confirmed the "out of scope: other emission sites
  unaffected" claim structurally, not just by trusting the plan: kickoff
  and full-time can't co-occur (mutually exclusive `state` values), and
  goal/kickoff/half-time are all emitted *before* full-time in code
  order, so any of them co-occurring with full-time already has
  full-time applied last (already covered green by
  `goal_and_full_time_in_one_poll_emit_in_order`/
  `live_card_on_goal_and_full_time_in_one_poll_share_meta`) — card is
  structurally the only site with this hazard. Zero issues found; no
  changes made to this plan. Ready to execute as written.

## Why this matters

`espn_live_card` (plan 039/042, opt-in, default `false`) turns a live
football match into one `Recurring` card that updates in place via Topic
supersession, instead of a burst of one-shot cards — the whole point
being that the full-time event, pushed as `RotationSpec::OneShot` on the
same Topic, cleanly retires the card when the match ends (per plan
031's design doc and plan 039's done-entry in `plans/README.md`).

That guarantee currently breaks whenever a card (yellow/red) is recorded
in the **same poll cycle** the match goes final — an entirely routine
football occurrence (a stoppage-time booking right around the final
whistle). `diff_scoreboard` pushes the full-time event (`OneShot`)
*before* the card event (`Recurring`), both on the same Topic. The
queue's Topic-supersession applies events in order, and the second
(`Recurring`) event's rotation unconditionally overwrites the first's —
so the card that was supposed to retire the match instead flips back to
`Recurring`. Once that happens, nothing can ever fix it: `diff_scoreboard`
also evicts a final match from its tracked `Snapshot` in the same pass,
so no future poll emits another same-Topic event to retire the card
properly. The live-match card is left permanently cycling through
rotation until a manual dismiss — every subsequent match's card is stuck
behind a phantom occupant of the single Slot.

## Current state

- `src-tauri/src/poller.rs` — `diff_scoreboard` (signature at line 461),
  inside the `Some(old) => { ... }` match arm for a previously-tracked
  match. The relevant block, as of `f2cbae6` (line numbers exact at this
  commit; locate by content if they've shifted):

  ```rust
  // poller.rs:572-584 — full-time event, OneShot on the match's Topic
  if final_now && old.state != "post" {
      let mut event = make_event(
          EventType::MatchState,
          title.clone(),
          "full-time".to_string(),
          ttl_secs,
          EventSignal::Fulltime,
          priority,
          card_topic(&topic, true),
      );
      event.meta = meta.clone();
      out.push(event);
  }

  // poller.rs:586-604 — card event, Recurring on the SAME Topic,
  // pushed unconditionally whenever the card count increased this poll
  if v.snap.total_cards() > old.total_cards() {
      let body = v.last_card.clone().unwrap_or_else(|| "card".to_string());
      let signal = if v.last_card_is_red {
          EventSignal::RedCard
      } else {
          EventSignal::YellowCard
      };
      let mut event = make_event(
          EventType::MatchState,
          title,
          body,
          ttl_secs,
          signal,
          priority,
          card_topic(&topic, false),
      );
      event.meta = meta.clone();
      out.push(event);
  }
  ```

  (This excerpt is byte-identical to the live file at `f2cbae6` — the
  only difference from the raw source is that the two
  `card_topic(&topic, true/false)` call sites carry no inline comment in
  the file itself; the mapping — `card_topic(&topic, true)` produces
  `CardTopic::FullTime` → `RotationSpec::OneShot`, `card_topic(&topic,
  false)` produces `CardTopic::Live` → `RotationSpec::Recurring` — is
  explained in prose just below instead, not quoted as a code comment.)

  `final_now` is defined earlier in the same function: `let final_now =
  v.snap.state == "post";` (line 474).

- `card_topic` (line 394-400) — the mapping from `(topic, is_full_time)`
  to a `CardTopic`, which callers convert to a `RotationSpec`:

  ```rust
  fn card_topic(topic: &Option<String>, is_full_time: bool) -> CardTopic {
      match (topic, is_full_time) {
          (Some(t), false) => CardTopic::Live(t.clone()),
          (Some(t), true) => CardTopic::FullTime(t.clone()),
          (None, _) => CardTopic::Off,
      }
  }
  ```

  (`CardTopic::Live` maps to `RotationSpec::Recurring`, `CardTopic::FullTime`
  maps to `RotationSpec::OneShot` — confirm the exact mapping by reading
  wherever `CardTopic` is consumed in `make_event`/its conversion, but do
  not change that mapping; this plan only reorders/gates emission.)

- The important detail that makes the safe fix cheap: `meta` (built once
  per match, lines 490-526, above both blocks quoted here) **already**
  bakes the latest Cards detail line into every event pushed for this
  match this poll — `v.snap.home_cards`/`v.snap.away_cards` reflect the
  fully-updated card counts regardless of which event carries them. So a
  card recorded the same poll the match ends is **not lost** if it's
  folded silently into the full-time card instead of pushed as its own
  trailing event — the full-time card's own `meta.details` "Cards" line
  already shows it.

- `src-tauri/src/queue.rs` — `apply_fresh_content` (the supersede path)
  unconditionally overwrites `existing.rotation = fresh.rotation` on a
  Topic match; this is correct, general behavior for the supersession
  primitive — this plan does not touch `queue.rs` at all. The bug is
  entirely in what `poller.rs` chooses to emit, not in how the queue
  applies it.

- Existing tests covering the same-poll-ordering pattern (goal +
  full-time), which this plan's fix must NOT break:
  - `goal_and_full_time_in_one_poll_emit_in_order` (`poller.rs:1188`,
    `espn_live_card = false`) — asserts 2 events, goal then full-time.
  - `live_card_on_goal_and_full_time_in_one_poll_share_meta`
    (`poller.rs:1329`, `espn_live_card = true`) — asserts 2 events
    (`ScoreUpdate` then `"full-time"`), same `meta` on both.
  - Neither test constructs a card alongside full-time in the same
    poll — that combination is untested today, which is why this gap
    shipped unnoticed.

## Commands you will need

| Purpose | Command (from `src-tauri/`) | Expected on success |
|---|---|---|
| Build | `cargo build --locked` | exit 0, no warnings |
| Rust tests | `cargo test --locked` | all pass; §0 count updated for the new test |
| Lint/format | `cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Targeted test | `cargo test --locked poller::` | all `poller` tests pass |

## Scope

**In scope** (the only files you should modify):
- `src-tauri/src/poller.rs` — the card-event `if` condition, and its
  `#[cfg(test)] mod tests` (add one regression test).
- `docs/TESTING_STRATEGY.md` §0 — bump the `poller` sub-count and the
  rust total if you add a test.

**Out of scope** (do NOT touch, even though they look related):
- `src-tauri/src/queue.rs` — `apply_fresh_content`/`supersede_if_topic_matches`
  are correct as-is; the fix belongs entirely in what `poller.rs` emits,
  not in how the queue applies a supersede.
- `card_topic`'s `CardTopic`→`RotationSpec` mapping — do not change which
  `RotationSpec` variant `Live`/`FullTime` map to.
- Any other emission site in `diff_scoreboard` (goal, kickoff, half-time)
  — unaffected by this fix.
- `src-tauri/src/engine.rs` — unaffected; this is a pure `poller.rs`
  change to what events are constructed, not how they're accepted.

## Git workflow

- Branch: `advisor/044-live-card-fulltime-supersession-fix` (or work
  directly if the operator dispatched you that way).
- Commit style (from `git log`): lowercase `area: imperative summary`,
  e.g. `poller: fold a same-poll card into the full-time event, don't emit it separately`.
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Gate the card event on `!final_now`

In `diff_scoreboard`'s card-event block (`poller.rs:586`), add
`&& !final_now` to the condition so a card discovered in the same poll
the match goes final is not emitted as its own trailing event — it's
already reflected in the full-time event's `meta.details` "Cards" line
(built once per match above both blocks, unaffected by this change):

```rust
if v.snap.total_cards() > old.total_cards() && !final_now {
    // ... unchanged body
}
```

This is the entire code change. Do not reorder the full-time and card
blocks instead (a reorder is a viable alternative but changes which
event number in the output vector carries the meta detail in existing
tests' assertions — the additive gate is smaller and leaves every
untouched scenario's event count and ordering exactly as today).

**Verify**: `cargo build --locked` (from `src-tauri/`) → exit 0, no
warnings.

### Step 2: Add the regression test

Add a test proving the exact bug scenario: a match records a card and
goes final in the same poll, with `espn_live_card = true`.

**Do not use the `USA` fixture for this test.** The plan's original
draft suggested modeling the poll-sequence harness on
`live_card_on_goal_and_full_time_in_one_poll_share_meta`
(`poller.rs:1329`, which uses `baseline(USA)`) while reusing the
card-detail-construction technique from `red_card_emits_red_card_signal`/
`ucl_fixture_cards_bucket_per_side_and_color` (which mutate an
already-populated `UCL`-fixture detail via `.details.last_mut()`). That
combination does not work: `src-tauri/tests/fixtures/scoreboard-usa.1.json`
has zero `details` entries for every event (confirmed by parsing the raw
JSON), so `.last_mut()` returns `None` and `.expect(...)` panics.
`SbDetail` also does not derive `Default` (unlike `SbTeam`/`SbDetailType`,
which do) — confirmed at `poller.rs:85`, `#[derive(Debug, Deserialize)]`
only — so there is no `..Default::default()` shortcut to hand-construct
one either.

Instead, reuse `red_card_emits_red_card_signal`'s exact technique
(`poller.rs:1068-1097`) end to end, on the `UCL` fixture, which is
already `state == "post"` with 6 real card details (2 home yellow, 4
away yellow — the same ground truth
`ucl_fixture_cards_bucket_per_side_and_color`, `poller.rs:1100`, pins).
Build a synthetic **old** state that's the fixture's real final state
minus one card, with `state` forced back to `"in"` — then diff against
the **unmodified** parsed `UCL` scoreboard (no mutation needed on it at
all, since it's already final with the full real card set):

```rust
#[test]
fn card_recorded_same_poll_as_fulltime_does_not_emit_separately_and_stays_in_meta() {
    let live = parse_scoreboard(UCL).unwrap();
    let mut old_view = view(&live.events[0]).snap;
    old_view.state = "in".to_string();
    old_view.home_cards.0 -= 1; // one home yellow "not yet recorded" in `old`
    let mut snap = Snapshot::new();
    snap.insert(live.events[0].id.clone(), old_view);

    let (events, _) = diff_scoreboard(&snap, &live, 8, "uefa.champions", Priority::High, true);

    assert_eq!(
        events.len(),
        1,
        "a card recorded the same poll as full-time must not emit as a separate event"
    );
    assert_eq!(events[0].payload.body, "full-time");
    assert!(
        events[0].meta.details.iter().any(|d| d.label == "Cards"),
        "the card must still be reflected in the full-time event's meta"
    );
}
```

Why this reproduces the bug precisely: `old_view.state = "in"` makes
`final_now && old.state != "post"` true (full-time fires); `live`'s real
card count (6) is greater than `old`'s manufactured count (5, after the
`-= 1`), so pre-fix the card branch would also fire — reproducing "both
fire in the same poll" — and post-fix (Step 1's `&& !final_now` gate)
the card branch is correctly suppressed, leaving exactly the one
full-time event with the card already reflected in its `meta.details`
"Cards" line (built from `v.snap.home_cards`/`away_cards`, i.e. the
*current*/`live` counts, not `old`'s).

**Verify**: `cargo test --locked poller::` (from `src-tauri/`) → all
pass, including the new test, and
`goal_and_full_time_in_one_poll_emit_in_order` +
`live_card_on_goal_and_full_time_in_one_poll_share_meta` are unaffected
(they don't touch cards, so Step 1's added condition is a no-op for
them — confirm by running them explicitly if you want extra confidence:
`cargo test --locked goal_and_full_time_in_one_poll_emit_in_order` and
`cargo test --locked live_card_on_goal_and_full_time_in_one_poll_share_meta`).
Also re-run `red_card_emits_red_card_signal` explicitly — the new test
uses the same UCL-fixture-mutation technique on the same match, so
confirm the two don't interfere (they're independent `#[test]` functions
with their own local `snap`/`live` bindings, so they shouldn't, but
verify rather than assume).

### Step 3: Full suite + lint + docs

Run the full gate and reconcile the test count.

**Verify**: `cargo test --locked` → all pass. `cargo fmt --check &&
cargo clippy --locked --all-targets -- -D warnings` → exit 0.

Update `docs/TESTING_STRATEGY.md` §0's rust row: bump the `poller`
sub-count by 1 and the total by 1 — but **recount from the actual
`cargo test --locked` summary line**, don't just add 1 to the numbers
printed in this plan (they drift with every merged plan; this plan was
written against poller 30 / total 326+3, per the doc as of `f2cbae6`).

## Test plan

- New test: `card_recorded_same_poll_as_fulltime_does_not_emit_separately_and_stays_in_meta`
  in `src-tauri/src/poller.rs`'s `#[cfg(test)] mod tests`, built on the
  `UCL` fixture using `red_card_emits_red_card_signal`'s (line 1068)
  synthetic-old-state technique — see Step 2 for the exact code. Covers
  the exact regression this plan fixes.
- Existing tests to re-run explicitly as a safety net (all currently
  green, must stay green): `goal_and_full_time_in_one_poll_emit_in_order`,
  `live_card_on_goal_and_full_time_in_one_poll_share_meta`,
  `ucl_fixture_cards_bucket_per_side_and_color`,
  `red_card_emits_red_card_signal` (the last one shares a fixture and
  technique with the new test — confirm it still passes unmodified).
- Verification: `cargo test --locked` (from `src-tauri/`) → all pass,
  including the new test.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `cargo build --locked` exits 0
- [ ] `cargo test --locked` exits 0; the new regression test exists and
      passes
- [ ] `cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings`
      exits 0
- [ ] `rg -n "total_cards\(\) > old.total_cards\(\) && !final_now" src-tauri/src/poller.rs`
      shows exactly 1 match (note: a bare `rg -n "v.snap.total_cards\(\)
      > old.total_cards\(\)"`, without the trailing `&& !final_now`, is
      NOT a valid done-check — it matches the pre-fix line too, since
      it's a substring match that doesn't require the new condition to
      be present; don't use it to confirm this step)
- [ ] No files outside the in-scope list are modified (`git status`)
- [ ] `docs/TESTING_STRATEGY.md` §0 counts reconciled against a live
      `cargo test --locked` run
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- The code at `poller.rs`'s card/full-time blocks doesn't match the
  excerpts above (the codebase has drifted since `f2cbae6`) — re-read
  the live function in full before adapting the fix.
- The drift check shows `poller.rs` changed and the new code no longer
  pushes full-time before the card event, or no longer shares a single
  `meta` across both — the fix's premise (fold into an already-correct
  meta) may no longer hold; report instead of improvising a different
  fix.
- Adding `&& !final_now` causes `live_card_cycle_collapses_to_one_slot_and_still_fans_out`
  (`poller.rs:1345`, the end-to-end Engine test) to fail in a way not
  explained by this plan's intended behavior change — that test doesn't
  construct a same-poll card+full-time scenario today, so it should be
  unaffected, but if it fails, don't force it green by weakening the
  assertion; report what changed.

## Maintenance notes

- This is the first bug found in the Topic-supersession machinery since
  it got its first production producer (plan 039). The general lesson
  for future producers of `Recurring`/Topic events: when two events for
  the same Topic can be emitted in the same batch, whichever is emitted
  **last** wins the queue's rotation state — order matters, and "the
  terminal event should be emitted last" is a rule worth stating
  explicitly if a future spike (see plan 053, if selected) generalizes
  Topic supersession to other sources.
- If plan 043 (richer live-match event coverage — fouls, offside, subs)
  is ever executed, it will add more per-match event kinds that could
  interact with the same same-poll-ordering hazard this plan fixes for
  cards. Whoever builds that plan should re-read this one's "Why this
  matters" section and check whether any new event kind needs the same
  `!final_now` gate or an equivalent.
- A reviewer should scrutinize: that the fix is the narrow additive
  gate (not a block reorder, which would be a larger and riskier diff),
  that the new test actually reproduces same-poll card+full-time (not
  just full-time alone, which was already tested), and that the card's
  information is provably still present in `meta.details` post-fix (the
  whole point is "fold in," not "silently drop").
