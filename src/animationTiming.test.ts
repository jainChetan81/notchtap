import { describe, expect, it } from "vitest";
import { SWAP_EXIT_MS } from "./animationTiming";

// plan 117: pins the single-sourced duration constant to the exact value
// every existing consumer/test already assumed (StatusRailCard's
// useDelayedSwap window) — a regression here is exactly the kind of
// silent drift this plan exists to prevent.
//
// plan 12x (wave 3, operator-feedback polish pass): 220 -> 175 (~20%
// quicker) — this pin moves WITH the constant, same as
// StatusRailCard.test.tsx's "compact->idle geometry" describe block
// (which steps fake timers against this same value); the assertion's
// MEANING (the constant is pinned to a known literal, not a symbol) is
// unchanged, only the literal itself.
describe("animationTiming (plan 117)", () => {
  it("SWAP_EXIT_MS matches useDelayedSwap's 175ms exit window", () => {
    expect(SWAP_EXIT_MS).toBe(175);
  });
});
