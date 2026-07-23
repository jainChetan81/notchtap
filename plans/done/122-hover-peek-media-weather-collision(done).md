# Plan 122: idle-hover peek — media row / weather glyph collision + richer ambient weather

> Filed 2026-07-23 (operator screenshot: media progress bar + elapsed
> time rendered under/through the weather cloud SVG). Drift check:
> authored against master `7ee95d3`. Re-verify citations if
> `src/components/IdleHoverPeek.tsx` or `src/overlay-card.css`
> (peek region ~:1550-1950) changed since.

## Problem (verified)

The weather glyph `<img class="wx-icon">` is absolutely positioned in
the peek's top-right corner (`position:absolute; top:10px; right:14px;
width:48px; height:48px`, `overlay-card.css:1575-1584`, mounted from
`IdleHoverPeek.tsx:70-72` inside `.wx-peek-backdrop` at z-index 0).
The media row's transport (`.media-transport`: play/pause + 56px
`.media-bar` + `.media-time` digits, `overlay-card.css:1909-1945`) is
flex-pinned to the SAME right edge at z-index 1. Whenever media is
playing, the two occupy the same region — structural, independent of
title length (`.media-title` already truncates,
`overlay-card.css:1860-1901`).

## Fix (Part A — layout)

Move the glyph INTO layout flow so overlap is impossible by
construction, in both states (media present / absent):

- Relocate the `<img class="wx-icon">` out of the absolute backdrop
  and into the `.peek-content` weather header row as a flex sibling
  of `.wx-peek-temp`/`.wx-peek-condition` (`IdleHoverPeek.tsx:78-88`).
  Size ~28-32px in-flow (executor picks the exact size that balances
  the row; keep the 48px look only if it fits in flow without pushing
  the row taller).
- `.wx-peek-backdrop` keeps its mood-gradient/texture role
  (`overlay-card.css:1808-1814` + `.wx-card` classes `:1558-1629`) —
  only the glyph moves out of it.
- The transport region must end up with NOTHING absolutely positioned
  over it. Delete the now-dead `.wx-icon` absolute rules rather than
  overriding them.

Structural test (jsdom has no layout, so assert structure): render the
peek with media present and assert the `.wx-icon` element is a
descendant of the weather header row (not of `.wx-peek-backdrop`),
and that no rule keeps it `position:absolute` (class-level
assertion). Existing peek tests must stay green.

## Fix (Part B — "better info", small)

Surface rain chance in the peek's weather header: `Rain 40%` style,
next to the condition word, only when ≥ some floor (use the existing
configured `weather_rain_threshold_pct` as the display floor — no new
config key).

- The poller already fetches `hourly.precipitation_probability`
  (`weather_poller.rs:53-76`, Open-Meteo). Extend the ambient
  `WeatherSummary` (`weather_poller.rs:151-162`) with
  `rain_pct: Option<u8>` = max probability over the existing
  configured lookahead window (reuse the alert path's lookahead
  logic/tests as the reference — `weather_poller.rs:164-223`).
- **CRITICAL — dedup rule:** before adding the field, trace how
  `WeatherSummary` reaches the frontend. If it rides any struct
  covered by `SlotState::dedup_eq`, the new field must be added to
  `dedup_eq` EXPLICITLY (CLAUDE.md rule: never rely on derived
  `PartialEq`; a per-poll-changing field must be a deliberate
  content-change decision — here it SHOULD count as content change,
  i.e. include it in the comparison, since it changes at most once
  per poll, not per tick). STOP if the wiring is unclear.
- Frontend: render in `IdleHoverPeek.tsx` weather header; hide
  entirely when `rain_pct` is absent or below the floor.

## Non-goals

- No new weather API calls or fields beyond `rain_pct`.
- No changes to the weather ALERT cards or thresholds.
- No new SVG assets (the Meteocons set in `src/assets/weather/` is
  already good; hail/wind/extreme-rain stay vendored-unused).
- Don't touch the exit choreography or any CSS outside the peek
  region (plan 123 owns `.exiting`/`.bare` — avoid merge conflicts).

## Verification ladder

`npx tsc --noEmit`, `npx vitest run`, `npx biome ci .`, `npx vite
build`; if rust touched: `cargo test --locked`, clippy `-D warnings`,
fmt. New rust tests for `rain_pct` derivation (below-floor, at-floor,
empty hourly). Update `docs/TESTING_STRATEGY.md` §0 counts. The final
look is operator-verified on device (flag it, per repo practice for
visual changes).

## STOP conditions

- `WeatherSummary`'s route to the frontend is unclear or touches
  `dedup_eq` in a way not covered above.
- Part A can't be done without changing the peek's outer height in
  the media-absent state (geometry feeds the hover rect — if peek
  height changes, check `src-tauri/src/hover.rs`'s mirrored rect and
  STOP if they'd desync).
