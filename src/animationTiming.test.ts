import { readdirSync, readFileSync } from "node:fs";
// `URL as NodeURL` (not the ambient global): jsdom's URL shadow resolves
// relative paths against a fake http: document location — the same trap
// entryImportOrder.test.ts documents and dodges identically.
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

// plan 117: pins the single-sourced duration constant to the exact value
// every existing consumer/test already assumed (StatusRailCard's
// useDelayedSwap window) — a regression here is exactly the kind of
// silent drift this plan exists to prevent.
//
// plan 12x (wave 3, operator-feedback polish pass): 220 -> 175 (~20%
// quicker) — this pin moves WITH the constant, same as
// StatusRailCard.test.tsx's "compact->idle geometry" describe block
// (which steps fake timers against this same value); the assertion's
// MEANING (the constant is pinned to a known literal, not a symbol) is
// unchanged, only the literal itself.
describe("animationTiming (plan 117)", () => {
  it("SWAP_EXIT_MS matches useDelayedSwap's 175ms exit window", () => {
    expect(SWAP_EXIT_MS).toBe(175);
  });

  // plan 148: the four tokens that replaced hand-typed literals in
  // App.tsx / IdleHoverPeek.tsx / IdleFace.tsx. Pinned to the exact
  // values those literals carried — plan 148 was tokenization, NOT a
  // retune, so a diff here means someone changed the feel.
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

  // plan 148 regression guard. The four hand-copied call sites this
  // spring replaced each carried a separate `opacity: { duration: 0.15 }`
  // per-property override, which ran on its own clock and so desynced
  // from the spring whenever a hover flip interrupted it mid-open —
  // height still collapsing after opacity hit 0 (ghost box), or height
  // at 0 while still partly opaque. The fix is precisely the ABSENCE of
  // any per-property override: one spring drives every animated
  // property, so an interruption retargets them together. Re-adding an
  // `opacity` key (or any other per-property override) reintroduces the
  // bug, so assert there is none.
  it("DISCLOSURE_SPRING carries no per-property opacity override (interruption desync guard)", () => {
    expect(DISCLOSURE_SPRING).not.toHaveProperty("opacity");
    expect(Object.keys(DISCLOSURE_SPRING).sort()).toEqual(["damping", "stiffness", "type"]);
  });

  // 2026-07-23 review fix (wave C, CSS custom-property injection): the
  // two regex-parsing guards that used to live here (CONTENT_EXIT_MS ↔
  // overlay-card.css's flank-round `border-radius` duration, SWAP_EXIT_MS
  // ↔ `.card-assembly.exiting`'s own `width` duration) are gone. They
  // existed only because the CSS carried its OWN copy of each number,
  // which could drift from the JS constant without either side erroring
  // — so a test had to parse the stylesheet and compare by hand. That
  // duplication is gone: overlay-card.css now reads these values via
  // `var(--content-exit-ms, ...)`/`var(--swap-exit-ms, ...)`, set on the
  // document root by `applyAnimationTiming` (below) directly from these
  // same constants. There is exactly one place either number is written
  // as a literal now, so there is nothing left for a parsing guard to
  // catch — the coverage that matters is "does applyAnimationTiming
  // actually set the properties it claims to", which the test below
  // pins instead.
  it("applyAnimationTiming sets the expected custom properties on the given root", () => {
    const setProperty = vi.fn();
    applyAnimationTiming({ setProperty });

    expect(setProperty).toHaveBeenCalledWith("--swap-exit-ms", `${SWAP_EXIT_MS}ms`);
    expect(setProperty).toHaveBeenCalledWith("--content-exit-ms", `${CONTENT_EXIT_MS}ms`);
    expect(setProperty).toHaveBeenCalledWith("--expand-ms", `${EXPAND_MS}ms`);
    // plan 127 (Step 1): the four new timing tokens, same injection
    // discipline as the three above.
    expect(setProperty).toHaveBeenCalledWith("--reveal-ms", `${REVEAL_MS}ms`);
    expect(setProperty).toHaveBeenCalledWith("--hover-ms", `${HOVER_MS}ms`);
    expect(setProperty).toHaveBeenCalledWith("--rotation-exit-ms", `${ROTATION_EXIT_MS}ms`);
    expect(setProperty).toHaveBeenCalledWith("--rotation-enter-ms", `${ROTATION_ENTER_MS}ms`);
    // plan 146b: the interrupt-exit timing token, same injection
    // discipline as the two rotation tokens above.
    expect(setProperty).toHaveBeenCalledWith("--interrupt-exit-ms", `${INTERRUPT_EXIT_MS}ms`);
    // plan 171 (tab-notch redesign): the icon strip's own two tokens,
    // same injection discipline as every token above.
    expect(setProperty).toHaveBeenCalledWith(
      "--icon-strip-stagger-ms",
      `${ICON_STRIP_STAGGER_MS}ms`,
    );
    expect(setProperty).toHaveBeenCalledWith("--news-charge-step-ms", `${NEWS_CHARGE_STEP_MS}ms`);
    expect(setProperty).toHaveBeenCalledTimes(10);
  });

  // 2026-07-23 review fix (Duplicated Code finding): NOTCHTAP_EASE is the
  // JS twin of shared-ui's `--ease-notchtap` cubic-bezier token. Parse the
  // vendored token and compare numerically so the pair can't drift. Kept
  // (not folded into the custom-property injection above): motion needs
  // the real JS array for its own consumers (this is not a CSS-only
  // duration), so this pair is a real cross-file lockstep, unlike the two
  // guards removed above.
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

  // plan 174 review follow-up (Standards axis): the curve actually has
  // THREE copies, and the third — styles.css's `:root` redeclaration
  // (plan 163's defense-in-depth twin, required because Tailwind's
  // `@theme` scoping never reaches the overlay bundle) — was the only
  // unguarded one; the 174 retune touched it by hand with nothing
  // failing if it hadn't. Same parse-and-compare treatment as the
  // vendored token above, so no copy of the ease can drift alone.
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

// item 6 (timing-parity enforcement): a scanner over every `src/overlay/*.css`
// source, string-level like `overlayCardMirror.test.ts`'s own selector
// scanner (no CSS parser — "cheap but effective", same register that file's
// own doc calls out). The invariant: every `transition:` declaration's
// duration must be sourced from an animationTiming.ts constant via
// `var(--*-ms, <fallback>)` (the injection discipline
// `applyAnimationTiming.ts` + this file's other describe block already
// guard), never a raw hand-typed ms/s literal sitting directly in the
// property list — that's exactly the "two things quietly drift apart"
// shape this whole file exists to prevent, just for CSS `transition:`
// durations specifically rather than JS constants.
describe("overlay CSS timing-parity (item 6): every transition duration is var(--*-ms, ...)", () => {
  const OVERLAY_DIR_URL = new NodeURL("./overlay/", import.meta.url);

  function stripComments(css: string): string {
    return css.replace(/\/\*[\s\S]*?\*\//g, "");
  }

  // matches a reference to an animationTiming-fed duration var, fallback
  // included (e.g. `var(--expand-ms, 300ms)`) — removed wholesale before
  // scanning for leftover raw literals, so a var's own ms-literal FALLBACK
  // (defense-in-depth for a context that skips applyAnimationTiming, same
  // reasoning as EXPAND_MS's own doc above) is never mistaken for a
  // hand-typed duration.
  const MS_VAR_REF = /var\(\s*--[\w-]+-ms\s*(?:,[^()]*)?\)/g;
  // a bare CSS time literal — digits, optional decimal, then `ms` or `s`
  // at a word boundary. Deliberately doesn't match plain numbers (e.g.
  // cubic-bezier's `0.3, 1.36, 0.44, 1`) or other units (`16px`): those
  // never end in a bare "s"/"ms" suffix immediately after the digits.
  const RAW_DURATION = /\b\d+(?:\.\d+)?m?s\b/g;

  function findRawDurations(transitionValue: string): string[] {
    return transitionValue.replace(MS_VAR_REF, "").match(RAW_DURATION) ?? [];
  }

  /** Extracts every `transition: <value>;` declaration's value (comment-
   * stripped, whitespace-normalized to one line for stable allowlist keys
   * and legible failure messages) from a CSS source. Deliberately matches
   * `transition:` only, not `transition-duration:`/`-property:`/
   * `-timing-function:` — grepping the real files found no standalone use
   * of those longhands anywhere in `src/overlay/*.css` today; if one is
   * ever added, this scanner should grow a matching extractor rather than
   * silently missing it. */
  function findTransitionDeclarations(css: string): string[] {
    const stripped = stripComments(css);
    const declarations: string[] = [];
    const re = /transition\s*:\s*([^;]+);/g;
    for (const m of stripped.matchAll(re)) {
      declarations.push(m[1].trim().replace(/\s+/g, " "));
    }
    return declarations;
  }

  // Reviewed, explicit allowlist — a whitespace-normalized `transition:`
  // VALUE (not a selector, not a file) that's permitted to carry a raw
  // duration literal instead of an animationTiming var. Every entry must
  // carry its own justification comment; an unjustified addition here
  // defeats the point of this test.
  const ALLOWLISTED_TRANSITIONS: ReadonlySet<string> = new Set([
    // item 4 (media progress glide): idle-peek.css's `.media-bar-fill`
    // glides continuously between IdleHoverPeek.tsx's own `useLiveTick`
    // ticks, which re-render on a hand-typed `window.setInterval(..., 1000)`
    // — the 1s transition duration IS that polling cadence, a structural
    // pairing with a JS interval literal that lives in a component file,
    // not an animation-feel pacing choice that belongs in
    // animationTiming.ts alongside the enter/exit/hover/reveal timings it
    // single-sources. Genuinely self-contained: there is no second CSS or
    // JS copy of "1s" this could drift from.
    "transform 1s linear",
    // Plan 171 (tab-notch redesign, icon-strip.css): each entry below is
    // a full multi-leg `transition:` value where the opacity/transform
    // legs are already var(--*-ms, ...)-sourced (real animation-timing
    // choices, including the reveal stagger — icon-strip-stagger-ms) and
    // only the trailing `visibility ... 0s` leg carries a raw literal.
    // `0s` there is not a tunable duration: visibility always snaps
    // instantly by construction, and the actual timing choice is the
    // DELAY value beside it (`var(--reveal-ms, 260ms)` on the rest-state
    // rule, deliberately `0s` — snap immediately, no delay — on the
    // hovered rule), both already token-sourced or intentionally zero.
    // Splitting `visibility` into its own longhand `transition-property`/
    // `-duration`/`-delay` declarations would dodge this scanner entirely
    // (it only matches the `transition:` shorthand, by this file's own
    // documented design) rather than actually justify the exemption, so
    // that's deliberately not done here.
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
