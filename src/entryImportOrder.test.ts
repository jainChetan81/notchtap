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

describe("entry-file CSS import order", () => {
  it("main.tsx imports overlay-card.css before styles.css", () => {
    const source = readSource("./main.tsx");
    const overlayIdx = importOrderIndex(source, "./overlay-card.css");
    const stylesIdx = importOrderIndex(source, "./styles.css");
    expect(overlayIdx).toBeLessThan(stylesIdx);
  });

  it("main.tsx imports shared-ui tokens.css before overlay-card.css", () => {
    const source = readSource("./main.tsx");
    const tokensIdx = importOrderIndex(source, "@chetanjain/shared-ui/design/tokens.css");
    const appTokensIdx = importOrderIndex(source, "./notchtap-tokens.css");
    const overlayIdx = importOrderIndex(source, "./overlay-card.css");
    expect(tokensIdx).toBeLessThan(appTokensIdx);
    expect(appTokensIdx).toBeLessThan(overlayIdx);
  });

  it("settings/base.css imports shared tokens before notchtap extensions", () => {
    const source = readSource("./settings/base.css");
    const sharedIdx = source.indexOf('@import "@chetanjain/shared-ui/design/tokens.css"');
    const appIdx = source.indexOf('@import "../notchtap-tokens.css"');
    expect(sharedIdx).toBeGreaterThanOrEqual(0);
    expect(appIdx).toBeGreaterThan(sharedIdx);
  });

  it("keeps the removed media-mint token out of both token layers", () => {
    const shared = readSource("../vendor/shared-ui/design/tokens.css");
    const local = readSource("./notchtap-tokens.css");
    expect(shared.includes("--media-mint")).toBe(false);
    expect(local.includes("--media-mint:")).toBe(false);
  });

  it("settings/main.tsx imports base.css before overlay-card.css", () => {
    const source = readSource("./settings/main.tsx");
    const baseIdx = importOrderIndex(source, "./base.css");
    const overlayIdx = importOrderIndex(source, "../overlay-card.css");
    expect(baseIdx).toBeLessThan(overlayIdx);
  });

  it("settings/main.tsx does not import settings.css — base.css owns its rules", () => {
    const source = readSource("./settings/main.tsx");
    expect(source.includes("settings.css")).toBe(false);
  });

  it("App.tsx does not import styles.css directly — main.tsx owns both CSS imports", () => {
    const source = readSource("./App.tsx");
    expect(source.includes('"./styles.css"')).toBe(false);
  });

  it("SettingsApp.tsx does not import preview-overlay.css — settings/main.tsx owns the shared import", () => {
    const source = readSource("./settings/SettingsApp.tsx");
    expect(source.includes("preview-overlay.css")).toBe(false);
  });
});
