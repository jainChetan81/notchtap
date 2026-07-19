# Plan 052 (spike): give News an ambient idle-rail summary, mirroring Football/Weather

> **Executor instructions**: This is a DESIGN SPIKE, not a build plan.
> The deliverable is a design document plus open questions for the
> maintainer — **zero production code changes**. Follow the steps, honor
> the STOP conditions, and when done update this plan's status row in
> `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat f2cbae6..HEAD -- src-tauri/src/status.rs src-tauri/src/rss_poller.rs src-tauri/src/engine.rs src/components/IdleView.tsx`
> Drift doesn't block a spike — but read the drifted regions before
> quoting them in the design doc.

## Status

- **Priority**: P3
- **Effort**: M (coarse — investigation + design doc, no build)
- **Risk**: LOW (docs only)
- **Depends on**: none — but see "Why this matters" for the overlap
  this spike must explicitly bound
- **Category**: direction
- **Planned at**: commit `f2cbae6`, 2026-07-19

## Why this matters

The idle-rail "status-state" channel (plan 034) exists to answer "what's
happening / what's next" while nothing is Visible. Two of its three
poller-backed sources already carry a live ambient summary:
`FootballStatus.live: Option<LiveMatchSummary>` (plan 034) and
`WeatherStatus.current: Option<WeatherSummary>` (plan 040 Part B), both
populated via a dedicated `Engine::update_X` side-channel
(`update_live_match`/`update_weather`) that the idle rail
(`src/components/IdleView.tsx`) renders as a live chip. `NewsStatus`
(`src-tauri/src/status.rs`, plan 034) is just `{ enabled: bool }` — the
idle rail can only show "News" or "News paused" for it
(`IdleView.tsx:29-30`), nothing about what's actually there. This is the
established two-out-of-three pattern's one gap, and News is arguably the
source users most want a glance-without-opening-a-card answer for
("is there anything new"), especially since it's also the source
explicitly excluded from outbound relay (overlay-only by design, per
`CONTEXT.md`'s Connector entry) — the idle rail is the *only* place a
user could ever see a hint of News content without it actually being
promoted to Visible.

**Bound this spike explicitly against the already-deferred "history
surface" finding** (`plans/README.md`'s rejected/deferred list): a
persisted "what did I miss" log is a bigger feature with its own
persistence-decision blocker, explicitly not what this spike is
scoping. This spike is narrower — a single most-recent ambient value,
mirroring the football/weather pattern exactly, not a history log. The
doc must state this boundary explicitly so a build from this spike
doesn't accidentally grow into the deferred feature by scope creep.

## Current state (grounding — quote-verified at `f2cbae6`)

- `src-tauri/src/status.rs:62-66`:

  ```rust
  /// "News paused" in the idle rail means `enabled == false`: the polling
  /// gates are boot-config since v6, so there is no runtime poll pause to
  /// report beyond the gate itself.
  #[derive(Debug, Clone, PartialEq, Serialize)]
  #[serde(rename_all = "camelCase")]
  pub struct NewsStatus {
      pub enabled: bool,
  }
  ```

- The two existing precedents to mirror, both in the same file:
  `FootballStatus` (`status.rs:31-35`, `live: Option<LiveMatchSummary>`)
  and `WeatherStatus` (`status.rs:73-76`, `current:
  Option<WeatherSummary>`), each populated via a dedicated `Engine`
  method (`update_live_match`/`update_weather`, `engine.rs` — locate
  with `rg -n "fn update_live_match|fn update_weather"`) that the
  poller calls whenever its tracked state changes.

- `src/components/IdleView.tsx:25-35` — the frontend rendering pattern
  to mirror:

  ```tsx
  <span className={`src-chip${status.football.enabled ? "" : " dim"}`}>
    {status.football.enabled ? "Football" : "Football off"}
  </span>
  ...
  <span className={`src-chip${status.news.enabled ? "" : " dim"}`}>
    {status.news.enabled ? "News" : "News paused"}
  </span>
  <span className={`src-chip${status.weather.enabled ? "" : " dim"}`}>
    {status.weather.current !== null
      ? `${status.weather.current.tempDisplay} ${status.weather.current.condition}`
      : status.weather.enabled
        ...
  ```

  News's chip today only ever shows two states; Football's/Weather's
  chips show a third, richer state when ambient data is present.

- **Important nuance the doc must engage with** (this spike's biggest
  open design question): unlike football/weather, News does not
  currently track "the newest item" as a distinct piece of state.
  `src-tauri/src/rss_poller.rs:39-77`'s `SeenStore` only proves
  *membership* (has this guid/link been seen before, for dedup) — it is
  a `HashSet<String>` of opaque dedup keys plus an insertion-ordered
  eviction queue, and does **not** retain the title/source/category of
  what it has seen. Getting a "last headline" ambient value therefore
  requires adding a **new**, small piece of state (e.g. "the most
  recently accepted NewsItem's title + source", tracked the same way
  `poller.rs` tracks `last_card`/`last_scoring_play` for football) —
  it is not simply "read an existing value out of `SeenStore`," and the
  doc must not understate this by treating it as a trivial plumbing-only
  change the way `update_weather` reusing an already-computed
  `WeatherSummary` was.

- `NewsItem`'s existing wire fields (title/source/category/
  publishedAtMs — confirm exact field names via `rg -n "struct
  NewsItem"` in `event.rs`) are the candidate content for whatever
  ambient value is chosen.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Read-only exploration | `grep`, `Read`, `rg` | — |
