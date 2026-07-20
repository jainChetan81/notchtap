# Plan 082: Weather art — vendored Meteocons + CSS mood backdrops on weather cards

> **Executor instructions**: This is a build plan for the locked weather
> art direction (079 item 9, locked 2026-07-20 — Meteocons static SVGs +
> purpose-built CSS gradient moods, gallery at
> `prototype/weather-art.html`). It covers: moving the vendored SVGs
> into the app bundle with attribution, the condition→(glyph, mood,
> day/night) data table, and applying mood+glyph to the existing weather
> ALERT cards. It does NOT build the hover weather-peek interaction —
> Step 5 only preps the table/classes so the peek can consume them when
> plan 086 unblocks hover. Follow the steps in order. License compliance
> is a hard requirement: the NOTICE file is Step 1, not an afterthought.
> The overlay has NO runtime network — every asset must be bundled by
> vite; an `<img>` pointing at anything fetched at runtime is a bug.
> When done, update the status row for this plan in `plans/README.md` —
> unless a reviewer dispatched you and told you they maintain the index.
>
> **Drift check (run first)**: `git diff --stat 71e54a7..HEAD -- src-tauri/src/weather_poller.rs src-tauri/src/config.rs src/components/StatusRailCard.tsx src/styles.css src/settings/preview-overlay.css prototype/weather-art.html prototype/assets/weather/`
> Any diff in the source files means line refs below have shifted —
> re-read before editing. Any diff in the prototype or its assets is a
> STOP condition (see below).

## Status

- **Priority**: P2
- **Effort**: S–M
- **Risk**: LOW — additive frontend styling + vendored assets; the only
  sharp edges are license attribution (must not be forgotten) and
  day/night derivation (must be honest about its data source)
- **Depends on**: none. (082 is independent; it shares only
  `StatusRailCard.tsx`/`styles.css` surface area with 080 — land after
  080 to avoid CSS rebase noise, but nothing structural requires it.)
- **Category**: direction (locked) → build
- **Planned at**: commit `71e54a7`, 2026-07-20

## Why this matters

The weather mood backgrounds are locked design (079 item 9): Meteocons
static fill SVGs (MIT, Bas Milius) as condition glyphs on CSS gradient
backdrops keyed to condition + day/night, replacing the CSS blob
stand-ins from the first mockup round. 12 SVGs are already vendored in
`prototype/assets/weather/` (10 used by the gallery; `extreme-rain`,
`wind`, `hail` are pulled-but-unassigned spares). The sourcing research
(`research/2026-07-20-icon-artwork-sourcing.md`) is explicit: when
these move into `src/assets/` for the real overlay, a NOTICE /
attribution line ships alongside them. Meanwhile the weather ALERT
cards (rain-incoming, hot, cold — edge-triggered from
`weather_poller.rs`) render today as plain generic cards; they're the
first real surface where the mood+glyph art belongs, and they need no
new plumbing to get it.

## Current state

- `prototype/assets/weather/` — 12 vendored Meteocons static fill SVGs
  (`clear-day`, `clear-night`, `partly-cloudy-day`,
  `partly-cloudy-night`, `overcast`, `rain`, `thunderstorms`, `snow`,
  `fog`, `extreme-rain`, `wind`, `hail`). License: MIT, by Bas Milius,
  `github.com/basmilius/weather-icons`.
- `prototype/weather-art.html` — the approved pairings:
  `.wx-card` shape (lines 29-42: black base, 14px bottom rounding,
  48px glyph top-right with drop shadow, scrim gradient, condition +
  temp row), 10 mood gradients (lines 49-58: `wx-clear-day`,
  `wx-clear-night`, `wx-partly-cloudy-day`, `wx-partly-cloudy-night`,
  `wx-overcast`, `wx-rain`, `wx-rainy-night`, `wx-thunderstorm`,
  `wx-snow`, `wx-fog`), plus `.wx-rain-streaks`/`.wx-snow-dots`
  texture overlays (lines 44-45). Day/night keyed off location time
  (footer, lines 224-236).
