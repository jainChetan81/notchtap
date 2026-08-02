// Plan 175: string-level parity pin between the icon strip's THREE
// geometry sites — `hover.rs`'s constants (what the NSEvent click monitor
// hit-tests each glyph against, via `icon_strip_rects`), `icon-strip.css`'s
// per-icon footprint (what actually gets painted), and `card-chrome.css`'s
// two strip-visible `--cw` formulas (how wide the flank those glyphs live
// in grows). Plan 171 shipped the first two paired against a design mock
// instead of each other, and they drifted: rust inset 14 vs. CSS 16, and a
// flat 85px CSS flank vs. rust's icon-count-driven one. At 3+ present tabs
// the rects slid right of the glyphs (~9px at 3, ~35px at 4, ~61px at 5,
// against a 26px pitch) so clicks hit the wrong tab or nothing, and the
// flank's `overflow: hidden` clipped the leftmost icons out of view.
// Nothing in either language can catch that at compile time, hence a test.
//
// Same cheap register as `src/settings/hookEventParity.test.ts` (read that
// file's header for the trade-off): the sources are read as TEXT, with no
// CSS or Rust parser. That deliberately makes this a pin on the literal
// numbers rather than on computed layout — it cannot prove the painted
// pixels line up, only that all three sites still agree on the same
// numbers. Proving the pixels is a hardware feel-check
// (`docs/TESTING_STRATEGY.md` §5), which is exactly why the cheap version
// is worth having.
import { readFileSync } from "node:fs";
import { fileURLToPath, URL as NodeURL } from "node:url";
import { describe, expect, it } from "vitest";

function readText(relativePath: string): string {
  const url = new NodeURL(relativePath, import.meta.url);
  return readFileSync(fileURLToPath(url), "utf-8");
}

const hoverRs = readText("../../src-tauri/src/hover.rs");
const cardChromeCss = readText("../overlay/card-chrome.css");
const iconStripCss = readText("../overlay/icon-strip.css");

/**
 * The value of a `const NAME: f64 = <n>;` in hover.rs. Throws on a miss
 * rather than returning `undefined`, so a renamed constant fails loudly
 * instead of silently comparing nothing (`hookEventParity.test.ts`'s own
 * no-vacuous-pass discipline).
 */
function rustConst(name: string): number {
  const match = hoverRs.match(new RegExp(`const ${name}: f64 = ([0-9.]+);`));
  if (match === null) throw new Error(`hover.rs constant not found: ${name}`);
  return Number(match[1]);
}

/**
 * A CSS rule's body, located by selector. Simplified from
 * `StatusRailCard.test.tsx`'s own `ruleBody`: that one skips `/* ... *\/`
 * spans because the rules it targets carry comments INSIDE the braces;
 * none of the three targeted here do, so a plain scan to the first `}` is
 * enough. Selector whitespace is collapsed to `\s+` the same way, so a
 * future re-wrap of a multi-line selector does not break the pin.
 *
 * The `(?<!,\s*)` guard is this file's own addition: two of the targets
 * (`.flank-right`, `.bare.hovered .flank-right`) also appear as the SECOND
 * half of a comma-separated left/right selector list, on its own line, so
 * an unguarded search returns that grouped rule's body — which carries the
 * shared paint/`min-width` declarations, not the per-side padding this
 * test is here to pin. Rejecting a match that a comma leads into skips the
 * list continuation and lands on the standalone rule.
 */
