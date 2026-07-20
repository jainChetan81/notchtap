import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useDelayedSwap } from "./useDelayedSwap";

const EXIT_MS = 220;

function renderSwap(initialValue: string, initialKey: string) {
  return renderHook(({ value, key }) => useDelayedSwap(value, key, EXIT_MS), {
    initialProps: { value: initialValue, key: initialKey },
  });
}

describe("useDelayedSwap", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("syncs a same-key value update immediately, with no exit phase", () => {
    const { result, rerender } = renderSwap("v1", "k1");
    expect(result.current).toEqual({ value: "v1", exiting: false });

    rerender({ value: "v2", key: "k1" });
    expect(result.current).toEqual({ value: "v2", exiting: false });

    act(() => vi.advanceTimersByTime(EXIT_MS * 2));
    expect(result.current).toEqual({ value: "v2", exiting: false });
  });

  it("freezes the old value on a key change, then swaps after exitDurationMs", () => {
    const { result, rerender } = renderSwap("old", "k1");

    rerender({ value: "new", key: "k2" });
    // synchronously after the key change (useLayoutEffect): old value
    // still shown, exit phase flagged.
    expect(result.current).toEqual({ value: "old", exiting: true });

    act(() => vi.advanceTimersByTime(EXIT_MS - 1));
    expect(result.current).toEqual({ value: "old", exiting: true });

    act(() => vi.advanceTimersByTime(1));
    expect(result.current).toEqual({ value: "new", exiting: false });
  });

  it("a second key change before the first timer fires cancels it — only the latest key swaps", () => {
    const { result, rerender } = renderSwap("v1", "k1");

    rerender({ value: "v2", key: "k2" });
    expect(result.current).toEqual({ value: "v1", exiting: true });

    act(() => vi.advanceTimersByTime(EXIT_MS / 2));
    rerender({ value: "v3", key: "k3" });
    // still frozen on the original snapshot — no intermediate swap to v2.
    expect(result.current).toEqual({ value: "v1", exiting: true });

    // the k2 timer would have fired within this window if it hadn't been
    // cleaned up; only k3's timer may fire, landing straight on v3.
    act(() => vi.advanceTimersByTime(EXIT_MS / 2));
    expect(result.current).toEqual({ value: "v1", exiting: true });

    act(() => vi.advanceTimersByTime(EXIT_MS / 2));
    expect(result.current).toEqual({ value: "v3", exiting: false });
  });
});
