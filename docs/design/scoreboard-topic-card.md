# Design spike: a live-match scoreboard card on the Topic/Recurring engine

Status: SPIKE — design only, no code changes. Plan 031.
Researched against commit `339156a` (branch `exec/031-scoreboard-topic-spike`).
All `file:line` citations below are valid at that commit; re-confirm with
`rg`/`grep` before relying on them if this doc is read later — the plan
that dispatched this spike flagged that its own line numbers had already
gone stale once (queue.rs grew ~600 lines from plan 033 alone) between
being planned and being executed.

## Why (one paragraph, for anyone skimming)

The queue's Topic/supersession machinery — `supersede_if_topic_matches`,
the capped extension on a Visible supersede, `Recurring` requeue in
`tick()` — is fully built and fully tested, and used by **zero**
production producers. Every real producer (`poller.rs`, `rss_poller.rs`,
`http.rs`, `settings.rs`) hardcodes `topic: None`; nothing in the tree
constructs `RotationSpec::Recurring` outside `queue.rs`'s own test
module and one `lib.rs` hotkey test. The natural first user is staring
at the code: `poller.rs`'s ESPN diff turns one live match into a burst
of one-shot cards (kickoff, three goals, a card, half-time, full-time —
each a separate item marching through the single Slot) instead of one
card that updates in place. This doc lays out what adopting the
machinery for that would look like, so the maintainer can approve or
kill it cheaply. It does not implement anything.

**This was a recorded choice, not an oversight.** `poller.rs:314-317`
says the poller "doesn't need the queue's topic supersession mechanism
in this pass" (the field itself, `topic: None`, is the next line,
`poller.rs:318`) — this doc is about what changes when that pass ends,
not about a bug being found.

## Confirmed grounding (re-verified against 339156a, not trusted from the plan)

- `grep -rn "topic: Some\|RotationSpec::Recurring" src-tauri/src --include='*.rs'`
  — every hit is in a test module (`queue.rs` tests, the plan-022
  proptest harness, `lib.rs`'s `skip_current_requeues_recurring_and_promotes_next`,
  the `event.rs` enum, and `poller.rs:305`'s comment). Confirmed
  independently: `http.rs`, `settings.rs`, `rss_poller.rs`, and
  `poller.rs`'s own `make_event` (`poller.rs:318`) all set `topic: None`,
  every construction site grep-checked directly.
- All 63 `queue::` tests pass at this commit (`cargo test --locked queue::`,
  run for understanding, not as a gate — this is a docs-only spike).
- No recorded *rejection* of this feature exists. `plans/README.md:296-302`
  records it as a "direction surfaced ... not selected for plans" that
  became this spike plan (031) and its sibling (030) — deferred, not
  killed.
- Plan 037 (the Engine — `plans/037-engine-propagation-module(done).md`)
  has since landed (merged 2026-07-19 as `src-tauri/src/engine.rs`):
  every Slot mutation now flows through the Engine, and `CONTEXT.md`'s
  **Engine** entry describes the guarantee as structural, not a
  convention. This design was written against the pre-037 code paths
  (`queue.rs`, `poller.rs`); the eventual build plan re-grounds against
  the Engine module, but the semantics below (topic identity, rotation
  kind, supersede/requeue behavior) carry over unchanged — 037 was a
  propagation-mechanism refactor, not a semantics change.

---

## 1. Topic identity

**RECOMMENDATION**: `format!("espn:{league}:{match_id}")`, e.g.
`espn:eng.1:401584543`, built once per `SbEvent` and threaded into
every `Event` the diff emits for that match. `league` is the slug
already used to key `HashMap<String, Snapshot>` in `poller.rs:604`
(`snapshots.entry(league.clone())`), and `match_id` is `SbEvent.id`
(`poller.rs:31`, `pub id: String`) — ESPN's own event id, already the
key of the per-league `Snapshot` map (`Snapshot = HashMap<String,
MatchSnapshot>`, `poller.rs:129`; `next.insert(v.id.to_string(), ...)`
at `poller.rs:368`/`438`). Both pieces are already threaded through
`diff_scoreboard`'s signature (`league: &str` at `poller.rs:353`; `v.id`
available inside the loop at `poller.rs:360`/`363`), so no new plumbing
is needed to compute the key — only to pass it into `make_event`.

