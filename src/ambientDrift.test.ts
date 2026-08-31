import { readFileSync } from "node:fs";
import { fileURLToPath, URL as NodeURL } from "node:url";
import { describe, expect, it } from "vitest";

function readCss(relativePath: string): string {
  return readFileSync(fileURLToPath(new NodeURL(relativePath, import.meta.url)), "utf-8");
}

function keyframesBody(css: string, name: string): string {
  const marker = `@keyframes ${name} {`;
  const start = css.indexOf(marker);
  if (start === -1) {
    throw new Error(`@keyframes not found: ${name}`);
  }
  let depth = 0;
  for (let i = start + marker.length - 1; i < css.length; i += 1) {
    if (css[i] === "{") {
      depth += 1;
    } else if (css[i] === "}") {
      depth -= 1;
      if (depth === 0) {
        return css.slice(start + marker.length, i);
      }
    }
  }
  throw new Error(`unterminated @keyframes: ${name}`);
}

const newsCategoryCss = readCss("./overlay/news-category.css");

describe("ambient drift shapes", () => {
  it("shade-drift wanders through intermediate stops, not along a single rail", () => {
    const body = keyframesBody(newsCategoryCss, "shade-drift");
    const stops = body.match(/translate3d\([^)]*\)/g) ?? [];
    expect(stops.length).toBeGreaterThanOrEqual(4);
    expect(body).toContain("33%");
    expect(body).toContain("66%");
  });

  it("keeps shade-drift's settled existence/ease decisions (shape-only change)", () => {
    expect(newsCategoryCss).toContain("animation: shade-drift 12s ease-in-out infinite alternate;");
  });

  it("keyframesBody throws on a name that doesn't exist — no vacuous pass", () => {
    expect(() => keyframesBody(newsCategoryCss, "no-such-keyframes")).toThrow();
  });
});
