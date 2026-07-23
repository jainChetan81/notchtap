# Plan 130: topic news + on-the-go search (Google News query feeds), and honest pacing copy

> Filed 2026-07-24 (operator-approved design: a Topics textarea that
> MERGES with feeds — no either/or mode; an ad-hoc one-shot search;
> clearer poll-cadence copy, executor/author decisions sanctioned).
> Authored at master `da986b5`. Read `rss_poller.rs` and the News
> section fully before coding. Plan 128 (Tavily, ON HOLD) must remain
> untouched — this is the zero-API-key path behind the same UX.

## Step 1 — config: `rss_topics`

`Config.rss_topics: Vec<String>` (`#[serde(default)]`, default empty).
Each entry is a plain-language topic ("aston villa transfers"). At
poller spawn, expand each topic to a Google News query-feed URL:
`https://news.google.com/rss/search?q={urlencode(topic)}&hl=en-US&gl=US&ceid=US:en`
(the `hl`/`gl`/`ceid` triple is required for reliable responses — known
Google News quirk; document it at the expansion fn). Merge the expanded
URLs with `rss_feeds` into ONE feed list for the existing poller loop —
same SeenStore dedup, same TTL/priority/max-per-poll, same News tier.
Topic-derived items: attach the topic string as the item's
subtitle/topic meta (whichever meta field the News cards already
surface — read how feed titles land today and mirror; truncate
sensibly). Google News links are redirect URLs — they flow through the
existing literal-text link rule unchanged.

Unit tests: expansion URL shape (encoding, the param triple), merge
order (feeds first, topics after), empty-topic lines skipped/trimmed.

## Step 2 — settings UI: Topics textarea

News section: a "Topics" textarea directly under "Feeds" (one topic
per line, same textarea component/conventions as feeds), bound to
`rss_topics` via the same text<->list plumbing feeds use. Help copy:
searched via Google News and merged into the same stream; leave Feeds
empty for topic-only news.

## Step 3 — on-the-go search: `search_news_now`

New settings command `search_news_now { query: String } -> usize`
(count of NEW cards enqueued): expands the query exactly like a topic
(Step 1's fn — one shared path, no fork), fetches ONCE, dedups against
the same SeenStore, enqueues through the same ingest, does NOT persist
the query anywhere. Reject empty/whitespace queries with a 4xx-style
error string. Rate-sanity: serialize concurrent calls (a second call
while one runs waits or errors "already searching" — executor picks,
document it).

Security triple discipline (CLAUDE.md "ipc & security"):
`settings_commands.rs` SETTINGS_COMMANDS 14 → **15**, lib.rs
`generate_handler!`, `capabilities/settings.json`, parity tests
updated (count assert), `ensure_settings_window` first line,
`capabilities/default.json` byte-identical (verified). Docs
present-tense counts fourteen → fifteen (CLAUDE.md, AGENTS.md,
`docs/V5_TECHNICAL_SPEC.md` §2; dated/historical statements stay, per
the plan-106 rule).

UI: an input + **Search** button at the bottom of the News section
(ActionStatus pattern: pending disables, success announces "N stories
queued", failure announces the error). The input clears on success.
Works live (no relaunch needed) — unlike the config fields above.

Frontend tests: invoke with typed args, ActionStatus outcomes, empty
input never invokes.

## Step 4 — pacing honesty (author-sanctioned copy/validation decisions)

- "Poll interval" help copy in News, Weather, AND Football sections:
  state plainly that this is also WHEN new cards appear — e.g. News:
  "How often feeds/topics are checked — new stories appear in a burst
  after each check. 600 = news every 10 minutes."
- News "Max per poll" help copy: it is the burst cap — "collecting
  more than this in one interval drops the extras; scale it up if you
  lengthen the poll interval."
- If the settings NumberControl for `rss_max_per_poll` has a max
  clamp below 50, raise it to 50 (rust-side validation too if it
  exists); leave the default at 10.

## Verification ladder

Full both-sides: cargo test/clippy(-D warnings)/fmt from src-tauri/;
tsc, vitest, biome ci, vite build from root. `git diff
src-tauri/capabilities/default.json` EMPTY. §0 both count lines.

## STOP conditions

- The shared expansion path can't serve both the poller and
  `search_news_now` without restructuring the poller loop.
- Any need to touch `capabilities/default.json` or add an HTTP route.
