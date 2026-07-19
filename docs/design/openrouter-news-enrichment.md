# Design spike: OpenRouter news enrichment into `EventMeta`

> **Status**: design spike (plan 030), zero production code changes.
> Researched against commit `503304097db7d90cda7a097661208152e9822880`
> (`5033040`, "ci: pin actions to commit shas; dependabot keeps them
> fresh"). Every `file:line` citation below was read fresh at this
> commit.
>
> **Drift note**: the plan that spawned this spike was reviewed at
> `f391d07`. Between `f391d07` and this commit the cited files drifted
> further (`rss_poller.rs`, `settings.rs`, `notifier.rs`, `event.rs`,
> `poller.rs` all moved) — most notably plan 037 ("The Engine") landed,
> so ingestion now goes through `Engine::accept` rather than the
> inline-loop-plus-connector-offer shape the original plan described.
> This doc quotes the current shape throughout; nothing below depends
> on the pre-037 ingestion path.

## Why this matters

The app stores, rotates, and 0600-protects an OpenRouter API key that
no code consumes. `src-tauri/src/settings.rs:318-320`:

```rust
/// Secret values are validated for shape only (spec §4): non-empty, no
/// whitespace. Nothing reads the openrouter key in v5 — it waits for the
/// first ai feature.
```

Meanwhile the RSS poller (`src-tauri/src/rss_poller.rs`) ships
`NewsItem` Events carrying `source` / `category` / `published_at_ms` /
`link` metadata into overlay-only status-rail cards. The obvious first
consumer of the stored key is best-effort LLM enrichment of those news
cards — a one-line summary and/or a better category — folded into
`EventMeta`. This doc turns that idea into a concrete, reviewable
design: call shape, placement, failure semantics, config surface,
privacy note, and cost envelope — so the maintainer can approve or
kill it before any build effort is spent.

**Grep confirms zero consumers today**: `grep -rni openrouter
src-tauri/src/` hits only `settings.rs` (the `OpenRouterTable` config
struct, `SecretField::OpenrouterApiKey`, masked-status plumbing) and
`notifier.rs` (secrets-file parsing plus its `[openrouter]`-only-file
test at `notifier.rs:471-474`, fixture value `sk-or-x`). No code reads
the key to make a request or references an OpenRouter endpoint.
`plans/README.md:306-311` confirms this direction was surfaced but
**not rejected** — it's recorded as "grounded... re-raise with
`improve next` when wanted," and is now this very spike.

## Current state (grounding, re-verified at `5033040`)

- **The inert key**: `settings.rs:250-256` defines `OpenRouterTable {
  api_key: Option<String>, extra: toml::Table }`; `settings.rs:262-266`
  lists `SecretField::OpenrouterApiKey` as one of three settable
  fields; `settings.rs:298-316` (`secret_status`) masks it for display
  and never returns the raw value. The secrets file lives at
  `~/.config/notchtap/secrets.toml`, 0600-enforced
  (`notifier.rs:168-187`, `load_secrets`). **This spike never read that
  file or any secret value** — confirmed by `git status` below.
- **The enrichment target**: `event.rs:133-150`:

  ```rust
  /// News-source metadata (v5) plus the rich-relay fields (plan 035:
  /// `subtitle`/`details`): the rss poller populates source/category/
  /// published/link, and `/notify` callers populate subtitle/details;
  /// every other source leaves them default. Presentation-only — never
  /// consulted by queue/rotation/priority logic.
  #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
  #[serde(default)]
  pub struct EventMeta {
      pub source: Option<String>,
      pub category: Option<String>,
      pub published_at_ms: Option<i64>,
      pub link: Option<String>,
      pub subtitle: Option<String>,
      pub details: Vec<DetailItem>,
  }
  ```

  Note `subtitle`/`details` — plan 035 ("rich-relay") already added a
  second meta-carrying source's fields **flat onto `EventMeta`
  itself**, not a nested sub-struct. This is the closest precedent for
  §4 below. The doc-comment's "presentation-only, never consulted by
  queue/rotation/priority logic" contract is a recorded decision (also
  in `plans/README.md`'s rejected-findings) that any enrichment fields
  must preserve.
- **The insertion pipeline** (drifted from plan 037 — "The Engine" has
  landed since this spike was originally scoped):
  - `rss_poller.rs::diff_feed` (`rss_poller.rs:260-359`) is the pure,
    well-tested set-difference-plus-event-builder: dedup against
    `SeenStore`, sanitize title/body, derive source/category, build one
    `Event` per new story, sort oldest-first, cap at `max_per_poll`.
  - `rss_poller.rs::spawn_rss_poller` (`rss_poller.rs:425-492`) is the
    fetch loop: builds an HTTP client via `net::build_poll_client`,
    ticks every `poll_secs.max(15)` seconds, fetches each configured
    feed (conditional GET via ETag/If-Modified-Since), calls
    `diff_feed`, then **for each new event calls
    `engine.accept(event, false).await`** (`rss_poller.rs:484-488`) —
    the one shared ingestion path (plan 037) that enqueues via the
    mutate→wake→emit protocol and fans accepted events out to
    connectors, with one recorded exception:
    `poller.rs:621-627` —

    ```rust
    /// plan 037: ingest goes through `Engine::accept` — the one shared
    /// path that enqueues with the mutate→wake→emit protocol and then
    /// fans accepted events out to every connector (plan §3: "every
    /// accepted event goes to every enabled connector, always" — with
    /// one recorded exception: rss/news events are overlay-only and
    /// never offered, `IMPLEMENTATION_PLAN.md` §4.6 — a rule `accept`
    /// encodes via the origin gate, so no per-caller flag is needed
    /// here).
    ```

    (This comment sits on the espn poller's `spawn_espn_poller`
    function, immediately above it in the same file, and documents the
    same origin-gated rule the rss poller relies on — confirmed
    separately by `engine.rs:378-404`'s
    `accept_offers_manual_but_never_news` test, which asserts
    `"News-origin events are never offered to connectors"`.) News
    events — and therefore any enrichment riding on them — never leave
    the overlay via a connector, regardless of what this doc decides.
  - `event.rs::SlotState::Showing` (`event.rs:169-195`) is the wire
    struct the frontend receives; it mirrors every `EventMeta` field
    flat (`source`, `category`, `published_at_ms` → `publishedAtMs`,
    `link`, `subtitle`, `details`) plus queue-position fields unrelated
    to `EventMeta` (`queue_total`/`queue_done`, plan 033 — added to
    `Showing` end-to-end without ever touching `EventMeta`, since it's
    a queue-position concern, not presentation metadata; a useful
    second example of wire-extension *mechanics*, but not of the
    `EventMeta`-shape question this doc has to answer).
  - Frontend: `src/useSlotState.ts:77-103` (`isValidSlotState`) checks
    every field of a "showing" payload — a new enriched field must be
    added here or a well-formed-but-incomplete payload silently falls
    back to `{state: "empty"}`. `src/components/StatusRailCard.tsx:99-137`
    renders the compact news card (masthead/source, headline title,
    category+age pills); `src/components/Manifest.tsx:45-70` renders
    the expanded manifest, where news items already show a "Summary"
    cell containing the *raw* `body` (the feed's own description) —
    this is exactly the cell an enriched summary would replace or
    supplement (see §4).
- **HTTP posture**: `src-tauri/src/net.rs:8-14`, `build_poll_client`
  (plan 025, landed) sets a 3-redirect limit, a fixed user agent, and a
  **10s timeout** — built for scoreboard/feed polling, not necessarily
  right for an LLM call (see §2).
- **Secret-loading pattern to mirror**: `notifier.rs:161-187`
  (`default_secrets_path`, `load_secrets`) plus its boot-time use in
  `lib.rs:139-162` — gated by a config `enabled` flag, missing/bad-perms/
  malformed file all just **disable the feature with a `tracing::warn!`,
  never a hard failure**; "the app runs overlay-only" is the existing
  house style for "secret absent or bad."
- **CONTEXT.md vocabulary used below**: Promotion, Rotation, Waiting,
  Visible, Topic (`CONTEXT.md:11-77`).
- **CI rule**: `.github/workflows/ci.yml:13-14` — "no live network
  calls... http via wiremock/oneshot" — enrichment's transport tests
  must follow this.

---

## 1. Placement

**Recommendation: enqueue-then-patch is rejected as the sole mechanism;
enrich the item *before* it becomes a `NewsItem` Event is also
rejected. Instead: accept the event immediately, unenriched, and run
enrichment as a detached background task that patches the item in
place via Topic supersession if it finishes in time.**

Concretely: `spawn_rss_poller`'s per-feed loop, after `diff_feed`
returns candidate events and immediately after each is handed to
`engine.accept`, spawns a **detached, budget-timed enrichment task**
per new item. That task later calls back into the engine to patch the
already-Waiting (or already-Visible) item's meta, reusing the Topic
supersession machinery that plan 039's live-match card already
exercises for "update this Notification in place." The event is
accepted (and, if a slot is free, promoted) immediately, unenriched;
if the enrichment task finishes before the item's Rotation ends, the
patch lands live.

Rejected alternatives:

- **Enrich-before-enqueue** (poller awaits the LLM call inline, per
  item, before calling `engine.accept`): rejected because
  `spawn_rss_poller`'s loop is sequential across every configured feed
  inside one `interval.tick()` iteration (`rss_poller.rs:450-489`) — an
  await-per-item here means one slow/stalled OpenRouter call delays
  every other feed's poll that tick, and (worse) delays *display* of a
  story that would otherwise be showable now. The plan's own framing —
  "cards wait in tiers, so enrichment often has the whole Waiting time
  for free" — is the whole point being given up by blocking here.
- **Enrich-only-Waiting items** (skip enrichment entirely once an item
  is Promoted/Visible, on the theory that a mid-Rotation patch is
  jarring): rejected as the *default* because it throws away exactly
  the case where enrichment finishes just as (or shortly after) the
  item gets promoted — Low-priority News-tier items in particular may
  be Promoted quickly on an otherwise-quiet queue, and a Visible
  supersede is already a designed capability (Topic, `CONTEXT.md:64-70`).
  Kept as a config knob, though (§5): a maintainer who finds any
  Visible-time meta change too jarring can disable it and fall back to
  Waiting-only patches.
- **Enqueue-then-patch as a brand-new emission mechanism** ("slot meta
  changed", separate from Topic supersession): rejected because it
  would mean inventing a second wire mechanism in parallel to `Topic`
  supersession, which plan 039 already built and tested for exactly
  "update the Notification the user is already looking at, in place."
  Reusing it is less new surface, not more.

## 2. Call shape

**Recommendation: one HTTP POST per new story, to OpenRouter's chat
completions endpoint, small fast model with a documented fallback,
input capped to what the feed already gave (title + source + category
— never fetch or send the full article body/link contents), 3-4s
timeout, one attempt, no retry.**

- **Endpoint**: `POST https://openrouter.ai/api/v1/chat/completions`
  (OpenRouter's OpenAI-compatible surface — unverified against a fresh
  fetch of OpenRouter's docs during this spike per the "no live calls"
  rule; treat as **unverified — confirm against
  https://openrouter.ai/docs before build**).
- **Model + fallback**: pick a small, cheap, fast instruction model as
  primary (the exact model id is a build-time decision, not this
  spike's; **unverified — check current OpenRouter model catalog/
  pricing before build**) with one fallback model id if the primary
  404s/is deprecated. No runtime model-selection logic beyond "try
  primary, on failure to *build* the request use fallback" — a failed
  *response* just drops (see §3), it does not cascade to the fallback
  mid-request (that would blow the latency budget below).
- **Prompt**: input is exactly `{title, source, category}` — never the
  full article, never `link` fetched server-side (the plan's own
  constraint, restated here as a hard rule: this must not turn into a
  second web-scraping subsystem). Output: a JSON object with an
  optional one-line `summary` (<=140 chars) and an optional `category`
  string, both nullable — the model is told it may return either,
  neither, or both, matching best-effort semantics end to end.
- **Token/latency budget**: prompt is small (well under 200 tokens);
  cap `max_tokens` on the response to ~60 (a 140-char summary plus a
  short category word). Timeout: **3-4 seconds**, deliberately tighter
  than `build_poll_client`'s 10s poll timeout — a poll cycle already
  budgets 10s for the *feed* fetch; an enrichment call riding on top of
  that budget for a background, best-effort feature should not also
  claim the full 10s. Use a **separate client** (or at least a
  per-call `.timeout()` override) rather than reusing
  `build_poll_client`'s client as-is, since 10s is calibrated for
  feed-sized HTTP, not this feature's own budget.
- **Retry: one attempt, no retry.** Rejected alternative: retry once on
  transient failure (mirroring `notifier.rs`'s telegram
  `on_send_failure` policy, `notifier.rs:76-92`) — rejected because
  telegram retries because a dropped outbound message is a *user-visible
  gap* (the whole point of the connector). Enrichment failing back to
  "ship the card as today" has no such gap. There is nothing to protect
  by retrying, and a retry only doubles the odds of a slow response
  outliving its usefulness — the card may already be well into or past
  its Rotation by the time a first attempt would have finished, let
  alone a second.

## 3. Failure semantics

**Recommendation (hard rule): enrichment failure or timeout must be
invisible — the card ships exactly as it does today, unenriched, with
no error surfaced to the user and no impact on Promotion/Rotation
timing.**

- The best-effort boundary lives at the call site of the enrichment
  task, not inside a shared `Result`-returning function that the
  ingestion path has to unwrap: the spawned task's own future resolves
  to `Option<Enrichment>` (`None` on any error, timeout, malformed
  response, or disabled config) and the only thing done with `None` is
  *nothing* — no log spam above `tracing::debug!`, no toast, no
  metadata field synthesized as an error placeholder.
- The task must never be `.await`ed anywhere on the path from `accept`
  to promotion/emission — it is `tauri::async_runtime::spawn`-ed and
  detached, exactly like the existing poller tasks. This guarantees
  timing independence: a card's presence and initial content in the
  overlay never depends on whether OpenRouter is reachable.
- Rejected alternative: surface a subtle "enrichment pending/failed"
  indicator on the card (e.g. a dimmed pill). Rejected because it
  reintroduces exactly the coupling this feature must not have — a
  `None` state that the frontend has to render *something* for is a
  contract, not a best-effort escape hatch. The frontend should not be
  able to tell an unenriched-because-still-pending card from an
  unenriched-because-never-tried card from an unenriched-because-feature-
  disabled card; all three render identically (today's card).

## 4. Data shape

**Recommendation: follow plan 035's flat-field precedent — add
`ai_summary: Option<String>` and `ai_category: Option<String>` as two
more flat fields directly on `EventMeta`, not a nested
`enrichment: Option<...>` sub-struct. Deviation note below.**

- Plan 035 already answered this exact question once: when a second
  meta-carrying source (`/notify`'s hook relay) needed new
  presentation fields, it added `subtitle`/`details` flat onto
  `EventMeta` (`event.rs:145-149`) rather than introducing e.g.
  `EventMeta { rich: Option<RichMeta> }`. `EventMeta` is already an
  all-`Option`/default-empty accretion by design (documented in
  `plans/README.md`'s rejected-findings, restated in the plan text
  above) — a fourth and fifth all-optional field is the same shape of
  change plan 035 already made, not a new category of change. Plan
  033's `queue_total`/`queue_done` addition to `SlotState::Showing`
  (without ever touching `EventMeta`) is a useful second example of
  the wire-extension *mechanics* — rust variant fields →
  `isValidSlotState` guard → card rendering — but it never touched
  `EventMeta`'s shape, so it doesn't bear on this specific choice; the
  recommendation leans on plan 035 alone for the actual decision.
- **Deviation from pure field-parity with 035**: name the new fields
  `ai_summary`/`ai_category` rather than reusing `subtitle`/`category`
  outright. Rejected: overloading `category` (letting an AI guess
  silently replace the feed-derived one) means the frontend and any
  future debugging can no longer tell "the feed said this" from "the
  model guessed this" — and category is the one field this doc's own
  open questions (§10) flag as maybe not even wanted. Keeping them
  distinct, additive fields costs two more `Option<String>`s and one
  more `None`-check everywhere, in exchange for the card being able to
  *prefer* `ai_category` but *fall back* to `category` — a strictly
  better position than an in-place overwrite that can't be undone once
  written.
- **`SlotState::Showing` wire addition**: two more `Option<String>`
  fields, `ai_summary`/`ai_category`, mirrored flat exactly as
  `subtitle`/`link`/etc. already are (`event.rs:179-187`). Same
  `#[serde(rename_all_fields = "camelCase")]` handling applies
  automatically (`aiSummary`/`aiCategory` on the wire) — no new serde
  attribute needed, matching every existing field in that struct.
- **Frontend validation impact**: `isValidSlotState`
  (`src/useSlotState.ts:85-102`) needs two more lines,
  `(obj.aiSummary === null || typeof obj.aiSummary === "string") &&`
  and the equivalent for `aiCategory` — the same pattern already used
  for `subtitle`/`source`/`category`/`link`. `StatusRailCard.tsx`'s
  `newsCategory` derivation (`StatusRailCard.tsx:27`,
  `categoryLabel(slot.category)`) would need to prefer `aiCategory`
  when present; `Manifest.tsx`'s "Summary" cell (`Manifest.tsx:47-50`,
  currently always `body`) would need to prefer `aiSummary` when
  present, falling back to `body` otherwise — never showing both.

## 5. Config & secrets

**Recommendation: `[openrouter]` config table with `enabled` (default
`false`) plus a model override; secret loaded via a new
`load_openrouter_secrets` mirroring `notifier.rs::load_secrets`
exactly; feature silently off (not an error) whenever the key is
absent, matching the existing `get_secret_status` UX and the
telegram-absent boot behavior.**

- **Config keys** (new `[openrouter]` table in `config.toml`, parallel
  to the existing `[connectors.telegram]`/rss keys in `config.rs`):
  `enabled: bool` (default `false` — opt-in, matching `rss_enabled`'s
  own default), `model: Option<String>` (falls back to a hardcoded
  default if unset). A per-source opt-out is deliberately **not**
  proposed here — there is exactly one enrichable source (rss news)
  today, so a per-source toggle would be dead config until a second
  one exists; add it when/if that happens, following the same "don't
  build for a second source that doesn't exist yet" restraint
  `plans/README.md` already applies elsewhere.
- **Loading**: a new function alongside `load_secrets` — same
  0600-or-refuse check, same "missing table is fine, other tables may
  exist" shape now that `SecretsDoc`/`SecretsFile` treat every table as
  optional (`V5_TECHNICAL_SPEC.md:198-201`). Read once at boot
  (`lib.rs`, alongside the existing telegram gate at `lib.rs:145-162`),
  not per-request — an OpenRouter key changed via the settings window
  already requires `save_config_and_relaunch` for every other secret,
  so this is consistent, not a new restriction.
- **Behavior when key absent**: `[openrouter].enabled = true` but no
  key on disk → log once at boot (`tracing::warn!`, same tone as
  "telegram connector disabled: ...") and the enrichment task is simply
  never spawned — identical in shape to how a missing telegram secret
  today means "the app runs overlay-only." No per-poll warning spam.

## 6. Privacy & egress

**Plainly stated: turning this on sends every enriched story's
headline, feed source name, and derived category to a third-party API
(OpenRouter, which itself proxies to one of several further third-party
model providers depending on the chosen model) — this is new egress
beyond anything the app does today, and it is an explicit maintainer
decision to accept, not a detail to bury in a changelog.**

- Today's `/notify` payload only ever leaves the machine when a
  connector (telegram) is configured, and that's *user-authored*
  content the user explicitly pushed via the CLI or cmux. RSS
  headlines are neither user-authored nor currently sent anywhere —
  this feature is the first time passive, ambient content (a news
  feed the user merely subscribed to look at) leaves the machine at
  all.
- **Proposed settings-window copy** (near the `[openrouter]` toggle,
  in the same spirit as the existing secrets-status UI):

  > **AI news enrichment (optional)** — when enabled, each new news
  > card's headline, source name, and category are sent to OpenRouter
  > (a third-party AI routing service) to generate a one-line summary
  > and/or a better category. No article text, links, or any other
  > app data is ever sent. This is best-effort: failures or timeouts
  > just show the card as usual. Off by default; requires your own
  > OpenRouter API key.

## 7. Cost envelope

**Stamped against today's shipped defaults**: `rss_poll_secs = 60`,
`rss_max_per_poll = 10`, one default feed (`config.rs:206-224`,
`default_rss_poll_secs`/`default_rss_max_per_poll`/`default_rss_feeds`).

- **Worst-case calls/day, ignoring dedup**: `(86400s / 60s) × 10 items
  × 1 feed = 14,400` enrichment calls/day per configured feed at
  default cadence, if literally every poll returned a full batch of 10
  brand-new stories (never realistic for a single RSS feed, but it's
  the ceiling the config alone imposes). Each additional configured
  feed scales this linearly.
- **Real-world number is far lower**, because of dedup: `SeenStore`
  (`rss_poller.rs:39-77`) already tracks every story key
  (guid-or-canonical-link, `dedup_key`, `rss_poller.rs:79-85`) across a
  7-day, 1000-key-capped window, and `diff_feed` only ever builds an
  `Event` for a key not already in `seen` (`rss_poller.rs:280-283`).
  **The same store is the dedup/cache line this section is required to
  propose** — no new cache is needed: gating the enrichment task spawn
  on "this is a newly-seen key this tick" (which is already guaranteed,
  since `diff_feed` only returns new-story events at all) means the
  same canonical link is never enriched twice, for free.
- A realistic feed posts perhaps 5-20 new stories/day; at one default
  feed that's ~5-20 calls/day, not 14,400 — the worst case is a
  config-abuse ceiling, not an expected number. The doc still states
  the ceiling because `rss_max_per_poll` is user-configurable and a
  misconfigured (very chatty, very short `rss_poll_secs`) setup really
  could approach it.

## 8. Test strategy

**Pure prompt-builder and response-parser functions, unit-tested; a
wiremock-based transport test; no live-network tests, per
`ci.yml:13-14`.**

- `build_prompt(title, source, category) -> String` (or a small typed
  request struct) — pure, unit-tested against a handful of fixture
  inputs, asserting the exact fields sent and that `link`/`body` never
  appear in the payload (a regression test for the "never the full
  article" rule).
- `parse_enrichment_response(raw_json) -> Option<Enrichment>` — pure,
  unit-tested against: a well-formed response, a response missing one
  of the two fields, a response with an empty/whitespace-only summary
  (should map to `None` for that field, same "shape-only validation"
  spirit as `settings.rs::validate_secret_value`), and malformed JSON
  (must return `None`, never panic/`unwrap`).
- Transport layer: one or two `wiremock`-based integration tests
  mirroring the pattern likely already used for `notifier.rs`'s
  telegram worker tests — a mock server returning success, a 5xx, and
  a slow/timeout response, asserting the timeout budget is honored and
  a failure yields `None` without retry.
- **No live OpenRouter calls anywhere in the test suite** — matches
  `.github/workflows/ci.yml:13-14`'s existing rule, and this spike
  itself made zero live calls (see STOP conditions in the originating
  plan).

## 9. Build estimate + phase fit

**Estimate: M** (medium) for the real build — comparable in shape to
plan 035 (new `EventMeta` fields end-to-end through the wire and
frontend) plus a new outbound HTTP call with its own timeout/retry/
test-double machinery (comparable to the smaller half of the original
v3 telegram connector work), but with no new UI *surface* beyond one
settings toggle and model field, since a masked-secret input this
close to `openrouter_api_key`'s existing settings-window handling
already exists.

- **`ARCHITECTURE.md` addition**: a new locked-decisions section
  (following the existing numbering — the last is `## 18. espn
  live-match card (locked 2026-07-19, plan 039)`, so this would be
  `## 19. openrouter news enrichment (locked <date>, plan <N>)`),
  recording: the enqueue-then-patch-via-Topic-supersession placement
  decision (§1), the best-effort/invisible-failure contract (§3), the
  flat-`EventMeta`-fields data shape (§4), and the privacy/egress
  decision + disclosure copy (§6) verbatim from this doc.
- **`IMPLEMENTATION_PLAN.md` phase row**: a new subsection following
  the existing numbering — the last landed news-related entry is `##
  4.6. v5 — news source (rss poller + status-rail news cards) — ✅
  landed 2026-07-17` (with its own `4.6.2` sub-phase for "open current
  story"); this would land as `## 4.7. news enrichment (openrouter,
  first ai feature)` with its own exit criteria mirroring §8's test
  list plus a manual checklist row (`docs/TESTING_STRATEGY.md` §5-style)
  for "enrichment visibly changes a real card within its Rotation
  window on a live feed" — physical/live-network behavior that, like
  notch geometry, isn't worth automating.

## 10. Open questions for the maintainer

Decisions this doc deliberately does not make:

1. **Is a one-line AI summary actually wanted on a glanceable card, or
   is a smarter *category* the real value?** The card is already
   optimized for a fast glance (`StatusRailCard.tsx`'s compact view
   shows only masthead + headline + two pills); an AI summary mostly
   helps the *expanded* Manifest view's existing "Summary" cell, which
   today just echoes the feed's own description. If the feed's own
   description is already good enough, `ai_summary` may be the weaker
   half of this feature relative to `ai_category` (which improves the
   compact card's category pill, always visible).
2. **Is per-item latency (enrichment sometimes lands mid-Rotation, on
   a Visible card the user is already looking at) an acceptable UX, or
   should this ship Waiting-only until proven non-jarring?** §1
   recommends allowing Visible patches by default with a config knob
   to disable; the maintainer may prefer starting conservative
   (Waiting-only) and loosening later.
3. **Which model, and is a hardcoded fallback model id acceptable, or
   should the settings window let the user pick between 2-3 curated
   options instead of a free-text model field?** A free-text field is
   less UI work but risks a typo silently disabling the feature (no
   validation beyond "non-empty" is proposed elsewhere in this spec).
4. **Should `ai_category`, once populated, ever get written back into
   the plain `category` field (e.g. to also improve `categoryClass`
   theming, `StatusRailCard.tsx:66`) or must it stay purely additive
   and display-preferential, never mutating the feed-derived value?**
   §4 recommends purely additive; a maintainer optimizing for visual
   consistency (category-based card theming) might want the reverse.
5. **Worth a "test enrichment" button in the settings window**,
   mirroring the existing per-source `send_test_notification` pattern
   — or does that cross into "the app's first outbound AI call is a
   manual settings-window button," which `V5_TECHNICAL_SPEC.md:244`
   explicitly ruled out once already ("no 'test key' button... out of
   scope")? This doc leans toward **no**, consistent with that existing
   ruling, but flags it since a maintainer approving a real *use* of
   the key might reconsider the *test* button too.

---

## Verification (this spike)

- `git status` at the end of this spike: only this file is new/changed
  (no `plans/README.md` edit — per the reviewer override, that file is
  intentionally untouched by this executor).
- No secret value was printed or read; `secrets.toml` was never opened.
- No third-party app name appears anywhere in this doc; "OpenRouter" is
  named because it is the actual product the stored key belongs to and
  the subject of this design, not a naming-convention violation (this
  repo's naming rule bars referencing *other notification/productivity
  apps'* names/branding — it does not bar naming the AI-routing vendor
  whose key the app already stores).
- No live network call was made to OpenRouter or any other host during
  this research; pricing/model-catalog facts are marked "unverified —
  check before build" per the plan's scope rule.