Namespacing by league matters: ESPN match ids are not guaranteed
globally unique across leagues/sports (nothing in the fetched payload
promises it), and the poller already treats leagues as separate
snapshot namespaces for exactly this reason. Skipping the `espn:`
prefix and league segment risks an accidental Topic collision between
two competitions' matches that happen to reuse an id from different
providers if a second scoreboard source is ever added — the prefix
future-proofs for that at zero cost today.

**Collision/reuse across days**: a match id reappearing after eviction
(the match was evicted from `Snapshot` after
`ABSENT_POLLS_BEFORE_EVICTION` consecutive misses, `poller.rs:157`, or
after going final) is *not* a collision to guard against — it is
functionally a new match by the time it recurs (ESPN does not reuse
event ids for a *different* fixture; a reappearing id after eviction
would mean the same fixture resumed, e.g. after a postponement-and-
reschedule that keeps the same id). Because `diff_scoreboard`'s
`None => { silent baseline }` branch (`poller.rs:364-369`) treats a
freshly-seen id (including a re-seen one, since the old snapshot entry
is gone) as a new baseline with no emitted event, a Topic reuse in this
case is *silent* the same way first sighting always is — the queue
never even sees a stale-Topic collision because the poller doesn't
emit anything until the next real delta, which naturally carries the
same Topic and correctly starts a fresh card (any old card under that
Topic has long since rotated out on its own bounded lifecycle — see
§3).

**REJECTED ALTERNATIVE**: use ESPN's match id alone (`match_id` with no
`espn:{league}:` prefix). Simpler, and today's snapshot map is already
double-keyed by league so this isn't a correctness gap *today* — but it
silently assumes single-provider, single-namespace match ids forever.
Rejected because the prefix costs nothing (it's a `format!` at
construction) and buys real headroom if a second live-score provider is
ever added (a stated non-goal today, but not one this doc should
foreclose for a one-line cost).

## 2. Which events carry the Topic

**RECOMMENDATION**: every `ScoreUpdate` and `MatchState` event emitted
for the same match shares one Topic — kickoff, goals, cards, half-time,
full-time all supersede the same card, "one card per match" for the
whole 90+ minutes. This is what `diff_scoreboard`'s five emission call
sites (`poller.rs:379` ScoreUpdate, `:390` kickoff, `:400` half-time,
`:410` full-time, `:427` card) would all pass the same computed Topic
string into `make_event`.

**Payload on each delta**: `apply_fresh_content`
(`queue.rs:519-524`) replaces `payload` (title *and* body), `priority`,
`rotation`, and `signal` wholesale on every supersede — it does **not**
merge or append. Concretely: `title` is already delta-independent
today (`matchup()`, `poller.rs:283-295`, always renders
`"{league}: {away} {away_score}–{home_score} {home}"` from current
state — the same string a fresh goal event and a fresh card event both
produce for the same match), so the title staying the matchup line
falls out for free with zero special-casing. `body` is where the loss
is real: a `"2-1"`-flavored ScoreUpdate superseding a
`"K. Havertz 6'"` MatchState card line *replaces* it — the scorer
detail from the previous delta is gone the instant the next delta
lands, visible only for whatever fraction of its rotation window
elapsed before the next event. **This doc's position: that's an
acceptable trade for this spike, not a defect to design around.** The
overlay's job (per `CONTEXT.md`'s **Notification** entry) is "what's
happening now," not a running log — a full delta history is exactly
the "what did I miss" history surface `plans/README.md:298-302` already
flags as a *separate*, unplanned, weaker-grounded direction. Trying to
solve both in this card conflates two different UI ambitions.

