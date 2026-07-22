import { describe, expect, it } from "vitest";
import { SWAP_EXIT_MS } from "./animationTiming";

// plan 117: pins the single-sourced duration constant to the exact value
// every existing consumer/test already assumed (StatusRailCard's
// useDelayedSwap window) — a regression here is exactly the kind of
// silent drift this plan exists to prevent.
describe("animationTiming (plan 117)", () => {
  it("SWAP_EXIT_MS matches useDelayedSwap's historical 220ms exit window", () => {
    expect(SWAP_EXIT_MS).toBe(220);
  });
});
