# Plan 053 (spike): give Manual/Cmux a second Topic-supersession producer

> **Executor instructions**: This is a DESIGN SPIKE, not a build plan.
> The deliverable is a design document plus open questions for the
> maintainer — **zero production code changes**. Follow the steps, honor
> the STOP conditions, and when done update this plan's status row in
> `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat f2cbae6..HEAD -- src-tauri/src/event.rs src-tauri/src/http.rs src-tauri/src/queue.rs hooks/notchtap-cmux-hook.sh`
> Drift doesn't block a spike — but read the drifted regions before
> quoting them in the design doc.

## Status

- **Priority**: P3
- **Effort**: L (coarse — this reopens a currently-closed wire-schema
  rule; the spike itself is M, but the doc must be honest that any
  resulting build is larger and riskier than a typical direction spike)
- **Risk**: LOW for this spike (docs only); the eventual build would be
  HIGH-risk (touches the `/notify` wire contract and a closed-by-design
  rule) — say so explicitly in the doc
- **Depends on**: none
- **Category**: direction
- **Planned at**: commit `f2cbae6`, 2026-07-19

## Why this matters

The Topic/`RotationSpec::Recurring` supersession machinery — fully
built, fully tested (`queue.rs`'s `supersede_if_topic_matches`, the
capped extension on a Visible supersede, `Recurring` requeue in
`tick()`) — has exactly **one** production producer: the opt-in ESPN
live-match card (plans 039/041/042). Even the second ambient source to
ship after that (Weather, plan 040) chose plain `OneShot`/`topic: None`
alerts, not Topic supersession
(`src-tauri/src/weather_poller.rs:198-210` — every alert event sets
`topic: None`). Meanwhile the highest-volume, burstiest source in this
app's actual daily use is Manual/Cmux: a chatty agent session (Claude
Code via `hooks/notchtap-claude-hook.sh`, or cmux via
`hooks/notchtap-cmux-hook.sh`) can post several related notifications in
quick succession with no coalescing — each one queues as a separate
one-shot card marching through the single Slot, exactly the texture
Topic supersession was built to replace for football. This spike
investigates what adopting the machinery for Manual/Cmux would look
like — the identical value proposition that motivated building it for
football, aimed at the tool's actual primary audience instead of a
secondary sports feature.

**This reopens a rule stated as deliberately closed, not merely
unexercised** — the important distinction from the football/weather
precedents. `src-tauri/src/event.rs:15-19`:

```rust
/// Which source produced this event (v6: rotation-order tie-break) —
/// orthogonal to `Priority`, which still decides cross-tier order
/// first. Always server-assigned, never accepted from the `/notify`
/// wire (same rule as `rotation`/`topic`).
pub origin: SourceKind,
```

`topic`/`rotation`/`origin` are ALL currently in the closed set of
fields a `/notify` caller may never set directly — they're
server-assigned only. This spike is explicitly about whether that rule
should partially reopen (a caller-supplied topic string, specifically)
for Manual/Cmux, which is a materially bigger decision than the
football/weather precedents (where `topic` was assigned internally by
the poller from ESPN's own match id, never accepted from an external
caller at all). The doc must engage with this honestly, not undersell
it as "just wire up the existing machinery."

## Current state (grounding — quote-verified at `f2cbae6`)

- `src-tauri/src/event.rs:15-19` — the closed-door comment quoted above.

- `src-tauri/src/http.rs:64-85` — `NotifyRequest`, the current `/notify`
  wire schema (title/body/priority/signal/source/subtitle/details — no
  `topic`). `http.rs:190-200`'s `Event` construction hardcodes
  `topic: None` for every `/notify`-originated event.

- `src-tauri/src/queue.rs:185-190` — the supersession primitive itself:

  ```rust
  fn supersede_if_topic_matches(&mut self, topic: &str, fresh: &Event, now: Instant) -> bool {
      if let Some(visible) = &mut self.visible {
          if visible.event.topic.as_deref() == Some(topic) {
              apply_fresh_content(&mut visible.event, fresh);
              self.top_up_visible_remaining_time(now);
              return true;
  ```

  (Read the rest of this function and its Waiting-side counterpart in
  full before writing the doc — this excerpt is the Visible-side
  branch only.)

