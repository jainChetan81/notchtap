# Source scope review — keep/cut/demote for the four ambient sources

**Status**: decision document (plan 153) — recommendations only, zero code changed.
**Researched against**: commit acdaeb0
**Awaiting**: operator decision on four verdicts.

> Terminology follows `CONTEXT.md`: **Origin** (which source category
> produced an Event), **Rotation Order** (the configured Origin ranking
> used as a same-tier tie-break), **Recurring** (a Rotation kind that
> requeues instead of dropping), **Topic** (the supersession identity a
> Recurring Event carries), **Poller** (an internal source that turns
> observed external *changes* into Events). "Ambient" is used here in the
> sense the code uses it — a summary folded into the idle rail's
> `StatusState` rather than accepted as an Event.

## Prior art consulted

- `CONTEXT.md` — the glossary. Settles that Origin's only job is
  Rotation Order; that a Poller emits deltas only; and that Recurring +
  Topic are queue concepts, not source concepts. Note the trap this
  creates: `rss_poller.rs`'s `topic` field (`rss_poller.rs:98-146`) is an
  RSS *search query* label, not the glossary's supersession Topic. The
  two are unrelated.
- `docs/ARCHITECTURE.md` §18 (espn live-match card, locked 2026-07-19) —
  settles that the live card is opt-in behind `espn_live_card`, default
  `false`; that its Topic identity is `espn:{league}:{match_id}`; and
  that it needed no `engine.rs`/`queue.rs` change because it is that
  machinery's *first caller*, not a change to it.
- `docs/ARCHITECTURE.md` §19 (weather source, locked 2026-07-19) —
  settles that weather is keyless Open-Meteo with no secrets; that
  current conditions are ambient (`Engine::update_weather`) while
  threshold alerts are ordinary Events; that alerts are edge-triggered;
  and that it added no `#[tauri::command]`.
- `docs/design/scoreboard-topic-card.md` (plan 031 spike) — settles that
  the queue's Topic/supersession machinery was fully built and fully
  tested with **zero** production producers, and that the ESPN poller was
  the natural first one. Its own header warns its `file:line` citations
  are stale; every citation in this document was read fresh at `acdaeb0`.
- `docs/design/news-ambient-status.md` (plan 052 spike) — recommended a
  `NewsSummary` in `status.rs` plus an `Engine::update_news_summary`
  side-channel so the idle rail could say what news is there. **This was
  never built**: `NewsStatus` is still exactly `{ enabled: bool }`
  (`src-tauri/src/status.rs:126-128`), and `engine.rs` has
  `update_live_match`, `update_weather` and `update_now_playing` but no
  news twin. So the spike's premise — that News is the source users most
  want a glance answer for — still stands unaddressed.
- `docs/design/now-playing-mediaremote.md` (plan 095 spike) — a
  well-evidenced **NO-GO** on calling `MediaRemote.framework` directly.
- `docs/design/now-playing-adapter.md` (plan 103 spike) — a **GO,
  conditional** that reverses 095 only for the Perl-hosted
  `ungive/mediaremote-adapter` mechanism, and explicitly says the GO
  "should be treated as void" if a stated condition breaks. This is the
  only one of the four sources whose existence rests on a conditional,
  revocable verdict.
- `docs/design/per-source-config-consolidation.md` (plan 049 spike) —
  recommends *not* consolidating the flat per-source config fields until
  a sixth source is seriously proposed (§7). Relevant here because
  "trim the weight" and "consolidate the config" are different moves,
  and this spike already declined the second one.
- `plans/README.md` — the Telegram-connector removal paragraph (line 46)
  is the removal template used by the checklists below: worker + config
  block + secret fields + one `#[tauri::command]` removed; the generic
  framework underneath kept; `ARCHITECTURE.md` given a dated reversal
  note; historical plan files left as filed history. Its "Findings
  considered and rejected" section was read so nothing already declined
  is re-raised here.

## Cost table

Every number below was produced by a command at `acdaeb0`, not estimated.
"rust lines (non-test)" is the last `#[cfg(test)]` line number minus one.
"tests" is `cargo test --lib -- --list | grep -c '^<module>::'`, with
`docs/TESTING_STRATEGY.md` §0's recorded number in parentheses — §0's
header pins its numbers to commit `9ca81f9`, so the gaps are expected
staleness, not drift.