- `src-tauri/src/weather_poller.rs:83-94` — `condition_word(code)`
  maps WMO codes to the app's 7 condition words: Clear, Cloudy, Fog,
  Rain, Snow, Showers, Storm (unknown → "—"). This is the canonical
  condition vocabulary the frontend table must key on — NOT raw WMO
  codes.
- `src-tauri/src/weather_poller.rs` alert cards: rain-incoming +
  hot/cold threshold alerts, `alert_event` at
  `weather_poller.rs:198-205`, `WEATHER_ALERT_TTL_SECS` = 8 (line 44),
  `origin: SourceKind::Weather`. Today they carry no condition word in
  a structured field — the condition lives in the title/body text. See
  Step 4 for the honest way to key the mood.
- `src-tauri/src/config.rs:68-69` — `weather_lat`/`weather_lon` already
  exist; Open-Meteo responses arrive in LOCATION-LOCAL time
  (`weather_poller.rs:104-106` parses `current.time` as a naive local
  datetime), so day/night needs no timezone math of our own.
- `src/components/StatusRailCard.tsx` — no weather-specific branch
  exists; weather alerts render through the generic card path
  (`eventType: "generic"`, `origin` isn't on the wire).
- `src/styles.css` — no `.wx-*` rules exist yet; the shipped
  `news-shade` precedent (styles.css:619-634) shows how a card-variant
  background layer composes with `.compact`/`.manifest` z-ordering.
- Vite: `src/` assets imported in TS are bundled by `vite build` (the
  overlay has no runtime network by architecture — ARCHITECTURE.md's
  receive-only/no-frontend-network law, restated in
  `plans/frontend-ui-consolidated.html`'s constraints).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Frontend unit tests | `npx vitest run` | all pass |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Lint + format gate | `npx biome ci .` | exit 0 |
| Frontend build | `npx vite build` | exit 0, SVGs emitted into `dist/` |
| Rust tests (only if Step 4 touches the poller) | `cd src-tauri && cargo test --locked` | all pass |

## Scope

**In scope**:
- Move `prototype/assets/weather/*.svg` → `src/assets/weather/` (all
  12, spares included), plus a `NOTICE` file with the Meteocons
  attribution.
- A new frontend data table (suggest `src/lib/weatherArt.ts`):
  condition word → (glyph import, mood class, texture class), with
  day/night variants.
- Apply mood+glyph to the weather alert cards (rain-incoming/hot/cold)
  via `StatusRailCard.tsx` + `src/styles.css`, mirrored in
  `src/settings/preview-overlay.css` same commit.
- Prep for the future hover weather-peek: keep the table and class
  names exactly the ones `prototype/notch-states.html`'s `.wx-block`
  peek will consume (Step 5 — naming/shape only, no interaction).
- Tests per the Test plan below.

**Out of scope**:
- The hover weather-peek interaction itself (gated on plan 086), and
  the idle expanded-on-hover weather scene (also 086-gated; the peek
  block markup lives in the future 079-shape plan, not here).
- Assigning the spare buckets (`extreme-rain`, `wind`, `hail`) — the
  consolidated page lists their assignment as open; ship them vendored
  but unassigned, exactly as the gallery does.
- Animated SVGs or Lottie — static only, per the lock.
- Changing `condition_word`'s vocabulary or the alert thresholds.
- The idle-rail weather chip's text (`{temp} {condition}`) — unchanged.

## Steps

### Step 1: Vendor assets + NOTICE

Move the 12 SVGs from `prototype/assets/weather/` to
`src/assets/weather/` (git mv semantics — the prototype copies are the
same files; leave the prototype's own copies in place so
`prototype/weather-art.html` keeps rendering standalone). Add
`src/assets/weather/NOTICE` (or a top-level `NOTICE` section if the
repo already has one — check first) containing: Meteocons by Bas
Milius, MIT license, upstream URL, and which files it covers. Keep the
SVGs byte-identical — no "optimization" pass that would muddy the
attribution/provenance story.

**Verify**: `ls src/assets/weather/ | wc -l` → 13 (12 SVGs + NOTICE);
`grep -l "Milius" src/assets/weather/NOTICE`.

### Step 2: The condition→art data table

New `src/lib/weatherArt.ts`: import each glyph with vite asset URLs
(`import clearDayUrl from "../assets/weather/clear-day.svg?url"` — or
the `new URL(..., import.meta.url)` form; check what this repo's vite
version favors and match it), then export a plain lookup keyed on the
SHIPPED condition vocabulary (`Clear | Cloudy | Fog | Rain | Snow |
Showers | Storm`, mirroring `weather_poller.rs:83-94`):

```ts
type WeatherArt = { glyphUrl: string; moodClass: string; textureClass: string | null };
export function weatherArtFor(condition: string, isDay: boolean): WeatherArt
```

Mapping per the gallery: Clear→clear-day/clear-night + wx-clear-day/
wx-clear-night; Cloudy→partly-cloudy-(day|night) + matching mood, and
overcast.svg + wx-overcast for the fully-grey case if the table
distinguishes it (the gallery shows both — if the 7-word vocabulary
can't distinguish Cloudy vs Overcast, key them together and note it);
Rain/Showers→rain.svg + wx-rain (day) / wx-rainy-night (night) +
wx-rain-streaks; Storm→thunderstorms.svg + wx-thunderstorm + streaks;
Snow→snow.svg + wx-snow + wx-snow-dots; Fog→fog.svg + wx-fog; unknown/
"—"→ a neutral default (overcast glyph + wx-overcast). Every switch
exhaustive with an `assertNever`-style default arm, matching
`src/lib/presentation.ts`'s table discipline.

**Verify**: `npx tsc --noEmit` → exit 0; `npx vite build` → exit 0 and
the SVGs appear under `dist/` (hashed assets) — `ls dist/assets | grep -c svg` ≥ 12.

### Step 3: Day/night derivation

The frontend must not invent timezone math. Day/night comes from
location time, which the data already has: Open-Meteo's
`current.time` is location-local (`weather_poller.rs:104-106` relies on
this). Two honest options, in preference order: (a) parse Open-Meteo's
`is_day` field (present in the current-weather payload) rust-side and
carry it on the weather alert's `EventMeta.details` or the
`WeatherSummary` — small, fixture-testable poller change; (b) derive
from the local hour in `current.time` rust-side (06:00–18:00 = day) —
same plumbing, one less field to trust. Either way the BOOLEAN crosses
the wire from rust; the frontend never computes it from the user's
wall clock + lat/lon. Pick (a), fall back to (b) if `is_day` isn't in
the captured fixture (`src-tauri/tests/fixtures/open-meteo-bangalore.json`
— check it first). If the fixture lacks it, extend the fixture and say
so in the completion report.

**Verify**: `cd src-tauri && cargo test --locked` → all pass (add a
fixture-backed test for the day/night boolean if the poller changes).

### Step 4: Apply mood+glyph to weather alert cards

Weather alerts arrive as `eventType: "generic"` with
`origin: SourceKind::Weather` — and `origin` is NOT on the slot-state
wire today (`event.rs:183-206`). Do NOT add origin to the wire for
this. Instead, rust-side, attach the condition + day/night to the
alert's existing `meta.details` mechanism (plan 035's
`{label, value}` pairs — display-only by contract, already validated
frontend-side): e.g. `{label: "wx-condition", value: "Rain"}` +
`{label: "wx-is-day", value: "1"}`. The weather_poller already builds
the alert's title/body from the same data, so this is additive, not a
new data path. Frontend: in `StatusRailCard.tsx`'s generic branch, when
`renderedSlot.details` carries the `wx-condition` marker, add the mood
class + texture class to the card and render the `<img class="wx-icon"
src={glyphUrl} alt="">` layer behind `.compact`, following the
`news-shade` z-order precedent (styles.css:633-634 — content sits above
the background layer). Keep it strictly opt-in via the marker: every
other generic card renders byte-identically to today. Add the
`.wx-card`-adapted mood gradients + `.wx-icon` + texture classes to
`src/styles.css` (adapted to the rail-card, not the prototype's
standalone 320px card), and mirror ALL of it in
`src/settings/preview-overlay.css` in the same commit.

**Verify**: `npx vitest run` → all pass; `npx tsc --noEmit` → exit 0;
mirror grep — `grep -c 'wx-icon\|wx-clear-day\|wx-rain\b' src/settings/preview-overlay.css` ≥ 3.

### Step 5: Prep (do not build) the hover weather-peek

Naming/shape only: the mood/texture/glyph class names from Steps 2-4
must be exactly the ones a future `.wx-block` peek block will reuse
(the prototype's `wx-*` vocabulary — already satisfied if Steps 2-4
kept it), and `weatherArtFor` must be importable without pulling in
StatusRailCard. No peek markup, no hover CSS, no interaction — one
comment at the table noting it feeds the 086-gated peek.

**Verify**: no command — structural check: `weatherArtFor` imports only
assets + types.

### Step 6: Full gate

**Verify**: every applicable command in the Commands table exits 0.

## Test plan

- **Rust (cargo)** — only if Step 3/4 touch the poller: fixture test
  that an alert event carries the `wx-condition`/`wx-is-day` detail
  pairs; day/night derivation boundary test against the Bangalore
  fixture's local time. Otherwise no rust tests needed.
- **Frontend (vitest)**: `weatherArtFor` returns the gallery's pairings
  for all 7 condition words × day/night (table test, same discipline as
  `presentation.test.ts`); unknown input → neutral default, never
  throws. StatusRailCard: a generic card WITH the wx markers renders
  the mood class + glyph img; one WITHOUT renders exactly the shipped
  card (regression pin).
- **Manual-only** (operator, TESTING_STRATEGY §5): visual check of a
  real rain-incoming/hot/cold alert against `prototype/weather-art.html`'s
  gallery; legibility of title/body over the gradients (scrim does its
  job); the Settings Appearance preview shows the same (mirror-law
  smoke).

## Done criteria

- [ ] 12 SVGs in `src/assets/weather/` + NOTICE with Meteocons MIT attribution; prototype copies untouched
- [ ] `weatherArtFor` table covers all 7 shipped condition words × day/night, exhaustive, gallery-faithful
- [ ] Day/night crosses the wire from rust (no frontend timezone math)
- [ ] Weather alert cards render mood+glyph; non-weather cards byte-identical
- [ ] All `.wx-*` CSS mirrored in `src/settings/preview-overlay.css`, same commit
- [ ] `npx vitest run`, `npx tsc --noEmit`, `npx biome ci .`, `npx vite build` (and `cargo test --locked` if the poller changed) all exit 0; SVGs bundled in `dist/`
- [ ] `plans/079-checklist.html` and `plans/frontend-ui-consolidated.html` statuses updated; `plans/README.md` row for 082 updated

## STOP conditions

- **Prototype drift**: `prototype/weather-art.html` or
  `prototype/assets/weather/` differs from what this plan describes
  (drift-check non-empty) — the pairings were awaiting final operator
  sign-off at filing; if they changed, re-confirm before implementing.
- **Mirror-law risk**: a `.wx-*` rule can't be scoped into
  `preview-overlay.css` — stop; do not land unmirrored CSS.
- License/attribution uncertainty: if any SVG in the folder is NOT
  Meteocons/MIT (check before moving — provenance matters here), stop
  and exclude it rather than shipping unattributed art.
- `is_day` absent from the fixture AND the local-hour derivation turns
  out wrong for a real edge (polar day/night, fixture timestamps near
  midnight) — present the approximation honestly rather than hiding it.
- You find yourself writing hover/peek interaction code — that's
  086-gated; stop.

## Maintenance notes

- Update `plans/079-checklist.html` and
  `plans/frontend-ui-consolidated.html` (the "Weather art direction"
  locked-decision entry → shipped; the "spare buckets assignment" open
  question stays open — `extreme-rain`/`wind`/`hail` remain
  vendored-unassigned).
- The `wx-condition` detail-pair marker is a deliberate reuse of plan
  035's display-only `details` channel; if a third art-carrying source
  ever appears, that pattern is ready to generalize — point at this
  plan.
- When plan 086 unblocks hover, the weather-peek block consumes
  `weatherArtFor` + these exact classes; don't rename them afterward
  without updating this note's readers.
