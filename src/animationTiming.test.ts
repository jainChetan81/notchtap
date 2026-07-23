import { readFileSync } from "node:fs";
// `URL as NodeURL` (not the ambient global): jsdom's URL shadow resolves
// relative paths against a fake http: document location — the same trap
// entryImportOrder.test.ts documents and dodges identically.
import { fileURLToPath, URL as NodeURL } from "node:url";
import { describe, expect, it, vi } from "vitest";
import { CONTENT_EXIT_MS, EXPAND_MS, NOTCHTAP_EASE, SWAP_EXIT_MS } from "./animationTiming";
import { applyAnimationTiming } from "./applyAnimationTiming";

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

  // 2026-07-23 review fix (wave C, CSS custom-property injection): the
  // two regex-parsing guards that used to live here (CONTENT_EXIT_MS ↔
  // overlay-card.css's flank-round `border-radius` duration, SWAP_EXIT_MS
  // ↔ `.card-assembly.exiting`'s own `width` duration) are gone. They
  // existed only because the CSS carried its OWN copy of each number,
  // which could drift from the JS constant without either side erroring
  // — so a test had to parse the stylesheet and compare by hand. That
  // duplication is gone: overlay-card.css now reads these values via
  // `var(--content-exit-ms, ...)`/`var(--swap-exit-ms, ...)`, set on the
  // document root by `applyAnimationTiming` (below) directly from these
  // same constants. There is exactly one place either number is written
  // as a literal now, so there is nothing left for a parsing guard to
  // catch — the coverage that matters is "does applyAnimationTiming
  // actually set the properties it claims to", which the test below
  // pins instead.
  it("applyAnimationTiming sets the expected custom properties on the given root", () => {
    const setProperty = vi.fn();
    applyAnimationTiming({ setProperty });

    expect(setProperty).toHaveBeenCalledWith("--swap-exit-ms", `${SWAP_EXIT_MS}ms`);
    expect(setProperty).toHaveBeenCalledWith("--content-exit-ms", `${CONTENT_EXIT_MS}ms`);
    expect(setProperty).toHaveBeenCalledWith("--expand-ms", `${EXPAND_MS}ms`);
    expect(setProperty).toHaveBeenCalledTimes(3);
  });

  // 2026-07-23 review fix (Duplicated Code finding): NOTCHTAP_EASE is the
  // JS twin of shared-ui's `--ease-notchtap` cubic-bezier token. Parse the
  // vendored token and compare numerically so the pair can't drift. Kept
  // (not folded into the custom-property injection above): motion needs
  // the real JS array for its own consumers (this is not a CSS-only
  // duration), so this pair is a real cross-file lockstep, unlike the two
  // guards removed above.
  it("NOTCHTAP_EASE numerically matches the vendored --ease-notchtap token", () => {
    const tokens = readFileSync(
      fileURLToPath(new NodeURL("../vendor/shared-ui/design/tokens.css", import.meta.url)),
      "utf8",
    );
    const m = tokens.match(
      /--ease-notchtap:\s*cubic-bezier\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*\)/,
    );
    expect(m).not.toBeNull();
    const tokenValues = m ? m.slice(1, 5).map(Number) : [];
    expect(tokenValues).toEqual([...NOTCHTAP_EASE]);
  });
});
