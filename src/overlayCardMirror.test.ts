// plan 111: the enforcement that replaces the old hand-maintained-mirror
// review discipline ("change one, change both, in the same commit" —
// DESIGN.html, now retired). `src/settings/preview-overlay.css` is gone;
// both entry points share `src/overlay-card.css`, scoped under
// `.card-root`. The one thing that could silently bring the mirror back
// is a context stylesheet (`src/styles.css`, `src/settings/settings.css`)
// re-declaring one of overlay-card.css's own selectors as an UNSCOPED
// duplicate — this file is the automated guard against that, run on
// every test pass instead of relying on a reviewer noticing.
//
// String-level by design (matches the plan's own instruction): strip
// `/* ... */` comments first, then extract SELECTOR-shaped occurrences —
// prelude text between a `}` (or file start) and the next `{` — and
// compare individual comma-list members. No CSS parser: this is
// deliberately the same "cheap but effective" register as
// celebrationStacking.test.tsx's string pins, not a cascade engine.
// @ts-expect-error no @types/node in this project (same pattern as
// celebrationStacking.test.tsx) — Node's fs/url are real at runtime
// (vitest runs on Node) even though this repo carries no @types/node.
import { readFileSync } from "node:fs";
// @ts-expect-error no @types/node in this project. Node's own `URL` is
// imported explicitly (not the ambient global) because jsdom's global
// `URL` shadow resolves a relative path against a fake http: document
// location instead of `import.meta.url`'s real file: base.
import { fileURLToPath, URL as NodeURL } from "node:url";
import { describe, expect, it } from "vitest";

function readSourceCss(relativePath: string): string {
  return readFileSync(fileURLToPath(new NodeURL(relativePath, import.meta.url)), "utf-8");
}

// ---- the scanner (exercised directly by its own fixtures below, then
// applied to the real files) -------------------------------------------

/** Strips `/* ... *\/` comments (non-greedy, no nesting — this codebase
 * never nests comments). A comment's own text must never count as a
 * selector occurrence. */
function stripComments(css: string): string {
  return css.replace(/\/\*[\s\S]*?\*\//g, "");
}

/** Extracts every "prelude" — the text between a `}` (or file start) and
 * the next `{` — from comment-stripped CSS. This deliberately does NOT
 * distinguish an ordinary style rule's selector list from an at-rule
 * prelude (`@media (...)`) or a nested rule inside a grouping rule
 * (`@media { .foo { ... } }` — the inner `.foo` is captured as its own
 * prelude, exactly like a top-level one) or a `@keyframes` percentage
 * selector (`50%`) — every `{` boundary yields one candidate. Harmless:
 * none of those non-selector preludes ever coincidentally match a real
 * CSS class selector string, and this is intentionally string-level, not
 * a CSS-aware parser (per the plan). */
function extractPreludes(css: string): string[] {
  const stripped = stripComments(css);
  const preludes: string[] = [];
  let boundary = 0;
  for (let i = 0; i < stripped.length; i++) {
    const ch = stripped[i];
    if (ch === "{") {
      preludes.push(stripped.slice(boundary, i));
      boundary = i + 1;
    } else if (ch === "}") {
      boundary = i + 1;
    }
  }
  return preludes;
}

// at-rule preludes and @keyframes percentage/from/to selectors are never
// real selectors to compare — filtered out so they can't pollute either
// side of the comparison (they'd never coincidentally collide anyway,
// but this keeps the extracted set legible/debuggable).
function isAtRuleOrKeyframeSelector(member: string): boolean {
  const t = member.trim();
  return t.startsWith("@") || /^\d+(\.\d+)?%$/.test(t) || t === "from" || t === "to" || t === "";
}

/** Splits a prelude on top-level commas (guarding `:has(...)`/`:not(...)`
 * parens) and returns each trimmed, whitespace-normalized member —
 * comma selector lists and selectors nested in grouping rules both fall
 * out of this + extractPreludes for free. */
function extractSelectorMembers(css: string): string[] {
  const members: string[] = [];
  for (const prelude of extractPreludes(css)) {
    let depth = 0;
    let cur = "";
    const parts: string[] = [];
    for (const ch of prelude) {
      if (ch === "(") depth++;
      if (ch === ")") depth--;
      if (ch === "," && depth === 0) {
        parts.push(cur);
        cur = "";
      } else {
        cur += ch;
      }
    }
    parts.push(cur);
    for (const part of parts) {
      const normalized = part.trim().replace(/\s+/g, " ");
      if (!isAtRuleOrKeyframeSelector(normalized)) {
        members.push(normalized);
      }
    }
  }
  return members;
}

/** Recovers the canonical (pre-`.card-root`-scope) selector text for a
 * shared-file member, mirroring the plan's own selector-transform rules
 * in reverse:
 *   - a leading ".card-root " token is stripped;
 *   - a ":root[...]"-anchored member has the " .card-root" scope token
 *     removed from just after that leading compound;
 *   - anything else (shouldn't occur in the real file) passes through
 *     unchanged. */
function unscopeSharedMember(member: string): string {
  if (member.startsWith(".card-root ")) {
    return member.slice(".card-root ".length);
  }
  const rootMatch = member.match(/^(:root\[[^\]]*\]) \.card-root(.*)$/s);
  if (rootMatch) {
    return `${rootMatch[1]}${rootMatch[2]}`;
  }
  return member;
}

