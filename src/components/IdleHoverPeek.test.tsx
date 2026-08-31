import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { StatusState } from "../useStatusState";
import { IdleHoverPeek } from "./IdleHoverPeek";

afterEach(cleanup);

const LIVE_MATCH_STATUS: StatusState = {
  paused: false,
  waiting: 0,
  agent: { activeSessions: 0 },
  football: { enabled: true, live: { label: "MTL 1-0 TOR", minute: "63'" } },
  news: { enabled: false, chargeFraction: 0, chargeCount: 0, isCharged: false },
};

const NOTHING_AMBIENT_STATUS: StatusState = {
  paused: false,
  waiting: 0,
  agent: { activeSessions: 0 },
  football: { enabled: false, live: null },
  news: { enabled: false, chargeFraction: 0, chargeCount: 0, isCharged: false },
};

describe("IdleHoverPeek", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders nothing while not hovered", () => {
    const { container } = render(<IdleHoverPeek status={LIVE_MATCH_STATUS} hovered={false} />);
    expect(container.querySelector(".idle-peek")).toBeNull();
  });

  it("opens (mounts a .below-block.idle-peek) when hovered is true", () => {
    const { container } = render(<IdleHoverPeek status={LIVE_MATCH_STATUS} hovered={true} />);
    expect(container.querySelector(".below-block.idle-peek")).not.toBeNull();
  });

  it("still opens with the day-progress timeline alone when no ambient data exists", () => {
    const { container } = render(<IdleHoverPeek status={NOTHING_AMBIENT_STATUS} hovered={true} />);
    expect(container.querySelector(".below-block.idle-peek")).not.toBeNull();
    expect(container.querySelector(".idle-peek-timeline")).not.toBeNull();
    expect(container.querySelector(".idle-reveal-scorecard")).toBeNull();
  });

  it("shows the scorecard reveal when a live match exists", () => {
    const { container } = render(<IdleHoverPeek status={LIVE_MATCH_STATUS} hovered={true} />);
    expect(container.querySelector(".idle-reveal-scorecard")).not.toBeNull();
    expect(container.querySelector(".idle-reveal-label")?.textContent).toBe("MTL 1-0 TOR");
    expect(container.querySelector(".clock-pill")?.textContent).toBe("63'");
    expect(container.querySelector(".idle-peek-timeline")).not.toBeNull();
  });

  it("renders with no status prop at all (settings preview / older callers)", () => {
    const { container } = render(<IdleHoverPeek hovered={true} />);
    expect(container.querySelector(".below-block.idle-peek")).not.toBeNull();
    expect(container.querySelector(".idle-peek-timeline")).not.toBeNull();
  });

  describe("prefer", () => {
    it("defaults to the shipped precedence chain — a live match fills the slot", () => {
      const { container } = render(<IdleHoverPeek status={LIVE_MATCH_STATUS} hovered={true} />);
      expect(container.querySelector(".idle-reveal-scorecard")).not.toBeNull();
    });

    it('prefer="football" shows the scorecard', () => {
      const { container } = render(
        <IdleHoverPeek status={LIVE_MATCH_STATUS} hovered={true} prefer="football" />,
      );
      expect(container.querySelector(".idle-reveal-scorecard")).not.toBeNull();
    });

    it("a preferred source with no data leaves the content slot empty rather than falling through", () => {
      const { container } = render(
        <IdleHoverPeek status={NOTHING_AMBIENT_STATUS} hovered={true} prefer="football" />,
      );
      expect(container.querySelector(".below-block.idle-peek")).not.toBeNull();
      expect(container.querySelector(".idle-reveal-scorecard")).toBeNull();
      expect(container.querySelector(".idle-peek-timeline")).not.toBeNull();
    });
  });
});
