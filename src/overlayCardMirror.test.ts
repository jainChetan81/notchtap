import { readFileSync } from "node:fs";
import { fileURLToPath, URL as NodeURL } from "node:url";
import { describe, expect, it } from "vitest";

function readSourceCss(relativePath: string): string {
  const url = new NodeURL(relativePath, import.meta.url);
  const raw = readFileSync(fileURLToPath(url), "utf-8");
  return raw.replace(/^@import\s+["'](\.[^"']+)["'];\s*$/gm, (_match, importPath: string) =>
    readFileSync(fileURLToPath(new NodeURL(importPath, url)), "utf-8"),
  );
}

function stripComments(css: string): string {
  return css.replace(/\/\*[\s\S]*?\*\//g, "");
}

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

function isAtRuleOrKeyframeSelector(member: string): boolean {
  const t = member.trim();
  return t.startsWith("@") || /^\d+(\.\d+)?%$/.test(t) || t === "from" || t === "to" || t === "";
}

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

const overlayCardCss = readSourceCss("./overlay-card.css");
const stylesCss = readSourceCss("./styles.css");
const baseCss = readSourceCss("./settings/base.css");

describe("overlay-card.css mirror invariant", () => {
  it("src/settings/preview-overlay.css does not exist", () => {
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

  it("settings/base.css never redefines a shared-inventory selector outside the allowlist", () => {
    const inventory = buildSharedInventory(overlayCardCss);
    expect(findRedefinitions(baseCss, inventory)).toEqual([]);
  });

  it("the allowlist (override budget) stays empty", () => {
    expect(ALLOWLISTED_SELECTORS.size).toBe(0);
  });
});
