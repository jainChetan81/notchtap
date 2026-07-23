# Plan 128: Tavily search-powered news connector (customized news, on demand + polled)

> Filed 2026-07-24 (operator feature request: "integrating tavily
> search along with rss feeds to get customized news at call … requires
> api key … customization option in config"). Authored from a dedicated
> API research pass (sources cited inline). NOT YET EXECUTED — ready
> for the standard executor pipeline on the operator's word. Drift
> check: authored against master at plan-125-merge; re-verify cited
> patterns (rss_poller.rs, secrets, settings command triple at 14)
> before executing.

## What it is

A second news source next to RSS: the operator writes search queries
("AI chip export rules", "Aston Villa transfers") in settings; a
poller runs each query against Tavily's search API
(`POST https://api.tavily.com/search`, `topic: "news"`), and fresh,
deduped results enqueue as News-tier one-shot cards exactly like RSS
headlines. Plus an on-demand "Fetch now" button ("at call").

## API facts the implementation encodes (researched 2026-07-24)

- Auth: `Authorization: Bearer tvly-…` header; JSON POST.
  [docs.tavily.com/documentation/api-reference/endpoint/search]
- Request fields used: `query` (≤400 chars), `topic: "news"`,
  `days` (int, news-only recency bound), `search_depth`
  ("basic" = 1 credit, "advanced" = 2), `max_results` (0–20).
  `days` must be omitted unless topic is news (`skip_serializing_if`).
- Response: `results: [{title, url, content, score: f64,
  published_date?: String}]` — `published_date` exists only for news
  topic and has NO guaranteed format → parse leniently (try RFC-2822,
  RFC-3339, `%Y-%m-%d`), fall back to fetch time. Ordering is by
  relevance `score`, NOT recency → sort client-side by parsed date;
  use `score` as a quality floor. Everything beyond
  title/url/content/score is `Option`.
- Errors: `401` bad key; `429` rate-limited (honor `retry-after`
  header); **`432` = plan usage limit exhausted — NOT retryable;
  cease polling until next app launch** and surface it (see Step 5).
  Error body `{"detail": {"error": "…"}}`.
- Budget: free tier 1,000 credits/month; hourly basic polling ≈ 720
  credits/month (fits); every 30 min ≈ 1,440 (doesn't). Default poll
  interval 3600s accordingly.
- No official Rust SDK; the `tavily` crate is unofficial,
  single-maintainer — use plain `reqwest` + serde structs per the
  fields above (matches the rss/espn poller pattern).
- ToS: no attribution mandate; personal-use display of
  title/snippet/link is in scope; API keys are personal → key lives in
  the app's secrets file, never in config.toml, never logged.

## Steps

1. **Secret.** Extend the existing secrets mechanism
   (`~/.config/notchtap/secrets.toml`, `set_secret`/
   `get_secret_status`): new `SecretField::TavilyApiKey` variant
   (follow the telegram/openrouter variants everywhere they appear:
   rust enum + storage key + status struct + frontend `SecretField`
   union + `SecretStatus` type). NO new invoke command needed for the
   key — `set_secret` already takes the field enum. The key must never
   appear in any log line (the notifier's token-redaction precedent).
2. **Config.** New fields on `Config` (all `#[serde(default)]`,
   settings-editable, mirroring the `rss_*` family):
   `tavily_enabled: bool` (default false),
   `tavily_queries: Vec<String>` (default empty; settings textarea,
   one per line, like `rss_feeds`),
   `tavily_poll_secs: u64` (default **3600**, floor 300 — doc the
   credit math in the field's comment),
   `tavily_days: u8` (default 1),
   `tavily_search_depth: String` ("basic"; validate basic|advanced),
   `tavily_max_results: u8` (default 5, clamp 1–20),
   `tavily_min_score: f64` (default 0.0, clamp 0–1),
   `tavily_ttl_secs: u64` (default = rss's 10),
   `tavily_max_per_poll: usize` (default 10),
   `tavily_priority: Priority` (default = rss's default).
3. **Poller** (`src-tauri/src/tavily_poller.rs`, modeled on
   `rss_poller.rs` — read it first and mirror its shape: spawn fn,
   interval with floor, SeenStore-style dedup, `engine.accept`
   ingest):
   - Per tick: for each configured query (serially, small N), POST the
     search; collect results; filter `score >= tavily_min_score`;
     canonicalize URLs (strip query params + trailing slash — Tavily's
     own dedup guidance) and drop already-seen; sort fresh ones by
     parsed `published_date` ascending; take `tavily_max_per_poll`
     across ALL queries combined (not per query); enqueue each as a
     News-tier `OneShot` event: title = result title, body = content
     snippet (truncate at the ingest caps), meta link = the ORIGINAL
     (non-canonicalized) url as literal text (the history/queue
     link-as-text rule), source attribution in meta (e.g. subtitle =
     the url's host).
   - Seen-store: same 7-day MAX_AGE pattern as rss's, keyed on the
     canonicalized URL, persisted alongside the rss one (separate
     file; follow its path conventions).
   - Skip the whole tick (log debug, not warn) when disabled, no key,
     or no queries.
4. **Error handling** (thiserror internally, per CLAUDE.md split):
   401 → warn once per key-change (not per tick), connector
   effectively idle; 429 → sleep `retry-after` (cap 5 min) then
   resume next tick; 432 → set a latched "credits exhausted" flag:
   stop all Tavily fetches until relaunch, log warn ONCE, and push a
   single Low-priority self-card ("Tavily credits exhausted — polling
   stopped") through `engine.accept` so the operator finds out on the
   overlay; transient network errors → the rss poller's existing
   backoff shape.
5. **"At call" — Fetch now.** New settings command `tavily_fetch_now`
   (runs one poll cycle immediately, returns the number of NEW cards
   enqueued). Security triple discipline (CLAUDE.md): add to
   `settings_commands.rs` `SETTINGS_COMMANDS` (14 → 15), lib.rs
   `generate_handler!`, `capabilities/settings.json`;
   `ensure_settings_window` first line; parity tests updated
   (14→15 count assert); `capabilities/default.json` byte-untouched;
   docs count prose updated (CLAUDE.md, AGENTS.md, V5 spec §2 —
   fifteen). Wire the poller so the command and the timer share ONE
   fetch path (no logic fork).
6. **Settings UI.** New "Tavily news" section (or a sub-block of the
   News section — executor picks based on the section layout, one
   screen must show: enable toggle, API-key secret row (SecretRow
   precedent from Connectors), queries textarea, poll interval /
   days / depth segmented (basic|advanced with a "2× credits" help
   line) / max results / min score / TTL / max-per-poll / priority
   controls, and a **Fetch now** button on the ActionStatus pattern
   reporting "N new stories queued" / errors (including the 432
   message verbatim so the user understands billing state).
7. **Tests.** Rust: request serialization (days present with news
   topic; skipped fields), response parse (published_date
   missing/RFC-2822/garbage → fallback), canonicalize_url cases,
   score floor, cross-query max_per_poll cap, dedup across polls,
   error mapping (401/429-with-retry-after/432 latch + single
   self-card), config defaults/clamps, secret-status round trip —
   wiremock for the HTTP layer like the espn/rss suites. Frontend:
   section render, secret row, fetch-now invoke + ActionStatus,
   config round-trip through save. Update TESTING_STRATEGY §0.

## Non-goals / constraints

- No `include_answer`/`extract`/crawl endpoints; search only.
- No connector-health DTO change in this plan (telegram-shaped;
  extending it is a separate decision) — diagnostics via log lines +
  the 432 self-card.
- No API key in config.toml, logs, error messages, or test fixtures
  (use `tvly-test-…` fakes in wiremock only).
- Frontend renders result titles/snippets/links as literal text —
  same untrusted-wire rule as RSS/history/queue.
- `capabilities/default.json` byte-identical. Overlay stays
  receive-only.

## Verification ladder

Full both-sides ladder (cargo test/clippy/fmt from src-tauri/; tsc,
vitest, biome ci, vite build from root) + `git diff
src-tauri/capabilities/default.json` empty.

## STOP conditions

- The secrets mechanism doesn't extend cleanly to a third field
  (report the friction, don't restructure it).
- The single shared fetch path between timer and Fetch-now can't be
  built without an engine/poller redesign.
- Any ambiguity about News-tier vs a new tier for Tavily cards —
  default is News tier (same as RSS); STOP only if that collides with
  a rotation/supersede behavior in a way that surprises.