function ruleBody(css: string, selector: string): string {
  const pattern = selector
    .split(/\s+/)
    .map((part) => part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
    .join("\\s+");
  const match = css.match(new RegExp(`(?<!,\\s*)${pattern}\\s*\\{`));
  if (!match || match.index === undefined) {
    throw new Error(`selector not found in stylesheet: ${selector}`);
  }
  const braceStart = match.index + match[0].length - 1;
  const braceEnd = css.indexOf("}", braceStart);
  if (braceEnd === -1) throw new Error(`unterminated rule for selector: ${selector}`);
  return css.slice(braceStart + 1, braceEnd);
}

// The strip's growth term, verbatim. The two `--cw` rules below are the
// ONLY ones that can match while the strip is up (rail mode reaches the
// revealed rail through `.idle`, bare notch mode through
// `.bare:has(.below-block)`) — every other `--cw` formula in the file
// governs a state where no icon is on screen to click.
//
// Plan 176 widened that second selector from `:has(.idle-peek)` to
// `:has(.below-block)` so it also covers the pulled-tab card, not just
// the ambient peek. That is a strict superset (the peek's own root
// carries both classes), so it is still exactly ONE rule and still the
// only bare-mode route to a revealed rail — the count assertion below is
// unaffected.
const STRIP_TERM = "(26 * var(--present-icons, 0) + 16)";
const STRIP_VISIBLE_RULES = [
  ".card-root .card-assembly.idle",
  ".card-root .card-assembly.bare:has(.below-block)",
];

describe("icon strip geometry: rust hit-test constants match the shipped CSS", () => {
  it("hover.rs pins the 18px box, 8px gap and 16px inset", () => {
    expect(hoverRs).toContain("const ICON_BOX: f64 = 18.0;");
    expect(hoverRs).toContain("const ICON_GAP: f64 = 8.0;");
    // 16, not plan 171's 14: the shipped flank padding won that
    // reconciliation because adopting it is a zero-pixel visual change.
    expect(hoverRs).toContain("const FLANK_INSET: f64 = 16.0;");
  });

  it("icon-strip.css paints each present icon at exactly that box and gap", () => {
    const body = ruleBody(iconStripCss, ".card-root .icon.is-present");
    expect(body).toContain(`width: ${rustConst("ICON_BOX")}px;`);
    expect(body).toContain(`margin-left: ${rustConst("ICON_GAP")}px;`);
  });

  it("the flank's own right padding is the inset rust packs icons from", () => {
    // `icon_strip_rects` lays the rightmost glyph flush against
    // `card_x_max - FLANK_INSET`; this is the declaration that decides
    // where that edge actually is. `.bare.hovered .flank-right` restores
    // the same number when a bare notch reveals its rail.
    const inset = `padding-right: ${rustConst("FLANK_INSET")}px;`;
    expect(ruleBody(cardChromeCss, ".card-root .flank-right")).toContain(inset);
    expect(
      ruleBody(cardChromeCss, ".card-root .card-assembly.bare.hovered .flank-right"),
    ).toContain(inset);
  });

  it("card-chrome.css grows the flank on icon count in exactly the two strip-visible rules", () => {
    const occurrences = cardChromeCss.split(STRIP_TERM).length - 1;
    expect(occurrences).toBe(STRIP_VISIBLE_RULES.length);
    for (const selector of STRIP_VISIBLE_RULES) {
      const body = ruleBody(cardChromeCss, selector);
      expect(body).toContain(STRIP_TERM);
      // the symmetric doubling rust's `total_width = cutout + 2 * flank_w`
      // assumes, and the `1fr auto 1fr` grid enforces.
      expect(body).toContain("+ 2 *");
    }
  });

  it("the CSS term's own numbers are the rust pitch and inset", () => {
    // The one assertion that would still fail if someone edited ONLY the
    // rust constants: the CSS carries the pitch pre-summed (26), so it
    // cannot be derived by `var()` reference — this recomputes it.
    const pitch = rustConst("ICON_BOX") + rustConst("ICON_GAP");
    expect(STRIP_TERM).toBe(`(${pitch} * var(--present-icons, 0) + ${rustConst("FLANK_INSET")})`);
    // and the floor the strip has to beat before it grows anything.
    expect(hoverRs).toContain("const FLANK_IDLE: f64 = 85.0;");
    for (const selector of STRIP_VISIBLE_RULES) {
      expect(ruleBody(cardChromeCss, selector)).toContain("max(85px * var(--card-scale)");
    }
  });
});
