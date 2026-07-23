import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TtlBar } from "./TtlBar";

// this project's vitest config doesn't set `test.globals`, so RTL's
// auto-cleanup (which hooks a global `afterEach`) never registers —
// without this, DOM from one test's render leaks into the next.
afterEach(cleanup);

// 2026-07-23 review fix (Performance finding): TtlBar.tsx now animates
// `.ttl-fill` via `transform: scaleX(<fraction>)` instead of mutating
// `style.width` every frame (see that file's own doc for the "why" — a
// layout property under `.card-assembly`'s `filter: drop-shadow` was
// forcing a re-layout/re-rasterize every frame). This helper reads the
// scaleX fraction back out and reports it on the SAME 0-100 percentage
// scale every existing assertion below already expects, so the
// assertions' MEANING (a percentage of remaining time) is unchanged —
// only the DOM property being read moved from `style.width` to
// `style.transform`.
function fillScalePercent(container: HTMLElement): number {
  const fill = container.querySelector(".ttl-fill") as HTMLElement | null;
  expect(fill).not.toBeNull();
  const transform = (fill as HTMLElement).style.transform;
  const match = transform.match(/^scaleX\(([\d.]+)\)$/);
  expect(match).not.toBeNull();
  return Number(match?.[1]) * 100;
}

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