| Confirm `SeenStore` tracks no content | `Read src-tauri/src/rss_poller.rs` around `SeenStore`'s definition | manual confirmation, cited in the doc |
| Confirm nothing changed | `git status` at the end | only the new doc + `plans/README.md` row |

## Scope

**In scope** (the only files you may create/modify):
- `docs/design/news-ambient-status.md` (create)
- `plans/README.md` (status row)

**Out of scope — hard rule for this spike**:
- ANY file under `src/`, `src-tauri/`, or config/build files. No
  prototype code in the repo; illustrative snippets live inside the
  doc.
- Designing the deferred "what did I miss" history/persistence feature
  — explicitly out of scope; if the doc's investigation starts pulling
  toward "we'd need to persist a list," that's a sign the finding has
  drifted into the bigger deferred feature — note the boundary was hit
  and stop there rather than designing the bigger feature.

## Git workflow

- Docs-only commit `docs(design): news ambient status spike` in repo
  style. Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Confirm the SeenStore limitation precisely

Read `rss_poller.rs`'s `SeenStore` and its call sites in full. Confirm
(a) it truly retains no content beyond the dedup key, and (b) where in
`rss_poller.rs` a "most recent NewsItem" value could be captured
alongside the existing dedup insert (the poller already constructs a
full `NewsItem`/`Event` for each fresh story before checking/inserting
into `SeenStore` — the capture point is nearby, not a new fetch).

### Step 2: Write the design doc

`docs/design/news-ambient-status.md`, each section with a
**recommendation and at least one rejected alternative with reason**:

1. **Ambient value shape**: propose a `NewsSummary` struct (mirroring
   `LiveMatchSummary`/`WeatherSummary`'s two-field simplicity) — e.g.
   `{ headline: String, source: String }` from the most recent accepted
   `NewsItem` vs. alternatives (a per-feed "last poll succeeded at"
   timestamp instead of content; an unread count). Recommend one,
   with the explicit trade-off that a headline value could be display-worthy
   in the idle rail while a bare timestamp is lower-value but simpler
   and avoids ever showing potentially sensitive/surprising headline
   text ambiently.
2. **Where the new state lives**: propose adding it as a small
   `Mutex<Option<NewsSummary>>` handle on `Engine`, mirroring the
   existing `live`/`weather` handles exactly (same lock-discipline
   comments apply — read `engine.rs`'s existing `live`/`weather` field
   doc-comments and mirror their "read/clone/drop before locking the
   queue" discipline).
3. **Update path**: a new `Engine::update_news_summary` mirroring
   `update_live_match`/`update_weather`'s shape exactly, called from
   `rss_poller.rs` at the point a fresh, accepted (non-deduped) NewsItem
   is identified — trace the exact call site and confirm it already has
   the needed title/source fields in scope at that point.
4. **Reset semantics**: does the ambient value ever clear (e.g. after
   some age, mirroring nothing today — football's `live` clears when no
   match is in-play, weather's `current` never really "clears", it's
   always the latest reading) — News has no natural "cleared" state the
   way a football match ending does; recommend whether it should persist
   indefinitely until the next item, or is there a staleness cutoff
   worth defining (e.g. don't show a 3-day-old headline as if it's
   current)? This is a real design question, not a mechanical one — flag
   it as the second (after Step 2.1's shape choice) genuinely open
   question.
5. **Frontend rendering**: mirror `IdleView.tsx`'s existing
   football/weather chip pattern (a three-state chip: off / enabled+no-data-yet
   / enabled+ambient-value) rather than inventing a new visual language.
6. **Explicit non-goals**: state plainly that this is NOT the deferred
   "what did I miss" history surface — no persistence across restarts,
   no list of missed items, exactly one most-recent value, gone on
   restart (same as `live`/`current` today).
7. **Test strategy**: name the concrete new cases (a `status.rs` test
   mirroring `snapshot_carries_weather_summary_and_gate`, a
   `rss_poller.rs` test confirming a fresh non-deduped item updates the
   summary and a deduped repeat does not).
8. **Build estimate**: S–M, with the exact file list
   (`status.rs`, `engine.rs`, `rss_poller.rs`, `IdleView.tsx`,
   `useStatusState.ts` if it needs a type update).
9. **Open questions for the maintainer** (e.g.: is a headline the right
   ambient value or does showing story titles ambiently in a
   glanceable overlay raise a different privacy/visibility
   consideration than football scores or weather do; is a staleness
   cutoff wanted).

### Step 3: Sanity-check citations

Every code claim gets a `file:line` valid at the commit read (stamped at
the top of the doc).

**Verify**: `git status` → only the design doc (+ `plans/README.md`
row).

## Test plan

N/A — docs-only spike.

## Done criteria

- [ ] `docs/design/news-ambient-status.md` exists, covers all 9
      sections, each with recommendation + rejected alternative
- [ ] The doc states the commit it was researched against
- [ ] The doc explicitly states the "not the history surface" boundary
      as a non-goal
- [ ] The doc explicitly confirms `SeenStore` needs a new companion
      piece of state (not a direct reuse) for whatever ambient value is
      chosen
- [ ] No source-code changes (`git status` proof)
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- Investigating the ambient-value shape starts requiring a persisted
  list/history rather than a single most-recent value — that's the
  deferred history-surface feature; stop expanding scope and write up
  what boundary was hit instead.
- `SeenStore` or the poller's ingestion flow has changed since `f2cbae6`
  in a way that already tracks a "most recent item" value somewhere —
  re-verify before writing the doc; if it already exists, this spike's
  premise is stale and the doc should instead evaluate exposing what's
  already there.

## Maintenance notes

- If approved, the build plan should mirror `update_weather`'s exact
  shape for `update_news_summary` (same lock discipline, same
  `Mutex<Option<T>>` pattern) and `WeatherStatus`'s exact shape for
  `NewsStatus`'s new field, for consistency with the two established
  precedents.
- This spike and plan 050 (read-only status endpoint) both touch
  `status.rs`/`StatusState` — whoever builds either first should note
  in their own plan/PR that the other's design may still be pending, so
  a reviewer doesn't need to guess whether `StatusState`'s shape is
  final.