| source | rust lines (total) | rust lines (non-test) | cfg(test) count | tests | config fields | settings file | overlay files | external dependency | files touched if removed |
|---|---|---|---|---|---|---|---|---|---|
| Football | 3207 (`poller.rs` 2806 + `crests.rs` 401) | 1768 (1552 + 216) | 3 (`poller.rs` 2 — last at 1553; `crests.rs` 2 — last at 217) | 71 = poller 59 (§0: 56) + crests 12 (§0: 10) | 7 | `src/settings/sections/FootballSection.tsx` | `src/components/LiveMatchScorecard.tsx`, `src/components/LiveMatchScorecard.test.tsx`, `src/overlay/live-scorecard.css` | ESPN's public `site.api.espn.com` + `sports.core.api.espn.com` JSON, polled every `espn_poll_secs` = 30s by default; unauthenticated, no key, no secret; crest images fetched from `a.espncdn.com` behind `crests.rs`'s host allowlist and cached on disk. No out-of-band build step. | 38 |
| News | 1822 (`rss_poller.rs`) | 760 | 1 (at 761) | 53 (§0: 28) | 7 | `src/settings/sections/NewsSection.tsx` | `src/overlay/news-category.css` | Arbitrary RSS/Atom feed URLs from `rss_feeds`, plus Google News RSS search URLs synthesised from `rss_topics` by `expand_topic_url` (`rss_poller.rs:98-100`); polled every `rss_poll_secs` = 60s by default; unauthenticated, no key. The stored `openrouter_api_key` secret is **not** read by the news path — `settings.rs:411` records that nothing reads it. No out-of-band build step. | 27 |
| Weather | 1272 (`weather_poller.rs`) | 633 | 1 (at 634) | 40 (§0: 29) | 11 | `src/settings/sections/WeatherSection.tsx` | `src/lib/weatherArt.ts`, `src/lib/weatherArt.test.ts`, `src/overlay/weather-art.css`, `src/assets/weather/` (13 files: 12 SVGs + `NOTICE`) | Open-Meteo `api.open-meteo.com/v1/forecast` (`weather_poller.rs:475`), polled every `weather_poll_secs` = 900s by default; keyless, unauthenticated, no secret handling. No out-of-band build step. | 29 |
| Now Playing | 805 (`now_playing.rs`) | 477 | 1 (at 478) | 22 (§0: 16) | 3 | **No section file of its own** — the user-facing toggle lives in `src/settings/sections/GeneralSection.tsx`, and `now_playing_adapter_enabled` is deliberately absent from the UI entirely (`config.rs:153-161`: it is the config-file-only kill switch), as is `now_playing_adapter_dir` (pinned server-side by `settings::pin_uneditable_fields`). | **No whole file of its own** — the media row is `MediaPeekRow` inside `src/components/IdleHoverPeek.tsx:273-330`, a component otherwise owned by the weather peek | Not a network service: a supervised local subprocess. `/usr/bin/perl` hosts the **vendored** `ungive/mediaremote-adapter` (`src-tauri/vendor/mediaremote-adapter/`, 33 files) to reach Apple's private `MediaRemote.framework`. **This is the only source with an out-of-band build step** — `just build-media-adapter` runs CMake twice and installs the framework under `~/Library/Application Support/notchtap/`. Push-based, no poll interval. | 10 |

**Shared, not attributable — named once.** These files serve more than
one source and would survive any single removal, so they appear in no
row above: `src/components/StatusRailCard.tsx` (+ its test),
`src/components/StatusDots.tsx` (+ its test),
`src/components/Manifest.tsx` (the generic expanded panel — despite the
name, not news-specific), `src/components/NotificationBody.tsx`,
`src/overlay/card-chrome.css`, `src/overlay/choreography.css`,
`src/overlay/status-dots.css`, `src/overlay/source-identity.css`,
`src/overlay/idle-peek.css`, `src/overlay/manifest.css`,
`src/overlay/masthead-content.css`, `src/lib/presentation.ts`,
`src/lib/sourceColors.ts`, `src/settings/previewFixtures.ts`,
`src/settings/types.ts`, `src/settings/sections/HistorySection.tsx`,
`src-tauri/src/config.rs`, `src-tauri/src/event.rs`,
`src-tauri/src/engine.rs`, `src-tauri/src/queue.rs`,
`src-tauri/src/status.rs`, `src-tauri/src/settings.rs`,
`src-tauri/src/lib.rs`, `src-tauri/src/net.rs`.

**One caveat on the "files touched if removed" counts.** They come from
the plan's prescribed search terms (`Football|espn_|espn`, `News|rss_`,
`Weather|weather_`, `now_playing|nowPlaying`) run across `src`,
`src-tauri/src` and `src-tauri/tests`. Those terms are deliberately
broad, so a handful of hits are cross-reference comments rather than
real coupling — e.g. `now_playing.rs:16` names espn/rss/weather only to
describe the shared poller shape, and `poller.rs:1227` cites
`rss_poller::expand_topic_url` for its URL-encoding rule. The
non-obvious *real* hits, per source:

- **Football** — `src-tauri/src/net.rs` (the espncdn-only redirect
  policy for crest fetches), `config.rs:486`'s `default_rotation_order`
  (Football is the first entry), `src/settings/sections/HistorySection.tsx`
  (`historyEspnSummary` renders stored score/clock/card meta),
  `src/useSlotState.ts` (the optional `EspnMeta` wire block),
  `src/lib/sourceColors.ts` (`football: "#7fe08d"`),
  `src/settings/previewFixtures.ts` (four of its preview cards are
  football), and six JSON fixtures under `src-tauri/tests/fixtures/`.
