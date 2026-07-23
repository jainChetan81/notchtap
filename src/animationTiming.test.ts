import { readFileSync } from "node:fs";
// `URL as NodeURL` (not the ambient global): jsdom's URL shadow resolves
// relative paths against a fake http: document location — the same trap
// entryImportOrder.test.ts documents and dodges identically.
import { fileURLToPath, URL as NodeURL } from "node:url";
import { describe, expect, it } from "vitest";
import { CONTENT_EXIT_MS, NOTCHTAP_EASE, SWAP_EXIT_MS } from "./animationTiming";

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

  // 2026-07-23 review fix: CONTENT_EXIT_MS ↔ the flank-round
  // `transition: border-radius <N>ms` in overlay-card.css — guard it the
  // same way the mirror/ease guards work: parse the real stylesheet,
  // compare.
  it("CONTENT_EXIT_MS matches overlay-card.css's flank-round transition duration", () => {
    const css = readFileSync(
      fileURLToPath(new NodeURL("./overlay-card.css", import.meta.url)),
      "utf8",
    );
    const durations = [...css.matchAll(/transition:\s*border-radius\s+(\d+)ms/g)].map((m) =>
      Number(m[1]),
    );
    expect(durations.length).toBeGreaterThanOrEqual(2); // both flanks
    for (const d of durations) {
      expect(d).toBe(CONTENT_EXIT_MS);
    }
  });

  // wave B (2026-07-23, "one overlapping collapse"): SWAP_EXIT_MS's new
  // CSS twin — `.card-assembly.exiting`'s own `transition: width <N>ms`
  // duration (overlay-card.css). Scoped specifically to that selector's
  // own declaration block (not a blanket "any transition: width Nms in
  // the file" match), because the base `.card-assembly` rule right above
  // it also declares a `transition: width 320ms` — a deliberately
  // DIFFERENT, unrelated duration (the entrance width-grow) that this
  // guard must never accidentally pin against.
  it("SWAP_EXIT_MS matches overlay-card.css's .card-assembly.exiting width transition duration", () => {
    const css = readFileSync(
      fileURLToPath(new NodeURL("./overlay-card.css", import.meta.url)),
      "utf8",
    );
    const block = css.match(/\.card-assembly\.exiting\s*\{[^}]*\}/);
    expect(block).not.toBeNull();
    const widthMatch = block?.[0].match(/transition:[^;]*width\s+(\d+)ms/);
    expect(widthMatch).not.toBeNull();
    expect(Number(widthMatch?.[1])).toBe(SWAP_EXIT_MS);
  });

  // 2026-07-23 review fix (Duplicated Code finding): NOTCHTAP_EASE is the
  // JS twin of shared-ui's `--ease-notchtap` cubic-bezier token. Parse the
  // vendored token and compare numerically so the pair can't drift.
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