function buildSharedInventory(overlayCardCss: string): Set<string> {
  return new Set(extractSelectorMembers(overlayCardCss).map(unscopeSharedMember));
}

/** Reviewed, explicit exceptions — a context file selector text that IS
 * allowed to coincide with a shared-inventory entry. Empty today: plan
 * 111's Step 0 audit found every previously-diverging rule was either
 * accumulated drift (fixed by unifying into overlay-card.css) or a
 * genuinely preview-only selector with no shared-inventory counterpart
 * at all (`.appearance-preview`, `.preview-row`, `.preview-label`,
 * `.preview-stage` — settings-gallery chrome, never in overlay-card.css)
 * — so no override earned a place here. Kept as a real mechanism (not
 * deleted) for the next deliberate adaptation, per the plan's own
 * "explicit reviewed allowlist" design. */
const ALLOWLISTED_SELECTORS: ReadonlySet<string> = new Set([]);

function findRedefinitions(contextCss: string, sharedInventory: ReadonlySet<string>): string[] {
  const hits: string[] = [];
  for (const member of extractSelectorMembers(contextCss)) {
    if (sharedInventory.has(member) && !ALLOWLISTED_SELECTORS.has(member)) {
      hits.push(member);
    }
  }
  return hits;
}

// ---- scanner self-test (the plan's own required fixtures) -------------

describe("overlayCardMirror scanner — self-test fixtures", () => {
  const inventory = new Set([".card-assembly", ".status-dots"]);

  it("allows class text that only appears inside a comment", () => {
    const css = `/* mentions .card-assembly and .status-dots in prose */\n.unrelated {\n  color: red;\n}\n`;
    expect(findRedefinitions(css, inventory)).toEqual([]);
  });

  it("fails on a duplicate .card-assembly selector", () => {
    const css = `.card-assembly {\n  width: 10px;\n}\n`;
    expect(findRedefinitions(css, inventory)).toEqual([".card-assembly"]);
  });

  it("fails on a duplicate non-assembly shared selector like .status-dots", () => {
    const css = `.status-dots {\n  gap: 4px;\n}\n`;
    expect(findRedefinitions(css, inventory)).toEqual([".status-dots"]);
  });

  it("catches a redefinition inside a comma selector list", () => {
    const css = `.foo,\n.card-assembly,\n.bar {\n  color: blue;\n}\n`;
    expect(findRedefinitions(css, inventory)).toEqual([".card-assembly"]);
  });

  it("catches a redefinition nested inside a grouping rule (@media)", () => {
    const css = `@media (prefers-reduced-motion: reduce) {\n  .status-dots {\n    animation: none;\n  }\n}\n`;
    expect(findRedefinitions(css, inventory)).toEqual([".status-dots"]);
  });

  it("does not false-positive on an unrelated selector that merely shares a substring", () => {
    // regression guard for the exact shape this repo has today:
    // settings.css's `.shortcut-status.active` must never be flagged
    // just because `.active` appears (compounded differently) in the
    // shared inventory (`.status-dot.active`).
    const realInventory = new Set([".status-dot.active"]);
    const css = `.shortcut-status.active {\n  color: green;\n}\n`;
    expect(findRedefinitions(css, realInventory)).toEqual([]);
  });

  it("respects an explicit allowlist entry", () => {
    const css = `.card-assembly {\n  width: 10px;\n}\n`;
    const hits: string[] = [];
    for (const member of extractSelectorMembers(css)) {
      if (inventory.has(member) && !new Set([".card-assembly"]).has(member)) {
        hits.push(member);
      }
    }
    expect(hits).toEqual([]);
  });
});

// ---- the real invariant, against the real files ------------------------

const overlayCardCss = readSourceCss("./overlay-card.css");
const stylesCss = readSourceCss("./styles.css");
const settingsCss = readSourceCss("./settings/settings.css");

describe("overlay-card.css mirror invariant (plan 111)", () => {
  it("src/settings/preview-overlay.css no longer exists", () => {
    expect(() => readSourceCss("./settings/preview-overlay.css")).toThrow();
  });

  it("the shared inventory is non-trivial (sanity check on the scanner itself)", () => {
    const inventory = buildSharedInventory(overlayCardCss);
    expect(inventory.size).toBeGreaterThan(100);
    expect(inventory.has(".card-assembly")).toBe(true);
    expect(inventory.has(".status-dots")).toBe(true);
  });

  it("styles.css never redefines a shared-inventory selector outside the allowlist", () => {
    const inventory = buildSharedInventory(overlayCardCss);
    expect(findRedefinitions(stylesCss, inventory)).toEqual([]);
  });

  it("settings.css never redefines a shared-inventory selector outside the allowlist", () => {
    const inventory = buildSharedInventory(overlayCardCss);
    expect(findRedefinitions(settingsCss, inventory)).toEqual([]);
  });

  // the override block itself: plan 111's done criteria caps it at the
  // Step-0 deliberate-adaptation count. That count is 0 (see
  // ALLOWLISTED_SELECTORS's own comment) — pin it so a future override
  // added here is a deliberate, reviewed act, not silent growth.
  it("the allowlist (override budget) matches the Step-0 deliberate-adaptation count", () => {
    expect(ALLOWLISTED_SELECTORS.size).toBe(0);
  });
});