- **News** — `src-tauri/src/engine.rs:314` (the Connector gate: `News`
  is the one Origin never offered to a Connector),
  `default_rotation_order` (News is last), `src/lib/sourceColors.ts`
  (the origin colour `#ff6b57` *plus* a whole per-category colour map
  with a parity test against `news-category.css`),
  `src/components/Manifest.tsx` and `src/overlay/masthead-content.css`
  (shared, but carry news-shaped branches).
- **Weather** — `default_rotation_order` (Weather sits right after
  Manual), `src-tauri/tests/fixtures/open-meteo-bangalore.json`,
  `src/useStatusState.ts` (+ its test) for the ambient chip,
  `src/lib/sourceColors.ts` (`weather: "#f0c46a"`).
- **Now Playing** — `src-tauri/src/agents/board.rs` (a naming/adjacency
  reference, not a data dependency), `src-tauri/src/status.rs`
  (`MediaStatus` on the wire), `src/settings/types.ts`.

## What each source gives

### Football (Origin `Football`)

The user sees goal/card/kickoff/half-time/full-time cards rendered
through the shared `src/components/StatusRailCard.tsx`, and — only when
`espn_live_card` is on — a single updating live scoreboard drawn by
`src/components/LiveMatchScorecard.tsx` with team crests fetched and
cached by `src-tauri/src/crests.rs`. The idle rail also carries a live
match chip via `FootballStatus.live` (`src-tauri/src/status.rs:32-35`).
The operator has `espn_enabled = true` in
`~/.config/notchtap/config.toml:6`, but also `espn_live_card = false`
(line 19) and `espn_rich_events = false` (line 20) — so on the live
machine the flagship card is off and football is a burst of ordinary
one-shot cards. `docs/ARCHITECTURE.md` §18 locks the design, and
`docs/design/scoreboard-topic-card.md` records why it exists.

**It is load-bearing, and this is the single most important finding in
this document.** `poller.rs:531` is the *only* place in production that
constructs `RotationSpec::Recurring`; every other occurrence
(`queue.rs`, `lib.rs:2088`, `engine.rs:1103`) is inside a test module.
Likewise `poller.rs`'s `card_topic`/`make_event` pair
(`poller.rs:511-538`) is the only production code that sets
`Event.topic` to anything but `None` — `http.rs:333`,
`settings.rs:633/649/674/693/719`, `weather_poller.rs:396`,
`agents/notification.rs:236` all hardcode `topic: None`. So removing
football would return the queue's Topic-supersession and Recurring
machinery to the producerless state `docs/design/scoreboard-topic-card.md`
described in the first place. Note the machinery is *already* inert on
the operator's machine, because their `espn_live_card` is `false`.
`src/components/LiveMatchScorecard.tsx` exists only for football; it is
imported once, by `StatusRailCard.tsx:31`.

### News (Origin `News`)

The user sees Low-priority headline cards with a per-category colour
scheme (`src/overlay/news-category.css`, mirrored in
`src/lib/sourceColors.ts` with a parity test), rendered by the shared
`src/components/StatusRailCard.tsx`, plus a Settings "search now" button
backed by the `search_news_now` invoke command
(`src-tauri/src/settings_commands.rs:64`). No section of
`docs/ARCHITECTURE.md` locks News specifically; the one design doc that
addresses it, `docs/design/news-ambient-status.md`, describes a gap
rather than a shipped feature.

**It is the least load-bearing of the four, and the only one with no
ambient presence at all.** `NewsStatus` is still exactly
`{ enabled: bool }` (`src-tauri/src/status.rs:126-128`), so the idle
rail can render "News" or "News paused" and nothing more — plan 052's
recommended `Engine::update_news_summary` was never built, while its two
siblings `update_live_match` and `update_weather`, and the later
`update_now_playing`, all exist (`engine.rs:343/351/360`). News is also
the one Origin structurally barred from leaving the machine
(`engine.rs:314`), so cards are its entire output. Whether that output
is worth 1822 lines depends on whether the operator reads the headlines,
which the code cannot tell you — see "Questions for the operator" #3.

### Weather (Origin `Weather`)

The user sees two distinct things: an always-on ambient chip in the idle
rail (temperature + condition art, `src/lib/weatherArt.ts` picking one of
12 SVGs in `src/assets/weather/`, drawn by
`src/components/IdleHoverPeek.tsx` and `src/components/StatusDots.tsx`),
and occasional Medium-priority rain/temperature alert cards.
`docs/ARCHITECTURE.md` §19 locks the design. The operator has
`weather_enabled = true` (`~/.config/notchtap/config.toml:34`).

**It is not load-bearing for anything else**, but it is the cheapest
real Poller in the tree: 1272 lines, keyless, no secrets, a 900s poll
interval, and no out-of-band build step. Its ambient half is also the
one thing on the idle surface that is useful without any agent running,
which makes it structurally different from the other three — it earns
screen time passively rather than by producing cards.

