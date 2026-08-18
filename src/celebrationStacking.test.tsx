// @types/node is a devDependency, so these two
// Node imports typecheck directly — no @ts-expect-error needed. Node's
// own `URL` is still imported explicitly (not the ambient global)
// because jsdom's global `URL` shadow resolves a relative path against a
// fake http: document location instead of `import.meta.url`'s real
// file: base.
import { readFileSync } from "node:fs";
import { fileURLToPath, URL as NodeURL } from "node:url";
import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { StatusRailCard } from "./components/StatusRailCard";
import type { SlotState } from "./useSlotState";

afterEach(cleanup);

// jsdom can't compute cascade from stylesheets (no layout/paint engine), so
// 's stacking contract — `.card-content` above the celebration
// burst — is pinned at the STRING level here: read the rule text straight
// out of the shared CSS file and assert the decisive declarations are
// present. Cross-referenced by name (`celebrationStacking.test.tsx`) from
// the contract comment on `.card-assembly::after`.
//
// Both entry points import ONE shared stylesheet (src/overlay-card.css,
// `.card-root`-scoped), so there is a single rule pair to pin;
// mirrorInvariant.test.ts is what stops a scoped copy appearing.
//
// Read via `node:fs`, not a `?raw`/`?inline` Vite import: under vitest's
// SSR-consumer transform, Vite's own css plugin intercepts anything
// matching `.css` (any query included) and hands back an empty module —
// confirmed empirically (`?raw` and `?inline` both resolved to a 0-length
// string here), so a real filesystem read is the only reliable path to
// the literal source text.
// overlay-card.css is split into src/overlay/*.css chunks pulled back
// together via plain `@import "./relative.css";` lines — inlined here so
// this returns the full literal stylesheet text callers expect (imports
// are one level deep; no chunk file itself contains an @import).
function readSourceCss(relativePath: string): string {
  const url = new NodeURL(relativePath, import.meta.url);
  const raw = readFileSync(fileURLToPath(url), "utf-8");
  return raw.replace(/^@import\s+["'](\.[^"']+)["'];\s*$/gm, (_match, importPath: string) =>
    readFileSync(fileURLToPath(new NodeURL(importPath, url)), "utf-8"),
  );
}

const overlayCardCss = readSourceCss("./overlay-card.css");

function ruleBody(css: string, selector: string): string {
  const marker = `${selector} {`;
  const start = css.indexOf(marker);
  if (start === -1) {
    throw new Error(`selector not found in stylesheet: ${selector}`);
  }
  const braceStart = start + marker.length - 1;
  const braceEnd = css.indexOf("}", braceStart);
  if (braceEnd === -1) {
    throw new Error(`unterminated rule for selector: ${selector}`);
  }
  return css.slice(braceStart + 1, braceEnd);
}

describe("celebration stacking — CSS string pins", () => {
  it("overlay-card.css: .card-root .card-content is `position: relative; z-index: 1` — the decisive declaration against the burst's `z-index: 0`", () => {
    const body = ruleBody(overlayCardCss, ".card-root .card-content");
    expect(body).toContain("position: relative");
    expect(body).toContain("z-index: 1");
  });

  it("overlay-card.css: the celebration burst (.card-root .card-assembly::after) stays at z-index: 0", () => {
    const body = ruleBody(overlayCardCss, ".card-root .card-assembly::after");
    expect(body).toContain("z-index: 0");
  });

  // Celebration pacing is pinned byte-exact. The counts are 1x, not 2x,
  // because exactly one copy of this CSS exists.
  it("celebration durations are byte-unchanged", () => {
    expect((overlayCardCss.match(/1240ms/g) ?? []).length).toBe(3);
    expect((overlayCardCss.match(/1440ms/g) ?? []).length).toBe(1);
    expect((overlayCardCss.match(/920ms/g) ?? []).length).toBe(1);
  });
});

const GOAL: SlotState = {
  state: "showing",
  id: "n1",
  title: "GOAL",
  body: "Arsenal 2-0",
  eventType: "score_update",
  priority: "high",
  signal: "goal",
  origin: "football",
  agentRuntime: null,
  expanded: true,
  source: null,
  category: null,
  publishedAtMs: null,
  link: null,
  subtitle: null,
  details: [],
  queueTotal: 1,
  queueDone: 0,
  ttlMs: 8000,
  remainingMs: 8000,
};

describe("celebration stacking — DOM", () => {
  it(".card-content mounts alongside the pulse-goal burst during a goal celebration", () => {
    const { container } = render(<StatusRailCard slot={GOAL} />);
    expect(container.querySelector(".card-assembly.pulse-goal")).not.toBeNull();
    expect(container.querySelector(".card-content")).not.toBeNull();
  });
});