**REJECTED ALTERNATIVE 1** — score-only Topic sharing (ScoreUpdate
events share a Topic; each MatchState event is its own one-shot,
Topic-less card as today). Preserves kickoff/half-time/full-time/card
moments as distinct pop-up beats, which arguably matters more for a
red card than a rolling scoreline does. Rejected because it produces
*two concurrent competing identities for the same match* — a Recurring
scoreline card cycling in one Priority tier's Waiting line, plus a
stream of one-shot MatchState cards jumping in front of or behind it —
which is a materially harder rotation-order story to reason about (and
to explain to the maintainer) than "one match, one card," for a
half-solution that still loses the scorer-line detail on every goal
supersede anyway (the goal ScoreUpdate would still blow away the
previous ScoreUpdate's scorer detail).

**REJECTED ALTERNATIVE 2** — running scoreline body (concatenate
deltas: `"2-1 (Havertz 6', Saka 54')"`, growing per delta). Preserves
history without a separate surface. Rejected: unbounded growth over a
90-minute match with many cards/goals eventually produces a body that
doesn't fit the overlay's fixed card geometry (`prototype/status-rail.html`
/ the Status Rail redesign is explicitly not a scrolling-log layout),
and it requires `apply_fresh_content` to know the event's own history —
a stateful merge function, a structural change to a function whose
entire value today (per its own doc comment, `queue.rs:513-518`) is
being "the one place a superseding event's content lands," applied
identically regardless of what kind of Topic it is.

## 3. Rotation kind — THE decision

**RECOMMENDATION**: `RotationSpec::Recurring { display_secs }` for the
duration of a live match, ending at full-time.

This is explicitly the decision the plan calls out as consequential: a
`Recurring` match card occupies a Waiting-tier slot in a permanent
rotate-back-in loop for 90+ minutes (`rotate_out_if_elapsed`,
`queue.rs:249-262`, pushes a `Recurring` item to the back of its own
tier's `VecDeque` — `queue.rs:258-261` — every single time its
rotation window elapses, over and over, for the life of the match). In
the single-slot overlay (`CONTEXT.md`'s **Slot**: "there is never more
than one Notification on screen at a time") this changes the whole
rail's felt cadence during any live match: instead of a scoreboard item
appearing once and going away, one is *always* somewhere in the
rotation, re-earning its turn every cycle, alongside news and cmux
items.

**Modeling the ending** (`CONTEXT.md`'s **Recurring**: "bounded by
supersession or the underlying state naturally ending, not a clock" —
this is the load-bearing clause this section has to satisfy). Full-time
does **not** self-terminate the card: `diff_scoreboard`'s eviction
(`poller.rs:444-462`, and the `!final_now` guard at `:437`/`:368`) only
removes the match from the *poller's own* `Snapshot` — it never touches
the *queue*. A `Recurring` item, once enqueued, keeps requeuing to the
back of its tier forever unless something else dequeues it; nothing in
`queue.rs` currently does that autonomously (no "N cycles and then
drop" counter exists — `rotate_out_if_elapsed` is unconditional for
`Recurring`). Two ways to close that gap, and this doc picks one:

- **(a)** the full-time `MatchState` event ("full-time",
  `poller.rs:409-418`) is emitted as a **`OneShot`-with-the-same-Topic**
  event, not `Recurring`. Because `supersede_if_topic_matches` treats
  content and `rotation` identically regardless of kind
  (`apply_fresh_content` copies `existing.rotation = fresh.rotation`
  unconditionally, `queue.rs:522`), a fresh `OneShot` supersede on a
  currently-`Recurring` Topic item **flips that item to `OneShot`** on
  its next promotion — its very next `rotate_out_if_elapsed` then drops
  it instead of requeuing (`queue.rs:258`'s `if let RotationSpec::
  Recurring` check reads the *current*, just-superseded `rotation`
  field, not the original). This is the existing machinery doing
  exactly the right thing with a one-line construction rule ("the
  full-time event is the one MatchState variant that doesn't set
  `Recurring`") and no queue changes at all.
- **(b)** a queue-level "drop this Topic" primitive. Rejected below.

**RECOMMENDATION detail**: (a). The full-time event supersedes the live
card one last time with `rotation: RotationSpec::OneShot { ttl_secs }`
(reusing the existing `ttl_secs` config knob), so the card's very last
turn shows "full-time" and then drops for good — no new queue API, no
new `Event` field, exactly the "bounded ... by the underlying state
naturally ending" language in `CONTEXT.md` realized literally: the
underlying state (the match) ending is what ends the Rotation kind.

One edge this doc flags rather than resolves: if the full-time event
lands while the card is *Waiting* (not Visible) — e.g. a lower-priority
match's card was in the Medium tier behind other items — the same-tier
supersede path (`queue.rs:200-202`) applies the `OneShot` flip in
place, so it still resolves correctly on its *own* eventual promotion.
No gap there.

**REJECTED ALTERNATIVE 1** — `OneShot`-with-Topic for the whole match
(never `Recurring`; every delta supersedes in place, but the item's
own natural TTL still governs when it ages fully out and, per
`extension_secs_resets_on_next_promotion`, `queue.rs:962-986`, an
un-superseded `OneShot`-with-Topic item simply vanishes once its window
elapses — nothing requeues it). This is the plan's own named
alternative ("OneShot-with-topic (card updates in place but still ages
out)"). It sidesteps the whole "how does it end" problem for free
(a `OneShot` item that goes untouched for `ttl_secs` just rotates out,
exactly like today) and never monopolizes a Waiting-tier slot for 90
minutes. **Rejected for this recommendation, but flagged as the safer
fallback (see Open Questions, §10)**: its failure mode is silent card
death mid-match — during a lull with no goals/cards/state changes for
longer than `ttl_secs` (the default is 8s, `config.rs`'s
`default_espn_ttl_secs`), the card simply disappears from rotation
entirely and does not reappear until the next delta re-enqueues it as
a *brand-new* card (a fresh promotion, losing whatever Waiting position
it had earned). `Recurring` avoids that: the card keeps re-earning its
turn in rotation even through a scoreless spell, which is the entire
point of "live match, ongoing state" as opposed to "here's a thing
that happened."

**REJECTED ALTERNATIVE 2** — a queue-level explicit "retire this Topic"
primitive (e.g. `queue.retire_topic(&str)`, called by the poller when a
match goes final, dequeuing/dropping the item outright regardless of
whether it's Visible or Waiting). More explicit than option (a)'s
"one-shot flip," and doesn't rely on the reader knowing that
`rotation` is one of the fields a supersede overwrites. Rejected
because it is new queue surface area (a new public method, new test
coverage, a new way for a caller to remove an item outside the
tick/dismiss/skip vocabulary `CONTEXT.md` already documents) to
express something the existing supersede-then-natural-rotation-out
path already expresses with zero new code — violates the doc's own
framing that "the machinery was designed for this" (§7).

## 4. Tier interplay

**Priority default**: `espn_priority` defaults `High`
(`config.rs:153`/`Config::default_espn_priority`), unchanged by this
design — a live match card is exactly the marquee-priority content the
default was chosen for.

**Expand semantics** (plan 033, now the locked, universal behavior —
`CONTEXT.md:68-74`): *every* Promotion starts Expanded and
auto-retracts at half the base Rotation window
(`set_expanded_for_promotion`, `queue.rs:283-287`; `retract_if_elapsed`,
`queue.rs:231-247`), independent of Priority. Plan 008's original
High-only auto-expand rule is now pure history — nothing in the
current code branches on Priority for expand, so a `Recurring` match
card is not special-cased relative to any other item on this axis: it
expands and half-window-retracts **on every single re-promotion**, for
the life of the match. At an 8s base `espn_ttl_secs`, retract fires at
~4s into each cycle — a High-priority live card visibly flicking
grown→collapsed roughly every 4–8 seconds, repeated for 90+ minutes.
This doc takes the position that this is *correct but conspicuous*:
it's the same rule every other item gets, but no other item currently
cycles through it hundreds of times in a row, so if it reads as
distracting on real hardware, that is grounds to revisit the *general*
auto-retract cadence (an ARCHITECTURE.md-level call), not to carve a
Recurring-specific exception into the expand logic — a special case
here would be the first Priority/kind-conditional branch in expand
semantics since plan 008 was deliberately generalized away in plan 033.

**Queue-slider interplay** (plan 033's batch counters,
`queue.rs:66-72`/`:480-510`): plan 033's own maintenance note
(`plans/033-queue-slider-and-expand-all(done).md:244-249`) explicitly defers this exact
question to this spike: *"If a future evergreen/Recurring producer
lands (plan 031 spike), its requeue-to-back is not a completion
either — revisit the counter rule then."* Traced against the live
code: `rotate_out_if_elapsed` increments `batch_done` unconditionally
(`queue.rs:257`) *before* checking whether the item is `Recurring`
and requeuing it (`:258-261`) — so today, a `Recurring` cycle *does*
count as a "done" for slider purposes, contradicting plan 033's stated
intent that it shouldn't. This is directly exercised by
`batch_done_caps_at_total_minus_one_while_an_item_is_visible`
(`queue.rs:1781-`), whose own comment says "a Recurring item completes
a turn every time it rotates out, but re-enters waiting — over a long
batch its completions can outnumber the batch size... done caps at
total - 1 while anything is visible." The cap keeps the *current*
segment lit, but does nothing about `batch_total` staying frozen: a
`Recurring` item never lets the queue reach the idle state that resets
the counters (`reset_batch_if_idle`, `queue.rs:437-442`, requires
`visible.is_none() && all_tiers_empty()` — a cycling `Recurring` item
is *always* in one of those two places), so once a batch starts with
the live card in it, the slider is pinned at essentially "N-1 of N"
(one segment shy of finished) for as long as the match runs, even as
genuinely new items (news, cmux) join and complete around it.

**RECOMMENDATION**: fix the batch-counter semantics as part of this
feature's build, not defer it further. Concretely: `rotate_out_if_elapsed`
should not increment `batch_done` for a `Recurring` item's own requeue
(only for a `OneShot` drop, a `dismiss_visible`, or a `skip_visible`'s
`OneShot` drop) — matching plan 033's stated intent, not today's
accidental behavior. This is a one-line, well-scoped `queue.rs` change
(move or guard the `self.batch_done += 1` at `queue.rs:257`) that a
build plan should make explicit as its own step, verified against a new
test asserting a `Recurring` cycle leaves `batch_done` unchanged. Until
that lands, the slider will visibly misbehave (stuck near "complete")
for the entire duration of any live match — this is a **build blocker
for this feature specifically**, not a pre-existing bug this spike is
introducing scope creep to fix.

**REJECTED ALTERNATIVE**: ship the feature first, leave the
`batch_done` discrepancy as documented tech debt (a follow-up note,
same shape as plan 033's own deferral). Rejected: plan 033 already
deferred this exact question once, explicitly, to "the spike" (this
doc) — deferring it a second time to "the build" produces a shipped
feature whose slider is visibly broken (pinned near "complete") for
the entire duration of every live match, which is a worse first
impression than a slightly larger build estimate. The fix is small
enough (§9) that bundling it costs little.

**Rotation-order / per-source priority mitigation** (v6): rotation
order (`queue.rs:74-78`, `:306-323`) only breaks ties *within* a
Priority tier and never overrides Priority itself
(`rotation_order_only_breaks_ties_within_a_tier_not_across_tiers`, an
existing passing test) — so a lower-priority `Football` source
configuration (dropping `espn_priority` to `Medium`/`Low` via the
existing per-source config knob) is the correct lever if a monopolizing
live card fights other High-tier content too aggressively; it needs no
new mechanism, just an operator config change, which is already the
documented purpose of `espn_priority` (`config.rs:22`).

## 5. Connector semantics

**Answer, traced to line references**: telegram (and any future
connector) receives **every delta**, unconditionally, regardless of
whether the queue merged it into an existing card via supersede or
promoted it as a fresh item.

`enqueue_and_fan_out` (`poller.rs:557-578`) clones the event *before*
calling `queue.enqueue(event)` (`:564`, `accepted = event.clone()`),
then offers `accepted` to every connector **only on `Ok(())`**
(`:566-569`). `SingleSlotQueue::enqueue` → `enqueue_with_options`
(`queue.rs:133-145`) returns `Ok(())` from **both** branches: the
supersede branch returns `Ok(())` immediately at `queue.rs:141` when
`supersede_if_topic_matches` succeeds, and the fallthrough
`enqueue_new` path (`:144`) also returns `Ok(())` on a normal accept
(`:182`). There is no signal anywhere in this return path that
distinguishes "merged into the existing card" from "became a new
Waiting/Visible item" — `enqueue_and_fan_out`'s `match queue.enqueue
(event) { Ok(()) => ... }` (`:565-570`) cannot see the difference, and
critically, `accepted` is the **clone taken before** `enqueue()` ran
(`:564`), so the connector always gets the original, unmerged event
content (e.g. the standalone "K. Havertz 6'" ScoreUpdate), never the
post-supersede merged state that overwrote it in the queue.

Net effect: adopting Topic/Recurring for the scoreboard **changes
nothing about what telegram receives** — it still gets one message per
delta, exactly as today (a burst of goal/card/state messages per
match), because fan-out happens at *acceptance*, upstream of and
blind to the queue's internal merge behavior. This matches
`CONTEXT.md`'s **Connector** definition verbatim: "observes acceptance,
not Promotion: the queue's display rules ... never apply to it." The
only thing that changes is what's displayed in the overlay's single
Slot, not what leaves the machine.

**REJECTED ALTERNATIVE**: make fan-out Topic-aware (skip offering a
delta to connectors if it merged into an existing card, on the theory
that "the overlay only shows the latest state, so outbound sends
should match"). Rejected: it would require the enqueue return path to
carry a merged/fresh distinction that does not exist today (a
non-trivial `queue.rs` API change, `enqueue` returning something richer
than `Result<(), QueueError>`), it breaks the standing invariant that a
Connector observes *acceptance*, not display state
(`CONTEXT.md`'s own words), and it silently drops real information
telegram users may want (every scorer/card detail) in favor of matching
the overlay's necessarily-lossy single-card view. Nothing in the spike
grounding suggests telegram output volume for ESPN is a live complaint
today — no reason to change it as a side effect of this feature.

## 6. Config surface

**RECOMMENDATION**: a new opt-in boolean, `espn_live_card: bool`
(default `false`), alongside the existing per-source `Config` fields
(`config.rs:9-`, next to `espn_priority`/`espn_ttl_secs`). When `false`
(the shipped default), `make_event` behaves exactly as today
(`topic: None`, always `OneShot`) — zero behavior change for anyone who
doesn't opt in. When `true`, `diff_scoreboard`/`make_event` compute the
`espn:{league}:{match_id}` Topic (§1) and set `rotation:
RotationSpec::Recurring { display_secs: espn_ttl_secs }` for every
event except the full-time `MatchState` (§3), which stays `OneShot`
with the same Topic.

Migration default is `false` because this is a felt-experience change
(§4's "always one live card cycling in rotation" cadence) that existing
users have not asked for and haven't seen — the burst-of-deltas
behavior is the shipped, live-verified default today, and this doc's
own §10 flags that whether "one card per match" is even the *preferred*
texture (vs. the current burst) is unresolved. An opt-in flag lets the
maintainer (or a future settings-window toggle, following the existing
per-source pattern already used for `espn_priority`/`rss_priority`)
turn it on selectively without a forced migration.

**REJECTED ALTERNATIVE**: replace the current behavior outright (no
flag; `Recurring`-with-Topic becomes the only espn behavior). Simpler
(no branch in `make_event`, no new config field, no "which mode am I
in" question when debugging). Rejected: this is a felt-UX change to
the app's *primary marquee source* with an open maintainer question
attached to it (§10) and a known build-blocking counter bug (§4) that
needs to land first — shipping it as the only behavior removes the
ability to compare against today's texture side-by-side or roll back
without a code change, for a decision the plan's own framing says
should be "approved or killed cheaply," which argues for reversibility
over commitment.

## 7. Phase/decision fit

**CONTEXT.md terms**: none need to change. This is the doc's own
strongest point in favor of the feature, verified by walking every term
this design touches: **Topic** (`:62-67`), **Recurring** (`:57-61`),
**Engine** (`:44-51`), **Expanded** (`:68-74`), **Score Update**
(`:124-125`), **Match State** (`:126-127`), **Connector** (`:109-114`),
**Poller** (`:120-123`) all already describe exactly the behavior this
design proposes — the glossary was written *for* this producer before
it existed. The one candidate edit is **Poller**'s "a Poller emits
deltas only" line (`:121-123`), which remains true (`diff_scoreboard`
is unchanged, §8) — no rewording needed.

**docs/V3_6_TECHNICAL_SPEC.md §3.4** (`:262-286`) currently says: "no
source in this pass constructs `Recurring` — that's the 'mechanism must
exist, no content yet' requirement (§3.6), still true" (`:284-286`).
This line becomes false the moment this feature ships and needs a
one-line amendment noting ESPN is now that content, with a pointer to
this design doc. `poller.rs:314-317`'s own "doesn't need the queue's
topic supersession mechanism in this pass" comment is the code-level
twin of that spec line and would need deleting/rewriting at build
time — its retirement is the single clearest signal that this feature
landed.

**ARCHITECTURE.md**: no scope-boundary or locked-decision change
identified. This is additive behavior behind an opt-in flag (§6),
riding entirely on infrastructure ARCHITECTURE.md already describes
(v3.6 permanent overlay, v6 per-source config) — nothing here
re-litigates a phasing or tech-stack decision. If the maintainer wants
"live match cards are the intended texture for espn" recorded as a
default behavior in a future phase, *that* would be an ARCHITECTURE.md-
level decision (a default, not a mechanism) — this doc explicitly
recommends staying opt-in (§6) precisely so that decision stays open.

## 8. Test strategy

**RECOMMENDATION**: extend the existing two test suites in place
(poller construction-rule assertions, queue regression + fixture
tests below) rather than introducing new test infrastructure.
`diff_scoreboard` stays pure (`poller.rs:349-465`, no I/O, already
fixture-tested against real captured ESPN payloads,
`tests/fixtures/scoreboard-*.json`) — the Topic/rotation-kind
assignment is purely a *construction-rule* change to `make_event`
(`poller.rs:297-324`), the same shape of change the existing delta
tests already cover (e.g. `assert_eq!(events[0].rotation,
RotationSpec::OneShot { ttl_secs: 8 })` at `poller.rs:821` — the new
tests are this same assertion style against `Recurring` and a computed
`topic` string instead). No new *kind* of test infrastructure is
needed for the poller side.

Queue-side supersede/requeue behavior is already covered exhaustively
(63 passing tests: visible supersede content/priority/rotation update,
extension floor-and-cap, same-tier/cross-tier moves, Recurring requeue
placement, dismiss-vs-skip Recurring semantics, plus a proptest
(`queue.rs:1812-2317`, generation strategies at `:1873-1906`, the case
runner at `:2306`) fuzzing arbitrary Topic/Recurring/priority
combinations against documented invariants) — adopting a real producer
doesn't need new *queue* unit tests, only:

- **A new `batch_done` regression test** for §4's identified bug
  (`Recurring` requeue must not increment `batch_done`) — this is a
  build *requirement*, not optional coverage, since it's the fix that
  makes plan 033's own stated intent true.
- **A cross-boundary fixture test**: a real multi-poll sequence
  (extend the existing `tests/fixtures/scoreboard-*.json` fixtures with
  a second, later-timestamped snapshot of the same match, or synthesize
  one from the existing away/home score bump already used in
  `diff_scoreboard`'s own tests) run through `diff_scoreboard` →
  `enqueue_and_fan_out` → a live `SingleSlotQueue`, asserting the
  second poll's events supersede-while-visible against the same Topic
  rather than stacking as separate items — this is the one new
  end-to-end shape (poller construction rule feeding the real queue
  supersede path) that isn't exercised by either side's existing suite
  in isolation.
- **A full-time-retires-the-card test**: enqueue a live `Recurring`
  card, supersede with the full-time `OneShot`-same-Topic event, assert
  the item drops (does not requeue) on its next `rotate_out_if_elapsed`
  — covers §3's chosen mechanism directly.

**REJECTED ALTERNATIVE**: a dedicated new integration-test module (e.g.
`tests/scoreboard_topic_lifecycle.rs`) standing up the full poller +
queue + fan-out stack end to end. Rejected: the existing split (pure
`diff_scoreboard` unit tests in `poller.rs`, pure `SingleSlotQueue`
unit/proptests in `queue.rs`, per `TESTING_STRATEGY.md`'s stated
per-component test plan) already covers both halves; the only truly
new *shape* of case is the one cross-boundary fixture test named above,
which fits inside `poller.rs`'s existing `#[cfg(test)] mod tests`
(it already has queue + connector fixtures, e.g. `score_event` at
`poller.rs:693`) without a new top-level test binary or fixture
harness.

## 9. Build estimate

**RECOMMENDATION**: **S/M** — most of the mechanism already exists and
is tested; the work is construction-rule wiring plus one queue fix.

Files a build would touch:
- `src-tauri/src/config.rs` — new `espn_live_card: bool` field +
  default + parse/override tests (small, follows the existing
  `espn_priority`-style pattern exactly).
- `src-tauri/src/poller.rs` — `make_event` gains a `topic: Option<
  String>` parameter (or reads a `live_card: bool` + computes the Topic
  internally); `diff_scoreboard` threads the flag/Topic through its
  five call sites; the full-time branch (`:409-418`) gets the
  `OneShot`-same-Topic special case (§3); new/extended fixture tests.
- `src-tauri/src/queue.rs` — the `batch_done` fix for `Recurring`
  requeue (§4, `rotate_out_if_elapsed`, `:249-262`) + its regression
  test. No other queue changes — this is the only queue-side code
  change this design requires.
- `docs/V3_6_TECHNICAL_SPEC.md` §3.4 — amend the "no source ... in this
  pass" line (§7).
- `CONTEXT.md` — no change identified (§7), but worth a final read-
  through at build time in case the build surfaces something this spike
  missed.
- Settings window (`src-tauri/capabilities/settings.json`,
  `src/settings/*`) — **out of scope for a first cut**: `espn_live_card`
  can ship as a config-file-only flag (like most `Config` fields already
  are) without a dedicated settings UI control; adding one is a
  separate, small follow-up if the flag proves worth exposing in-app.

Not included in the estimate: any Engine (plan 037) migration — that
plan is unbuilt (see grounding), and this design's semantics are
Engine-agnostic per its own maintenance note.

**REJECTED ALTERNATIVE**: fold the settings-window toggle
(`src/settings/*`, `capabilities/settings.json`) into this same build,
bumping the estimate to **M/L**. Rejected for a first cut: every other
per-source `Config` field shipped config-file-only before (if ever)
gaining a settings-window control — `espn_priority` itself has no
dedicated settings-window widget today despite being user-configurable
since v6 — so there's precedent for shipping the flag alone and adding
UI later only if the opt-in proves worth surfacing.

## 10. Open questions for the maintainer — RESOLVED 2026-07-19

All four were resolved by the maintainer on 2026-07-19; each
**Decision:** below is locked and inherited by the build plan(s).
Summary: build the single updating card **opt-in** (`espn_live_card`,
default `false`); **defer** the multi-match case to a later pass (reuse
today's tier sharing); **reuse** the existing 8s `ttl_secs` for dwell;
and land the §4 `batch_done` counter fix **as its own small plan
first**, before the card build.

This splits the work into two plans: (A) a small, isolated `batch_done`
correctness fix (lands first), then (B) the opt-in live-match card,
single-match-scoped, reusing `ttl_secs`.

(The questions below were deliberately recommendation-free — §§1–7 and
9 carry the doc's design recommendations; these four were the
maintainer's to call.)

1. **Is "one live card per match" actually the wanted texture, or is
   today's burst-of-deltas the preferred one?** This design assumes
   consolidation is strictly better, but a burst of distinct
   kickoff/goal/card/full-time cards has its own value (each moment
   gets its own dedicated turn in rotation, however brief) that a
   single cycling card trades away. Recommend a short manual A/B on
   real hardware (flag on vs. off) before committing to this as the new
   default anywhere.

   **Decision (2026-07-19): Build it, opt-in.** Ship the single updating
   card behind `espn_live_card` (default `false`), leaving today's burst
   as the default — the A/B is then a flag toggle on real hardware, at
   zero risk to current behavior.
2. **Does a `Recurring` card monopolizing rotation slots fight the news
   cards (`rss_poller.rs`, default `Low` priority) or cmux items during
   a busy multi-match Saturday?** §4 addresses the single-match case
   (per-source priority is the lever); the *multi*-match case — several
   live matches, each wanting its own `Recurring` Topic, all in the
   `High` tier simultaneously — is not modeled here at all and would
   need its own pass (do all live matches share the tier and rely on
   rotation-order/arrival FIFO among themselves, same as today's
   multi-match burst already does?).

   **Decision (2026-07-19): Defer.** The first build targets single-match
   correctness; multiple live matches each get their own `Recurring`
   Topic and share the tier via rotation-order/FIFO exactly as today's
   multi-match burst already does. Explicit multi-match arbitration is a
   later pass, taken up only if it proves annoying in practice.
3. **Is `display_secs = espn_ttl_secs` (today's `ttl_secs`, default 8s)
   the right cycle length for a `Recurring` card, or should live cards
   get their own, longer, config knob** — separate from the `ttl_secs`
   used for one-shot goal/card alerts, since a `Recurring` card's job
   (a persistent status readout) is arguably different from a one-shot
   alert's (a brief interruption)? This design reused the existing
   field for minimal scope; a build plan should confirm that's still
   right rather than inheriting it by default.

   **Decision (2026-07-19): Reuse `ttl_secs`.** The live card uses the
   existing 8s `espn_ttl_secs` for dwell — minimal scope, no new knob. A
   dedicated `live_card_secs` is deferred; add it only if 8s feels wrong
   once the card is running.
4. **Should the queue-slider fix (§4's `batch_done` change) ship as
   part of this feature, or land as its own small, separately-reviewed
   plan first**, given it's a real (if currently inert-in-production)
   discrepancy from plan 033's stated intent that this design merely
   surfaced rather than caused?

   **Decision (2026-07-19): Land it first, separately.** The `batch_done`
   fix (`queue.rs:257`) ships as its own small, independently-reviewed
   plan before the card build — it's isolated, and the card build depends
   on it being correct anyway (a cycling `Recurring` card would otherwise
   pin the 033 queue-slider near "complete").
