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

function rustConst(name: string): number {
  const match = hoverRs.match(new RegExp(`const ${name}: f64 = ([0-9.]+);`));
  if (match === null) throw new Error(`hover.rs constant not found: ${name}`);
  return Number(match[1]);
}

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

const STRIP_TERM = "(26 * var(--present-icons, 0) + 16)";
const STRIP_VISIBLE_RULES = [
  ".card-root .card-assembly.idle",
  ".card-root .card-assembly.bare:has(.below-block)",
];

describe("icon strip geometry: rust hit-test constants match the shipped CSS", () => {
  it("hover.rs pins the 18px box, 8px gap and 16px inset", () => {
    expect(hoverRs).toContain("const ICON_BOX: f64 = 18.0;");
    expect(hoverRs).toContain("const ICON_GAP: f64 = 8.0;");
    expect(hoverRs).toContain("const FLANK_INSET: f64 = 16.0;");
  });

  it("icon-strip.css paints each present icon at exactly that box and gap", () => {
    const body = ruleBody(iconStripCss, ".card-root .icon.is-present");
    expect(body).toContain(`width: ${rustConst("ICON_BOX")}px;`);
    expect(body).toContain(`margin-left: ${rustConst("ICON_GAP")}px;`);
  });

  it("the flank's own right padding is the inset rust packs icons from", () => {
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
      expect(body).toContain("+ 2 *");
    }
  });

  it("the CSS term's own numbers are the rust pitch and inset", () => {
    const pitch = rustConst("ICON_BOX") + rustConst("ICON_GAP");
    expect(STRIP_TERM).toBe(`(${pitch} * var(--present-icons, 0) + ${rustConst("FLANK_INSET")})`);
    expect(hoverRs).toContain("const FLANK_IDLE: f64 = 85.0;");
    for (const selector of STRIP_VISIBLE_RULES) {
      expect(ruleBody(cardChromeCss, selector)).toContain("max(85px * var(--card-scale)");
    }
  });
});
