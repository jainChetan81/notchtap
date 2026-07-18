# Plan 030 (spike): Design the first OpenRouter consumer — best-effort news enrichment into EventMeta

> **Executor instructions**: This is a DESIGN SPIKE, not a build plan.
> The deliverable is a design document plus open questions for the
> maintainer — **zero production code changes**. Follow the steps, honor
> the STOP conditions, and when done update this plan's status row in
> `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat a58f115..HEAD -- src-tauri/src/rss_poller.rs src-tauri/src/settings.rs src-tauri/src/notifier.rs src-tauri/src/event.rs`
> Drift here doesn't block a spike — but read the drifted regions before
> quoting them in the design doc.

## Status

- **Priority**: P2
- **Effort**: M (coarse — investigation + design doc, no build)
- **Risk**: LOW (no code changes; the risk is designing something the
  maintainer rejects, which is what the open-questions section is for)
- **Depends on**: none
- **Category**: direction
- **Planned at**: commit `a58f115`, 2026-07-18
- **Reviewed**: 2026-07-18 at `4281d2c` (review-plan pass) — zero
  drift in the four grounding files; every citation re-verified
  (settings.rs:321-323 comment, EventMeta at event.rs:122-132,
  `SlotState::Showing` at :144-158, poller.rs:516-520 overlay-only
  comment, diff_feed :263 / spawn_rss_poller :442, StatusRailCard.tsx,
  the rejected-findings accretion entry, ci.yml:13-14 no-live-network
  rule); `docs/design/` confirmed absent; openrouter grep confirmed
  storage-plumbing-only. STOP condition calibrated so the plumbing
  hits don't trigger a spurious stop
- **Re-reviewed**: 2026-07-19 at `7430b4b` — drift check now hits
  `event.rs` only (+15, from plan 033): `SlotState::Showing` moved to
  :146-163 and gained `queue_total`/`queue_done` wire fields — a
  worked example of the exact wire extension this spike designs (see
  Current state). The overlay-only comment moved to
  poller.rs:556-560 (plan 034 shifted the file). All other citations
  re-verified in place (settings.rs "waits for the first ai feature"
  now at :322, EventMeta at :122-132, diff_feed :263 /
  spawn_rss_poller :442, `docs/design/` still absent, openrouter grep
  still settings.rs + notifier.rs only). Plan 025 LANDED during this
  reconcile (`920d4e4`, master `12eaefe`): `src-tauri/src/net.rs` now
  exists with `pub(crate) fn build_poll_client()` and
  `read_body_capped` — so the doc's HTTP-posture section should build
  on `net::build_poll_client` (exactly as this plan's grounding
  predicted), while noting its 10 s polling timeout is likely wrong
  for an LLM call and the enrichment path wants its own budget. Also
  new since planning: `useStatusState.ts` /
  `status-state` is a SECOND overlay channel (plan 034) — the doc's
  placement section may cite it as precedent for adding an emission,
  but enrichment rides `slot-state`, not `status-state`

## Why this matters

The app stores, rotates, and 0600-protects an OpenRouter API key that
**no code consumes** — `settings.rs`'s own comment says the key "waits
for the first ai feature." Meanwhile the RSS poller ships `NewsItem`
events carrying `source`/`category`/`published_at_ms`/`link` metadata
into overlay-only status-rail cards. The obvious first consumer, twice
deferred-but-not-rejected by prior audits, is best-effort LLM
enrichment of those news cards (a one-line summary and/or a better
category), folded into `EventMeta`. This spike turns that idea into a
concrete, reviewable design — call shape, placement, failure semantics,
config surface, privacy note — so the maintainer can approve or kill it
before any build effort is spent.

## Current state (grounding — quote-verified at `a58f115`)

- The inert key: `src-tauri/src/settings.rs:321-323` —

  ```rust
  /// Secret values are validated for shape only (spec §4): non-empty, no
  /// whitespace. Nothing reads the openrouter key in v5 — it waits for the
  /// first ai feature.
  ```

  Full plumbing exists and is tested: config table (`OpenRouterTable`,
  ~`settings.rs:238-266`), get/set/status commands, masked display,
  0600 secrets file at `~/.config/notchtap/secrets.toml` (see
  `notifier.rs:179-182` for the mode check and the telegram loader as
  the pattern for reading a secret at boot). **Never print or read the
  actual key value during this spike.**
