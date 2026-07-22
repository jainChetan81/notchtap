import { cleanup, render } from "@testing-library/react";
import { Globe, Music, Play, Tv } from "lucide-react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { NowPlayingSummary, StatusState } from "../useStatusState";
import { IdleHoverPeek, iconForBundleId } from "./IdleHoverPeek";

afterEach(cleanup);

const WEATHER_STATUS: StatusState = {
  paused: false,
  waiting: 0,
  football: { enabled: false, live: null },
  news: { enabled: false },
  weather: { enabled: true, current: { tempDisplay: "27°", condition: "Cloudy", isDay: true } },
  media: { enabled: false, current: null },
};

const LIVE_MATCH_STATUS: StatusState = {
  paused: false,
  waiting: 0,
  football: { enabled: true, live: { label: "MTL 1-0 TOR", minute: "63'" } },
  news: { enabled: false },
  weather: { enabled: true, current: { tempDisplay: "27°", condition: "Cloudy", isDay: true } },
  media: { enabled: false, current: null },
};

const NOTHING_AMBIENT_STATUS: StatusState = {
  paused: false,
  waiting: 0,
  football: { enabled: false, live: null },
  news: { enabled: false },
  weather: { enabled: false, current: null },
  media: { enabled: false, current: null },
};

const NOW_PLAYING: NowPlayingSummary = {
  title: "Midnight City",
  artist: "M83",
  album: "Hurry Up, We're Dreaming",
  playing: true,
  elapsedMs: 1500,
  durationMs: 243_000,
  capturedAtMs: Date.now(),
  appBundleId: "app.zen-browser.zen",
};

const MEDIA_STATUS: StatusState = {
  paused: false,
  waiting: 0,
  football: { enabled: false, live: null },
  news: { enabled: false },
  weather: { enabled: true, current: { tempDisplay: "27°", condition: "Cloudy", isDay: true } },
  media: { enabled: true, current: NOW_PLAYING },
};

const MEDIA_AND_LIVE_MATCH_STATUS: StatusState = {
  ...LIVE_MATCH_STATUS,
  media: { enabled: true, current: NOW_PLAYING },
};

