# Design spike: News ambient status in the idle rail

> **Status**: design spike (plan 052), zero production code changes.
> Researched against commit `f2cbae6` (2026-07-19), the commit plan 052
> was written at. Every `file:line` citation below was re-verified by
> reading the file directly (this spike's working copy has no `.git`,
> so citations are grounded in fresh reads, not in the plan's own
> "Current state" quotes).

## Why this matters

The idle-rail `status-state` channel (plan 034) answers "what's
happening / what's next" while nothing is Visible. Two of its three
poller-backed sources already carry a live ambient summary:
`FootballStatus.live: Option<LiveMatchSummary>` (`src-tauri/src/status.rs:31-35`)
and `WeatherStatus.current: Option<WeatherSummary>` (`src-tauri/src/status.rs:73-76`),
each populated through a dedicated `Engine` side-channel
(`Engine::update_live_match`, `src-tauri/src/engine.rs:195-208`;
`Engine::update_weather`, `src-tauri/src/engine.rs:215-228`) that the
idle rail renders as a rich chip (`src/components/IdleView.tsx:19-28`
for football, `src/components/IdleView.tsx:32-38` for weather).

`NewsStatus` is the gap: it is just `{ enabled: bool }`
(`src-tauri/src/status.rs:62-66`), so the idle rail can only ever show
"News" or "News paused" (`src/components/IdleView.tsx:29-31`) — nothing
about what is actually there. News is arguably the source users most
want a glance-without-opening-a-card answer for ("is there anything
new"), and since News events are never offered to Connectors
(`src-tauri/src/rss_poller.rs:422-424`'s comment on the origin gate),
the idle rail is the *only* place a hint of News content can appear
without promotion to Visible.

## Boundary: this is NOT the deferred "what did I miss" history surface

`plans/README.md`'s rejected/deferred list carries a "history surface"
finding — a persisted log of missed items with its own
persistence-decision blocker. This spike is deliberately narrower:
**exactly one most-recent ambient value, mirroring the
football/weather pattern, gone on restart, never a list.** Any design
element that requires persisting more than one item, or surviving a
restart, has crossed into the deferred feature and is out of scope
here (restated as non-goals in §6).

## 1. Ambient value shape

**Recommendation**: a `NewsSummary` struct in `status.rs`, mirroring
`LiveMatchSummary` (`src-tauri/src/status.rs:37-44`) and
`WeatherSummary` (`src-tauri/src/status.rs:52-57`) two-field
simplicity:

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsSummary {
    /// The sanitized `EventPayload.title` of the most recently accepted
    /// NewsItem (already ≤120 chars, `rss_poller.rs:14` TITLE_MAX_CHARS).
    pub headline: String,
    /// `EventMeta.source` — the configured feed name or feed title.
    pub source: String,
}
```

with `NewsStatus` gaining `pub latest: Option<NewsSummary>` next to
`enabled`, mirroring `WeatherStatus` (`src-tauri/src/status.rs:71-76`)
exactly. Both fields are already computed for every fresh item:
`title` is sanitized at `src-tauri/src/rss_poller.rs:289-296` and lands
in `EventPayload.title` (`src-tauri/src/event.rs:129-132`);
`source` is derived at `src-tauri/src/rss_poller.rs:271` and lands in
`EventMeta.source` (`src-tauri/src/event.rs:153`). No new fetch, no new
parse — the value is a byproduct of work the poller already does.

Trade-off to state plainly: a headline is display-worthy in the idle
rail, but it is also *content* — showing story titles ambiently in a
glanceable overlay is a different privacy/visibility posture than
football scores or weather (see §9, open question 1).

**Rejected alternatives**:

- *Per-feed "last poll succeeded at" timestamp instead of content*:
  simpler and never leaks headline text, but it answers "is the feed
  healthy", not "is there anything new" — the question the chip exists
  for. It also has no precedent: both existing ambient values are
  content, not health telemetry. Rejected as the primary value; noted
  in §9 as a fallback if the maintainer rules out showing headlines.
- *Unread count*: "unread" requires a read/acked concept the app does
  not have — the queue is a single-slot rotation, not an inbox; an
  item leaving the slot is not "read". A count would also grow
  unbounded between views or need its own reset semantics, which pulls
  straight toward the deferred history surface. Rejected.

## 2. Where the new state lives

**Recommendation**: a third private ambient handle on `Engine`,
mirroring the existing two exactly (`src-tauri/src/engine.rs:34-35`):

```rust
news_summary: Arc<StdMutex<Option<NewsSummary>>>,
```

constructed next to the others in `Engine::new`
(`src-tauri/src/engine.rs:77-78`), cloned in the hand-written `Clone`
impl (`src-tauri/src/engine.rs:43-57`), and read with the same lock
discipline the rotation loop and the reload re-emit already use:
read/clone/drop the handle BEFORE locking the queue, so nobody ever
holds both locks — the existing comment at
`src-tauri/src/engine.rs:267-269` ("read/clone/drop the live-match
handle BEFORE locking the queue") and its `emit_current_status_blocking`
twin (`src-tauri/src/engine.rs:323-325`, applied at
`src-tauri/src/engine.rs:327-328`) are the pattern to copy verbatim.

**Rejected alternatives**:

- *Store the summary inside `SeenStore`*: conflates dedup state with
  presentation state. `SeenStore` (`src-tauri/src/rss_poller.rs:39-77`)
  is poller-local, testable in isolation, and has a single job
  (membership + bounded eviction). Adding display state to it would
  give the pure `diff_feed` core a second responsibility and would not
  get the value onto `Engine`, where the status snapshot is built.
  Rejected.
- *Derive the summary from the queue* (peek the most recent News
  event): the single-slot queue only holds what is currently Visible
  or waiting; a News item that has already rotated out leaves no
  trace, so the chip would go blank precisely when nothing is showing
  — the exact moment the idle rail exists for. Rejected.

## 3. Update path

**Recommendation**: a new `Engine::update_news_summary(summary:
Option<NewsSummary>)` mirroring `update_live_match`
(`src-tauri/src/engine.rs:195-208`) / `update_weather`
(`src-tauri/src/engine.rs:215-228`) shape-for-shape: lock the handle,
compare, store, `wake.notify_waiters()` only if changed — the same
compare-then-store-then-wake-only-on-change shape, with the same "not
`apply`/`accept`: it never touches the queue" doc-comment
(`src-tauri/src/engine.rs:186-194`).

Call site: the accept loop in `spawn_rss_poller`
(`src-tauri/src/rss_poller.rs:484-488`). At that point every event in
`events` is, by construction, fresh and accepted-worthy — `diff_feed`
has already skipped deduped keys (`src-tauri/src/rss_poller.rs:280-282`),
baseline-pass items (`src-tauri/src/rss_poller.rs:285-287`), and
title-less entries (`src-tauri/src/rss_poller.rs:297-299`), and has
sorted survivors oldest-first with undated last
(`src-tauri/src/rss_poller.rs:343-350`) and capped to the *newest*
`max_per_poll` (`src-tauri/src/rss_poller.rs:352-356`). So the LAST
event of each poll's returned vec is the most recent accepted item,
and it carries both fields in scope: `event.payload.title` and
`event.meta.source` (populated together at
`src-tauri/src/rss_poller.rs:319-339`). After the accept loop, one
call: `engine.update_news_summary(events.last().map(summary_from))`
— mirroring how the espn poller recomputes its summary once per tick
at a single chokepoint (`src-tauri/src/poller.rs:795-803`) rather than
per-event.

Note the deliberate ordering consequence: because `diff_feed` returns
oldest-first, `events.last()` is the newest by `published_at_ms` —
the chip shows the newest headline of the poll, not merely the last
one enqueued.

**Rejected alternatives**:

- *Capture inside `diff_feed` (out-param or side-channel)*: `diff_feed`
  is the pure, exhaustively-tested set-difference core
  (`src-tauri/src/rss_poller.rs:253-259`'s own description) with no
  `Engine` handle; threading state through it would pollute its
  signature (already 8 args, `src-tauri/src/rss_poller.rs:256-259`)
  and every test call site. The accept loop already has the same
  information post-filter. Rejected.
- *Update per accepted event inside the loop*: each intermediate event
  of a multi-item poll would wake the rotation loop for a value that
  is superseded within the same tick; the compare-then-store guard
  would emit them all. Once-per-tick mirrors the espn precedent.
  Rejected.

## 4. Reset semantics

**Recommendation**: the value persists until the next accepted item
replaces it — the weather model (`current` never clears; it is always
the latest reading, `src-tauri/src/status.rs:68-70`), NOT the football
model (`live` clears to `None` when no watched match is in-play,
`src-tauri/src/status.rs:33-34`). News has no natural "ended" state
the way a finished match does; a headline going stale is a gradual,
judgment-call process, not an event. On restart the value is simply
gone (handle starts `None`, as `live`/`weather` do at
`src-tauri/src/engine.rs:77-78`), and the per-feed baseline pass
(`src-tauri/src/rss_poller.rs:285-287`,
`FeedState.baseline` defaulting true at
`src-tauri/src/rss_poller.rs:368-377`) means the boot poll repopulates
`SeenStore` without emitting — so nothing ambient appears until the
first genuinely-new story, which is the correct behavior.

**Rejected alternatives**:

- *Age-based clearing (e.g. hide after N hours)*: there is no natural
  tick to hang the expiry on — the rotation loop wakes on queue
  deadlines and status wakes, so an age cutoff needs either a new
  timer or a staleness check inside `StatusState::snapshot`
  (`src-tauri/src/status.rs:82-106`), which today is a pure read of
  handles. It also needs the cutoff to be a decision (how old is too
  old to show?), which is a product question, not a mechanical one.
  Rejected for v1; flagged as open question 2 (§9) — if the maintainer
  wants it, carrying `published_at_ms` in `NewsSummary` (a third
  field, already available at `src-tauri/src/rss_poller.rs:308-312`)
  makes a frontend-side "dim when older than X" treatment possible
  without any rust-side timer.
- *The football model (clear when "nothing new")*: "nothing new" is
  not observable — the poller cannot distinguish "feed is quiet" from
  "feed is down" from "backoff is waiting"
  (`src-tauri/src/rss_poller.rs:452-454` skips a feed whose backoff
  isn't ready). Rejected.

## 5. Frontend rendering

**Recommendation**: mirror the weather chip's three-state pattern
(`src/components/IdleView.tsx:32-38`) exactly — off / enabled + no
data yet / enabled + ambient value:

```tsx
<span className={`src-chip${status.news.enabled ? "" : " dim"}`}>
  {status.news.latest !== null
    ? `${status.news.latest.headline} · ${status.news.latest.source}`
    : status.news.enabled
      ? "News"
      : "News paused"}
</span>
```

replacing the current two-state chip (`src/components/IdleView.tsx:29-31`).
The TS side mirrors too: a `NewsSummary` type next to
`LiveMatchSummary`/`WeatherSummary` (`src/useStatusState.ts:9-20`),
`news: { enabled: boolean; latest: NewsSummary | null }` in
`StatusState` (`src/useStatusState.ts:22-28`), a matching entry in
`FALLBACK_STATUS` (`src/useStatusState.ts:39-45`), and an
`isValidNewsSummary` check folded into `isValidStatusState`
(`src/useStatusState.ts:73-99`) with the same
`latest === null || isValidNewsSummary(latest)` shape as football's
(`src/useStatusState.ts:94`) and weather's (`src/useStatusState.ts:97`)
guards. `statusRailActive` (`src/useStatusState.ts:106-116`) needs no
change — `status.news.enabled` already activates the rail.

**Rejected alternatives**:

- *A visually distinct "news" treatment* (icon, different chip class,
  marquee/scroll for long headlines): invents a new visual language
  for one source, against the spike's mirror-the-pattern mandate.
  Headlines are already capped at 120 chars
  (`src-tauri/src/rss_poller.rs:14`); CSS truncation on the chip, if
  needed, is a build-time detail, not a new component. Rejected.
- *Reusing the football chip's `live` pulsing-dot style*: the dot
  (`src/components/IdleView.tsx:20-22`) means "a match is in-play
  right now"; applying it to a persistent last-headline value would
  assert a liveness the value does not have (§4). Rejected.

## 6. Explicit non-goals

- **Not the "what did I miss" history surface.** No persisted log, no
  list of missed items, no unread tracking, no restart survival.
  Exactly one most-recent value in process memory, gone on restart —
  same lifetime as `live`/`current` today
  (`src-tauri/src/engine.rs:77-78`).
- No changes to `SeenStore`'s dedup semantics, eviction policy
  (`src-tauri/src/rss_poller.rs:46-47`), or the baseline pass.
- No new outbound surface: News events remain overlay-only and are
  never offered to Connectors (`src-tauri/src/rss_poller.rs:422-424`);
  the ambient summary rides the existing listen-only `status-state`
  channel (`src-tauri/src/status.rs:15`), no invoke.
- No staleness timer in v1 (§4); no per-feed breakdown (one summary
  across all feeds, matching the single `SeenStore` across feeds at
  `src-tauri/src/rss_poller.rs:442`).

**Rejected alternative**: *persist just this one value across restarts*
(so the chip survives a relaunch). It sounds small, but it is the
deferred history feature's persistence decision in miniature — where
to store it, when to expire it, whether a day-old headline should
reappear on boot — and `live`/`current` set the precedent of
process-lifetime-only. Rejected; if wanted, it belongs to the history
surface's own plan.

## 7. Test strategy

- `status.rs`: a `snapshot_carries_news_summary_and_gate` test
  mirroring `snapshot_carries_weather_summary_and_gate`
  (`src-tauri/src/status.rs:264-269`) — snapshot with
  `Some(news_summary())` + `rss_enabled: true` round-trips both; plus
  serialization asserts in the style of
  `serializes_weather_summary_camel_case`
  (`src-tauri/src/status.rs:197-207`) pinning `news.latest.headline` /
  `news.latest.source` camelCase, and `latest` serializing as `null`
  when `None` (mirroring `serializes_live_as_null_when_nothing_in_play`,
  `src-tauri/src/status.rs:190-194`).
- `engine.rs`: an `update_news_summary_wakes_only_on_change` test
  mirroring `update_live_match_wakes_only_on_change`
  (`src-tauri/src/engine.rs:646-678`) and its weather twin
  (`src-tauri/src/engine.rs:688` onward) — store-on-change wakes the
  rotation loop; re-store of the same value does not.
- `rss_poller.rs`: a `diff_feed`-level test asserting the value the
  accept loop would capture — after a baseline pass, a fresh
  non-deduped item appears as `events.last()` with the expected
  title/source (the fixtures at
  `src-tauri/src/rss_poller.rs:729-770` already build exactly this
  scenario), and a deduped repeat returns an empty vec (so no summary
  update), extending the existing dedup coverage at
  `src-tauri/src/rss_poller.rs:698-727`.
- `useStatusState.ts` (frontend): validator tests mirroring the
  football/weather guard cases — `latest: null` valid, a well-shaped
  `latest` valid, a malformed `latest` (missing `headline`) falling
  back to `FALLBACK_STATUS`.

**Rejected alternative**: *relying on the existing generic tests plus
manual frontend checking.* The football/weather landings each shipped
dedicated mirror tests (the ones cited above); a third ambient value
without its own wake-on-change test would leave the compare-then-store
guard — the thing that keeps the channel silent at steady state —
unverified for exactly the new writer. Rejected.

## 8. Build estimate

**S–M.** The two established precedents make every step a copy of an
existing shape; the only genuinely new decision was §1/§4, made above.

- `src-tauri/src/status.rs` — `NewsSummary` struct, `NewsStatus.latest`
  field, `StatusState::snapshot` gains a parameter
  (`src-tauri/src/status.rs:82-89`), tests.
- `src-tauri/src/engine.rs` — `news_summary` handle (struct field,
  `new`, `Clone`), `update_news_summary`, two read sites (rotation
  loop `src-tauri/src/engine.rs:270-271` and
  `emit_current_status_blocking` `src-tauri/src/engine.rs:327-328`),
  tests.
- `src-tauri/src/rss_poller.rs` — the one `update_news_summary` call
  after the accept loop (`src-tauri/src/rss_poller.rs:484-488`) plus
  a small `NewsSummary::from(&Event)` helper, tests.
- `src/components/IdleView.tsx` — the three-state chip (§5).
- `src/useStatusState.ts` — `NewsSummary` type, `StatusState.news`
  field, `FALLBACK_STATUS`, validator (§5).

Note for sequencing: this spike and plan 050 (read-only status
endpoint) both touch `status.rs`/`StatusState` — whichever builds
first should note the other may still be pending, so a reviewer does
not assume `StatusState`'s shape is final.

**Rejected alternative**: *estimating M–L to cover a per-feed design in
the same build.* That would smuggle open question 3 (§9) into the
estimate before the maintainer has answered it; the S–M figure is only
honest for the single cross-feed summary recommended in §1/§2.
Rejected — the per-feed variant, if approved, gets its own estimate.

## 9. Open questions for the maintainer

1. **Is a headline the right ambient value?** Showing story titles
   ambiently in a glanceable always-on overlay is a different
   privacy/visibility posture than football scores or weather — a
   headline can be political, upsetting, or simply something the user
   does not want on their screen at a glance (and News is precisely
   the source excluded from outbound relay for content-sensitivity
   reasons). The fallback if not: the "last poll succeeded" timestamp
   variant rejected in §1, which keeps the chip content-free.
2. **Is a staleness cutoff wanted?** §4 recommends persist-until-replaced
   (the weather model). If a 3-day-old headline should not render as
   if current, the cheapest path is carrying `published_at_ms` in
   `NewsSummary` and dimming/omitting in the frontend — no rust-side
   timer — but whether any cutoff is wanted at all is a product call.
3. **One summary across all feeds, or per-feed?** §6 fixes a single
   cross-feed summary (matching the single shared `SeenStore`). If
   per-feed ambient values are ever wanted, that multiplies the
   handle/snapshot/chip shape and should be its own plan.

**Rejected alternative**: *resolving these questions unilaterally in
this doc* (e.g. just declaring headlines fine, or picking a cutoff).
Question 1 is a product/privacy posture call about what appears on an
always-on overlay, and question 2's cutoff value is arbitrary without
user intent — a spike recommends mechanics, it does not make policy.
Rejected; the recommendations above are deliberately built so either
answer lands without redesign (the timestamp fallback for Q1, the
`published_at_ms` field for Q2).