### Now Playing (no Origin — ambient only)

The user sees one row inside the hover peek: track title, artist/album,
an app icon, and a progress bar that glides in real time
(`MediaPeekRow`, `src/components/IdleHoverPeek.tsx:273-330`). It never
becomes a Notification and never enters the Slot, which is why it has no
`SourceKind` variant. The operator has both gates on:
`now_playing_enabled = true` and `now_playing_adapter_enabled = true`
(`~/.config/notchtap/config.toml:53-54`). No `ARCHITECTURE.md` section
locks it; `docs/design/now-playing-adapter.md` §10 records a
**conditional** GO that explicitly says it should be treated as void if
its conditions break.

**It is not load-bearing for anything else, but it carries a maintenance
shape none of the others do.** It depends on Apple's private
`MediaRemote.framework`, reached through a vendored third-party Perl
loader (`src-tauri/vendor/mediaremote-adapter/`, 33 files) that must be
compiled by hand via `just build-media-adapter` and installed outside the
app bundle. `config.rs:154-162` records the risk in the repo's own
words: if Apple closes the oversight this relies on, "the failure
degrades silently to 'no data,' indistinguishable from 'nothing
playing'" — which is exactly why the file-only kill switch exists.
Whether that risk is worth carrying depends on whether the operator
actually hovers to look at the media row; the code cannot tell you — see
"Questions for the operator" #5.

## If cut — removal checklists

These are hypothetical. Nothing below has been done, and nothing below
should be started without an operator CUT verdict on that source. Each
line describes work that *would* be required, not work to do now.

### Football

- [ ] `src-tauri/src/poller.rs` (2806 lines) and `src-tauri/src/crests.rs`
      (401 lines) would be deleted, along with the `mod` lines in
      `src-tauri/src/lib.rs` and the spawn call in its `setup`.
- [ ] `src-tauri/src/net.rs`'s espncdn-specific redirect policy would
      become dead and would need either deletion or re-justification for
      the remaining rss/weather callers.
- [ ] The seven `pub espn_*` fields (`config.rs:18-37`) and their
      `default_espn_*` functions would be removed. **Migration**: safe —
      `Config` carries `#[serde(default)]` and *not*
      `deny_unknown_fields` (`config.rs:8-9`), so leftover `espn_*` keys
      in an operator's existing `config.toml` are ignored rather than
      turned into a parse error. `config.rs:1009-1017` already pins that
      behaviour for the removed `[connectors.telegram]` table; an
      equivalent leftover-`espn_*` test would be the natural addition.
- [ ] The `SourceKind::Football` variant (`src-tauri/src/event.rs:88`)
      and its 34 occurrences across `config.rs`, `event.rs`, `queue.rs`,
      `settings.rs` and `poller.rs` would go, including the exhaustive
      `match` arms at `queue.rs:61` (`source_kind_label`) and
      `settings.rs:626` (the per-source test-notification builder).
- [ ] `default_rotation_order` (`config.rs:486`) would drop its first
      entry, leaving `[Manual, Weather, Agent, News]`.
- [ ] `src/settings/sections/FootballSection.tsx` would be deleted. **No
      `#[tauri::command]` is football-only**, so the 4-place
      seventeen-command allowlist parity
      (`settings_commands.rs`'s array + its
      `assert_eq!(SETTINGS_COMMANDS.len(), 17)`, `lib.rs`'s
      `generate_handler!`, `capabilities/settings.json`, `build.rs`)
      would be untouched.
- [ ] Frontend: `src/components/LiveMatchScorecard.tsx` and its test and
      `src/overlay/live-scorecard.css` would be deleted; the
      `LiveMatchScorecard` import and branch in `StatusRailCard.tsx`, the
      football branches in `src/lib/presentation.ts` (including
      `footballEventKindFor`), the `football` token in
      `src/lib/sourceColors.ts`, the `EspnMeta` block in
      `src/useSlotState.ts`, `historyEspnSummary` in
      `HistorySection.tsx`, and the four football entries in
      `src/settings/previewFixtures.ts` would all be pruned.
- [ ] Six JSON fixtures under `src-tauri/tests/fixtures/` and 71 rust
      tests would be deleted, followed by a
      `docs/TESTING_STRATEGY.md` §0 recount.
- [ ] `docs/ARCHITECTURE.md` §18 would gain a dated reversal note
      (Telegram template), and §19's reference to "espn High" bracketing
      the weather priority would need rewording. `CONTEXT.md`'s Origin,
      Poller, Score Update and Match State entries all name football and
      would need editing.
- [ ] The queue's Topic/Recurring machinery would be left with zero
      production producers again — a deliberate decision the operator
      would need to make explicitly: keep it (Telegram-template: keep the
      generic framework for a future consumer) or delete it too. Deleting
      it touches `queue.rs`, which this review treats as out of scope.

