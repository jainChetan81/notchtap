# Design spike: a second Topic-supersession producer for Manual/Cmux

> **Status**: design spike (plan 053), zero production code changes.
> Researched against commit `f2cbae6` (the plan's planned-at commit,
> 2026-07-19). Every `file:line` citation below was re-verified by
> reading the file directly in this working copy; line numbers match
> `f2cbae6` unless noted.
>
> **Risk framing the plan requires up front**: this spike is LOW-risk
> (docs only), but the build it proposes would be **HIGH-risk** — it
> touches the `/notify` wire contract and reopens a rule stated in the
> code as deliberately closed, not merely unexercised. That is a
> materially bigger decision than the football precedent (plan 031's
> spike, built as plans 039/041/042), where the Topic string was
> computed by the poller from ESPN's own API response and never came
> from an external caller at all.

## Why this matters

The Topic/`RotationSpec::Recurring` supersession machinery — fully
built, fully tested (65 `queue::` tests green at this commit, including
the supersede, capped-extension, and requeue suites) — has exactly
**one** production producer: the opt-in ESPN live-match card (plans
039/041/042). Even Weather (plan 040), the second ambient source to ship
after it, chose plain `OneShot`/`topic: None` alerts
(`src-tauri/src/weather_poller.rs:198-210` — the `alert_event`
constructor sets `topic: None` at `weather_poller.rs:204`).

Meanwhile the highest-volume, burstiest source in this app's actual
daily use is Manual/Cmux: a chatty agent session (Claude Code via
`hooks/notchtap-claude-hook.sh`, or cmux via
`hooks/notchtap-cmux-hook.sh`) can post several related notifications in
quick succession with no coalescing — each one queues as a separate
one-shot card marching through the single Slot, exactly the texture
Topic supersession was built to replace for football. This spike asks
what adopting the machinery for Manual/Cmux would look like.

**The rule being reopened is stated as closed in three places**, all
verified at this commit:

1. `src-tauri/src/event.rs:15-19` (doc comment on the `origin` field):

   ```rust
   /// Which source produced this event (v6: rotation-order tie-break) —
   /// orthogonal to `Priority`, which still decides cross-tier order
   /// first. Always server-assigned, never accepted from the `/notify`
   /// wire (same rule as `rotation`/`topic`).
   pub origin: SourceKind,
   ```

2. `src-tauri/src/http.rs:190-200` — the `Event` construction in
   `notify_handler` hardcodes `topic: None` (`http.rs:195`) and
   `rotation: RotationSpec::OneShot { ttl_secs }` (`http.rs:194`) for
   every `/notify`-originated event; `NotifyRequest` (`http.rs:64-85`)
   has no `topic` field at all.

3. `docs/V3_6_TECHNICAL_SPEC.md:256-261` — *"`rotation` is never
   accepted from the wire — always constructed server-side from the
   resolved source's ttl config. `topic` is never accepted from the
   wire — always `None`. This keeps the http schema narrow while
   satisfying 'every source feeds this one queue' (§3.6) without opening
   `Recurring`/`topic` to untrusted/external input in this pass."* —
   with the companion statement at `V3_6_TECHNICAL_SPEC.md:196-200` that
   *"the wire (`/notify`, cli) can only ever produce `OneShot` …
   `Recurring` is constructed by internal Rust sources only."*

This spike is explicitly about whether that rule should **partially**
reopen — a caller-supplied `topic` string, specifically, while
`rotation` and `origin` stay closed — for Manual/Cmux. It must not be
undersold as "just wire up the existing machinery."

## Premise re-verification (the plan's STOP-condition checks)

- **No production producer accepts a caller-supplied `topic` today.**
  `NotifyRequest` (`http.rs:64-85`) carries only
  `title`/`body`/`priority`/`signal`/`source`/`subtitle`/`details`.
  Every `/notify` event gets `topic: None` (`http.rs:195`). The only
  producer setting `topic: Some(..)` is the ESPN poller, computed
  server-side: `poller.rs:479`
  (`espn_live_card.then(|| format!("espn:{league}:{}", v.id))`), and it
  is gated behind the opt-in `Config.espn_live_card` flag
  (`config.rs:30`, default `false` at `config.rs:185-187`). The closed
  set premise is **not stale**.
