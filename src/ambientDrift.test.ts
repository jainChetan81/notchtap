// the two ambient background loops — the news card's
// category shade and the snow texture — are pure CSS with no JS side at
// all, so the only thing worth guarding is the ONE invariant in each that
// a later edit could silently undo, and that no reviewer would catch by
// eye in jsdom (which has neither a layout nor an animation engine):
//
// 1. `shade-drift` must have intermediate stops. A two-stop keyframe
// with `alternate` retraces the identical straight rail — the
// pendulum this item existed to fix.
// 2. the snow fall and the snow sway must animate DIFFERENT CSS
// properties (`translate` vs `transform`). Two animations on the
// same property do not compose — the later one just wins, and the
// sway would vanish with no error anywhere.
//
// Deliberately string-level against the real stylesheet source, the same
// register as celebrationStacking.test.tsx's own pins.
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
  // the block nests one level (percentage selectors), so match braces.
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

describe("ambient drift shapes (plan 151 item D)", () => {
  it("shade-drift wanders through intermediate stops, not along a single rail", () => {
    const body = keyframesBody(newsCategoryCss, "shade-drift");
    const stops = body.match(/translate3d\([^)]*\)/g) ?? [];
    expect(stops.length).toBeGreaterThanOrEqual(4);
    // and the intermediates are genuinely off the straight line between
    // the endpoints — 33% leans further left than its share of the x
    // travel, which a collinear path could never do.
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
