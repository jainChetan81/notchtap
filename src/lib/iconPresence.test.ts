import { describe, expect, it } from "vitest";
import type { StatusState } from "../useStatusState";
import { iconPresenceFor } from "./iconPresence";

const QUIET: StatusState = {
  paused: false,
  waiting: 0,
  agent: { activeSessions: 0 },
  football: { enabled: false, live: null },
  news: { enabled: false, chargeFraction: 0, chargeCount: 0, isCharged: false },
};

describe("iconPresenceFor — the presence/liveness table", () => {
  it("hides agent and football when nothing is running", () => {
    const presence = iconPresenceFor(QUIET);
    expect(presence.agent).toBe("hidden");
    expect(presence.football).toBe("hidden");
  });

  it("keeps news present whenever the strip is up, even with every gate off", () => {
    const presence = iconPresenceFor(QUIET);
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

  it("treats a missing status wire as the all-gates-off fallback", () => {
    expect(iconPresenceFor(undefined)).toEqual({
      agent: "hidden",
      football: "hidden",
      news: "present",
    });
    expect(iconPresenceFor(undefined)).toEqual(iconPresenceFor(QUIET));
  });

  it("maps every source independently — one live source never lights another", () => {
    const presence = iconPresenceFor({ ...QUIET, agent: { activeSessions: 2 } });
    expect(presence).toEqual({
      agent: "live",
      football: "hidden",
      news: "present",
    });
  });
});