**Removal effort: L** — the largest module pair in the review, the most
`SourceKind` occurrences (34), six fixtures, 71 tests, and a knock-on
decision about producerless queue machinery that reaches into a file
explicitly not under review.

### News

- [ ] `src-tauri/src/rss_poller.rs` (1822 lines) would be deleted, along
      with its `mod` line and spawn call in `src-tauri/src/lib.rs`.
- [ ] `expand_topic_url` would go with it — `poller.rs:1227` only cites
      it in a comment about URL encoding, so no code dependency survives.
- [ ] The seven `pub rss_*` fields (`config.rs:40-64`), the
      `RssFeedConfig` struct and the `default_rss_*` functions would be
      removed. **Migration**: safe for the same reason as football — no
      `deny_unknown_fields`, so leftover `rss_*` keys and `[[rss_feeds]]`
      tables are ignored.
- [ ] The `SourceKind::News` variant and its 28 occurrences across
      `config.rs`, `event.rs`, `rss_poller.rs`, `queue.rs`, `engine.rs`
      and `settings.rs` would go, including `queue.rs:61`'s
      `source_kind_label` arm and `settings.rs`'s test-notification arm.
- [ ] `engine.rs:314`'s `if to_offer.origin != SourceKind::News` Connector
      gate would collapse to an unconditional offer. **This changes a
      documented contract** — `CONTEXT.md`'s Connector entry and
      `docs/IMPLEMENTATION_PLAN.md` §4.6 both state that News never
      leaves the machine; both would need editing. The gate is currently
      moot in practice (zero Connectors are registered since the Telegram
      removal), but it is a stated guarantee, not an implementation
      detail.
- [ ] `default_rotation_order` (`config.rs:486`) would drop its last
      entry, leaving `[Football, Manual, Weather, Agent]`.
- [ ] `NewsStatus` (`status.rs:126-128`) and the `news` field on
      `StatusSnapshot` would be removed.
- [ ] `src/settings/sections/NewsSection.tsx` would be deleted, **and the
      `search_news_now` command would become unused**. That is the one
      security-load-bearing part of this checklist: all four places must
      change together — `settings_commands.rs`'s array *and* its
      `assert_eq!(SETTINGS_COMMANDS.len(), 17)` (which would become 16),
      `lib.rs:297`'s `generate_handler!` entry,
      `capabilities/settings.json`'s `allow-search-news-now`, and
      `build.rs`'s `AppManifest::commands(&[...])` list. The Telegram
      removal is the exact precedent (it took the count 18 → 17).
      `CLAUDE.md`'s "seventeen invoke commands" paragraph would also need
      the new number.
- [ ] Frontend: `src/overlay/news-category.css` would be deleted; the
      news category map and its parity test in `src/lib/sourceColors.ts`
      (`sourceColors.test.ts` asserts the literal hexes appear in that
      stylesheet), the `news_item` event type in
      `src/lib/presentation.ts`, the news branches in
      `Manifest.tsx`/`NotificationBody.tsx`/`masthead-content.css`, the
      `news_item` history label in `HistorySection.tsx`, and the news
      preview fixture would all be pruned.
- [ ] 53 rust tests plus the news cases in `src/useStatusState.test.ts`
      and `src/settings/SettingsApp.test.tsx` would be deleted, followed
      by a `docs/TESTING_STRATEGY.md` §0 recount.
- [ ] `docs/design/news-ambient-status.md` would be marked
      superseded/void, and `docs/IMPLEMENTATION_PLAN.md` §4.6,
      `CONTEXT.md`'s Connector entry, and the news mentions in the v3.6
      and v5 specs would be edited.

**Removal effort: M** — a single self-contained poller with no shared
helpers, but it is the only one of the four that forces the
four-place command-allowlist parity change and the only one that
retires a stated cross-cutting guarantee (News never leaves the machine).

### Weather

- [ ] `src-tauri/src/weather_poller.rs` (1272 lines) would be deleted,
      along with its `mod` line and spawn call in `src-tauri/src/lib.rs`.
- [ ] `Engine::update_weather` (`engine.rs:351`) and its private ambient
      handle would be removed. The generalized per-channel handle
      mechanism (`engine.rs:28`) would be kept — it still serves
      `update_live_match` and `update_now_playing`.
- [ ] The eleven `pub weather_*` fields (`config.rs:99-121`), the `Units`
      enum if it has no other user, and the `default_weather_*` functions
      would be removed. **Migration**: safe, same
      no-`deny_unknown_fields` reasoning as above.
- [ ] The `SourceKind::Weather` variant and its 26 occurrences across
      `weather_poller.rs`, `config.rs`, `event.rs`, `queue.rs` and
      `settings.rs` would go, including `queue.rs:61`'s
      `source_kind_label` arm and `settings.rs`'s test-notification arm.
- [ ] `default_rotation_order` (`config.rs:486`) would drop its third
      entry, leaving `[Football, Manual, Agent, News]`.
