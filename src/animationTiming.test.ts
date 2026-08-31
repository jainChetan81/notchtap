import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath, URL as NodeURL } from "node:url";
import { describe, expect, it, vi } from "vitest";
import {
  CONTENT_EXIT_MS,
  DISCLOSURE_SPRING,
  EXPAND_MS,
  HOVER_MS,
  ICON_STRIP_STAGGER_MS,
  IDLE_GLANCE_MS,
  IDLE_REVEAL_MS,
  INTERRUPT_EXIT_MS,
  NEWS_CHARGE_STEP_MS,
  NOTCHTAP_EASE,
  REVEAL_MS,
  ROTATION_ENTER_MS,
  ROTATION_EXIT_MS,
  SURFACE_SWAP_MS,
  SWAP_EXIT_MS,
} from "./animationTiming";
import { applyAnimationTiming } from "./applyAnimationTiming";

describe("animationTiming", () => {
  it("SWAP_EXIT_MS matches useDelayedSwap's 175ms exit window", () => {
    expect(SWAP_EXIT_MS).toBe(175);
  });

  it("SURFACE_SWAP_MS matches App.tsx's previous 0.18s board<->rail crossfade", () => {
    expect(SURFACE_SWAP_MS).toBe(180);
  });

  it("IDLE_REVEAL_MS and IDLE_GLANCE_MS match IdleFace's previous literals", () => {
    expect(IDLE_REVEAL_MS).toBe(240);
    expect(IDLE_GLANCE_MS).toBe(200);
  });

  it("DISCLOSURE_SPRING matches the hover-disclosure spring's previous config", () => {
    expect(DISCLOSURE_SPRING).toEqual({ type: "spring", stiffness: 480, damping: 37 });
  });

  it("DISCLOSURE_SPRING carries no per-property opacity override (interruption desync guard)", () => {
    expect(DISCLOSURE_SPRING).not.toHaveProperty("opacity");
    expect(Object.keys(DISCLOSURE_SPRING).sort()).toEqual(["damping", "stiffness", "type"]);
  });

  it("applyAnimationTiming sets the expected custom properties on the given root", () => {
    const setProperty = vi.fn();
    applyAnimationTiming({ setProperty });

    expect(setProperty).toHaveBeenCalledWith("--swap-exit-ms", `${SWAP_EXIT_MS}ms`);
    expect(setProperty).toHaveBeenCalledWith("--content-exit-ms", `${CONTENT_EXIT_MS}ms`);
    expect(setProperty).toHaveBeenCalledWith("--expand-ms", `${EXPAND_MS}ms`);
    expect(setProperty).toHaveBeenCalledWith("--reveal-ms", `${REVEAL_MS}ms`);
    expect(setProperty).toHaveBeenCalledWith("--hover-ms", `${HOVER_MS}ms`);
    expect(setProperty).toHaveBeenCalledWith("--rotation-exit-ms", `${ROTATION_EXIT_MS}ms`);
    expect(setProperty).toHaveBeenCalledWith("--rotation-enter-ms", `${ROTATION_ENTER_MS}ms`);
    expect(setProperty).toHaveBeenCalledWith("--interrupt-exit-ms", `${INTERRUPT_EXIT_MS}ms`);
    expect(setProperty).toHaveBeenCalledWith(
      "--icon-strip-stagger-ms",
      `${ICON_STRIP_STAGGER_MS}ms`,
    );
    expect(setProperty).toHaveBeenCalledWith("--news-charge-step-ms", `${NEWS_CHARGE_STEP_MS}ms`);
    expect(setProperty).toHaveBeenCalledTimes(10);
  });

  it("NOTCHTAP_EASE numerically matches the vendored --ease-notchtap token", () => {
    const tokens = readFileSync(
      fileURLToPath(new NodeURL("../vendor/shared-ui/design/tokens.css", import.meta.url)),
      "utf8",
    );
    const m = tokens.match(
      /--ease-notchtap:\s*cubic-bezier\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*\)/,
    );
    expect(m).not.toBeNull();
    const tokenValues = m ? m.slice(1, 5).map(Number) : [];
    expect(tokenValues).toEqual([...NOTCHTAP_EASE]);
  });

  it("styles.css's :root --ease-notchtap redeclaration matches NOTCHTAP_EASE", () => {
    const styles = readFileSync(
      fileURLToPath(new NodeURL("./styles.css", import.meta.url)),
      "utf8",
    );
    const m = styles.match(
      /--ease-notchtap:\s*cubic-bezier\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*\)/,
    );
    expect(m).not.toBeNull();
    const twinValues = m ? m.slice(1, 5).map(Number) : [];
    expect(twinValues).toEqual([...NOTCHTAP_EASE]);
  });
});

