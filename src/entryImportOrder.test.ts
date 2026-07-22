// plan 111 Step 1.2: the shared card-shape stylesheet must load BEFORE
// each window's own residue, at both real entry points, so context-only
// declarations win any specificity tie by source order (same discipline
// the old single-file styles.css/preview-overlay.css pair relied on
// implicitly). CSS `@import` was deliberately not used (plan's own
// instruction) — the ordering lives in these two TypeScript entry files
// instead, so it's pinned here by reading their literal source text: a
// jsdom/vitest run doesn't otherwise observe CSS load order at all.
// plan 112: @types/node is now a devDependency (Step 1), so these two
// Node imports typecheck directly — no @ts-expect-error needed. Node's
// own `URL` is still imported explicitly (not the ambient global)
// because jsdom's global `URL` shadow resolves a relative path against a
// fake http: document location instead of `import.meta.url`'s real
// file: base.
import { readFileSync } from "node:fs";
import { fileURLToPath, URL as NodeURL } from "node:url";
import { describe, expect, it } from "vitest";

function readSource(relativePath: string): string {
  return readFileSync(fileURLToPath(new NodeURL(relativePath, import.meta.url)), "utf-8");
}

function importOrderIndex(source: string, specifier: string): number {
  const marker = `import "${specifier}"`;
  const idx = source.indexOf(marker);
  if (idx === -1) {
    throw new Error(`import not found: ${specifier}`);
  }
  return idx;
}

describe("entry-file CSS import order (plan 111)", () => {
  it("main.tsx imports overlay-card.css before styles.css", () => {
    const source = readSource("./main.tsx");
    const overlayIdx = importOrderIndex(source, "./overlay-card.css");
    const stylesIdx = importOrderIndex(source, "./styles.css");
    expect(overlayIdx).toBeLessThan(stylesIdx);
  });

  it("settings/main.tsx imports overlay-card.css before settings.css", () => {
    const source = readSource("./settings/main.tsx");
    const overlayIdx = importOrderIndex(source, "../overlay-card.css");
    const settingsIdx = importOrderIndex(source, "./settings.css");
    expect(overlayIdx).toBeLessThan(settingsIdx);
  });

  it("App.tsx no longer imports styles.css directly (main.tsx owns both CSS imports)", () => {
    const source = readSource("./App.tsx");
    expect(source.includes('"./styles.css"')).toBe(false);
  });

  it("SettingsApp.tsx no longer imports preview-overlay.css (deleted; settings/main.tsx owns the shared import)", () => {
    const source = readSource("./settings/SettingsApp.tsx");
    expect(source.includes("preview-overlay.css")).toBe(false);
  });
});