- The enrichment target: `src-tauri/src/event.rs:122-132` —

  ```rust
  /// News-source metadata (v5): populated only by the rss poller; every
  /// other source leaves it default. Presentation-only — never consulted
  /// by queue/rotation/priority logic.
  pub struct EventMeta {
      pub source: Option<String>,
      pub category: Option<String>,
      pub published_at_ms: Option<i64>,
      pub link: Option<String>,
  }
  ```

  The "presentation-only, never consulted by queue logic" contract is a
  recorded decision (also in `plans/README.md`'s rejected-findings:
  "EventMeta all-`Option` news fields — documented accretion; revisit
  at a second meta-carrying source"). **Enrichment IS that second
  meta-carrying source** — the design doc must address whether fields
  are added to `EventMeta` or nested (e.g. an `enrichment: Option<...>`
  sub-struct), and must keep the presentation-only contract.
- The insertion pipeline: `rss_poller.rs::diff_feed` (~line 263) builds
  `NewsItem` events (pure, well-tested); `spawn_rss_poller`
  (~line 442) fetches, diffs, then enqueues via the shared path which
  emits `SlotState` (whose `Showing` variant at `event.rs:146-163`
  carries the meta fields to the frontend — new enriched fields would
  need to ride the same wire). **Worked example to study**: plan 033
  (landed) added `queue_total`/`queue_done` to `Showing` end-to-end —
  rust variant fields (`event.rs:157-163`) → `isValidSlotState`
  guard in `src/useSlotState.ts:72` → card rendering. The design
  doc's data-shape section should reference that change as the
  template for how enrichment fields would land.
- Relevant repo rules the design must honor:
  - `CONTEXT.md` vocabulary (Promotion, Rotation, Waiting/Visible) —
    use these terms in the doc.
  - `docs/ARCHITECTURE.md` holds locked decisions; a new feature of
    this size is a scope/phase decision → the doc should end with the
    exact ARCHITECTURE/IMPLEMENTATION_PLAN edits it would require, not
    make them.
  - rss/news events are overlay-only, never offered to connectors
    (`poller.rs:556-560` comment as of `7430b4b`,
    `IMPLEMENTATION_PLAN.md` §4.6) — an enriched summary therefore
    never leaves the overlay either.
  - HTTP posture: `net::build_poll_client` if plan 025 has landed,
    else the pollers' inline builder — but note its 10 s poll timeout
    may be wrong for an LLM call; the doc should pick its own budget.
  - Naming: never reference third-party app names; the product name is
    `notchtap`.

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Read-only exploration | `grep`, `Read` | — |
| Confirm nothing changed | `git status` at the end | only the new design doc + plans/README.md row |

## Scope

**In scope** (the only files you may create/modify):
- `docs/design/openrouter-news-enrichment.md` (create; `docs/design/`
  is a new directory — creating it is fine)
- `plans/README.md` (status row)

**Out of scope — hard rule for this spike**:
- ANY file under `src/`, `src-tauri/`, or any config/build file. No
  prototype code in the repo. (Illustrative snippets belong *inside*
  the design doc as fenced blocks.)
- Reading `~/.config/notchtap/secrets.toml` or any secret value.
- Live calls to OpenRouter (spending the user's credits) — pricing and
  model facts may come from public docs (cite links; if offline, mark
  the numbers "unverified — check before build").

## Git workflow

- Branch: work directly on the operator's instruction; a docs-only
  commit `docs(design): openrouter news enrichment spike` in repo
  style.
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Read the pipeline end-to-end

Read `rss_poller.rs` (diff → enqueue → emit), `event.rs` (EventMeta →
SlotState wire), the frontend consumption (`src/useSlotState.ts`
validation + `src/components/StatusRailCard.tsx` rendering of
source/category), and the telegram secret-loading pattern in
`notifier.rs`. Note in the doc where enriched fields would surface in
the card and what `isValidSlotState` would need.

### Step 2: Write the design doc

`docs/design/openrouter-news-enrichment.md` must cover, each as its own
section with a **recommendation and at least one rejected alternative
with the reason**:

1. **Placement**: enrich-before-enqueue (poller await with budget, card
   arrives enriched but delayed) vs enqueue-then-patch (card appears
   instantly, meta updates in place — requires a new "slot meta
   changed" emission or supersession reuse) vs enrich-only-Waiting
   items. Address the single-slot queue's timing reality: cards wait in
   tiers, so enrichment often has the whole Waiting time for free.
2. **Call shape**: endpoint, model choice + fallback, prompt (input =
   title/source/category — is that enough? never the full article:
   only what the feed already gave), token/latency budget, timeout,
   retry (recommendation should default to: one attempt, short
   timeout, no retry — poll cycles come around again).
3. **Failure semantics**: hard rule to propose — enrichment failure or
   timeout must be invisible (card ships exactly as today). Where the
   best-effort boundary lives in code terms.
4. **Data shape**: `EventMeta` extension vs nested struct; what the
   `SlotState::Showing` wire adds; frontend validation impact.
5. **Config & secrets**: config keys (`[openrouter] enabled`, model,
   maybe per-source opt-out), loaded how (mirror the telegram boot
   pattern); behavior when key absent (feature silently off — matches
   `get_secret_status` UX).
6. **Privacy & egress**: headlines/source names leave the machine to a
   third-party API — say it plainly, name it as an explicit maintainer
   decision, and propose the settings-window copy that discloses it.
7. **Cost envelope**: worst-case calls/day from config defaults
   (`rss_poll_secs`, `max_per_poll`, feed count) × prompt size; a
   dedup/cache line (same canonical link never enriched twice —
   the poller already tracks seen keys).
8. **Test strategy**: pure prompt-builder + response-parser fns
   (unit-tested), wiremock for the transport, no live-network tests
   (CI rule at `ci.yml:13-14`: "no live network calls").
9. **Build estimate + phase fit**: S/M/L for the real plan, and the
   exact `ARCHITECTURE.md` §-additions / `IMPLEMENTATION_PLAN.md`
   phase row it would need.
10. **Open questions for the maintainer** — decisions the doc
    deliberately does not make (e.g. is a summary even wanted on a
    glanceable card, or is smarter *category* the actual value?).

### Step 3: Sanity-check the doc against the code

Every code claim in the doc gets a `file:line` reference valid at the
commit you read (state the commit at the top of the doc).

**Verify**: `git status` → only `docs/design/openrouter-news-enrichment.md`
(+ the plans/README.md row) changed/added.

## Test plan

N/A — docs-only spike. The "test" is the Done criteria below.

## Done criteria

- [ ] `docs/design/openrouter-news-enrichment.md` exists and covers all
      10 sections above, each with a recommendation + rejected
      alternative (10 headers greppable)
- [ ] The doc states the commit it was researched against
- [ ] No secret value, no third-party app names, no source-code changes
      (`git status` proof)
- [ ] `plans/README.md` status row updated (spike DONE = doc delivered;
      the *feature* stays undecided until the maintainer reads it)

## STOP conditions

Stop and report back (do not improvise) if:

- You find existing OpenRouter-consuming code (the "zero consumers"
  premise would be stale — re-grep `openrouter` case-insensitively
  across `src-tauri/src/` first). Calibration so you don't STOP on
  plumbing: at review time that grep hits only `settings.rs` (config
  table, secret validation/status) and `notifier.rs` (secrets-file
  parsing + its tests, e.g. the `[openrouter]`-only-file test with a
  dummy `sk-or-x` fixture value). Those are the key's *storage*
  plumbing and are expected. "Consumer" means code that reads the key
  to make a request or references an OpenRouter endpoint — only that
  triggers this STOP.
- The maintainer's docs already contain a decision rejecting AI
  enrichment (search `docs/` + `plans/README.md` for it) — report the
  contradiction instead of writing the doc.
- You feel the need to make a live API call to answer a design
  question — write the question into the open-questions section
  instead.

## Maintenance notes

- If the maintainer approves, the follow-up build plan should reference
  this doc and inherit its budgets/semantics verbatim; if rejected,
  move the finding to `plans/README.md`'s rejected list with the reason
  so the next audit doesn't resurface it.
- The doc's cost envelope goes stale whenever rss defaults change —
  stamp the config values it was computed from.
