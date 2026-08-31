import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TtlBar } from "./TtlBar";

afterEach(cleanup);

function fillScalePercent(container: HTMLElement): number {
  // SAFETY: nullable cast because querySelector may miss; the very next
  // line's `not.toBeNull()` guards it before `fill` is dereferenced.
  const fill = container.querySelector(".ttl-fill") as HTMLElement | null;
  expect(fill).not.toBeNull();
  // SAFETY: `fill` is confirmed non-null one line above, so reading `.style`
  // through this narrowed cast cannot dereference null here.
  const transform = (fill as HTMLElement).style.transform;
  const match = transform.match(/^scaleX\(([\d.]+)\)$/);
  expect(match).not.toBeNull();
  return Number(match?.[1]) * 100;
}

describe("TtlBar", () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame", "performance"] });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("anchors the fill to remainingMs/ttlMs and drains it over real time", () => {
    const { container } = render(<TtlBar slotId="n1" ttlMs={8000} remainingMs={4000} />);

    act(() => {
      vi.advanceTimersByTime(16);
    });
    const firstPct = fillScalePercent(container);
    expect(firstPct).toBeGreaterThan(0);
    expect(firstPct).toBeLessThanOrEqual(50);

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
    act(() => {
      vi.advanceTimersByTime(900);
    });
    expect(fillScalePercent(container)).toBeLessThan(15);

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

    rerender(<TtlBar slotId="n1" ttlMs={2000} remainingMs={2000} />);
    act(() => {
      vi.advanceTimersByTime(16);
    });
    expect(fillScalePercent(container)).toBeGreaterThan(90);
  });

  it("cancels the rAF loop on unmount", () => {
    const cancelSpy = vi.spyOn(window, "cancelAnimationFrame");
    const { unmount } = render(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} />);
    act(() => {
      vi.advanceTimersByTime(16);
    });
    unmount();
    expect(cancelSpy).toHaveBeenCalled();
    cancelSpy.mockRestore();
  });

  describe("hoverPaused", () => {
    it("marks the fill .paused exactly while hoverPaused is true", () => {
      const { container, rerender } = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={false} />,
      );
      expect(container.querySelector(".ttl-fill")?.classList.contains("paused")).toBe(false);

      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={true} />);
      expect(container.querySelector(".ttl-fill")?.classList.contains("paused")).toBe(true);

      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={false} />);
      expect(container.querySelector(".ttl-fill")?.classList.contains("paused")).toBe(false);
    });

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

      act(() => {
        vi.advanceTimersByTime(5000);
      });
      expect(fillScalePercent(container)).toBeCloseTo(beforePause, 0);

      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={false} />);
      act(() => {
        vi.advanceTimersByTime(16);
      });
      expect(fillScalePercent(container)).toBeCloseTo(beforePause, 0);

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

      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={4000} hoverPaused={true} />);
      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={4000} hoverPaused={false} />);
      act(() => {
        vi.advanceTimersByTime(16);
      });
      expect(fillScalePercent(container)).toBeCloseTo(before, 0);
    });

    it("stops requesting new animation frames while hoverPaused (no idle-CPU loop)", () => {
      const { rerender } = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={false} />,
      );
      act(() => {
        vi.advanceTimersByTime(100);
      });

      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={true} />);
      act(() => {
        vi.advanceTimersByTime(16);
      });

      const rafSpy = vi.spyOn(window, "requestAnimationFrame");
      act(() => {
        vi.advanceTimersByTime(3000);
      });
      expect(rafSpy).not.toHaveBeenCalled();
      rafSpy.mockRestore();
    });

  });

  describe("queue segments (stories merge)", () => {
    function segs(container: HTMLElement) {
      return Array.from(container.querySelectorAll(".ttl-bar .ttl-seg"));
    }

    it("renders one segment per batch item when the batch is at most 10", () => {
      const { container } = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} total={5} done={2} />,
      );
      const all = segs(container);
      expect(all).toHaveLength(5);
      expect(all.slice(0, 2).every((s) => s.classList.contains("done"))).toBe(true);
      expect(all[2].classList.contains("done")).toBe(false);
      expect(all.slice(2).every((s) => s.className === "ttl-seg")).toBe(true);

      // SAFETY: TtlBar always renders `.ttl-fill` (fixture passes
      // ttlMs/remainingMs); the next line's null-guard protects the cast.
      const fill = container.querySelector(".ttl-fill") as HTMLElement;
      expect(fill).not.toBeNull();
      expect(fill.style.gridColumn).toBe("3");
    });

    it("defaults to a single, un-segmented bar when total/done are omitted", () => {
      const { container } = render(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} />);
      const all = segs(container);
      expect(all).toHaveLength(1);
      expect(all[0].classList.contains("done")).toBe(false);
      // SAFETY: TtlBar renders `.ttl-fill` even when total/done are omitted
      // (single un-segmented bar), so the match is non-null.
      const fill = container.querySelector(".ttl-fill") as HTMLElement;
      expect(fill.style.gridColumn).toBe("1");
    });


    it("caps the segment count at 10 for batches beyond the ceiling", () => {
      const { container } = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} total={15} done={0} />,
      );
      const all = segs(container);
      expect(all).toHaveLength(10);
      expect(container.querySelectorAll(".ttl-bar .ttl-seg.done")).toHaveLength(0);
    });

    it("maps the current index proportionally past the 10-segment ceiling", () => {
      const mid = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} total={20} done={10} />,
      );
      const midSegs = segs(mid.container);
      expect(midSegs).toHaveLength(10);
      expect(midSegs.slice(0, 5).every((s) => s.classList.contains("done"))).toBe(true);
      expect(midSegs[5].classList.contains("done")).toBe(false);
      // SAFETY: `.ttl-fill` always renders inside a mid-batch segmented bar
      // (total=20 fixture), so the inline match is non-null.
      expect((mid.container.querySelector(".ttl-fill") as HTMLElement).style.gridColumn).toBe("6");

      const last = render(
        <TtlBar slotId="n2" ttlMs={8000} remainingMs={8000} total={20} done={19} />,
      );
      const lastSegs = segs(last.container);
      expect(lastSegs.slice(0, 9).every((s) => s.classList.contains("done"))).toBe(true);
      expect(lastSegs[9].classList.contains("done")).toBe(false);
      // SAFETY: `.ttl-fill` always renders inside the last-batch segment bar
      // (total=20 fixture), so the inline match is non-null.
      expect((last.container.querySelector(".ttl-fill") as HTMLElement).style.gridColumn).toBe(
        "10",
      );
    });

    it("hands the segment count to the grid via the --queue-n custom property", () => {
      const { container } = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} total={4} done={0} />,
      );
      // SAFETY: TtlBar always renders its `.ttl-bar` container for this
      // total/done fixture, so the match to read --queue-n is non-null.
      const bar = container.querySelector(".ttl-bar") as HTMLElement;
      expect(bar.style.getPropertyValue("--queue-n")).toBe("4");
    });

    it("keeps the fill node itself stable when the current segment changes (no remount)", () => {
      const { container, rerender } = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} total={5} done={0} />,
      );
      const fillBefore = container.querySelector(".ttl-fill");
      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} total={5} done={2} />);
      const fillAfter = container.querySelector(".ttl-fill");
      expect(fillAfter).toBe(fillBefore);
      // SAFETY: `.ttl-fill` stays mounted across the rerender (pinned by the
      // `toBe(fillBefore)` check just above), so this read is non-null.
      expect((fillAfter as HTMLElement).style.gridColumn).toBe("3");
    });

    it("keeps the fill's own scaleX animation running when total/done change (same clock, unaffected)", () => {
      const { container, rerender } = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={4000} total={3} done={0} />,
      );
      act(() => {
        vi.advanceTimersByTime(16);
      });
      const before = fillScalePercent(container);
      expect(before).toBeGreaterThan(0);

      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={4000} total={5} done={1} />);
      act(() => {
        vi.advanceTimersByTime(16);
      });
      expect(fillScalePercent(container)).toBeLessThanOrEqual(before);
    });
  });
});