describe("IdleHoverPeek (plan 093)", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders nothing while not hovered", () => {
    const { container } = render(<IdleHoverPeek status={WEATHER_STATUS} hovered={false} />);
    expect(container.querySelector(".idle-peek")).toBeNull();
  });

  // plan 093 constraint 2: hover is NOT CSS `:hover` — the peek is driven
  // entirely by the prop, with no dependency on any CSS pseudo-class.
  // plan 12x: mount is now owned by `motion`'s `AnimatePresence`, which
  // renders the node synchronously in jsdom — no `.open`/`.closing`
  // classes anymore, just DOM presence.
  it("opens (mounts a .below-block.idle-peek) when hovered is true", () => {
    const { container } = render(<IdleHoverPeek status={WEATHER_STATUS} hovered={true} />);
    expect(container.querySelector(".below-block.idle-peek")).not.toBeNull();
  });

  // item 18: the timeline lives here unconditionally — it must not
  // silently become unreachable for a user with nothing ambient
  // configured, since 091 removed its only other home.
  it("still opens with the day-progress timeline alone when no ambient data exists", () => {
    const { container } = render(<IdleHoverPeek status={NOTHING_AMBIENT_STATUS} hovered={true} />);
    expect(container.querySelector(".below-block.idle-peek")).not.toBeNull();
    expect(container.querySelector(".idle-peek-timeline")).not.toBeNull();
    expect(container.querySelector(".wx-peek-backdrop")).toBeNull();
    expect(container.querySelector(".idle-reveal-scorecard")).toBeNull();
  });

  it("renders the weather mood backdrop and readout when weather data is available", () => {
    const { container } = render(<IdleHoverPeek status={WEATHER_STATUS} hovered={true} />);
    expect(container.querySelector(".wx-peek-backdrop.wx-card")).not.toBeNull();
    expect(container.querySelector("img.wx-icon")).not.toBeNull();
    expect(container.querySelector(".wx-peek-temp")?.textContent).toBe("27°");
    expect(container.querySelector(".wx-peek-condition")?.textContent).toBe("Cloudy");
    expect(container.querySelector(".idle-peek-timeline")).not.toBeNull();
  });

  // plan 110 (Step B): the mood art must key off the wire's `isDay`, never
  // the wall clock (the deleted `isDaytimeNow()`) — freeze the system
  // clock at each side's OPPOSITE time of day and prove the art still
  // follows the payload, not `Date`.
  describe("weather art keys off the wire's isDay, not the wall clock (plan 110)", () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    it("renders the day mood at isDay: true even with the system clock at midnight", () => {
      vi.setSystemTime(new Date("2026-07-22T00:00:00"));
      const status: StatusState = {
        ...WEATHER_STATUS,
        weather: {
          enabled: true,
          current: { tempDisplay: "27°", condition: "Cloudy", isDay: true },
        },
      };
      const { container } = render(<IdleHoverPeek status={status} hovered={true} />);
      expect(container.querySelector(".wx-partly-cloudy-day")).not.toBeNull();
      expect(container.querySelector(".wx-partly-cloudy-night")).toBeNull();
    });

    it("renders the night mood at isDay: false even with the system clock at noon", () => {
      vi.setSystemTime(new Date("2026-07-22T12:00:00"));
      const status: StatusState = {
        ...WEATHER_STATUS,
        weather: {
          enabled: true,
          current: { tempDisplay: "27°", condition: "Cloudy", isDay: false },
        },
      };
      const { container } = render(<IdleHoverPeek status={status} hovered={true} />);
      expect(container.querySelector(".wx-partly-cloudy-night")).not.toBeNull();
      expect(container.querySelector(".wx-partly-cloudy-day")).toBeNull();
    });
  });

  // plan 105 (Step B): the operator wanted the weather art kept behind the
  // media row rather than replaced by it — the backdrop is now independent
  // of the precedence chain that picks what renders in `.peek-content`.
  it("keeps the weather backdrop behind the media row when both are available", () => {
    const { container } = render(<IdleHoverPeek status={MEDIA_STATUS} hovered={true} />);
    expect(container.querySelector(".wx-peek-backdrop.wx-card")).not.toBeNull();
    expect(container.querySelector(".media-row")).not.toBeNull();
    // the readout itself still yields to the media row — one visible
    // "content" slot at a time, per the existing precedence rule.
    expect(container.querySelector(".wx-peek-readout")).toBeNull();
  });

  it("has no weather backdrop when a live match is showing (scorecard keeps its own visual)", () => {
    const { container } = render(<IdleHoverPeek status={LIVE_MATCH_STATUS} hovered={true} />);
    expect(container.querySelector(".idle-reveal-scorecard")).not.toBeNull();
    expect(container.querySelector(".wx-peek-backdrop")).toBeNull();
  });

  // plan 092 (item 10) retired `.pill` entirely — the condition label
  // must reuse `.chip`, never reintroduce a pill class.
  it("reuses .chip for the condition label, never a pill", () => {
    const { container } = render(<IdleHoverPeek status={WEATHER_STATUS} hovered={true} />);
    const label = container.querySelector(".wx-peek-condition");
    expect(label?.classList.contains("chip")).toBe(true);
    expect(container.querySelector(".pill")).toBeNull();
  });

  // item 3's precedence rule: football outranks ambient weather, one
  // below-block at a time.
  it("shows the scorecard reveal instead of the weather peek when a live match exists", () => {
    const { container } = render(<IdleHoverPeek status={LIVE_MATCH_STATUS} hovered={true} />);
    expect(container.querySelector(".idle-reveal-scorecard")).not.toBeNull();
    expect(container.querySelector(".idle-reveal-label")?.textContent).toBe("MTL 1-0 TOR");
    expect(container.querySelector(".clock-pill")?.textContent).toBe("63'");
    expect(container.querySelector(".wx-peek-readout")).toBeNull();
    // the timeline still rides along underneath the reveal.
    expect(container.querySelector(".idle-peek-timeline")).not.toBeNull();
  });

  // plan 104: media row precedence + rendering.

  it("renders the media row and timeline when a now-playing session is available", () => {
    const { container } = render(<IdleHoverPeek status={MEDIA_STATUS} hovered={true} />);
    expect(container.querySelector(".media-row")).not.toBeNull();
    expect(container.querySelector(".media-title")?.textContent).toBe("Midnight City");
    expect(container.querySelector(".media-subtitle")?.textContent).toBe("M83");
    // plan 118: play/paused state renders as a lucide icon (svg), not
    // text — lucide-react stamps a "lucide-<name>" class on every icon.
    expect(container.querySelector(".media-state .lucide-play")).not.toBeNull();
    expect(container.querySelector(".idle-peek-timeline")).not.toBeNull();
    // media outranks weather in the content slot — one visible readout at
    // a time — but (plan 105) the weather backdrop itself still shows
    // behind it; see the dedicated backdrop test above.
    expect(container.querySelector(".wx-peek-readout")).toBeNull();
  });

  it("renders the pause icon for a paused now-playing session", () => {
    const paused: StatusState = {
      ...MEDIA_STATUS,
      media: { enabled: true, current: { ...NOW_PLAYING, playing: false } },
    };
    const { container } = render(<IdleHoverPeek status={paused} hovered={true} />);
    expect(container.querySelector(".media-state .lucide-pause")).not.toBeNull();
  });

  it("football outranks media — the scorecard reveal wins when both are available", () => {
    const { container } = render(
      <IdleHoverPeek status={MEDIA_AND_LIVE_MATCH_STATUS} hovered={true} />,
    );
    expect(container.querySelector(".idle-reveal-scorecard")).not.toBeNull();
    expect(container.querySelector(".media-row")).toBeNull();
  });

  it("renders no media row when media.current is null (no-media renders nothing extra)", () => {
    const { container } = render(<IdleHoverPeek status={WEATHER_STATUS} hovered={true} />);
    expect(container.querySelector(".media-row")).toBeNull();
  });

  describe("iconForBundleId (plan 104 Step 7, plan 118 lucide swap)", () => {
    it("maps a Music bundle id to the note icon", () => {
      expect(iconForBundleId("com.apple.Music")).toBe(Music);
    });

    it("maps a TV bundle id to the tv icon", () => {
      expect(iconForBundleId("com.apple.TV")).toBe(Tv);
    });

    it("maps a browser bundle id to the globe icon, case-insensitively", () => {
      expect(iconForBundleId("com.apple.Safari")).toBe(Globe);
      expect(iconForBundleId("app.zen-browser.zen")).toBe(Globe);
      expect(iconForBundleId("com.google.Chrome")).toBe(Globe);
      expect(iconForBundleId("org.mozilla.firefox")).toBe(Globe);
    });

    it("falls back to the play icon for anything else, including null", () => {
      expect(iconForBundleId("com.example.SomeApp")).toBe(Play);
      expect(iconForBundleId(null)).toBe(Play);
    });
  });

  it("renders with no status prop at all (settings preview / older callers)", () => {
    const { container } = render(<IdleHoverPeek hovered={true} />);
    expect(container.querySelector(".below-block.idle-peek")).not.toBeNull();
    expect(container.querySelector(".idle-peek-timeline")).not.toBeNull();
  });
});
