import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { NOTCHTAP_EASE } from "../animationTiming";
import { IdleFace } from "./IdleFace";

afterEach(cleanup);

const REVEAL_DELAY_MS = 4500;

describe("IdleFace", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe("sparser gaze/blink cadence", () => {
    it("holds the eyes' transform steady for at least 6000ms after becoming visible (no gaze/blink wakeup before the sparser floor)", () => {
      const { container } = render(<IdleFace idle={true} />);
      act(() => vi.advanceTimersByTime(REVEAL_DELAY_MS));

      // SAFETY: IdleFace always renders `.idle-face-eyes` for this idle
      // fixture; the following null-guard protects the cast.
      const eyes = container.querySelector(".idle-face-eyes") as HTMLElement;
      expect(eyes).not.toBeNull();
      const initialTransform = eyes.style.transform;

      act(() => vi.advanceTimersByTime(5999));
      expect(eyes.style.transform).toBe(initialTransform);
    });

    it("a glance actually happens — the eyes' transform changes once the gaze cycle's own floor has definitely elapsed", () => {
      const { container } = render(<IdleFace idle={true} />);
      act(() => vi.advanceTimersByTime(REVEAL_DELAY_MS));

      // SAFETY: IdleFace always renders `.idle-face-eyes` for this idle
      // fixture; the following null-guard protects the cast.
      const eyes = container.querySelector(".idle-face-eyes") as HTMLElement;
      expect(eyes).not.toBeNull();
      const initialTransform = eyes.style.transform;

      act(() => vi.advanceTimersByTime(11001));
      expect(eyes.style.transform).not.toBe(initialTransform);
    });
  });

  describe("eyes: CSS transition, not a motion spring", () => {
    it("carries an inline transition on transform using the house curve, not a spring", () => {
      const { container } = render(<IdleFace idle={true} />);
      act(() => vi.advanceTimersByTime(REVEAL_DELAY_MS));

      // SAFETY: IdleFace always renders `.idle-face-eyes` for this idle
      // fixture; the following null-guard protects the cast.
      const eyes = container.querySelector(".idle-face-eyes") as HTMLElement;
      expect(eyes).not.toBeNull();
      expect(eyes.style.transition).toContain("transform");
      expect(eyes.style.transition).toContain(`cubic-bezier(${NOTCHTAP_EASE.join(", ")})`);
    });

    it("renders the eyes as a plain div (not a motion-managed node) carrying both eye dots", () => {
      const { container } = render(<IdleFace idle={true} />);
      act(() => vi.advanceTimersByTime(REVEAL_DELAY_MS));

      const eyes = container.querySelector(".idle-face-eyes");
      expect(eyes?.tagName).toBe("DIV");
      expect(eyes?.querySelectorAll(".idle-face-eye").length).toBe(2);
    });
  });

  describe("reveal on the house curve", () => {
    it("mounts at the house entrance scale (0.92)", () => {
      const { container } = render(<IdleFace idle={true} />);
      act(() => vi.advanceTimersByTime(REVEAL_DELAY_MS));

      // SAFETY: IdleFace always renders its `.idle-face` root for the idle
      // fixture; the following null-guard protects the cast.
      const face = container.querySelector(".idle-face") as HTMLElement;
      expect(face).not.toBeNull();
      expect(face.style.transform).toContain("scale(0.92)");
    });
  });
});
