import { describe, expect, it } from "vitest";
import type { NowPlayingSummary, StatusState } from "../useStatusState";
import { iconPresenceFor } from "./iconPresence";

// The same all-gates-off shape `useStatusState.ts`'s own FALLBACK_STATUS
// uses — every test below starts from "nothing is happening" and turns
// exactly one thing on, so a mapping that leaked between sources would
// fail loudly rather than being masked by a busy fixture.
const QUIET: StatusState = {
  paused: false,
  waiting: 0,
  agent: { activeSessions: 0 },
  football: { enabled: false, live: null },
  news: { enabled: false, chargeFraction: 0, chargeCount: 0, isCharged: false },
  weather: { enabled: false, current: null },
  media: { enabled: false, current: null },
};

const TRACK: NowPlayingSummary = {
  title: "Midnight City",
  artist: "M83",
  album: "Hurry Up, We're Dreaming",
  playing: true,
  elapsedMs: 1500,
  durationMs: 243_000,
  capturedAtMs: 1_753_000_000_000,
  appBundleId: "app.zen-browser.zen",
};

describe("iconPresenceFor (plan 171, spec §6's presence/liveness table)", () => {
  it("hides agent, football, and music when nothing is running", () => {
    const presence = iconPresenceFor(QUIET);
    expect(presence.agent).toBe("hidden");
    expect(presence.football).toBe("hidden");
    expect(presence.music).toBe("hidden");
  });

  it("keeps weather and news present whenever the strip is up, even with every gate off", () => {
    const presence = iconPresenceFor(QUIET);
    expect(presence.weather).toBe("present");
    expect(presence.news).toBe("present");
  });

  describe("agent — present is live", () => {
    it("goes live on the first registered session", () => {
      expect(iconPresenceFor({ ...QUIET, agent: { activeSessions: 1 } }).agent).toBe("live");
    });

    it("stays live for many sessions (count is a presence gate, not a tier)", () => {
      expect(iconPresenceFor({ ...QUIET, agent: { activeSessions: 7 } }).agent).toBe("live");
    });

    it("hides again at zero sessions", () => {
      expect(iconPresenceFor({ ...QUIET, agent: { activeSessions: 0 } }).agent).toBe("hidden");
    });
  });

  describe("football — present is live", () => {
    it("goes live when a match is in play", () => {
      const presence = iconPresenceFor({
        ...QUIET,
        football: { enabled: true, live: { label: "Arsenal 2–0 Chelsea", minute: "45'" } },
      });
      expect(presence.football).toBe("live");
    });

    it("stays hidden when the source is enabled but nothing is in play", () => {
      const presence = iconPresenceFor({ ...QUIET, football: { enabled: true, live: null } });
      expect(presence.football).toBe("hidden");
    });
  });

  describe("music — the one source with a real present-but-not-live tier", () => {
    it("goes live while audio is genuinely playing", () => {
      const presence = iconPresenceFor({ ...QUIET, media: { enabled: true, current: TRACK } });
      expect(presence.music).toBe("live");
    });

    it("stays present, not live, while a track is loaded but paused", () => {
      const presence = iconPresenceFor({
        ...QUIET,
        media: { enabled: true, current: { ...TRACK, playing: false } },
      });
      expect(presence.music).toBe("present");
    });

    it("hides entirely when there is no now-playing session at all", () => {
      const presence = iconPresenceFor({ ...QUIET, media: { enabled: true, current: null } });
      expect(presence.music).toBe("hidden");
    });
  });

  describe("weather — always present, never live", () => {
    it("stays present rather than escalating once a real reading arrives", () => {
      const presence = iconPresenceFor({
        ...QUIET,
        weather: {
          enabled: true,
          current: {
            tempDisplay: "27°",
            condition: "Cloudy",
            isDay: true,
            rainPct: null,
            todayHighDisplay: null,
            todayLowDisplay: null,
            outlook: [],
          },
        },
      });
      expect(presence.weather).toBe("present");
    });
  });

  describe("news — always present, live only once genuinely charged", () => {
    it("stays dim while merely charging (a rising fraction is not a charge)", () => {
      const presence = iconPresenceFor({
        ...QUIET,
        news: { enabled: true, chargeFraction: 0.8, chargeCount: 4, isCharged: false },
      });
      expect(presence.news).toBe("present");
    });

    it("escalates to live once the charge has fired", () => {
      const presence = iconPresenceFor({
        ...QUIET,
        news: { enabled: true, chargeFraction: 1, chargeCount: 5, isCharged: true },
      });
      expect(presence.news).toBe("live");
    });
  });

  it("maps every source independently — one live source never lights another", () => {
    const presence = iconPresenceFor({ ...QUIET, agent: { activeSessions: 2 } });
    expect(presence).toEqual({
      agent: "live",
      football: "hidden",
      music: "hidden",
      weather: "present",
      news: "present",
    });
  });
});
