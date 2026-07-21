import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { StatusState } from "../useStatusState";
import { IdleHoverPeek } from "./IdleHoverPeek";

afterEach(cleanup);

function mockReducedMotion(matches: boolean) {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches,
    media: query,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    onchange: null,
    dispatchEvent: () => false,
  }));
}

const WEATHER_STATUS: StatusState = {
  paused: false,
  waiting: 0,
  football: { enabled: false, live: null },
  news: { enabled: false },
  weather: { enabled: true, current: { tempDisplay: "27°", condition: "Cloudy" } },
};

const LIVE_MATCH_STATUS: StatusState = {
  paused: false,
  waiting: 0,
  football: { enabled: true, live: { label: "MTL 1-0 TOR", minute: "63'" } },
  news: { enabled: false },
  weather: { enabled: true, current: { tempDisplay: "27°", condition: "Cloudy" } },
};

const NOTHING_AMBIENT_STATUS: StatusState = {
  paused: false,
  waiting: 0,
  football: { enabled: false, live: null },
  news: { enabled: false },
  weather: { enabled: false, current: null },
};

describe("IdleHoverPeek (plan 093)", () => {
  beforeEach(() => {
    mockReducedMotion(false);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it("renders nothing while not hovered", () => {
    const { container } = render(<IdleHoverPeek status={WEATHER_STATUS} hovered={false} />);
    expect(container.querySelector(".idle-peek")).toBeNull();
  });

  // plan 093 constraint 2: hover is NOT CSS `:hover` — the peek is driven
  // entirely by the prop, with no dependency on any CSS pseudo-class.
  it("opens (mounts a .below-block.idle-peek.open) when hovered is true", () => {
    const { container } = render(<IdleHoverPeek status={WEATHER_STATUS} hovered={true} />);
    const peek = container.querySelector(".below-block.idle-peek.open");
    expect(peek).not.toBeNull();
    expect(peek?.classList.contains("closing")).toBe(false);
  });

  // item 18: the timeline lives here unconditionally — it must not
  // silently become unreachable for a user with nothing ambient
  // configured, since 091 removed its only other home.
  it("still opens with the day-progress timeline alone when no ambient data exists", () => {
    const { container } = render(<IdleHoverPeek status={NOTHING_AMBIENT_STATUS} hovered={true} />);
    expect(container.querySelector(".below-block.idle-peek.open")).not.toBeNull();
    expect(container.querySelector(".idle-peek-timeline")).not.toBeNull();
    expect(container.querySelector(".wx-peek-scene")).toBeNull();
    expect(container.querySelector(".idle-reveal-scorecard")).toBeNull();
  });

  it("renders the weather mood scene and timeline when weather data is available", () => {
    const { container } = render(<IdleHoverPeek status={WEATHER_STATUS} hovered={true} />);
    expect(container.querySelector(".wx-peek-scene.wx-card")).not.toBeNull();
    expect(container.querySelector("img.wx-icon")).not.toBeNull();
    expect(container.querySelector(".wx-peek-temp")?.textContent).toBe("27°");
    expect(container.querySelector(".wx-peek-condition")?.textContent).toBe("Cloudy");
    expect(container.querySelector(".idle-peek-timeline")).not.toBeNull();
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
    expect(container.querySelector(".wx-peek-scene")).toBeNull();
    // the timeline still rides along underneath the reveal.
    expect(container.querySelector(".idle-peek-timeline")).not.toBeNull();
  });

  it("renders with no status prop at all (settings preview / older callers)", () => {
    const { container } = render(<IdleHoverPeek hovered={true} />);
    expect(container.querySelector(".below-block.idle-peek.open")).not.toBeNull();
    expect(container.querySelector(".idle-peek-timeline")).not.toBeNull();
  });

  describe("mount lifecycle (close delay)", () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    it("stays mounted with .closing for the exit window, then unmounts", () => {
      const { container, rerender } = render(
        <IdleHoverPeek status={WEATHER_STATUS} hovered={true} />,
      );
      expect(container.querySelector(".idle-peek.open")).not.toBeNull();

      rerender(<IdleHoverPeek status={WEATHER_STATUS} hovered={false} />);
      // still mounted immediately after the flip, now closing.
      expect(container.querySelector(".idle-peek.closing")).not.toBeNull();
      expect(container.querySelector(".idle-peek.open")).toBeNull();

      act(() => {
        vi.advanceTimersByTime(260);
      });
      expect(container.querySelector(".idle-peek")).toBeNull();
    });

    it("re-opens immediately (no lingering close delay) if hovered flips back true mid-close", () => {
      const { container, rerender } = render(
        <IdleHoverPeek status={WEATHER_STATUS} hovered={true} />,
      );
      rerender(<IdleHoverPeek status={WEATHER_STATUS} hovered={false} />);
      expect(container.querySelector(".idle-peek.closing")).not.toBeNull();

      rerender(<IdleHoverPeek status={WEATHER_STATUS} hovered={true} />);
      expect(container.querySelector(".idle-peek.open")).not.toBeNull();
      expect(container.querySelector(".idle-peek.closing")).toBeNull();

      // the stale close timer must not fire and un-mount the now-open peek.
      act(() => {
        vi.advanceTimersByTime(260);
      });
      expect(container.querySelector(".idle-peek.open")).not.toBeNull();
    });
  });

  it("unmounts immediately with no .closing window under prefers-reduced-motion", () => {
    mockReducedMotion(true);
    const { container, rerender } = render(
      <IdleHoverPeek status={WEATHER_STATUS} hovered={true} />,
    );
    expect(container.querySelector(".idle-peek.open")).not.toBeNull();

    rerender(<IdleHoverPeek status={WEATHER_STATUS} hovered={false} />);
    expect(container.querySelector(".idle-peek")).toBeNull();
  });
});
