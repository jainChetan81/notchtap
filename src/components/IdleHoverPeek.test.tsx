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

  // hover is NOT CSS `:hover` — the peek is driven
  // entirely by the prop, with no dependency on any CSS pseudo-class.
  // mount is now owned by `motion`'s `AnimatePresence`, which
  // renders the node synchronously in jsdom — no `.open`/`.closing`
  // classes anymore, just DOM presence.
  it("opens (mounts a .below-block.idle-peek) when hovered is true", () => {
    const { container } = render(<IdleHoverPeek status={LIVE_MATCH_STATUS} hovered={true} />);
    expect(container.querySelector(".below-block.idle-peek")).not.toBeNull();
  });

  // the timeline lives here unconditionally — this is its only home, so
  // it must not become unreachable for a user with nothing ambient
  // configured.
  it("still opens with the day-progress timeline alone when no ambient data exists", () => {
    const { container } = render(<IdleHoverPeek status={NOTHING_AMBIENT_STATUS} hovered={true} />);
    expect(container.querySelector(".below-block.idle-peek")).not.toBeNull();
    expect(container.querySelector(".idle-peek-timeline")).not.toBeNull();
    expect(container.querySelector(".idle-reveal-scorecard")).toBeNull();
  });

  // precedence rule: a live match fills the content slot.
  it("shows the scorecard reveal when a live match exists", () => {
    const { container } = render(<IdleHoverPeek status={LIVE_MATCH_STATUS} hovered={true} />);
    expect(container.querySelector(".idle-reveal-scorecard")).not.toBeNull();
    expect(container.querySelector(".idle-reveal-label")?.textContent).toBe("MTL 1-0 TOR");
    expect(container.querySelector(".clock-pill")?.textContent).toBe("63'");
    // the timeline still rides along underneath the reveal.
    expect(container.querySelector(".idle-peek-timeline")).not.toBeNull();
  });

  it("renders with no status prop at all (settings preview / older callers)", () => {
    const { container } = render(<IdleHoverPeek hovered={true} />);
    expect(container.querySelector(".below-block.idle-peek")).not.toBeNull();
    expect(container.querySelector(".idle-peek-timeline")).not.toBeNull();
  });

  // The football TAB selection reaches this component rather than a
  // second copy of it — `prefer` is how the caller says so. With
  // `prefer` omitted, behaviour is exactly the default precedence chain,
  // which the rest of this file already pins.
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

    // a preference NARROWS, it never re-orders and falls through — being
    // shown something else because you asked for football would be a
    // worse lie than being shown nothing.
    it("a preferred source with no data leaves the content slot empty rather than falling through", () => {
      const { container } = render(
        <IdleHoverPeek status={NOTHING_AMBIENT_STATUS} hovered={true} prefer="football" />,
      );
      expect(container.querySelector(".below-block.idle-peek")).not.toBeNull();
      expect(container.querySelector(".idle-reveal-scorecard")).toBeNull();
      // the timeline is unconditional — it never depended on ambient data
      expect(container.querySelector(".idle-peek-timeline")).not.toBeNull();
    });
  });
});
