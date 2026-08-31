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
    expect(result.current).toEqual({ value: "v1", exiting: true });

    act(() => vi.advanceTimersByTime(EXIT_MS / 2));
    expect(result.current).toEqual({ value: "v1", exiting: true });

    act(() => vi.advanceTimersByTime(EXIT_MS / 2));
    expect(result.current).toEqual({ value: "v3", exiting: false });
  });

  it("freezes the LAST-rendered same-key value on a key change, not the value from when the key first appeared", () => {
    const { result, rerender } = renderSwap("A", "k1");
    expect(result.current).toEqual({ value: "A", exiting: false });

    rerender({ value: "B", key: "k1" });
    expect(result.current).toEqual({ value: "B", exiting: false });

    rerender({ value: "C", key: "k2" });
    expect(result.current).toEqual({ value: "B", exiting: true });

    act(() => vi.advanceTimersByTime(EXIT_MS));
    expect(result.current).toEqual({ value: "C", exiting: false });
  });

  it("swaps to the freshest incoming value if the new key re-renders again before its exit timer fires", () => {
    const { result, rerender } = renderSwap("old", "k1");

    rerender({ value: "mid", key: "k2" });
    expect(result.current).toEqual({ value: "old", exiting: true });

    act(() => vi.advanceTimersByTime(EXIT_MS / 2));
    rerender({ value: "final", key: "k2" });
    expect(result.current).toEqual({ value: "old", exiting: true });

    act(() => vi.advanceTimersByTime(EXIT_MS / 2));
    expect(result.current).toEqual({ value: "final", exiting: false });
  });
});
