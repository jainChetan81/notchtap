// plan 125 (/improve-animations audit — finding #1 HIGH perf, finding #11
// LOW character): pins the idle-cost fix (sparser gaze/blink wakeups, the
// eyes off the motion spring and onto a self-ending CSS transition) and
// the character fix (the reveal moved onto the house scale/duration/ease).
// StatusRailCard.test.tsx's own "idle face" describe block already covers
// the reveal-delay gating (idle/hovered/showing interplay) — this file is
// scoped to IdleFace's own internals instead, so those two files don't
// duplicate coverage.
import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { NOTCHTAP_EASE } from "../animationTiming";
import { IdleFace } from "./IdleFace";

afterEach(cleanup);

const REVEAL_DELAY_MS = 4500;

describe("IdleFace (plan 125)", () => {
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

      const eyes = container.querySelector(".idle-face-eyes") as HTMLElement;
      expect(eyes).not.toBeNull();
      const initialTransform = eyes.style.transform;

      // both useGazeCycle and useBlink now schedule their first wakeup no
      // earlier than 6000ms after `visible` flips true (plan 125: was
      // 1500/3000ms) — advancing right up to, but not past, that floor
      // must leave the eyes exactly where they started.
      act(() => vi.advanceTimersByTime(5999));
      expect(eyes.style.transform).toBe(initialTransform);
    });

    // plan 129 (T5, deep-review fix): the test above only proves nothing
    // happens BEFORE the sparser floor — it never actually lets a glance
    // fire, so a gaze cycle that silently stopped scheduling altogether
    // (e.g. a broken cleanup, or `active` read backwards) would still
    // pass it. `randomBetween(6000, 11000)` never reaches its upper bound
    // (`Math.random()` is exclusive of 1), so 11001ms after `visible`
    // flips true is enough to GUARANTEE the first gaze step has fired.
    // `GAZE_SEQUENCE`'s own first step (center -> left, IdleFace.tsx) is
    // deterministic regardless of which random delay actually fired, so
    // the transform is guaranteed to differ from its initial value here.
    it("a glance actually happens — the eyes' transform changes once the gaze cycle's own floor has definitely elapsed", () => {
      const { container } = render(<IdleFace idle={true} />);
      act(() => vi.advanceTimersByTime(REVEAL_DELAY_MS));

      const eyes = container.querySelector(".idle-face-eyes") as HTMLElement;
      expect(eyes).not.toBeNull();
      const initialTransform = eyes.style.transform;

      act(() => vi.advanceTimersByTime(11001));
      expect(eyes.style.transform).not.toBe(initialTransform);
    });
  });

  describe("eyes: CSS transition, not a motion spring", () => {
    // plan 129 (C6, deep-review fix): built from the NOTCHTAP_EASE import
    // itself, not a hand-typed "0.22, 1, 0.36, 1" echo — the component
    // now builds its own transition string the same way
    // (`NOTCHTAP_EASE.join(", ")`), so this test would silently stop
    // proving anything if a future edit to the constant weren't also
    // reflected here.
    it("carries an inline transition on transform using the house curve, not a spring", () => {
      const { container } = render(<IdleFace idle={true} />);
      act(() => vi.advanceTimersByTime(REVEAL_DELAY_MS));

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

  describe("reveal on the house curve (character finding #11)", () => {
    it("mounts at the house entrance scale (0.92), not the old 0.85", () => {
      const { container } = render(<IdleFace idle={true} />);
      act(() => vi.advanceTimersByTime(REVEAL_DELAY_MS));

      const face = container.querySelector(".idle-face") as HTMLElement;
      expect(face).not.toBeNull();
      // motion applies `initial` synchronously on mount, before any
      // animation ticks — with fake timers still active (no rAF flush),
      // this is the actual committed style, same technique confirmed
      // against this repo's own motion usage.
      expect(face.style.transform).toContain("scale(0.92)");
    });
  });
});
