import { describe, expect, it } from "vitest";
import { weatherArtFor } from "./weatherArt";

// Every glyph URL points at a real vendored asset (src/assets/weather/*)
// resolved by vite's asset pipeline at import time — under jsdom/vitest
// that resolves to some string, so these tests assert on the class
// names (the stable, testable contract) rather than the exact URL.
function moodOf(condition: string, isDay: boolean): string {
  return weatherArtFor(condition, isDay).moodClass;
}

describe("weatherArtFor", () => {
  it("maps every shipped condition word to the gallery's day mood + glyph", () => {
    expect(moodOf("Clear", true)).toBe("wx-clear-day");
    expect(moodOf("Cloudy", true)).toBe("wx-partly-cloudy-day");
    expect(moodOf("Fog", true)).toBe("wx-fog");
    expect(moodOf("Rain", true)).toBe("wx-rain");
    expect(moodOf("Showers", true)).toBe("wx-rain");
    expect(moodOf("Storm", true)).toBe("wx-thunderstorm");
    expect(moodOf("Snow", true)).toBe("wx-snow");
  });

  it("maps every shipped condition word to the gallery's night mood + glyph", () => {
    expect(moodOf("Clear", false)).toBe("wx-clear-night");
    expect(moodOf("Cloudy", false)).toBe("wx-partly-cloudy-night");
    expect(moodOf("Fog", false)).toBe("wx-fog");
    expect(moodOf("Rain", false)).toBe("wx-rainy-night");
    expect(moodOf("Showers", false)).toBe("wx-rainy-night");
    expect(moodOf("Storm", false)).toBe("wx-thunderstorm");
    expect(moodOf("Snow", false)).toBe("wx-snow");
  });

  it("gives Rain/Showers the rain-streaks texture and Snow the snow-dots texture", () => {
    expect(weatherArtFor("Rain", true).textureClass).toBe("wx-rain-streaks");
    expect(weatherArtFor("Showers", false).textureClass).toBe("wx-rain-streaks");
    expect(weatherArtFor("Storm", true).textureClass).toBe("wx-rain-streaks");
    expect(weatherArtFor("Snow", true).textureClass).toBe("wx-snow-dots");
  });

  it("has no texture for Clear/Cloudy/Fog", () => {
    expect(weatherArtFor("Clear", true).textureClass).toBeNull();
    expect(weatherArtFor("Cloudy", false).textureClass).toBeNull();
    expect(weatherArtFor("Fog", true).textureClass).toBeNull();
  });

  // CLOUDY/OVERCAST: the 7-word vocabulary (weather_poller.rs's
  // condition_word) has no distinct "Overcast" word — WMO codes 1..=3
  // all collapse to "Cloudy". So "Cloudy" is keyed to the partly-cloudy
  // glyph/mood, and wx-overcast is reserved exclusively for the neutral
  // fallback below — this table never emits wx-overcast for "Cloudy".
  it("keys Cloudy to partly-cloudy, never to wx-overcast", () => {
    expect(moodOf("Cloudy", true)).not.toBe("wx-overcast");
    expect(moodOf("Cloudy", false)).not.toBe("wx-overcast");
  });

  it("falls back to the neutral overcast glyph/mood for the unknown '—' sentinel", () => {
    const neutral = weatherArtFor("—", true);
    expect(neutral.moodClass).toBe("wx-overcast");
    expect(neutral.textureClass).toBeNull();
    expect(moodOf("—", false)).toBe("wx-overcast");
    // day and night both resolve to the same neutral entry — unlike the
    // real conditions, the fallback doesn't fork on isDay.
    expect(weatherArtFor("—", false).glyphUrl).toBe(neutral.glyphUrl);
  });

  it("never throws on an arbitrary unrecognized string, day or night", () => {
    expect(() => weatherArtFor("Hurricane", true)).not.toThrow();
    expect(() => weatherArtFor("", false)).not.toThrow();
    expect(moodOf("Hurricane", true)).toBe("wx-overcast");
  });
});