- The one existing production precedent to compare against and learn
  from: `docs/design/scoreboard-topic-card.md` (plan 031's spike doc)
  and its resulting build (plans 039/041/042). Read the spike doc in
  full — it already worked out the general shape of Topic identity,
  rotation-kind choice, tier interplay, and connector semantics for
  ESPN; this new spike's job is to answer the SAME nine questions for a
  materially different producer (caller-supplied, not poller-computed,
  topic identity), not to re-derive the machinery's behavior from
  scratch.

- The candidate Topic-identity source for cmux: `hooks/notchtap-cmux-hook.sh:30,39`
  already extracts `context.cwd` (the originating project path) and
  passes it as a `--detail "Project=$project"` value. A natural
  candidate Topic key is "one card per project" (`cmux:{cwd}` or
  similar) — but this is a caller-influenced value (the hook script
  controls what it sends), which is exactly the new trust question this
  spike must address that plan 031 didn't have to (ESPN's match id
  comes from ESPN's own API response, never from the pusher).

- Plan 044 (if executed) fixes a same-poll ordering bug in the one
  existing Topic producer (ESPN's card+full-time emission order) — read
  its "Why this matters" section, since it's a concrete example of a
  failure mode ("whichever same-Topic event is emitted last wins the
  rotation state") that a caller-supplied-topic design must also guard
  against, likely more easily exploited if a caller can control emission
  order/content directly rather than going through a single poller's
  internally-ordered diff function.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Read-only exploration | `grep`, `Read`, `rg` | — |
| Confirm today's closed set | `grep -n "never accepted from the" src-tauri/src/event.rs` | shows the `rotation`/`topic`/`origin` closed-door comments |
| Run the queue's supersession tests (understanding, not gating) | `cd src-tauri && cargo test --locked queue::` | pass |
| Confirm nothing changed | `git status` at the end | only the new doc + `plans/README.md` row |

## Scope

**In scope** (the only files you may create/modify):
- `docs/design/manual-cmux-topic-supersession.md` (create)
- `plans/README.md` (status row)

**Out of scope — hard rule for this spike**:
- ANY file under `src/`, `src-tauri/`, `hooks/`, or `notchtap` (the CLI
  script). No prototype code in the repo; illustrative snippets live
  inside the doc.
- Rewriting `CONTEXT.md`/`docs/ARCHITECTURE.md` — the doc *proposes*
  what amendment either would need; it doesn't make the edit.
- Designing a general "any source can supply a topic" mechanism beyond
  Manual/Cmux specifically — scope this to the one concrete producer
  this spike investigates, not a fully generic redesign.

## Git workflow

- Docs-only commit `docs(design): manual/cmux topic-supersession spike`
  in repo style. Do NOT push or open a PR unless the operator
  instructed it.

## Steps

### Step 1: Read the existing precedent doc and its build in full

Read `docs/design/scoreboard-topic-card.md` end to end, and the
done-entries for plans 039/041/042 in `plans/README.md`, before writing
anything. This spike's doc should explicitly cross-reference which of
that doc's nine decisions carry over unchanged (rotation-kind mechanics,
tier interplay, connector semantics are all Engine/queue-level and
producer-agnostic) versus which need fresh answers specific to a
caller-supplied topic (topic identity/trust, since ESPN's spike didn't
have to consider a hostile or careless caller).

### Step 2: Write the design doc

`docs/design/manual-cmux-topic-supersession.md`, each section with a
**recommendation and at least one rejected alternative with reason**:

1. **Topic identity & trust**: THE central new question this spike adds
   beyond plan 031's. Options to evaluate: (a) caller-supplied topic
   string, wire-accepted and sanitized/capped like `subtitle`/`details`
   (risk: a collision — accidental or crafted — could clobber an
   unrelated card; namespace it, e.g. server-prefixes
   `manual:{caller-supplied-suffix}` so a caller can never collide with
   `espn:`/future internal topics); (b) server-derived from
   `context.cwd`/project path for cmux specifically (no caller-supplied
   string at all — the hook already sends `cwd`, so the server could
   derive a topic from it without trusting an arbitrary string field);
   (c) reject caller-supplied topics entirely and only support
   server-derived ones per known caller shape (cmux's cwd), leaving
   plain Manual `/notify` callers without topic support at all.
   Recommend one, with the trust/collision trade-off spelled out
   concretely (what happens on a collision under each option).
2. **Which events share a Topic**: for cmux specifically — is "one card
   per project (cwd)" the right grouping (a burst of unrelated
   notifications from the same project all collapse into one updating
   card), or is a finer grain needed (e.g. per originating tool-call/
   session id, if cmux's payload carries one — check
   `hooks/notchtap-cmux-hook.sh`'s actual `$input` shape for what
   identifiers are available beyond `cwd`)?
3. **Rotation kind**: `Recurring` (cycles until some end condition) vs.
   `OneShot`-with-topic (updates in place but still ages out normally).
   Unlike football (which has a clear "match ends" terminal signal),
   Manual/Cmux has no natural "session ended" event — model what would
   retire a `Recurring` Manual/Cmux card (a timeout? an explicit
   "session ended" signal a hook could send? never retiring except by
   manual dismiss?). This is likely harder than football's case, where
   full-time is a clean terminal signal — say so.
4. **Content on supersede**: today, a superseding event fully replaces
   `payload`/`meta` (per `apply_fresh_content`) — a burst of distinct
   messages ("tests passed" → "build started" → "deploy complete")
   collapsing into one card means only the LATEST message is ever seen
   if the user doesn't catch each rotation. Evaluate whether this is
   the right trade for Manual/Cmux (where each message may be
   independently actionable, unlike football's "just show the current
   score" case) — this is a materially different content semantics
   question than football's, where a running scoreline naturally
   subsumes prior state.
5. **Config surface**: opt-in flag following the `espn_live_card`/
   `weather_enabled` precedent, or does this need to be per-caller
   (e.g. only cmux gets topic support, not arbitrary `/notify` posts)?
6. **Connector semantics**: trace `Engine::accept`'s fan-out behavior
   under a Manual/Cmux supersede — does Telegram still receive every
   distinct message (probably yes, mirroring football's answer in the
   031 doc), or does a superseded Manual/Cmux notification change what
   reaches the connector in a way that matters more here (since Manual/
   Cmux messages, unlike football score deltas, are often the entire
   point of the outbound relay)?
7. **Interaction with plan 044's bug class**: state explicitly that
   whatever ordering discipline plan 044 establishes for same-poll
   same-Topic events (if that plan has landed) must be reasoned about
   for a caller-supplied-topic design too — a careless or hostile caller
   controls emission order/content directly, unlike a poller's
   internally-consistent diff function.
8. **Test strategy & build estimate**: name concrete new
   http.rs-integration-test cases (a caller-supplied topic colliding
   with an internal one, a Manual burst collapsing to one card) and
   give an S/M/L estimate with file list — expect this to land at L
   once the trust/namespacing question is resolved, given the wire
   contract change.
9. **Open questions for the maintainer** (e.g.: is this actually wanted
   given no user has asked for it yet — this is a `/improve next`-sourced
   idea, not a stated request; is the collision/trust risk acceptable
   for a single-user local tool at all, given `/notify`'s existing
   unauthenticated posture; would cmux/Claude Code hook authors actually
   use this if built).

### Step 3: Sanity-check citations

Every code claim gets a `file:line` valid at the commit read (stamped at
the top of the doc).

**Verify**: `git status` → only the design doc (+ `plans/README.md`
row).

## Test plan

N/A — docs-only spike.

## Done criteria

- [ ] `docs/design/manual-cmux-topic-supersession.md` exists, covers all
      9 sections, each with recommendation + rejected alternative
- [ ] The doc states the commit it was researched against
- [ ] The doc explicitly cross-references `docs/design/scoreboard-topic-card.md`
      and states which of its decisions carry over vs. need fresh
      answers
- [ ] The Topic-identity-&-trust section explicitly names the
      collision/namespacing risk a caller-supplied topic introduces that
      ESPN's server-derived topic never had
- [ ] No source-code changes (`git status` proof)
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- A production producer already sets a caller-supplied `topic:` (re-grep
  `event.rs`'s closed-door comment and `http.rs`'s `NotifyRequest`
  first) — the "closed set" premise would be stale.
- You find a recorded *rejection* (not deferral) of this direction in
  `docs/` or `plans/README.md`.
- Understanding the trust/collision question seems to require actually
  writing wire-schema code to see what breaks — write the uncertainty
  into the doc instead (this is exactly the kind of question a spike
  exists to surface, not resolve by prototyping in the repo).

## Maintenance notes

- If approved, the build plan inherits this doc's topic-identity/trust
  decision verbatim; if rejected or deferred, record it in
  `plans/README.md`'s rejected/deferred list with the reason.
- This is the highest-risk of the direction spikes from this audit pass
  (050/051/052/053) precisely because it's the only one reopening an
  explicitly-closed wire-schema rule — a reviewer of the eventual build
  (if approved) should scrutinize the namespacing/collision defense
  most heavily of anything this audit pass produced.
- Whoever builds this, if approved, should re-read plan 044's fix (same-poll
  same-Topic ordering) first, since a caller-supplied-topic design
  inherits that failure class and must defend against it more
  proactively than a single internally-consistent poller does.