- **No recorded rejection of this direction exists.**
  `plans/README.md`'s "Findings considered and rejected" section
  contains nothing on caller-supplied topics; row 053 records this spike
  as TODO. The football direction was *deferred, then built* — never
  rejected.
- **Understanding the trust/collision question did not require writing
  wire-schema code.** The mechanics are fully traceable from the
  existing supersede path (see §1, §6, §7).

## What carries over from plan 031's spike doc vs. what needs fresh answers

`docs/design/scoreboard-topic-card.md` worked out the general shape of
Topic identity, rotation-kind choice, tier interplay, and connector
semantics for ESPN; its build (plans 039/041/042) is now the one
production producer. Cross-referenced decision by decision:

**Carries over unchanged (Engine/queue-level, producer-agnostic):**

- The supersede mechanics themselves: `supersede_if_topic_matches`
  (`src-tauri/src/queue.rs:185-213`) updates a Visible match in place
  with a capped time top-up (`queue.rs:334-350`, floor/cap constants at
  `queue.rs:531-532`) and repositions a Waiting match across tiers
  (`queue.rs:193-211`) — all of it Topic-string-agnostic.
- `apply_fresh_content` semantics (`queue.rs:524-529`): wholesale
  replacement of `payload`/`priority`/`rotation`/`signal` on supersede.
- Connector fan-out blindness: `Engine::accept`
  (`src-tauri/src/engine.rs:158-184`) clones the event *before*
  enqueueing (`engine.rs:163`) and offers the clone to connectors on
  `Ok(())` (`engine.rs:178-181`), and the supersede branch returns
  `Ok(())` identically to a fresh enqueue (`queue.rs:139-143`) — so
  connectors receive every accepted event regardless of merging, exactly
  as the 031 doc traced for the pre-037 `enqueue_and_fan_out` path.
