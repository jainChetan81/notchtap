import { afterEach, describe, expect, it, vi } from "vitest";
import { prefersReducedMotion } from "./prefersReducedMotion";

// The former `usePrefersReducedMotion` subscribing hook (and its 4 tests)
// was removed 2026-07-23: zero consumers, and reduced-motion is a permanent
// non-goal per operator decree — see prefersReducedMotion.ts's header.

function mockMatchMedia(initialMatches: boolean) {
  const mql = {
    matches: initialMatches,
    media: "(prefers-reduced-motion: reduce)",
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    onchange: null,
    dispatchEvent: () => false,
  };
  vi.stubGlobal("matchMedia", () => mql);
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("prefersReducedMotion() (plan 117)", () => {
  it("returns matchMedia's current .matches value", () => {
    mockMatchMedia(true);
    expect(prefersReducedMotion()).toBe(true);
  });

  it("returns false when matches is false", () => {
    mockMatchMedia(false);
    expect(prefersReducedMotion()).toBe(false);
  });

  it("returns false (never throws) when window.matchMedia is unavailable", () => {
    vi.stubGlobal("matchMedia", undefined);
    expect(prefersReducedMotion()).toBe(false);
  });
});
