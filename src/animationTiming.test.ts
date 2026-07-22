import { describe, expect, it } from "vitest";
import { IDLE_PEEK_CLOSE_MS, SWAP_EXIT_MS } from "./animationTiming";

// plan 117: pins the two single-sourced duration constants to the exact
// values every existing consumer/test already assumed (StatusRailCard's
// useDelayedSwap window, IdleHoverPeek's close-unmount delay) — a
// regression here is exactly the kind of silent drift this plan exists to
// prevent.
describe("animationTiming (plan 117)", () => {
  it("SWAP_EXIT_MS matches useDelayedSwap's historical 220ms exit window", () => {
    expect(SWAP_EXIT_MS).toBe(220);
  });

  it("IDLE_PEEK_CLOSE_MS matches IdleHoverPeek's historical 260ms close delay", () => {
    expect(IDLE_PEEK_CLOSE_MS).toBe(260);
  });
});