- [ ] `WeatherStatus`/`WeatherSummary` (`status.rs:73-76` and
      surrounding) and the `weather` field on `StatusSnapshot` would be
      removed.
- [ ] `src/settings/sections/WeatherSection.tsx` would be deleted. **No
      `#[tauri::command]` is weather-only** (§19 explicitly notes it
      added none), so the seventeen-command allowlist parity would be
      untouched.
- [ ] Frontend: `src/lib/weatherArt.ts` and its test,
      `src/overlay/weather-art.css`, and the 13 files in
      `src/assets/weather/` (12 SVGs plus the `NOTICE` attribution file)
      would be deleted; the weather branches in
      `src/components/IdleHoverPeek.tsx` (47 references — the largest
      single-file pruning in any of these checklists),
      `src/components/StatusDots.tsx`, `src/useStatusState.ts` and
      `src/lib/sourceColors.ts` would be pruned, as would the weather
      preview fixture. **`IdleHoverPeek.tsx` itself would survive** only
      because `MediaPeekRow` lives in it; if Now Playing were cut too,
      the whole file would go.
- [ ] `src-tauri/tests/fixtures/open-meteo-bangalore.json`, 40 rust tests
      and the weather cases in `src/useStatusState.test.ts` and
      `src/settings/SettingsApp.test.tsx` would be deleted, followed by a
      `docs/TESTING_STRATEGY.md` §0 recount.
- [ ] `docs/ARCHITECTURE.md` §19 would gain a dated reversal note, and
      §18/§20's references to weather's rotation slot and priority
      bracketing would need rewording, as would `CONTEXT.md`'s Origin
      entry.

**Removal effort: M** — no shared helpers and no command-allowlist
change, but the widest frontend footprint of the three Origins
(`IdleHoverPeek.tsx` alone carries 47 references) plus a licensed asset
directory and an `ARCHITECTURE.md` reversal note.

### Now Playing

**The `SourceKind` line does not apply here** — Now Playing has no Origin
variant, produces no Events, and never enters the Slot. Its equivalent
removal surface is the four things named below in its place: the
`now_playing_*` config keys, the media fields on the status wire, the
`MediaPeekRow` in `IdleHoverPeek.tsx`, and the vendored adapter
directory.

- [ ] `src-tauri/src/now_playing.rs` (805 lines) would be deleted, along
      with its `mod` line and the config-gated spawn in
      `src-tauri/src/lib.rs`'s `setup`. The supervised-child lifecycle
      (spawn, restart, shutdown) is entirely inside this module, so
      nothing shared becomes dead.
- [ ] `Engine::update_now_playing` (`engine.rs:360`) would be removed;
      the generalized handle mechanism would be kept for
      `update_live_match`/`update_weather`.
- [ ] The three `pub now_playing_*` fields (`config.rs:152-173`) and
      their `default_now_playing_*` functions would be removed, plus the
      `now_playing_adapter_dir` entry in
      `settings::pin_uneditable_fields`. **Migration**: safe, same
      no-`deny_unknown_fields` reasoning as above.
- [ ] `MediaStatus`/`NowPlayingSummary` (`status.rs:27`, `status.rs:157-197`)
      and `StatusSnapshot.media` would be removed, along with the
      `now_playing_enabled` input on the snapshot builder
      (`status.rs:225-227`).
- [ ] **No `SourceKind` variant, no `source_kind_label` arm, no
      `settings.rs` test-notification arm, and no `default_rotation_order`
      change** — it is not an Origin.
- [ ] No Settings *section* would be deleted; instead the now-playing
      controls inside `src/settings/sections/GeneralSection.tsx` would be
      pruned, plus the `now_playing*` entries in
      `src/settings/types.ts`. **No `#[tauri::command]` is
      now-playing-only**, so the seventeen-command allowlist parity would
      be untouched.
- [ ] Frontend: `MediaPeekRow` and its helpers
      (`src/components/IdleHoverPeek.tsx:273-330`, the `useLiveTick`
      progress glide, `iconForBundleId`) would be pruned from a file that
      otherwise survives for weather, along with the `.media-bar-fill`
      rules in `src/overlay/idle-peek.css` and the animation-timing
      assertion in `src/animationTiming.test.ts:186`.
- [ ] `src-tauri/vendor/mediaremote-adapter/` (33 files, including a
      third-party `LICENSE` and `VENDORED.md`) would be deleted, along
      with the `build-media-adapter` recipe in `justfile:62-68` and its
      entries in `CLAUDE.md`'s command list. Whatever is already
      installed under `~/Library/Application Support/notchtap/mediaremote-adapter/`
      on a live machine would be left orphaned and would want an uninstall
      note.
- [ ] 22 rust tests plus the media cases in
      `src/components/IdleHoverPeek.test.tsx` and
      `src/settings/SettingsApp.test.tsx` would be deleted, followed by a
      `docs/TESTING_STRATEGY.md` §0 recount.