describe("overlay CSS timing-parity (item 6): every transition duration is var(--*-ms, ...)", () => {
  const OVERLAY_DIR_URL = new NodeURL("./overlay/", import.meta.url);

  function stripComments(css: string): string {
    return css.replace(/\/\*[\s\S]*?\*\//g, "");
  }

  const MS_VAR_REF = /var\(\s*--[\w-]+-ms\s*(?:,[^()]*)?\)/g;
  const RAW_DURATION = /\b\d+(?:\.\d+)?m?s\b/g;

  function findRawDurations(transitionValue: string): string[] {
    return transitionValue.replace(MS_VAR_REF, "").match(RAW_DURATION) ?? [];
  }

  function findTransitionDeclarations(css: string): string[] {
    const stripped = stripComments(css);
    const declarations: string[] = [];
    const re = /transition\s*:\s*([^;]+);/g;
    for (const m of stripped.matchAll(re)) {
      declarations.push(m[1].trim().replace(/\s+/g, " "));
    }
    return declarations;
  }

  const ALLOWLISTED_TRANSITIONS: ReadonlySet<string> = new Set([
    "opacity var(--hover-ms, 160ms) var(--ease-notchtap), transform var(--reveal-ms, 260ms) var(--ease-notchtap), visibility 0s linear var(--reveal-ms, 260ms)",
    "opacity var(--hover-ms, 160ms) var(--ease-notchtap) var(--icon-strip-stagger-ms, 60ms), transform var(--reveal-ms, 260ms) var(--ease-notchtap), visibility 0s linear 0s",
  ]);

  it("sanity check: the scanner finds a nonzero number of transition declarations", () => {
    const cardChromeCss = readFileSync(
      fileURLToPath(new NodeURL("card-chrome.css", OVERLAY_DIR_URL)),
      "utf8",
    );
    expect(findTransitionDeclarations(cardChromeCss).length).toBeGreaterThan(0);
  });

  it("every overlay CSS transition duration is var(--*-ms, ...) or an explicit, justified allowlist entry", () => {
    const overlayDir = fileURLToPath(OVERLAY_DIR_URL);
    const files = readdirSync(overlayDir)
      .filter((name) => name.endsWith(".css"))
      .sort();
    expect(files.length).toBeGreaterThan(0);

    const violations: string[] = [];
    for (const file of files) {
      const css = readFileSync(fileURLToPath(new NodeURL(file, OVERLAY_DIR_URL)), "utf8");
      for (const decl of findTransitionDeclarations(css)) {
        if (ALLOWLISTED_TRANSITIONS.has(decl)) {
          continue;
        }
        const raw = findRawDurations(decl);
        if (raw.length > 0) {
          violations.push(
            `${file}: \`transition: ${decl};\` has raw duration(s) [${raw.join(", ")}] not sourced from a var(--*-ms, ...) reference`,
          );
        }
      }
    }

    expect(
      violations,
      [
        "Found overlay CSS `transition:` declaration(s) with a raw ms/s literal duration instead",
        "of a var(--*-ms, ...) reference sourced from animationTiming.ts. Either:",
        "  (a) feed the duration from a new or existing animationTiming.ts constant, injected via",
        "      applyAnimationTiming.ts and consumed here as var(--your-const-ms, <fallback>ms); or",
        "  (b) if the literal is genuinely self-contained (not an animation-feel/pacing choice —",
        "      see ALLOWLISTED_TRANSITIONS's own entries for the bar this has to clear), add the",
        "      exact, whitespace-normalized transition VALUE text to ALLOWLISTED_TRANSITIONS above",
        "      with a comment justifying why it's exempt.",
        "",
        ...violations,
      ].join("\n"),
    ).toEqual([]);
  });
});
