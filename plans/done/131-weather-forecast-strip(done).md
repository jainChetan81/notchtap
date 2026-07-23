# Plan 131: weather prediction in the peek — minimal forecast strip

> Filed 2026-07-24 (operator: "the minimal version, do it" — each poll
> shows current AND prediction). Authored at master `da986b5`. Read
> `weather_poller.rs`, `status.rs`'s WeatherSummary, and
> `IdleHoverPeek.tsx`'s weather readout fully before coding. Plan 122
> and 129 recently touched all three — respect their comments.

## Data (rust)

Extend the existing Open-Meteo request (`weather_poller.rs` URL
builder) — same single call, no extra API cost:
- add `temperature_2m,weather_code` to the `hourly=` list (it already
  fetches `precipitation_probability`),
- add `daily=temperature_2m_max,temperature_2m_min`,
- add `timezone=auto` if not already present (hourly `time` values
  must be local for hour labels; the existing rain-lookahead time
  parsing must keep working — its tests are the guard; STOP if
  `timezone=auto` would change the semantics its tests pin).

Extend `WeatherSummary` (status.rs) — all following the
`temp_display`/`is_day`/`rain_pct` conventions (display-ready strings,
camelCase wire, ordinary derived-PartialEq change guard — this is NOT
SlotState::dedup_eq territory, changes at most once per poll; say so
in the doc comment like `rain_pct` does):
- `today_high_display: Option<String>`, `today_low_display:
  Option<String>` (same `"{:.0}°"` format as temp_display),
- `outlook: Vec<OutlookPoint>` with `OutlookPoint { hour_label:
  String /* "15:00", local */, temp_display: String, condition:
  String /* same condition_word() mapping */, is_day: bool }` —
  exactly **3 points**: +2h, +4h, +6h from the poll's current hour
  (nearest hourly slots; skip points past the fetched window rather
  than fabricating). Empty vec when hourly data is missing/malformed
  (never fail the whole summary for a missing forecast).

Unit tests: point selection (+2/+4/+6 nearest-slot logic incl. the
end-of-day boundary), hi/lo formatting, missing-hourly → empty outlook
+ None hi/lo, serialization camelCase shape (outlook array + null
hi/lo pinned like `rainPct`'s null pin), a lone outlook change is a
content change under the derived-PartialEq guard (mirror the
`rain_pct` test).

## Frontend (minimal — the peek stays glanceable)

`useStatusState.ts`: validate the new fields (lenient like `rainPct`:
missing → reject per current strictness conventions — mirror exactly
how `rainPct` was handled, including the wire-shape pin rationale).

`IdleHoverPeek.tsx` weather readout:
- hi/lo inline with the current temp: `21° · H 24° L 16°` style (the
  existing `.wx-peek-temp` row; small muted spans, only when both are
  present).
- a **forecast strip** — one compact row of the 3 OutlookPoints (tiny
  glyph via the existing `weatherArtFor(condition, isDay)`, temp,
  hour label) — rendered ONLY in the weather-owned peek state (no
  media, no football — the precedence chain already yields that
  state), positioned between the readout row and the timeline.
- HARD CONSTRAINT: the peek's outer height is a fixed 100px mirrored
  by rust (`hover.rs` `IDLE_PEEK_BELOW_BLOCK_H` — "any change to this
  height MUST change that constant in the same commit", its own doc).
  The strip must fit WITHIN the existing 100px (the weather-owned
  state currently has spare vertical room below the readout; verify
  by reading the layout, shrink the strip's type/glyph sizes to fit).
  STOP if it genuinely cannot fit without changing the height — do
  NOT change the height or hover.rs in this plan.
- Mount/appearance: the strip renders with the peek's own container
  spring (no separate entrance animation); if outlook flips
  empty↔present mid-peek, reuse the rain chip's AnimatePresence
  idiom (`initial={false}`, ROTATION_ENTER_MS — same documented
  reuse).

Frontend tests: strip renders 3 points with glyph/temp/label; absent
when outlook empty; never renders alongside media/football; hi/lo
inline rendering + absence.

## Verification ladder

cargo test/clippy(-D warnings)/fmt from src-tauri/; tsc, vitest,
biome ci, vite build. §0 both lines. Final look operator-verified on
device (flag it).

## STOP conditions

- `timezone=auto` changes rain-lookahead semantics its tests pin.
- The strip can't fit the fixed 100px peek.
- Any temptation to touch hover.rs or the peek height.
