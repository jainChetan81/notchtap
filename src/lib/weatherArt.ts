// Condition → art data table for weather ALERT cards (plan 082). Pure
// data, no rendering — same "config table, not a new render path"
// discipline as lib/presentation.ts. Feeds StatusRailCard's weather
// branch today, and is deliberately import-standalone (no dependency on
// StatusRailCard or any React code) so a future hover weather-peek
// (plan 087 — NOT built yet) can reuse it without pulling in the card.
//
// Glyphs are Meteocons (MIT, Bas Milius) vendored at src/assets/weather/
// — see the NOTICE file there for full attribution. Mood gradients and
// class names below are lifted verbatim from the approved gallery,
// prototype/weather-art.html — do not rename the wx-* classes without
// updating both that file's role as the source of truth and this
// module's future readers (plan 087).
//
// Glyph URLs use the explicit `new URL(..., import.meta.url).href` form
// with a `?no-inline` query suffix. All 12 SVGs are 745-3017 bytes,
// under vite's default 4 KB assetsInlineLimit — verified empirically
// against this repo's installed vite (v7.3.6): NEITHER the plain `?url`
// suffix NOR a bare `new URL(...).href` bypasses that limit on its own,
// so without `?no-inline` every glyph is silently inlined as a base64
// data-URL and zero .svg files are emitted into dist/. `?no-inline` is
// vite's own asset-pipeline escape hatch (`fileToBuiltUrl`'s
// `shouldInline` check) for exactly this: force a real emitted asset
// file regardless of size. This repo has no prior asset-import
// precedent, so this file sets it.
function glyphUrl(name: string): string {
  return new URL(`../assets/weather/${name}.svg?no-inline`, import.meta.url).href;
}

const CLEAR_DAY = glyphUrl("clear-day");
const CLEAR_NIGHT = glyphUrl("clear-night");
const PARTLY_CLOUDY_DAY = glyphUrl("partly-cloudy-day");
const PARTLY_CLOUDY_NIGHT = glyphUrl("partly-cloudy-night");
const OVERCAST = glyphUrl("overcast");
const RAIN = glyphUrl("rain");
const THUNDERSTORMS = glyphUrl("thunderstorms");
const SNOW = glyphUrl("snow");
const FOG = glyphUrl("fog");

export type WeatherArt = {
  glyphUrl: string;
  moodClass: string;
  textureClass: string | null;
};

// The shipped condition vocabulary — mirrors weather_poller.rs:83-94's
// `condition_word` exactly. That function is the canonical source; this
// list must stay in lockstep with it (a 7th word added there needs a case
// added here too, or it silently falls into the neutral default below).
type ConditionWord = "Clear" | "Cloudy" | "Fog" | "Rain" | "Snow" | "Showers" | "Storm";

// The neutral fallback for unknown/"—" conditions (Design decision in
// plan 082): the overcast glyph + mood, reserved exclusively for this
// case — see the CLOUDY/OVERCAST note below for why "Cloudy" itself
// does NOT use this glyph.
const NEUTRAL: WeatherArt = { glyphUrl: OVERCAST, moodClass: "wx-overcast", textureClass: null };

// CLOUDY/OVERCAST: the 7-word vocabulary (condition_word) has no
// "Overcast" word — WMO codes 1..=3 all collapse to "Cloudy". So
// "Cloudy" is keyed to the partly-cloudy glyph/mood (day and night), and
// `overcast.svg` + `wx-overcast` are reserved solely for the neutral
// unknown-condition fallback, exactly as the plan specifies. The
// vocabulary cannot distinguish a fully grey sky from a partly cloudy
// one, so this table doesn't pretend to either.
//
// Fog/Storm/Snow have only one vendored glyph each (no night variant is
// vendored), so their night entry reuses the same glyph — day/night
// still forks the mood class where the gallery defines one (Storm/Snow
// don't get a distinct night mood either, for the same reason).
const TABLE: Record<ConditionWord, { day: WeatherArt; night: WeatherArt }> = {
  Clear: {
    day: { glyphUrl: CLEAR_DAY, moodClass: "wx-clear-day", textureClass: null },
    night: { glyphUrl: CLEAR_NIGHT, moodClass: "wx-clear-night", textureClass: null },
  },
  Cloudy: {
    day: { glyphUrl: PARTLY_CLOUDY_DAY, moodClass: "wx-partly-cloudy-day", textureClass: null },
    night: {
      glyphUrl: PARTLY_CLOUDY_NIGHT,
      moodClass: "wx-partly-cloudy-night",
      textureClass: null,
    },
  },
  Fog: {
    day: { glyphUrl: FOG, moodClass: "wx-fog", textureClass: null },
    night: { glyphUrl: FOG, moodClass: "wx-fog", textureClass: null },
  },
  Rain: {
    day: { glyphUrl: RAIN, moodClass: "wx-rain", textureClass: "wx-rain-streaks" },
    night: { glyphUrl: RAIN, moodClass: "wx-rainy-night", textureClass: "wx-rain-streaks" },
  },
  Showers: {
    day: { glyphUrl: RAIN, moodClass: "wx-rain", textureClass: "wx-rain-streaks" },
    night: { glyphUrl: RAIN, moodClass: "wx-rainy-night", textureClass: "wx-rain-streaks" },
  },
  Storm: {
    day: { glyphUrl: THUNDERSTORMS, moodClass: "wx-thunderstorm", textureClass: "wx-rain-streaks" },
    night: {
      glyphUrl: THUNDERSTORMS,
      moodClass: "wx-thunderstorm",
      textureClass: "wx-rain-streaks",
    },
  },
  Snow: {
    day: { glyphUrl: SNOW, moodClass: "wx-snow", textureClass: "wx-snow-dots" },
    night: { glyphUrl: SNOW, moodClass: "wx-snow", textureClass: "wx-snow-dots" },
  },
};

function isConditionWord(condition: string): condition is ConditionWord {
  return condition in TABLE;
}

/// Look up the glyph/mood/texture for a condition word + day/night flag.
/// `condition` is a plain wire string (rust's `condition_word` output,
/// never a compile-time literal type), so an unrecognized value (a
/// future WMO code the vocabulary doesn't cover yet, or the "—" unknown
/// sentinel) falls back to `NEUTRAL` rather than throwing — this must
/// never crash the render path just because a new condition word landed
/// on one side before the other.
export function weatherArtFor(condition: string, isDay: boolean): WeatherArt {
  if (!isConditionWord(condition)) {
    return NEUTRAL;
  }
  const entry = TABLE[condition];
  return isDay ? entry.day : entry.night;
}