- [ ] `docs/design/now-playing-adapter.md`'s conditional GO would be
      recorded as retired (the doc's own §10 anticipates this), and
      `docs/design/now-playing-mediaremote.md`'s NO-GO would be left
      standing. No `ARCHITECTURE.md` section covers it, so no reversal
      note is owed there.

**Removal effort: S** — the smallest module, no Origin, no queue surface,
no exhaustive matches, no command-allowlist change, and only 10 files
touched; the one unusual step is deleting a vendored third-party
directory and its build recipe.

## Verdicts

**Verdict: DEMOTE** — Football. It is the most expensive source in the
review by every measure (3207 rust lines, 71 tests, 38 files touched, 34
`SourceKind` occurrences) and the only one that ships `true`, yet the
operator has its flagship feature switched off (`espn_live_card = false`
and `espn_rich_events = false` in their own config), so what actually
runs on the live machine is the plain burst-of-cards path. The
recommendation is to flip `espn_enabled` to `false` so the shipped
default matches the convention `config.rs:145-147` states, and to group
its Settings section with News and Weather under one "Ambient sources"
heading. That costs one default and one heading, changes nothing for
this operator (whose config sets the key explicitly), and leaves every
line of code in place. A CUT would additionally strand the queue's
Topic/Recurring machinery producerless, which is a separate decision
about a file this review does not cover.

**Verdict: NEEDS OPERATOR INPUT** — News. It is the second-largest source
(1822 lines, 53 tests) and the least structurally connected: it has no
ambient presence at all (`NewsStatus` is still `{ enabled: bool }`,
seven months after plan 052 recommended otherwise), it is barred from
Connectors by design, and cards are its entire output. Nothing in the
repo indicates whether those Low-priority headline cards are read or
ignored, and that is the whole decision. If the answer is "I skim them",
KEEP is correct and cheap; if the answer is "I never look", this is the
best CUT candidate in the review — a self-contained M-effort removal that
also drops one invoke command.

**Verdict: KEEP** — Weather. It is the cheapest real Poller in the tree
(1272 lines, keyless, no secrets, a 900-second poll interval, no
out-of-band build step) and the only source that earns its screen time
passively: the temperature chip is useful on an idle machine with no
agent running and no card showing, which is the opposite of the
card-burst pattern the other Origins follow. Its cost is proportionate to
that, and `docs/ARCHITECTURE.md` §19 locks a design that has not drifted.
Recommend leaving it alone, and including it in the same "Ambient
sources" Settings grouping proposed for Football.

**Verdict: NEEDS OPERATOR INPUT** — Now Playing. On code cost it is the
clear KEEP: 805 lines, 22 tests, 10 files, no Origin, no queue surface,
S-effort to remove. On maintenance risk it is the clear CUT: it is the
only source depending on an undocumented Apple private framework reached
through a vendored third-party Perl loader that must be hand-compiled,
and the repo's own `config.rs:154-162` records that its failure mode is
silent and indistinguishable from "nothing playing" — which is why a
config-file-only kill switch had to be invented for it. Those two
readings do not resolve against each other from the repo; only the
operator's answer to "do you actually look at that row" does.

## Findings

**F1 — the `espn_enabled = true` default is inconsistent with the stated
convention, but no repo-wide policy is being violated.**
`default_espn_enabled()` returns `true` (`config.rs:346-348`) while
`default_rss_enabled()`, `default_weather_enabled()` and
`default_now_playing_enabled()` all return `false`
(`config.rs:381-383`, `401-403`, `453-455`). The convention is written
down once, attached to `now_playing_enabled` at `config.rs:145-147`:

> Default `false` — same opt-in convention as `weather_enabled`/
> `rss_enabled`: ambient sources never default on top of the app's
> primary agent-notification purpose.

Read it precisely: it cites weather and RSS as its precedent and says
nothing about ESPN. `docs/ARCHITECTURE.md` §19 repeats the same wording,
again naming only rss. So this is an inconsistency worth an operator
decision, **not** ESPN violating a documented rule. Nothing was changed.

**F2 — no source in this review is dead code, so no CUT can be grounded
in the repo alone.** All four are switched on in
`~/.config/notchtap/config.toml` (`espn_enabled` line 6, `rss_enabled`
line 21, `weather_enabled` line 34, `now_playing_enabled` line 53). That
is why two of the four verdicts are NEEDS OPERATOR INPUT rather than CUT:
the deciding evidence is usage, and usage is not in the tree.

**F3 — football is the only production producer of the queue's
Topic/Recurring machinery, and on the live machine it is not producing.**
`poller.rs:531` is the sole non-test `RotationSpec::Recurring`
construction and `poller.rs:511-538` the sole non-test source of a
non-`None` `Event.topic`; both are gated on `espn_live_card`, which the
operator has set to `false`. So the machinery
`docs/design/scoreboard-topic-card.md` was written to give a first
consumer currently has zero live consumers again. This does **not** block
removing football — the queue is not coupled to it — but any football
decision should be made knowing it.

**F4 — plan 052's news ambient summary was never built, and the gap is
visible in the code shape.** `engine.rs` has `update_live_match`,
`update_weather` and `update_now_playing` but no news equivalent, and
`NewsStatus` remains `{ enabled: bool }` (`status.rs:126-128`). News is
therefore the only source in the review that cannot say anything on the
idle surface. Either building the summary or cutting News would close
this; leaving it as-is keeps a documented gap open.

**F5 — Now Playing is the only source with an out-of-band build step.**
`just build-media-adapter` (`justfile:62-68`) runs CMake twice against
`src-tauri/vendor/mediaremote-adapter/` and installs a framework under
`~/Library/Application Support/notchtap/`. Nothing in `cargo build` or
`vite build` produces it, so a fresh clone silently has no media data
until someone runs that recipe by hand. The other three sources need
nothing beyond a normal build.

**F6 — `docs/TESTING_STRATEGY.md` §0's per-module numbers are stale for
every module in this review, most sharply for news.** Live counts at
`acdaeb0` versus §0's recorded numbers: poller 59 vs 56, rss_poller 53 vs
28, weather_poller 40 vs 29, now_playing 22 vs 16, crests 12 vs 10. §0's
header pins its figures to commit `9ca81f9` (2026-07-26), so this is
expected staleness and not drift. Recorded here only so a later reader
does not mistake the rss_poller gap (nearly double) for a measurement
error. No doc was edited.

**F7 — cross-cutting, cheapest available trim: the Settings window.**
Three of its twelve sections (`FootballSection`, `NewsSection`,
`WeatherSection`) are ambient sources, and a fourth source's controls sit
loose inside `GeneralSection`. Grouping all four under one "Ambient
sources" heading is a pure presentation change — no config keys, no
`SourceKind`, no commands — and it is the only move in this document that
reduces perceived weight without a single behavioural decision.

## Questions for the operator

1. Should `espn_enabled` ship `false` so the default matches the
   convention written at `config.rs:145-147`? Your own config sets the
   key explicitly, so this changes nothing on your machine. **Yes / No.**
2. You have `espn_live_card = false`, which means the live scoreboard
   card, the team crests, and the whole Topic/Recurring path never run
   for you. Do you want it (a) switched on so it earns its keep, or
   (b) cut — deleting `LiveMatchScorecard.tsx`, `crests.rs` and the
   crest-fetch code while keeping ordinary football cards, or
   (c) left exactly as it is?
3. Do you read the News headline cards? **Yes, I skim them / No, I let
   them go by.** A "no" makes News the best CUT candidate in this review.
4. Do you look at the now-playing media row when you hover the idle pill?
   **Yes / No.** A "no" makes it a CUT despite its small size, because it
   is the only source carrying private-framework risk and a hand-run
   build step.
5. Should the Football, News and Weather Settings sections plus the
   now-playing controls be grouped under one collapsed "Ambient sources"
   heading? **Yes / No.** This is presentation only — no behaviour, no
   config keys.
6. If football were ever cut, the queue's Topic and Recurring
   supersession machinery would have zero production producers again.
   Should that machinery then be (a) kept as a generic framework, the way
   `ConnectorHandle` was kept after Telegram, or (b) deleted too? Answer
   only if question 2 heads toward a cut.

## What this document does not decide

No code changed. Not one line under `src/` or `src-tauri/`, no config
default, no `ARCHITECTURE.md` section. The only file this plan created is
this one.

Each verdict above is a **recommendation pending operator sign-off**, not
an instruction. A CUT verdict is not an authorisation to remove anything;
it would need its own follow-up plan, and the "If cut" checklists above
are research inputs for writing that plan, not a task list. Two of the
four verdicts are explicitly unresolved and wait on answers to the
questions above.

The four verdict values mean:

- **KEEP** — the cost is proportionate; the recommendation is to leave it
  alone.
- **DEMOTE** — the recommendation is to keep the code but ship it off by
  default, and/or to group it under a single "Ambient sources" Settings
  heading. The cheapest verdict; often the right one.
- **CUT** — the recommendation is removal; the matching checklist above
  is the work that a follow-up plan would have to cover.
- **NEEDS OPERATOR INPUT** — the deciding factor is usage, which the code
  cannot answer.

Out of scope by design, and untouched here: Origin `Agent`, Origin
`Manual` (the `./notchtap` CLI and `notchtap run`), the notification
queue, the overlay, the Settings window itself, silence and preemption,
and the Agent Board. Also out of scope: any recommendation that would
change *how* a locked `ARCHITECTURE.md` decision works rather than
whether the feature stays. None of the four verdicts collides with §18,
§19 or §20 — the DEMOTE on Football touches a default, which §18 does not
fix (§18 locks the live-card design, not the `espn_enabled` default).
Finally, `docs/design/per-source-config-consolidation.md` §7 already
declined to consolidate the flat per-source config fields until a sixth
source is proposed; this document does not reopen that.
