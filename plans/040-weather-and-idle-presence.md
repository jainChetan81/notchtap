# Plan 040: weather source + idle-rail ambient presence (incl. football chip)

> **Executor instructions**: Follow this plan step by step (Part B only
> — Part A is DONE, verified below). Run every verification command and
> confirm the expected result before moving on. If anything in "STOP
> conditions" occurs, stop and report. When done, update this plan's
> status row in `plans/README.md`.
>
> **Drift check (run first)**:
> `git diff --stat e7adfb0..HEAD -- src-tauri/src/status.rs src-tauri/src/poller.rs src-tauri/src/rss_poller.rs src-tauri/src/settings.rs src-tauri/src/config.rs src-tauri/src/engine.rs src-tauri/src/event.rs src-tauri/src/net.rs src-tauri/src/lib.rs src/components/IdleView.tsx src/useStatusState.ts`
> `e7adfb0` is this review-plan pass's baseline (2026-07-20) — every
> citation was re-verified against live code at that commit, including
> repointing three line numbers that drifted when 039 and 041 landed
> (`espn_live_card` shifted `config.rs`'s `default_rotation_order`; both
> plans' `poller.rs` rewrites moved the `engine.accept` call site and
> `lib.rs`'s `espn_enabled`/`rss_enabled` gate by a couple of lines). A
> PRIOR pass re-grounded everything at `15df3cc` (2026-07-19), right
> after 037 (the Engine) landed; the original filing predates 037 and
> its "no rust change needed" / "settings.rs + Config" framing is
> corrected below regardless of which baseline you're diffing from.

## Status

- **Part A DONE (verified this review-plan pass), Part B TODO** — filed
  2026-07-19 from a live-session product discussion. **Part A**:
  `IdleView.tsx` renders a bright `Football` chip when
  `football.enabled && !live` (dim `Football off` when disabled),
  upgrading to the live green-dot chip at kickoff. Confirmed landed at
  commit `9b5bd62` ("feat(idle): football presence chip when armed
  (plan 040 Part A)"); `IdleView.tsx:24-28` matches the plan's
  description exactly; `docs/TESTING_STRATEGY.md` §0 confirms
  `IdleView rail 6` (was 4) and frontend total 107 (was 105). **No
  further action on Part A.**
- **Priority**: P2
- **Effort**: Part B (weather) M
- **Risk**: LOW-MEDIUM — additive, no queue-semantics change, but this
  review-plan pass found Part B needs one genuine Engine extension (see
  "Re-grounded against the Engine" below) that the original filing
  didn't anticipate, since it predates 037.
- **Depends on**: none for the architecture (037 is landed, not a
  blocker). **Provider is now RESOLVED** (Open-Meteo — see "Provider
  decision" below). **Three defaults are still open**, deferred to a
  separate design-doc session (not this one): `weather_poll_secs`'s
  default, `SourceKind::Weather`'s position in `default_rotation_order`,
  and the exact rain-probability/temp threshold numbers. Step 0 still
  gates on these — see "Design decisions" below.
- **Review-plan pass (2026-07-19, at `15df3cc`)**: verified Part A DONE
  against live code and test counts (see Status above). For Part B:
  corrected "settings.rs + Config" — the struct is in `config.rs`
  (confirmed during the 039 review-plan pass, same finding applies
  here); found that Part B's original filing predates 037 and doesn't
  address how ambient weather data reaches the rotation loop's
  `StatusState` snapshot — resolved by extending the `Engine`'s
  existing `live`/`update_live_match` pattern (see below) rather than
  inventing a new mechanism; grounded every file reference against live
  code (`status.rs`, `event.rs`, `config.rs`, `net.rs`,
  `useStatusState.ts`, `IdleView.tsx`); wrote concrete numbered Steps
  (none existed before, same gap as 039's original filing).
- **Grilled 2026-07-20 (`/grill-me`, operator)**: eight decisions
  resolved — see "Provider decision" and "Design decisions" below for
  the full record. Three numeric/positional defaults deliberately left
  open for a later design-doc session (see Status line above and
  "Design decisions" §7).
- **Review-plan pass 2 (2026-07-20, at `e7adfb0`)**, run right after the
  grilling session and after 039/041 both landed and rewrote large
  parts of `poller.rs`/`config.rs`/`lib.rs`: repointed three drifted
  line citations; closed the units-threading gap the grilling session
  settled the config field for but not the data flow (see "Design
  decisions" §3 — `weather_poller.rs` now specified to request the unit
  directly from Open-Meteo and carry pre-formatted display strings,
  plus a WMO-code → condition-word mapping neither this plan nor the
  codebase had anywhere); a fresh-context subagent cold-read then found
  two verified compile/validation breaks a naive `SourceKind::Weather`
  addition causes (`build_test_event`'s exhaustive match,
  `expected_sources`'s hardcoded 4-variant permutation check — both
  confirmed by direct read, not just claimed), a step-ordering bug
  (`WeatherSummary` referenced in Step 2 before Step 3 defined it —
  reordered), and the full 9-call-site fan-out for `Engine::new` /
  4-call-site fan-out for `StatusState::snapshot` that the original
  "Re-grounded against the Engine" section described only at the
  definition site, not the call sites.

## Motivation

Two gaps surfaced watching the app during a live match:

1. **Football has no idle presence.** *(Part A — DONE, see Status.)*
2. **No weather.** A glanceable weather readout (and rain/severe alerts)
   is high-value ambient info the notch/HUD is well suited to, and there
   is zero weather code in the tree today.

## Part A — football presence chip (DONE, verified)

No action. `IdleView.tsx:24-28`:
```tsx
<span className={`src-chip${status.football.enabled ? "" : " dim"}`}>
  {status.football.enabled ? "Football" : "Football off"}
</span>
```
matches the plan's design exactly, gated the same way the live-chip
branch is (`live !== null` at `IdleView.tsx:19`).

## Provider decision (RESOLVED 2026-07-20 — Open-Meteo)

The original filing named "Open-Meteo (no key, lat/lon in, JSON out)"
as an example, flagged for confirmation, not a locked choice. This
review-plan pass verified it live (fetched Open-Meteo's actual docs,
not just trusting the name) and the operator confirmed it during
`/grill-me`:

- **Keyless, no rate limit documented** for non-commercial use — their
  own docs: "Only required to commercial use to access reserved API
  resources for customers." Fits `net.rs`'s existing client posture
  (10s timeout, 3 redirects, capped body — a plain public JSON
  endpoint, no OAuth/secret handling needed, so `notifier.rs`'s
  telegram-secret pattern is NOT needed here).
- **Forecast endpoint**: `GET
  https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&current=...`
  — current-conditions fields include temperature, precipitation,
  weather code (WMO 0–99), wind. Covers the ambient chip.
- **Short-term precipitation**: `&minutely_15=precipitation` (native
  for North America/Central Europe, interpolated elsewhere) or
  `&hourly=precipitation_probability` — covers the "rain incoming"
  threshold alert (see "Design decisions" below).
- **No severe-weather-warnings feed** — confirmed: Open-Meteo delivers
  only numeric data and WMO codes, no human-authored storm/flood
  warning text. This ruled out "severe weather warnings" as originally
  imagined in the Motivation section — see "Design decisions" §3.
- A free, keyless **geocoding endpoint** also exists
  (`https://geocoding-api.open-meteo.com/v1/search?name=...`) but is
  explicitly NOT used by this plan — see "Design decisions" §1.

## Design decisions (grilled 2026-07-20, operator)

Seven decisions resolved via `/grill-me`, in dependency order:

1. **Location input: raw `lat`/`lon` config fields only — no
   geocoding, no city-name lookup.** Open-Meteo's geocoding endpoint
   (above) is real and free, but adding it means a second live API
   dependency, a settings-UI disambiguation flow (0 or multiple
   matches), and a new failure mode independent of the weather poll
   itself. This is a single-user desktop app — the operator looks up
   their own coordinates once. `Config` gets `weather_lat: f64` /
   `weather_lon: f64`, plain numbers, no resolution step. (A
   "resolve from city name" settings-UI convenience button is a
   plausible cheap follow-up plan later, not in this one.)
2. **Ambient chip format: plain text, no icons/emoji** (e.g. `"12°
   Rain"`), matching the existing chip vocabulary exactly (`Football`/
   `News paused`/`Live`/`Card`/`Off` — confirmed zero emoji/icon glyphs
   anywhere in this codebase during the 041 review). A background-
   color-shift / weather-SVG visual treatment was floated but
   deliberately deferred — see "Notes" below, not built in this plan.
3. **Units: configurable (`weather_units: Celsius | Fahrenheit` on
   `Config`), default `Celsius`.** `Priority` (`event.rs:59-65`) is the
   pattern to copy — quote it exactly, don't just "mirror" from memory:
   `#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord,
   Serialize, Deserialize)]` + `#[serde(rename_all = "snake_case")]`.
   `Units` doesn't need `PartialOrd`/`Ord` (nothing orders it), but DOES
   need `Serialize`/`Deserialize` + the snake_case rename, or TOML
   round-tripping breaks silently (checked: `rg -n "enum Units|struct
   Units" src-tauri/src` — no collision, the name is free).

   **Where the unit actually gets applied (a review-plan-pass fix — the
   original grilling session settled the config field but not the data
   flow)**: request the unit directly from Open-Meteo via its
   `&temperature_unit=fahrenheit` query param (default is Celsius if
   omitted) — do NOT always fetch Celsius and convert client-side; that
   duplicates unit-conversion math the provider already does correctly.
   `weather_poller.rs` reads `config.weather_units` and builds the query
   string accordingly. `WeatherSummary` (`status.rs`, mirroring
   `LiveMatchSummary`'s two-plain-`String`-fields shape,
   `status.rs:36-43`) carries the result **already formatted**, the same
   "carried verbatim" precedent: `WeatherSummary { temp_display: String,
   condition: String }` — e.g. `temp_display: "12°"`, `condition:
   "Rain"`. The frontend concatenates them (`{temp_display}
   {condition}"`), exactly like `IdleView.tsx:22`'s existing `{live.label}
   · {live.minute}` pattern for `LiveMatchSummary`. This keeps
   unit-symbol formatting and WMO-code mapping in the one Rust place
   that already owns "make this provider's response presentable," not
   scattered into frontend string logic.

   **`condition` needs a WMO-code → word mapping** — nothing in the
   codebase provides one; build a small pure function in
   `weather_poller.rs`, unit-tested directly (no live network), covering
   at minimum the ranges already surfaced during grilling: `0` → "Clear",
   `1`–`3` → "Cloudy", `45`/`48` → "Fog", `51`–`67` → "Rain", `71`–`77` →
   "Snow", `80`–`82` → "Showers", `95`–`99` → "Storm" (this last range is
   presentation-only here — it does NOT feed a "severe" threshold alert,
   per decision §4). Fall back to a generic label (e.g. "—") for any WMO
   code outside these ranges rather than panicking or omitting the chip.
4. **Threshold alerts for v1: rain-incoming + temperature threshold
   only.** The original filing's third category, "severe weather
   warnings," isn't buildable as imagined — Open-Meteo has no warnings
   feed (see "Provider decision" above); the only way to approximate
   "severe" would be a WMO-thunderstorm-code threshold, which is
   deliberately deferred, not built now. Don't add it speculatively.
5. **Alert re-fire semantics: edge-triggered, not level-triggered.**
   A threshold alert fires once on crossing INTO alert territory, stays
   silent while the condition holds, and only re-arms after it clears
   and re-crosses. A level-triggered design (re-firing every poll while
   e.g. rain continues) would directly violate the plan's own "not
   spammy cards" goal — a 2-hour rain spell at the default poll
   interval would otherwise produce a card every cycle. **Implication
   for `weather_poller.rs`**: it needs to track "already fired for this
   occurrence" state per alert type — the same shape `poller.rs`'s
   `Snapshot` already carries forward to avoid re-emitting a kickoff/
   half-time event every poll. This is new state the original filing's
   "diff → emit" framing didn't call out explicitly; Step 3 below
   should build it in from the start, not bolt it on after discovering
   spam in testing.
6. **`weather_priority` default: `Medium`.** Bracketed by existing
   precedent — `default_espn_priority() -> Priority::High` (live sports
   = urgent) and `default_rss_priority() -> Priority::Low` (news =
   ambient), both `config.rs:158-176`. Weather alerts are more
   actionable than a news headline but less urgent than a live goal.
7. **Engine plumbing: confirmed as originally proposed** — see
   "Re-grounded against the Engine" below; the operator signed off on
   mirroring `live`/`update_live_match` exactly, no alternative design
   considered necessary.

**Deliberately left open, for a separate design-doc session (not this
one)**: `weather_poll_secs`'s default value, `SourceKind::Weather`'s
position in `default_rotation_order` relative to
Football/Manual/Cmux/News, and the exact threshold numbers (rain
probability % + lookahead window in minutes; hot/cold temperature
cutoffs in Celsius). Step 0 below still gates on these — `config.rs`'s
`Default` impl can't be written without real numbers, even though
they're all operator-tunable via Settings after the fact.

## Two verified breaks a naive `SourceKind::Weather` addition causes (this review-plan pass)

Adding a fifth `SourceKind` variant is not the "round-trips for free"
change the Scope's `event.rs` bullet implies for the rest of the
codebase — two sites are exhaustive over the CURRENT four variants and
either fail to compile or silently corrupt validation. Both are
verified against live code, not theoretical:

1. **`settings::build_test_event` (`settings.rs:502-575`) is an
   exhaustive `match source { Football => .., News => .., Cmux => ..,
   Manual => .. }` with no wildcard arm.** The moment `SourceKind`
   gains `Weather`, the crate fails to compile — including Step 1's own
   `cargo test --locked config:: event::` verification, which still has
   to build the whole crate first. **Step 1 must add a
   `SourceKind::Weather` arm**, modeled on the `Manual` arm (simplest —
   generic `EventType::Generic`, `RotationSpec::OneShot { ttl_secs:
   config.default_ttl }`, `EventMeta::default()`, `signal:
   EventSignal::Generic`) but with `priority: config.weather_priority`
   and `origin: SourceKind::Weather`. This is also a product decision,
   not just a compile fix: it means Weather gets a "send test
   notification" button like every other source (CLAUDE.md's v5.1
   feature) — add `"weather"` to the frontend's separate `TestSource`
   union (`SettingsApp.tsx:890`, NOT the same type as the `SourceKind`
   union at `:25` — they're two different hardcoded unions, checked
   directly) and wire a `TestButtonRow` into the new Weather settings
   group, following the shape at one of the existing groups (`rg -n
   "<TestButtonRow" src/settings/SettingsApp.tsx` to find an exemplar).
2. **`settings::validate`'s rotation-order permutation check
   (`settings.rs:142-160`) is hardcoded to exactly four `SourceKind`
   variants**, with its own comment stating "the ui is a fixed 4-row
   reorder list, never add/remove." This is NOT a compile error — it's
   a silent logic break: once `default_rotation_order()` becomes a
   5-element vec (per decision above), `Config::default().validate()`
   starts failing, because `expected_sources` still only lists four.
   That means **the shipped default config stops validating** — this
   survives Step 1's narrow verification and only surfaces at Step 4's
   full `cargo test --locked` (`default_config_validates_clean` fails),
   by which point downstream Steps may already be built on top of it.
   **Step 1 must add `SourceKind::Weather` to `expected_sources`** and
   update the error message text ("must contain each of football,
   manual, news, cmux, and weather exactly once"). The frontend's
   rotation-order UI needs the parallel fix: `SettingsApp.tsx:25`'s
   `SourceKind` union (`"football" | "manual" | "news" | "cmux"`) needs
   `"weather"` added by hand — this one does NOT self-correct via `tsc`
   the way `SOURCE_LABELS` (a `Record<SourceKind, string>`, exhaustive-
   checked) does; if skipped, `tsc` stays green but the rotation-order
   reorder list in Settings silently never shows Weather.

Also note (cosmetic — self-correcting, but worth a deliberate choice in
Step 1 rather than an oversight): `event.rs`'s
`source_kind_round_trips_every_variant` test and `queue.rs`'s proptest
generators `arb_origin`/`arb_rotation_order` all hardcode today's four
variants. None of these fail to compile or fail to pass — they simply
never exercise `Weather`-origin events until updated. Add `Weather` to
`source_kind_round_trips_every_variant` in Step 1 (cheap, closes a real
coverage gap); leave the proptest generators as a judgment call — call
out in the commit message either way rather than silently leaving
`Weather` unexercised by the fuzz suite.

## `Engine::new` and `StatusState::snapshot`: every call site (this review-plan pass)

The "Re-grounded against the Engine" section below describes the field/
method additions but not their blast radius. Checked directly (`rg -n
"Engine::new\("` / `"StatusState::snapshot\("`):

- **`Engine::new` has 9 call sites**, not just the one inside
  `engine.rs`. **One is production**: `lib.rs:189` (inside `run()`'s
  `setup`) — this needs `config.weather_enabled` threaded in as the
  new argument, the same way `espn_enabled`/`rss_enabled` already are
  at that call site. The other 8 are test helpers that need a bool
  literal added: `engine.rs:333,400,427,578`; `poller.rs:750,1161`;
  `http.rs:268`; `lib.rs:747`. None of these tests are ABOUT weather, so
  the literal value shouldn't matter to their assertions — pass `false`
  at each unless a specific test's own assertions require otherwise
  (read the test before assuming `false` is harmless in every case).
- **`StatusState::snapshot` has 4 call sites**: `engine.rs:245,291` are
  already covered by Step 2's text below ("thread through
  `spawn_rotation`/`emit_current_status_blocking`" IS these two sites).
  `status.rs:204,215` are `status.rs`'s own unit tests — not called out
  elsewhere in this plan, but they're in the same file Step 3 already
  edits and will be caught immediately by a compile error, not a silent
  gap; update them in the same Step 3 pass rather than treating them as
  a separate task.

## Re-grounded against the Engine (this review-plan pass)

The original filing predates plan 037 and describes Part B's ambient
chip purely in terms of `status.rs`, without addressing **who writes
the ambient weather data and how it reaches the rotation loop's
`StatusState` snapshot**. Plan 037/034 already solved exactly this
problem for football's live-match chip — read that solution and mirror
it, don't invent a second mechanism:

- **Today's football precedent**: `Engine` privately owns
  `live: Arc<StdMutex<Option<LiveMatchSummary>>>` (`engine.rs:32`),
  constructed internally in `Engine::new` (`engine.rs:58-74`, nothing
  outside can hold it). The espn poller calls
  `engine.update_live_match(summary)` (`engine.rs:186-199`) once per
  poll pass — a narrow method that locks `live`, compares, stores, and
  wakes the rotation loop ONLY on change (never touches the queue).
  `spawn_rotation`'s loop (`engine.rs:218-263`) reads `live` (BEFORE
  locking the queue — lock-discipline comment at `engine.rs:236-238`)
  and folds it into `StatusState::snapshot` (`status.rs:58-75`) every
  pass, emitting on change via the loop's own `last_status` dedup.
  `emit_current_status_blocking` (`engine.rs:287-295`, the
  `on_page_load` reload re-emit) does the same read, unconditionally.
- **Weather's ambient chip needs the identical shape**: `Engine` gains
  a `weather: Arc<StdMutex<Option<WeatherSummary>>>` field +
  `weather_enabled: bool` (constructed in `Engine::new` exactly like
  `live`/`espn_enabled`/`rss_enabled` are today), a
  `pub fn update_weather(&self, summary: Option<WeatherSummary>)`
  method with the identical compare-then-store-then-maybe-wake body as
  `update_live_match`, and `StatusState::snapshot`/
  `emit_current_status_blocking`/`spawn_rotation`'s StatusState block
  all thread the new field through. This is a mechanical extension of
  an established pattern, not new design — but it IS a real
  `engine.rs` edit the original filing's "no rust change needed" (which
  only applied to Part A) and "extend `snapshot()`" (true, but
  incomplete — `snapshot()` alone doesn't explain the write path) don't
  cover.
- **Threshold alerts are ordinary `accept()`-routed Events**, NOT part
  of this ambient mechanism — a threshold-crossing weather event is a
  normal `Event { origin: SourceKind::Weather, topic: None,
  rotation: RotationSpec::OneShot { .. }, .. }` passed to
  `engine.accept(event, false)` from the weather poller, identical in
  shape to how the espn/rss pollers push cards today
  (`poller.rs:692` at last check, drifted since 039/041 landed — locate
  with `rg -n "engine.accept" src-tauri/src/poller.rs`;
  `rss_poller.rs:485`). The two mechanisms
  (ambient status vs. queued card) are already cleanly separated in the
  codebase for football; weather should use the same separation, not
  conflate "current conditions" with "a card."
- **`StatusState::snapshot`'s signature will grow to 6 params**
  (`queue, live, espn_enabled, rss_enabled, weather, weather_enabled`)
  — check `cargo clippy` doesn't flag `too_many_arguments` on it once
  added (the default threshold is 7, so this should still be fine, but
  confirm rather than assume).

## Scope

**In scope**:
- `src-tauri/src/event.rs` — add `Weather` to `SourceKind`
  (`event.rs:72-77`, `#[serde(rename_all = "snake_case")]` already on
  the enum, so `"weather"` round-trips for free).
- `src-tauri/src/config.rs` — per "Design decisions" above:
  `weather_enabled: bool` (default `false`); `weather_lat: f64` /
  `weather_lon: f64` (no geocoding, no location-string field);
  `weather_units: Units` (new small enum, `Celsius`/`Fahrenheit`,
  default `Celsius`, following the `Priority` enum's existing
  config-field pattern); `weather_poll_secs: u64` (default TBD — open,
  see "Design decisions" §deferred); `weather_rain_threshold_pct: u8` +
  `weather_rain_lookahead_mins: u16` (defaults TBD); `weather_temp_hot_c`
  / `weather_temp_cold_c: f64` (defaults TBD, always stored in Celsius
  regardless of `weather_units` — that field is display-only);
  `weather_priority: Priority` (default `Medium`, resolved). Add
  `SourceKind::Weather` to `default_rotation_order()`
  (`config.rs:190+` at last check — drifted since 039 added
  `espn_live_card`; locate with
  `rg -n "fn default_rotation_order"`) at a position — still open,
  see "Design decisions" §deferred.
- `src-tauri/src/settings.rs` — three separate, unrelated edits, don't
  conflate them:
  1. `validate()` (`settings.rs:49-90` pattern) gains range checks,
     concretely: `weather_lat` in `-90.0..=90.0`, `weather_lon` in
     `-180.0..=180.0`, `weather_rain_threshold_pct` in `0..=100`,
     `weather_temp_hot_c > weather_temp_cold_c` (a cross-field check,
     not a single-field range — no existing exemplar for this shape in
     `validate()`, write it as its own `if` block after the per-field
     checks). `weather_poll_secs`'s range: mirror `espn_poll_secs`'s
     `5..=3600` (`settings.rs:70-75` pattern) — the exact DEFAULT is
     still open (Step 0), but the acceptable RANGE isn't a Step-0
     unknown and can be specified now. `weather_rain_lookahead_mins`:
     pick a sane range once Step 0's default is known (e.g. it should
     comfortably contain whatever default gets chosen).
  2. **`build_test_event`'s exhaustive match needs a `SourceKind::Weather`
     arm** — see "Two verified breaks" above; this is a compile-breaking
     fix, not optional polish.
  3. **`expected_sources`'s rotation-order permutation check needs
     `SourceKind::Weather` added** — see "Two verified breaks" above;
     this is a validation-breaking fix, not optional polish.
  No new `#[tauri::command]` (confirmed: `get_config`/
  `get_default_config` serialize the whole struct, same as the 039
  finding — no per-field wiring needed there).
- `src-tauri/src/net.rs` — reuse `build_poll_client()`/
  `read_body_capped()` as-is (`net.rs:8-38`) — no changes, just call
  them from the new poller.
- `src-tauri/src/weather_poller.rs` (new) — mirrors
  `rss_poller.rs`'s shape: a `spawn_weather_poller(engine: Engine,
  ...)` function, a poll loop, a pure diff/parse function (fixture-
  tested, no live network — same discipline as `poller.rs`'s
  `diff_scoreboard`/`rss_poller.rs`'s `diff_feed`), calling
  `engine.update_weather(...)` every pass and `engine.accept(event,
  false)` only for threshold-crossing events. **Per "Design decisions"
  §5 (edge-triggered alerts)**: this pure diff function needs to carry
  forward "already fired" state per alert type (rain, hot, cold) across
  polls — a small struct alongside the parsed response, mirroring how
  `poller.rs`'s `Snapshot` is threaded through `diff_scoreboard` calls
  to avoid re-emitting kickoff/half-time every pass. An alert transitions
  fired→armed only when the underlying condition clears (drops back
  under threshold), not just because a poll returned a value.
- `src-tauri/src/engine.rs` — the `weather`/`weather_enabled` field +
  `update_weather` method + `Engine::new` signature (**and all 9 of its
  call sites across `lib.rs`/`http.rs`/`poller.rs`/`engine.rs` itself**
  — see "`Engine::new` and `StatusState::snapshot`" above, do not scope
  this to `engine.rs` alone) + `spawn_rotation`/
  `emit_current_status_blocking` wiring, per "Re-grounded against the
  Engine" above.
- `src-tauri/src/status.rs` — `WeatherSummary { temp_display: String,
  condition: String }` (Step 2 — mirroring `LiveMatchSummary`,
  `status.rs:36-43`, and see "Design decisions" §3 for why it's
  pre-formatted); `WeatherStatus { enabled: bool, current:
  Option<WeatherSummary> }` on `StatusState` (Step 3 — mirroring
  `FootballStatus`'s shape, `status.rs:28-34`); `snapshot()`
  (`status.rs:58-75`) gains the two new params, all 4 call sites (see
  "`Engine::new` and `StatusState::snapshot`" above).
- `src/useStatusState.ts` — the `StatusState`/`WeatherSummary` TS types
  (`useStatusState.ts:9-19`, hand-mirrored, same as `SettingsApp.tsx`'s
  `Config` interface found during the 039 review — `WeatherSummary` is
  `{ tempDisplay: string; condition: string }`, camelCase per this
  file's existing wire convention, mirroring Rust's `temp_display`/
  `condition` from "Design decisions" §3), the `FALLBACK_STATUS` object
  (`useStatusState.ts:30-35`), AND `isValidStatusState`
  (`useStatusState.ts:55+`) — this validator explicitly type-guards
  each field; a `weather` field the validator doesn't check means a
  malformed payload could still be treated as valid `StatusState`
  overall. Do not skip this file — it's easy to add the type and the
  fallback and miss the validator, since nothing will visibly break
  until a malformed weather payload actually arrives.
- `src/components/IdleView.tsx` — a weather chip, following the
  News/Football chip pattern at `IdleView.tsx:24-31` (dim when
  disabled, otherwise showing `{tempDisplay} {condition}` if present —
  same concatenation shape as the existing `{live.label} · {live.minute}`
  at `IdleView.tsx:22`).
- `src/settings/SettingsApp.tsx` — several separate pieces, don't
  conflate them: (1) the config UI — enable toggle, `lat`/`lon` number
  fields (no city-name/geocoding input — see "Design decisions" §1), a
  units toggle (Celsius/Fahrenheit), poll interval, and the threshold
  fields, following the ESPN group's pattern (`SettingsApp.tsx:645-661`,
  established during the 039 review); (2) the `SourceKind` TS union
  (`SettingsApp.tsx:25`) needs `"weather"` added by hand — see "Two
  verified breaks" above, this does NOT self-correct via `tsc`; (3) the
  separate `TestSource` union (`SettingsApp.tsx:890` — a DIFFERENT
  hardcoded union from `SourceKind`, confirmed by direct read) needs
  `"weather"` too, plus a `TestButtonRow` wired into the new Weather
  group (`rg -n "<TestButtonRow" src/settings/SettingsApp.tsx` for an
  exemplar).
- `src/settings/SettingsApp.test.tsx` — its two hardcoded `Config`
  fixture objects (confirmed at `:17`/`:53`, e.g. `espn_live_card:
  false,`) need every new `weather_*` field added — this one IS
  self-correcting via `tsc` (`Config` is a plain TS `interface`, so a
  fixture missing a required field is a type error, not a silent gap),
  but expect the compile errors and fix them here rather than being
  surprised by them mid-Step-5.
- `docs/TESTING_STRATEGY.md` §0, `docs/ARCHITECTURE.md` (new source +
  the ambient-vs-card design split).

**Out of scope**:
- Any `#[tauri::command]` addition or `capabilities/*.json` change
  (STOP condition, unchanged from the original filing).
- A keyed/OAuth weather provider's secret storage — moot now that
  Open-Meteo is confirmed keyless (see "Provider decision" above).
- Geocoding / city-name location input (decided against, "Design
  decisions" §1) — `lat`/`lon` only.
- A weather-reactive background-color or SVG treatment for the idle
  card (floated during `/grill-me`, deliberately deferred — see
  "Notes") — this plan ships the plain-text chip only.
- A thunderstorm-WMO-code-derived "severe weather" alert ("Design
  decisions" §4) — only rain-incoming and temperature-threshold alerts
  ship in this plan.
- Any change to the football/news pollers or the Topic/Recurring
  machinery (039's territory) — this plan's producer is independent.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust suite | `cd src-tauri && cargo test --locked` | all pass; recount against §0 |
| Gates | `cargo clippy --locked --all-targets -- -D warnings && cargo fmt --check` | exit 0 |
| Frontend tests | `npx vitest run` | all pass |
| Frontend gates | `npx biome ci . && npx tsc --noEmit && npx vite build` | exit 0 |

## Steps

### Step 0: remaining defaults gate

Provider, location input, units, alert scope, re-fire semantics, and
`weather_priority` are all resolved (see "Design decisions" above) —
Step 0 is no longer a provider-choice gate. What's STILL open, and
must be resolved (in the deferred design-doc session) before writing
`config.rs`'s `Default` impl: `weather_poll_secs`'s default value,
`SourceKind::Weather`'s position in `default_rotation_order`, and the
exact rain-probability/lookahead/temp-threshold numbers. STOP here
until those three are picked — do not guess reasonable-sounding
numbers and proceed; they're operator-tunable later, but Step 1 needs
literal values to compile against now.

(This gate is real for Step 1 specifically, since `config.rs`'s
`Default` impl needs literal values. It's looser than it looks for
Steps 2-3: `Engine`/`status.rs` plumbing doesn't reference any of the
three deferred numbers at all, and Step 3's edge-trigger STATE MACHINE
logic doesn't either — only its fixture TESTS need some concrete
threshold to construct test data against. If the three numbers are
taking a while to land, Steps 2 and most of Step 3 can reasonably start
once Step 1's other fields are in, using placeholder test-only values
for anything Step 3's fixtures need — just don't let a placeholder
leak into `config.rs`'s actual `Default` impl.)

### Step 1: `SourceKind::Weather` + config surface

Add the enum variant (`event.rs:72-77`) and the config fields
(`config.rs`, per Scope above — `weather_lat`/`weather_lon`/
`weather_units`/`weather_priority` are fully specified now;
`weather_poll_secs` and the threshold fields need Step 0's remaining
defaults), including `default_rotation_order`'s new entry and
`settings::validate`'s new range checks.

**Verify**: `cargo test --locked config:: event::` → all pass.

### Step 2: `WeatherSummary` + `Engine` ambient-weather plumbing

**Ordering note (this review-plan pass — the original filing had this
backwards)**: `Engine`'s new `weather` field is typed
`Arc<StdMutex<Option<WeatherSummary>>>`, and `engine.rs:19` already
does `use crate::status::{..., LiveMatchSummary, ...}` for the football
precedent — `WeatherSummary` has to exist in `status.rs` BEFORE
`engine.rs` can name it, or this step's own `cargo test --locked
engine::` fails with "cannot find type `WeatherSummary` in
`crate::status`". So: first, define `WeatherSummary { temp_display:
String, condition: String }` in `status.rs` (mirroring
`LiveMatchSummary`'s shape exactly, `status.rs:36-43` — per "Design
decisions" §3's units-threading fix above; do NOT add a
`WeatherStatus`/`snapshot()` wiring yet, that's Step 3). Then add the
`weather`/`weather_enabled` field and `update_weather` method to
`engine.rs`, thread through `Engine::new` (and its 9 call sites — see
"`Engine::new` and `StatusState::snapshot`" above; `lib.rs:189` gets
`config.weather_enabled` for real, the other 8 get a `false` literal),
`spawn_rotation`, and `emit_current_status_blocking`. Add a test
mirroring `update_live_match_wakes_only_on_change` (`engine.rs:609-642`)
for `update_weather`.

**Verify**: `cargo test --locked status:: engine::` → all pass,
including the new `update_weather` test.

### Step 3: `WeatherStatus`/`snapshot()` + `weather_poller.rs`

Add `WeatherStatus { enabled: bool, current: Option<WeatherSummary> }`
to `StatusState` (mirroring `FootballStatus`, `status.rs:28-34`;
`WeatherSummary` itself already exists from Step 2) and thread through
`snapshot()` (`status.rs:58-75`, now growing to 6 params — see
"`Engine::new` and `StatusState::snapshot`" above for its 4 call
sites, 2 of which Step 2 already touched via `spawn_rotation`/
`emit_current_status_blocking`; the other 2, `status.rs:204,215`, are
`status.rs`'s own tests — update them here, same file). Build
`weather_poller.rs` per the Scope description — pure diff/parse
function fixture-tested against a captured real response (same
discipline as the ESPN/RSS fixtures under
`src-tauri/tests/fixtures/`), requesting `temperature_unit` per "Design
decisions" §3, mapping WMO codes to `condition` text via the small pure
function specified there, `spawn_weather_poller` wired the same way
`spawn_espn_poller`/`spawn_rss_poller` are (`lib.rs:314-337` at last
check — locate with `rg -n "if espn_enabled" src-tauri/src/lib.rs`).
Build the edge-triggered alert state (Scope's `weather_poller.rs`
bullet, "Design decisions" §5) into the diff function from the start —
write the fixture tests to explicitly cover: threshold not crossed →
no event; crossed → one event; still crossed on the next poll → no
second event; clears then re-crosses → a second event. All four cases,
not just the happy "it fires" path.

**Verify**: `cargo test --locked status:: weather_poller::` → all pass,
including the four edge-trigger cases above.

### Step 4: wire into `lib.rs`

Config-gate the spawn (`if weather_enabled { ... }`, matching the
`espn_enabled`/`rss_enabled` pattern at `lib.rs:318,328` at last
check — locate with `rg -n "if espn_enabled|if rss_enabled" src-tauri/src/lib.rs`).

**Verify**: `cargo test --locked` (full) → all pass; `cargo clippy
--locked --all-targets -- -D warnings && cargo fmt --check` → exit 0.

### Step 5: frontend

Add the TS types, fallback, validator branch, `IdleView.tsx` chip,
`SettingsApp.tsx` config UI + `SourceKind`/`TestSource` union entries +
`TestButtonRow`, and `SettingsApp.test.tsx`'s two fixture updates, per
Scope above.

**Verify**: `npx vitest run && npx tsc --noEmit && npx biome ci .` →
all pass/exit 0.

### Step 6: docs + status

Update `docs/TESTING_STRATEGY.md` §0 and `docs/ARCHITECTURE.md`. Flip
this plan's `plans/README.md` row to DONE.

**Verify**: `cargo test --locked 2>&1 | grep "test result"` and `npx
vitest run` totals match §0.

## Done criteria

| Check | Command | Expected |
|---|---|---|
| Rust tests | `cargo test --locked` (from `src-tauri/`) | all pass; §0 updated |
| Frontend tests | `npx vitest run` | all pass; §0 updated |
| Lint/format/build | `cargo clippy --locked --all-targets -D warnings && cargo fmt --check`; `npx biome ci . && npx tsc --noEmit && npx vite build` | exit 0 |
| Ambient chip | weather enabled → idle chip shows current conditions; disabled → no chip, byte-identical behavior | pass |
| Threshold alert | a threshold-crossing weather event enqueues a card via `accept`; plain conditions never do | pass |
| Ambient ≠ card | `rg -n "SourceKind::Weather" src-tauri/src/weather_poller.rs` shows it used ONLY on the `accept()` path, never on the `update_weather()` path (the ambient write takes a plain `WeatherSummary`, not an `Event`) | pass |
| Edge-triggered alerts | the four cases in Step 3 (no-cross/first-cross/holds/re-cross) all pass, specifically confirming a sustained threshold breach produces exactly ONE card, not one per poll | pass |
| Receive-only intact | `git diff -- src-tauri/capabilities/default.json src-tauri/capabilities/settings.json` | byte-identical |
| Validator complete | `rg -n "weather" src/useStatusState.ts` hits the type, the fallback, AND `isValidStatusState` — not just the first two | pass |
| Default config still validates | `cargo test --locked default_config_validates_clean` (or the equivalent settings.rs test name at execution time) | pass — confirms `expected_sources`/rotation_order permutation check was updated, not just `default_rotation_order()` |
| Test-notification button works for weather | manual: Settings → Weather → "Send test" produces a card | pass (confirms `build_test_event`'s new arm + `TestSource`/`TestButtonRow` wiring, not just that the crate compiles) |

## STOP conditions

- Step 0's three remaining defaults (poll interval, rotation-order
  position, threshold numbers) are unresolved → STOP, do not guess.
- Any change would touch `capabilities/default.json` or add a
  `#[tauri::command]` → STOP (receive-only / command-ACL guarantee).
- `StatusState::snapshot`'s new 6-arg signature trips
  `clippy::too_many_arguments` → STOP and report rather than adding an
  `#[allow(...)]` silently; consider whether ambient inputs should
  bundle into a small struct instead (a judgment call for whoever's
  executing, informed by how the codebase already resolved this for
  `spawn_espn_poller` in plan 037 — collapsing scattered params into
  one owned type, not an attribute).

## Notes

- Part A is done and needs no further action.
- Part B is independent of the sports-card work (037/039) — no queue
  changes; it only adds a producer + an Engine-owned ambient handle +
  a status field + a chip, following the football precedent throughout.
- **Future enhancement, explicitly not this plan** (floated during
  `/grill-me` 2026-07-20, deliberately deferred): a weather-reactive
  background treatment for the idle card — background color shifting
  with conditions, and/or a weather SVG/illustration. Worth a follow-up
  plan once the plain-text v1 has shipped and the operator has lived
  with it; this plan ships text-only per "Design decisions" §2.
- **Deferred to a separate design-doc session** (not part of this
  plan's `/grill-me` pass): `weather_poll_secs`'s default,
  `SourceKind::Weather`'s `default_rotation_order` position, and the
  exact rain/temp threshold numbers. See "Design decisions" (deferred
  list) and Step 0.
