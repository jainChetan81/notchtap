// Plan 147: string-level parity pin between sourceColors.ts (the
// Settings-window swatch table) and the CSS that actually paints the
// overlay — same "cheap but effective" register as
// src/overlayCardMirror.test.ts's selector scanner, not a CSS parser.
// Every hex in the TS tables must appear (case-insensitive) somewhere
// in the relevant stylesheet(s), so the two can't silently drift.
import { readFileSync } from "node:fs";
import { fileURLToPath, URL as NodeURL } from "node:url";
import { describe, expect, it } from "vitest";
import {
  SOURCE_CATEGORY_COLORS,
  SOURCE_ORIGIN_COLORS,
  SOURCE_RUNTIME_COLORS,
  type SourceCategoryToken,
  type SourceOriginToken,
  type SourceRuntimeToken,
} from "./sourceColors";

function readCss(relativePath: string): string {
  const url = new NodeURL(relativePath, import.meta.url);
  return readFileSync(fileURLToPath(url), "utf-8");
}

const sourceIdentityCss = readCss("../overlay/source-identity.css");
const newsCategoryCss = readCss("../overlay/news-category.css");
const tokensCss = readCss("../../vendor/shared-ui/design/tokens.css");

// var()-backed entries: their hex lives in tokens.css, not the overlay
// stylesheet itself.
const VAR_BACKED_CSS = tokensCss;

function expectHexAppearsIn(hex: string, css: string) {
  expect(css.toLowerCase()).toContain(hex.toLowerCase());
}

describe("sourceColors parity with source-identity.css / news-category.css", () => {
  const ORIGIN_TOKENS: SourceOriginToken[] = ["manual", "football", "weather", "agent", "news"];
  const RUNTIME_TOKENS: SourceRuntimeToken[] = ["claude-code", "codex", "kimi", "opencode"];
  const CATEGORY_TOKENS: SourceCategoryToken[] = [
    "politics",
    "tech",
    "sports",
    "business",
    "world",
    "science",
    "generic",
  ];

  it("is total over every known origin token", () => {
    expect(Object.keys(SOURCE_ORIGIN_COLORS).sort()).toEqual([...ORIGIN_TOKENS].sort());
  });

  it("is total over every known agent runtime token", () => {
    expect(Object.keys(SOURCE_RUNTIME_COLORS).sort()).toEqual([...RUNTIME_TOKENS].sort());
  });

  it("is total over every known news category token", () => {
    expect(Object.keys(SOURCE_CATEGORY_COLORS).sort()).toEqual([...CATEGORY_TOKENS].sort());
  });

  // var()-backed: manual/football/weather/agent/news all resolve to
  // --overlay-blue / --overlay-green / --overlay-amber / --overlay-coral
  // tokens.
  const VAR_BACKED_ORIGINS: SourceOriginToken[] = [
    "manual",
    "football",
    "weather",
    "agent",
    "news",
  ];

  it("every var()-backed origin hex appears in tokens.css", () => {
    for (const token of VAR_BACKED_ORIGINS) {
      expectHexAppearsIn(SOURCE_ORIGIN_COLORS[token], VAR_BACKED_CSS);
    }
  });

  it("claude-code/codex/opencode literal hexes appear in source-identity.css", () => {
    expectHexAppearsIn(SOURCE_RUNTIME_COLORS["claude-code"], sourceIdentityCss);
    expectHexAppearsIn(SOURCE_RUNTIME_COLORS.codex, sourceIdentityCss);
    expectHexAppearsIn(SOURCE_RUNTIME_COLORS.opencode, sourceIdentityCss);
  });

  it("kimi's var()-backed hex (--overlay-fg) appears in tokens.css", () => {
    expectHexAppearsIn(SOURCE_RUNTIME_COLORS.kimi, VAR_BACKED_CSS);
  });

  it("literal news category hexes appear in news-category.css", () => {
    expectHexAppearsIn(SOURCE_CATEGORY_COLORS.politics, newsCategoryCss);
    expectHexAppearsIn(SOURCE_CATEGORY_COLORS.tech, newsCategoryCss);
    expectHexAppearsIn(SOURCE_CATEGORY_COLORS.world, newsCategoryCss);
    expectHexAppearsIn(SOURCE_CATEGORY_COLORS.science, newsCategoryCss);
    expectHexAppearsIn(SOURCE_CATEGORY_COLORS.generic, newsCategoryCss);
  });

  it("sports/business var()-backed hexes appear in tokens.css", () => {
    expectHexAppearsIn(SOURCE_CATEGORY_COLORS.sports, VAR_BACKED_CSS);
    expectHexAppearsIn(SOURCE_CATEGORY_COLORS.business, VAR_BACKED_CSS);
  });
});