- The namespacing *convention* from `V3_6_TECHNICAL_SPEC.md:215-226`
  ("sources namespace their own topics … a documentation convention,
  not a new validation layer") — but see §1: a caller-supplied topic
  turns that convention into something that must become an *enforcement*.

**Needs a fresh answer here (the 031 doc never faced it):**

- **Topic identity & trust** (§1) — ESPN's Topic is computed from ESPN's
  own match id by the poller; a Manual/Cmux Topic would be influenced or
  supplied by the caller. Hostile/careless-caller analysis is new.
- **Rotation kind** (§3) — football has full-time as a clean terminal
  signal; Manual/Cmux has no natural "session ended" event. The 031
  doc's elegant "full-time `OneShot` retires the card" mechanism has no
  analogue here.
- **Content-on-supersede weighting** (§4) — a running scoreline
  naturally subsumes prior state; distinct agent messages do not.
- **The plan-044 ordering bug class** (§7) — a poller emits its
  same-Topic events in one internally-consistent order; a caller
  controls emission order and content directly.

---

## 1. Topic identity & trust — THE central new question

**RECOMMENDATION: option (a) — accept an optional caller-supplied
`topic` string on the wire, sanitized/capped like `subtitle`/`details`,
and server-namespaced by prefixing with the resolved origin
(`manual:{suffix}` / `cmux:{suffix}`).**

Concretely, the eventual build would: add `topic: Option<String>` to
`NotifyRequest` (next to `subtitle` at `http.rs:83-84`); sanitize it in
`notify_handler` with the same discipline as
`sanitize_subtitle`/`sanitize_details` (`http.rs:111-131`) — empty →
`None`, a hard char cap (shorter than `SUBTITLE_MAX_CHARS = 120` at
`http.rs:92`; ~64 is plenty for an identity string), and arguably a
character whitelist; then **prefix it server-side with the origin
resolved at `http.rs:172-179`** so the stored Topic is
`manual:{suffix}` or `cmux:{suffix}`, never the caller's raw string.

**Why the server-side prefix is the load-bearing piece.** Today the
collision surface is nil because only the poller sets Topics and it uses
the `espn:` namespace (`poller.rs:479`). The moment a caller can name a
Topic, two collision classes appear that ESPN's server-derived Topic
never had:

- **Accidental cross-source collision**: a caller passes
  `topic: "espn:eng.1:401584543"` (by copy-paste or bad luck) and its
  next push silently hijacks a live match card — `apply_fresh_content`
  (`queue.rs:524-529`) would overwrite the scoreboard's payload,
  priority, rotation, and signal with the caller's content. With the
  server-side prefix this is *structurally* impossible: the stored Topic
  becomes `manual:espn:eng.1:401584543`, which matches nothing the
  poller emits.
- **Caller-on-caller collision**: two unrelated Manual callers (or one
  careless hook script) pick the same suffix, e.g. `topic: "build"`.
  Their notifications collapse into one card. Under the recommendation
  this is *possible but self-inflicted and visible*: the Topic namespace
  is exactly the set of strings the caller community chooses, same as
  filename or env-var collisions, and the blast radius is one merged
  card on a loopback-only, unauthenticated endpoint
  (`http.rs:140-145` hardcodes `127.0.0.1` — any process that can post
  can already flood the queue with arbitrary one-shot cards today, so a
  Topic collision grants no capability a local process doesn't already
  have). The doc's position: for a single-user local tool, this residual
  risk is acceptable *if namespacing is enforced*; it is the honest
  price of the feature and belongs in the maintainer decision (§9).

**REJECTED ALTERNATIVE 1: option (b)/(c) — server-derived Topic from
cmux's `context.cwd`, no caller-supplied string at all.** The cmux hook
already extracts `context.cwd` (`hooks/notchtap-cmux-hook.sh:30`) and
sends it as a `--detail "Project=$project"` pair
(`notchtap-cmux-hook.sh:39`), which lands in `EventMeta.details` via the
sanitized wire field (`http.rs:83-84`, `:184-188`). The server could
scan `details` for a `Project` pair and derive `cmux:{cwd}` without
trusting any new field. Rejected on three grounds: (i) it couples
*queue identity* to *display metadata* — `details` is documented as
"Presentation-only — never consulted by queue/rotation/priority logic"
(`event.rs:145-149`), and this would make it load-bearing for
supersession, the first breach of that invariant; (ii) the grouping key
becomes invisible to the caller — a hook that renames the label,
reorders the pairs, or hits the 8-pair cap (`DETAILS_MAX_PAIRS` at
`http.rs:93`) silently changes or loses its grouping; (iii) it leaves
plain Manual `/notify` callers (the CLI, `notchtap:4`'s usage line) with
no Topic support at all — option (c) explicitly accepts that, but the
CLI burst case is half the motivation for this spike. The trust win
(never parsing an arbitrary caller string) is real but small given the
loopback trust posture, and doesn't pay for (i)–(iii).

**REJECTED ALTERNATIVE 2: caller-supplied topic with no namespacing
(raw string stored verbatim).** Rejected because it makes the
accidental-collision class above a live footgun for zero savings — the
prefix is one `format!` at construction, exactly the reasoning the 031
doc used for keeping `espn:{league}:` in ESPN's own Topic
(`scoreboard-topic-card.md` §1's rejected "bare match id" alternative).

## 2. Which events share a Topic

**RECOMMENDATION: for cmux, "one card per project" — the hook (once
the wire supports it) passes a topic suffix derived from `context.cwd`
(`notchtap-cmux-hook.sh:30`), so the server stores
`cmux:{cwd}`.** A burst of notifications from the same project — "tests
passed", "build started", "deploy complete" from one agent session —
collapses into one card that updates in place, which is precisely the
coalescing this spike wants. Different projects keep separate cards,
which matches how a developer actually partitions attention.

**What identifiers are actually available, checked against the hook:**
the documented stdin shape is
`{ "notification": {title, subtitle, body, ...}, "context": {cwd, ...}, "effects": {...} }`
(`notchtap-cmux-hook.sh:12-13`), and the hook extracts exactly
title/body/subtitle/cwd (`notchtap-cmux-hook.sh:27-30`). No session id,
tool-call id, or notification id is extracted today; the `...` in the
shape comment means finer identifiers *may* exist in cmux's payload, but
nothing in this repo confirms one, so no finer grain can be recommended
on evidence.

**REJECTED ALTERNATIVE: per-session or per-tool-call grouping.** A
finer grain (one card per agent session, or per tool call) would reduce
unrelated-message collapse within one project. Rejected: (i) no such
identifier is confirmed available in the cmux payload (above) —
designing around an unverified field is how plan 043 ended up gated on a
Step 0 ("confirm the data exists before building"); (ii) within one
project, a burst is usually one logical work session anyway, so
per-project grouping captures most of the value with an identifier that
is verified present. If real use shows unrelated concurrent sessions in
one cwd stepping on each other's card, a finer suffix
(`cmux:{cwd}:{session}`) is a hook-side change with no server redesign —
the wire field carries whatever suffix the caller chooses.

## 3. Rotation kind — harder than football, and the doc says so

**RECOMMENDATION: `OneShot`-with-Topic — the card updates in place via
supersede but still ages out on its normal TTL** (the resolved
`default_ttl` / `cmux_ttl_secs`, `http.rs:172-179`, `:194`). No
`Recurring` for Manual/Cmux.

Football's `Recurring` choice rested on a clean terminal signal:
full-time. The 031 doc's mechanism — the full-time event emitted as
`OneShot`-on-the-same-Topic, flipping the card so its next rotation-out
drops it — works because `apply_fresh_content` copies
`existing.rotation = fresh.rotation` unconditionally
(`queue.rs:527`), and the poller *guarantees* a full-time event arrives.
Manual/Cmux has **no equivalent**: there is no natural "session ended"
event in either hook's payload (`notchtap-cmux-hook.sh:12-13` carries no
lifecycle field; the CLI is one-shot invocations), so a `Recurring`
Manual/Cmux card would cycle through rotation *forever* unless retired
by supersede-flip, `dismiss_visible` (`queue.rs:374-380` — drops a
Recurring item outright), or `skip_visible`. "Cycles until dismissed" is
a notification that never leaves — the exact monopolization failure the
`Recurring` design notes warn about (`V3_6_TECHNICAL_SPEC.md:552-564`).
An idle agent session would haunt the rail indefinitely.

`OneShot`-with-Topic degrades gracefully by construction: while the
session is chatty, supersedes keep the card fresh (with the existing
capped top-up, `queue.rs:334-350`, preventing a mid-burst rotation-out
from truncating the latest message); when the session goes quiet, the
card simply ages out like every Manual/Cmux card does today. The failure
mode of this choice — a long quiet gap kills the card, and the next
message arrives as a *fresh* card rather than resuming the old one — is
mild and arguably correct ("that was a while ago; this is a new thing").

**REJECTED ALTERNATIVE 1: `Recurring` + an idle-timeout retirement
rule** (queue drops a `Recurring` item that hasn't been superseded in N
seconds). Rejected: it is *new queue machinery* — today
`rotate_out_if_elapsed` requeues `Recurring` items unconditionally
(`queue.rs:249-263`, requeue at `:257-259`), so this needs a
last-superseded timestamp, a retirement check, new tests, and a new
`CONTEXT.md` clause amending the **Recurring** definition
(`CONTEXT.md:59-63`: "bounded by supersession or the underlying state
naturally ending, not a clock" — an idle timeout is precisely "a
clock"). All that to recover behavior `OneShot`-with-Topic already has.

**REJECTED ALTERNATIVE 2: `Recurring` + an explicit "session ended"
signal a hook sends as `OneShot`-same-Topic (the football pattern).**
Rejected as speculative: neither hook has a verified session-end
lifecycle event to hang it on (cmux's documented payload has none,
`notchtap-cmux-hook.sh:12-13`), and plan 044 — still TODO at this
commit (`plans/README.md` row 044) — exists precisely because even
football's *guaranteed* terminal signal has an ordering edge case
(same-poll card + full-time un-retires the card). Adopting that pattern
for a producer whose terminal signal is *unguaranteed* inherits the bug
class without the trigger reliability.

## 4. Content on supersede

**RECOMMENDATION: keep `apply_fresh_content`'s wholesale-replacement
semantics (latest message wins the card) — but flag a real gap the
build must close: `meta` is not superseded.**

Today's function (`queue.rs:524-529`) replaces `payload`, `priority`,
`rotation`, and `signal` — and nothing else. For ESPN that's harmless:
the 039/042 build attaches identical per-match `meta` (Clock, per-side
Cards) at every emission site, so a stale `meta` never differs from a
fresh one. For Manual/Cmux it's a **live inconsistency**: each
notification carries its own `subtitle`/`details` (`http.rs:184-188`),
so after a supersede the card would show the *latest* title/body with
the *first* message's subtitle/details — e.g. "deploy complete" titled
over a "Permission request" subtitle. The build should extend
`apply_fresh_content` to also replace `meta` (safe for ESPN, whose meta
is per-match-constant; its doc comment at `queue.rs:518-523` already
presents the function as "the one place a superseding event's content
lands," so extending it fits its stated purpose).

On the deeper question — is latest-wins the right *trade* when each
Manual/Cmux message may be independently actionable, unlike a
subsuming scoreline? — this doc's position is **yes for the overlay, no
for the record**, and the design already separates the two: the Slot is
a "what's happening now" surface, and *no information is lost* to the
user's other channels because connectors and the log see every accepted
event (§6). The user who needs "what did I miss" has telegram and the
plaintext log; the overlay collapsing a burst into one current-state
card is the feature, not the loss.

**REJECTED ALTERNATIVE: merge/append content on supersede** (running
log body, or coalesced details). Rejected for the same reasons the 031
doc rejected concatenation for football: the manifest lives in a fixed
500×300 window (`http.rs:87-91` states the geometry constraint behind
the existing caps), appending needs a stateful merge in a function whose
value is being stateless and uniform (`queue.rs:518-523`), and "what did
I miss" is a separate, still-unplanned direction (`plans/README.md`'s
deferred-directions list) that should get its own surface rather than
being smuggled into card bodies.

## 5. Config surface

**RECOMMENDATION: no new config flag — the wire field is opt-in per
call.** A caller that never sends `topic` gets byte-identical behavior
to today (`topic: None`, `http.rs:195`). This is different from
`espn_live_card` (`config.rs:30`, default `false` at `config.rs:185-187`)
and `weather_enabled` (`config.rs:65`, default `false` at
`config.rs:209-211`): those flags change the behavior of a *server-side
producer* that runs unconditionally otherwise, so they need an off
switch. A caller-supplied `topic` is off-by-omission; there is no
ambient behavior to gate. The namespacing prefix (§1) is always applied,
not configurable — it's a trust boundary, not a preference.

**REJECTED ALTERNATIVE 1: a global opt-in flag (`manual_topic_enabled`,
default false) following the `espn_live_card` precedent.** Rejected:
the only thing it would gate is whether the server *honors* a field the
caller chose to send — a caller sophisticated enough to send `topic` and
sophisticated enough to be hurt by it being ignored is the same caller;
the flag adds config/settings surface (with the parse/override tests
every `Config` field needs) to protect against a confusion the 400-on-
unknown-field discipline doesn't even allow (unknown fields are
currently *ignored* by serde, not rejected — so with a flag-off, a
`topic`-sending caller gets silent no-op, arguably worse than either
honoring it or rejecting it).

**REJECTED ALTERNATIVE 2: per-caller gating (only `source: "cmux"`
requests may set a topic; plain Manual posts get 400 or silent-ignore).**
Rejected as arbitrary: the Manual CLI burst is the same use case as the
cmux burst, `RequestSource` (`http.rs:52-62`) is a *self-declared* label
any caller can set anyway (it's not authentication — nothing stops a
"Manual" caller from claiming `"cmux"`), so gating on it buys no trust
and only splits the feature across an artificial line.

## 6. Connector semantics

**RECOMMENDATION: unchanged — every accepted Manual/Cmux event still
fans out, superseded or not.** Traced at this commit:
`Engine::accept` (`engine.rs:158-184`) clones the event before
enqueueing (`engine.rs:163`), the supersede path returns `Ok(())`
exactly like a fresh enqueue (`queue.rs:139-143`, supersede-returns-Ok
at `:140-142`), and the clone is offered to every connector on success
(`engine.rs:178-181`, with the News-origin exclusion at `:178` not
touching Manual/Cmux). So telegram receives **every distinct message**
in a burst even when the overlay merged them into one card.

This matters *more* here than it did for football, in the opposite
direction of concern: for ESPN, telegram getting every delta was
"harmless redundancy" (the deltas restate one scoreline); for
Manual/Cmux, the outbound relay **is often the entire point** — the user
is away from the screen and each agent message ("permission needed",
"tests failed", "done") is independently actionable. The current
fan-out-at-acceptance semantics is exactly right for this producer and
the build should not touch it.

**REJECTED ALTERNATIVE: topic-aware fan-out suppression** (don't offer
an event to connectors if it merged into an existing card). Rejected,
harder than the 031 doc rejected it for football: it would silently drop
the primary content of the relay (the actual messages) to match the
overlay's necessarily-lossy view, it would require `enqueue` to return a
merged/fresh distinction it doesn't have today (`queue.rs:115-145`
returns `Result<(), QueueError>`), and it breaks the `CONTEXT.md`
**Connector** invariant ("observes acceptance, not Promotion") that
`Engine::accept`'s doc comment encodes (`engine.rs:145-157`).

## 7. Interaction with plan 044's bug class

Plan 044 (TODO at this commit, `plans/README.md` row 044) fixes a
same-poll ordering bug in the one existing Topic producer: when a
booking and full-time land in the same `diff_scoreboard` pass, the
later-emitted `Recurring` card event's rotation unconditionally
overwrites the earlier full-time event's retiring `OneShot` rotation —
supersession is last-applied-wins with no tiebreak
(`apply_fresh_content`, `queue.rs:524-529`, every assignment
unconditional) — permanently un-retiring the match card.

**This doc states explicitly: whatever ordering discipline plan 044
establishes must be reasoned about for a caller-supplied-topic design,
and the caller's position is strictly worse than the poller's.** A
poller emits its same-Topic events from one internally-ordered diff
function; a caller controls emission order and content *directly* and
*across processes* — two hooks posting the same Topic concurrently can
interleave in any arrival order, and the last arrival silently wins the
payload, priority, and (if `rotation` were ever opened — it must not
be) rotation.

The specific defense this design gets **for free** from keeping
`rotation`/`origin` in the closed set: plan 044's concrete failure mode
(a `Recurring` rotation overwriting a retiring `OneShot`) **cannot
arise** here, because §3 recommends every wire-originated event stays
server-constructed `OneShot` (`http.rs:194`) — supersede can overwrite a
Manual/Cmux card's `rotation` only with another `OneShot`, so no
caller can flip a retiring card back to `Recurring`, and no caller can
create a never-retiring card at all. This is the strongest argument in
the doc for reopening the rule *for `topic` only* and never for
`rotation`.

What remains un-guarded (and should be named in the build plan, not
solved by it): last-writer-wins on payload across concurrent same-Topic
callers, and priority manipulation via re-supersede
(`apply_fresh_content` copies `priority`, `queue.rs:526`, so a caller
can upgrade a visible card's tier — bounded by the same loopback trust
posture as §1). **REJECTED ALTERNATIVE: add a timestamp/sequence
tiebreak to supersession.** Rejected: new queue machinery and new
invariants to defend against a disordering that, on a loopback
single-user tool, produces at worst a briefly-stale card — the
namespacing + OneShot-only constraints already remove the *damaging*
cases.

## 8. Test strategy & build estimate

**RECOMMENDATION: L (large) — as the plan predicted, the wire-contract
change plus the trust/namespacing defense push this well past the
football build's M**, even though the queue machinery itself needs
almost nothing.

Concrete new tests, all in the existing suites' shapes:

- `http.rs` integration tests (alongside the existing
  `notify_round_trips_subtitle_and_details_into_slot_state` at
  `http.rs:937` and the sanitize unit tests at `http.rs:883-934`):
  - *a caller-supplied topic is namespaced server-side*: POST
    `{"title","body","topic":"espn:eng.1:123"}` → the queued event's
    topic is `manual:espn:eng.1:123`, proving no caller string can
    collide with an internal `espn:` Topic.
  - *a Manual burst collapses to one card*: three POSTs with the same
    topic in quick succession → one Visible item with the third
    message's payload, `total_waiting() == 0`.
  - *sanitize_topic caps and empties*: empty string → `topic: None`
    (today's behavior preserved); over-cap → truncated; the cap
    constant pinned.
  - *superseded card still fans out*: two same-topic POSTs with a
    test connector attached (pattern from
    `accepted_push_fans_out_to_connectors`, `http.rs:467`) → connector
    receives both (guards §6).
  - *absent topic is byte-identical to today*: regression pin mirroring
    `live_card_off_keeps_one_shot_topicless_events` in `poller.rs`.
- `queue.rs`: one test extending the supersede suite for the §4 meta
  replacement (supersede updates `meta.subtitle`/`meta.details`), if
  that recommendation is accepted.
- No new *kind* of test infrastructure; the 65-test `queue::` suite
  (verified green at this commit via `cargo test --locked queue::`)
  already covers the supersede mechanics exhaustively.

Files a build would touch: `src-tauri/src/http.rs` (wire field,
sanitize, namespacing, tests); `src-tauri/src/event.rs` (the
`event.rs:15-19` closed-door comment reworded — `topic` leaves the
closed set *for `/notify` with server-side namespacing*, `rotation`/
`origin` stay); `src-tauri/src/queue.rs` (`apply_fresh_content` meta
replacement + one test — the only queue change); `notchtap` (a `--topic`
flag next to `--source`, `notchtap:4`); `hooks/notchtap-cmux-hook.sh`
(pass a cwd-derived topic suffix); possibly
`hooks/notchtap-claude-hook.sh`; `docs/V3_6_TECHNICAL_SPEC.md` (amend
the `:256-261` and `:196-200` closed-wire statements, same shape as the
plan-039 amendment at `:304-314`); `CONTEXT.md` (the **Topic** entry,
`CONTEXT.md:64-69`, gains the namespacing rule; **Recurring** needs no
change since wire events stay OneShot).

**REJECTED ALTERNATIVE: estimate M by deferring the namespacing/docs
amendments to a follow-up.** Rejected: the namespacing *is* the feature's
safety case (§1) — shipping wire-accepted raw Topics first and
namespacing later is the one sequencing that strands a live collision
footgun in a release; and a wire-contract change without the spec/
glossary amendments in the same build violates this repo's own
docs-truth discipline (plan 046 exists because drift accumulates
exactly this way).

## 9. Open questions for the maintainer

1. **Is this actually wanted?** No user has asked for it — this is an
   `/improve next`-sourced direction, not a stated request. The football
   card had a concrete watching-the-match motivation (plan 041's filing
   notes operator feedback from live use); the Manual/Cmux burst
   annoyance is asserted by the audit, not reported.
2. **Is the collision/trust risk acceptable at all for a single-user
   local tool?** §1 argues the residual risk (caller-on-caller same-
   suffix collision, after server-side namespacing) is within the
   existing loopback/unauthenticated posture (`http.rs:140-145`) — but
   "consistent with today's posture" and "worth reopening a deliberately
   closed rule for" are different judgments, and the second one is the
   maintainer's.
3. **Would hook authors actually use it?** The cmux hook is this repo's
   own, so adoption there is certain if built; the Claude Code hook's
   payload (session/tool identifiers) hasn't been surveyed for what
   suffix it could pass — a cheap pre-build check, same discipline as
   plan 043's Step 0.
4. **Does the `meta`-on-supersede gap (§4) get fixed as part of this
   build or split out?** It's one line plus one test, but it touches
   shared queue semantics the ESPN producer also rides.
5. **Should the eventual build wait for plan 044 to land first?** §7
   argues the failure *mode* can't arise under OneShot-only, but the
   *reasoning* about same-Topic ordering should be inherited from a
   landed-and-reviewed 044, not a TODO one.