describe("TtlBar (plan 081)", () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame", "performance"] });
    mockReducedMotion(false);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it("renders the ttl-bar/ttl-fill DOM nodes", () => {
    const { container } = render(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} />);
    expect(container.querySelector(".ttl-bar")).not.toBeNull();
    expect(container.querySelector(".ttl-fill")).not.toBeNull();
  });

  it("anchors the fill to remainingMs/ttlMs and drains it over real time", () => {
    const { container } = render(<TtlBar slotId="n1" ttlMs={8000} remainingMs={4000} />);

    // first animation frame: freshly anchored, ~50%.
    act(() => {
      vi.advanceTimersByTime(16);
    });
    const firstPct = fillScalePercent(container);
    expect(firstPct).toBeGreaterThan(0);
    expect(firstPct).toBeLessThanOrEqual(50);

    // advance real (faked) time by 2s: remaining drops to ~2000ms of 8000ms (~25%).
    act(() => {
      vi.advanceTimersByTime(2000);
    });
    const laterPct = fillScalePercent(container);
    expect(laterPct).toBeLessThan(firstPct);
    expect(laterPct).toBeCloseTo(25, 0);
  });

  it("clamps the fill at 0 once remainingMs has fully elapsed", () => {
    const { container } = render(<TtlBar slotId="n1" ttlMs={1000} remainingMs={500} />);
    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(fillScalePercent(container)).toBe(0);
  });

  it("re-anchors the countdown when slotId changes (a new promotion)", () => {
    const { container, rerender } = render(<TtlBar slotId="n1" ttlMs={8000} remainingMs={1000} />);
    // let n1 nearly drain.
    act(() => {
      vi.advanceTimersByTime(900);
    });
    expect(fillScalePercent(container)).toBeLessThan(15);

    // a new slot (new id) with a fresh full window must restart at ~100%,
    // not continue counting down from n1's near-zero remainder.
    rerender(<TtlBar slotId="n2" ttlMs={8000} remainingMs={8000} />);
    act(() => {
      vi.advanceTimersByTime(16);
    });
    expect(fillScalePercent(container)).toBeGreaterThan(90);
  });

  it("re-anchors on a same-id re-emit with a fresh remainingMs (supersede/extension)", () => {
    const { container, rerender } = render(<TtlBar slotId="n1" ttlMs={2000} remainingMs={100} />);
    act(() => {
      vi.advanceTimersByTime(90);
    });
    expect(fillScalePercent(container)).toBeLessThan(20);

    // same slotId, but a supersede top-up granted a fresh, larger window —
    // the bar must jump back up, not keep counting toward zero.
    rerender(<TtlBar slotId="n1" ttlMs={2000} remainingMs={2000} />);
    act(() => {
      vi.advanceTimersByTime(16);
    });
    expect(fillScalePercent(container)).toBeGreaterThan(90);
  });

  it("renders a static, un-scaled fill and skips the rAF loop under prefers-reduced-motion", () => {
    mockReducedMotion(true);
    const rafSpy = vi.spyOn(window, "requestAnimationFrame");
    const { container } = render(<TtlBar slotId="n1" ttlMs={8000} remainingMs={4000} />);

    expect(fillScalePercent(container)).toBe(100);
    expect(rafSpy).not.toHaveBeenCalled();

    // advancing time must not start ticking it down either — the loop was
    // never armed (idle-CPU discipline, plans 015/018), not merely paused.
    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(fillScalePercent(container)).toBe(100);
  });

  it("cancels the rAF loop on unmount", () => {
    const cancelSpy = vi.spyOn(window, "cancelAnimationFrame");
    const { unmount } = render(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} />);
    act(() => {
      vi.advanceTimersByTime(16);
    });
    unmount();
    expect(cancelSpy).toHaveBeenCalled();
    // plan 093: this project's vitest config sets neither `restoreMocks`
    // nor `clearMocks` (vite.config.ts), so an unrestored spy on a global
    // like `cancelAnimationFrame` silently outlives this test — the next
    // test's fake-timers instance (a fresh one per `beforeEach` above)
    // then calls through a spy still wrapping the TORN-DOWN previous
    // instance's fake `cancelAnimationFrame`, so a later mount's cleanup
    // silently fails to cancel its rAF loop. Harmless on its own, but a
    // real, previously-latent bug: any later test in this file that
    // mounts/unmounts TtlBar more than once accumulates orphaned tick()
    // loops from every prior mount, which compounds into a genuine
    // `RangeError: Maximum call stack size exceeded` under
    // `vi.advanceTimersByTime` (found while adding the hoverPaused tests
    // below, which are exactly that shape — multiple renders/rerenders in
    // one test).
    cancelSpy.mockRestore();
  });

  // plan 093: 081's deferred hover-pause half.
  describe("hoverPaused (plan 093)", () => {
    it("freezes the fill while hoverPaused is true", () => {
      const { container, rerender } = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={true} />,
      );
      act(() => {
        vi.advanceTimersByTime(16);
      });
      const frozenAt = fillScalePercent(container);

      act(() => {
        vi.advanceTimersByTime(3000);
      });
      expect(fillScalePercent(container)).toBe(frozenAt);

      // still frozen across a re-render with the same props too — not
      // just a one-shot skip.
      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={true} />);
      act(() => {
        vi.advanceTimersByTime(3000);
      });
      expect(fillScalePercent(container)).toBe(frozenAt);
    });

    it("resumes counting down from where it froze once hoverPaused clears, granting no extra time", () => {
      const { container, rerender } = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={false} />,
      );
      // drain 2s of real active time.
      act(() => {
        vi.advanceTimersByTime(2000);
      });
      const beforePause = fillScalePercent(container);
      expect(beforePause).toBeCloseTo(75, 0);

      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={true} />);
      act(() => {
        vi.advanceTimersByTime(16);
      });
      const duringPause = fillScalePercent(container);
      expect(duringPause).toBeCloseTo(beforePause, 0);

      // 5s spent hovering — must not count against the countdown at all.
      act(() => {
        vi.advanceTimersByTime(5000);
      });
      expect(fillScalePercent(container)).toBeCloseTo(beforePause, 0);

      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={false} />);
      act(() => {
        vi.advanceTimersByTime(16);
      });
      // resumes from ~75%, not from wherever an un-paused countdown would
      // have reached after the same 7s of real wall-clock time (~12.5%).
      expect(fillScalePercent(container)).toBeCloseTo(beforePause, 0);

      // and it keeps counting down normally from there.
      act(() => {
        vi.advanceTimersByTime(2000);
      });
      expect(fillScalePercent(container)).toBeCloseTo(50, 0);
    });

    it("does not reset the countdown when only hoverPaused toggles (no re-anchor)", () => {
      const { container, rerender } = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={4000} hoverPaused={false} />,
      );
      act(() => {
        vi.advanceTimersByTime(1000);
      });
      const before = fillScalePercent(container);

      // toggling hoverPaused true then immediately false again, with zero
      // time elapsed in between, must not perturb the reading at all —
      // proves this isn't secretly keyed into the anchoring effect.
      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={4000} hoverPaused={true} />);
      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={4000} hoverPaused={false} />);
      act(() => {
        vi.advanceTimersByTime(16);
      });
      expect(fillScalePercent(container)).toBeCloseTo(before, 0);
    });

    // 2026-07-23 review fix (Performance finding): the actual bail-out
    // behavior FIX B adds — no new `requestAnimationFrame` calls while
    // hoverPaused, not merely a frozen visual value. Distinct from
    // "freezes the fill" above, which only pins the OUTPUT; this pins the
    // MECHANISM (spy on rAF itself) so a regression that keeps looping
    // but happens to keep painting the same number would still be
    // caught.
    it("stops requesting new animation frames while hoverPaused (no idle-CPU loop)", () => {
      const { rerender } = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={false} />,
      );
      act(() => {
        vi.advanceTimersByTime(100);
      });

      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={true} />);
      act(() => {
        // let the in-flight frame (scheduled before the pause) fire and
        // bail out.
        vi.advanceTimersByTime(16);
      });

      const rafSpy = vi.spyOn(window, "requestAnimationFrame");
      act(() => {
        vi.advanceTimersByTime(3000);
      });
      expect(rafSpy).not.toHaveBeenCalled();
      rafSpy.mockRestore();
    });

    it("byte-identical when hoverPaused is omitted (regression pin)", () => {
      const withDefault = render(<TtlBar slotId="n1" ttlMs={8000} remainingMs={4000} />);
      const withFalse = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={4000} hoverPaused={false} />,
      );
      expect(withDefault.container.innerHTML).toBe(withFalse.container.innerHTML);
    });
  });
});
