// plan 112: @types/node is now a devDependency (Step 1), so these two
// Node imports typecheck directly — no @ts-expect-error needed. Node's
// own `URL` is still imported explicitly (not the ambient global)
// because jsdom's global `URL` shadow resolves a relative path against a
// fake http: document location instead of `import.meta.url`'s real
// file: base.
import { readFileSync } from "node:fs";
import { fileURLToPath, URL as NodeURL } from "node:url";
import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { StatusRailCard } from "./components/StatusRailCard";
import type { SlotState } from "./useSlotState";

// plan 084's `Crest` (StatusRailCard.tsx) calls `convertFileSrc` itself at
// module scope — mocked here (same shim as StatusRailCard.test.tsx) purely
// so importing the component doesn't require a real tauri runtime; the
// goal fixture below never hits the live-match/Crest branch.
vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://converted${path}`,
}));

afterEach(cleanup);

// jsdom can't compute cascade from stylesheets (no layout/paint engine), so
// plan 107's stacking contract — `.card-content` above the celebration
// burst — is pinned at the STRING level here: read the rule text straight
// out of the shared CSS file and assert the decisive declarations are
// present. Cross-referenced by name (`celebrationStacking.test.tsx`) from
// the contract comment on `.card-assembly::after`.
//
// plan 111: this used to pin the SAME contract twice — once in styles.css,
// once in its hand-maintained mirror (src/settings/preview-overlay.css,
// now deleted). Both entry points now import ONE shared stylesheet
// (src/overlay-card.css, `.card-root`-scoped) — there is only one rule
// pair to pin, and mirrorInvariant.test.ts is what stops a scoped copy
// from ever coming back.
//
// Read via `node:fs`, not a `?raw`/`?inline` Vite import: under vitest's
// SSR-consumer transform, Vite's own css plugin intercepts anything
// matching `.css` (any query included) and hands back an empty module —
// confirmed empirically (`?raw` and `?inline` both resolved to a 0-length
// string here), so a real filesystem read is the only reliable path to
// the literal source text.
function readSourceCss(relativePath: string): string {
  return readFileSync(fileURLToPath(new NodeURL(relativePath, import.meta.url)), "utf-8");
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

describe("celebration stacking — CSS string pins (plan 107)", () => {
  it("overlay-card.css: .card-root .card-content is `position: relative; z-index: 1` — the decisive declaration against the burst's `z-index: 0`", () => {
    const body = ruleBody(overlayCardCss, ".card-root .card-content");
    expect(body).toContain("position: relative");
    expect(body).toContain("z-index: 1");
  });

  it("overlay-card.css: the celebration burst (.card-root .card-assembly::after) stays at z-index: 0", () => {
    const body = ruleBody(overlayCardCss, ".card-root .card-assembly::after");
    expect(body).toContain("z-index: 0");
  });

  // plan 100's 2x celebration pacing is explicitly out of scope for this
  // plan (Step A spends none of that timing budget) — pin the durations
  // byte-unchanged, matching the plan's own baseline counts. plan 111:
  // was 2x these counts pre-unification (once per mirrored file) — now
  // 1x, since there is exactly one copy of this CSS.
  it("plan-100 celebration durations are byte-unchanged", () => {
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

describe("celebration stacking — DOM (plan 107)", () => {
  it(".card-content mounts alongside the pulse-goal burst during a goal celebration", () => {
    const { container } = render(<StatusRailCard slot={GOAL} />);
    expect(container.querySelector(".card-assembly.pulse-goal")).not.toBeNull();
    expect(container.querySelector(".card-content")).not.toBeNull();
  });
});
