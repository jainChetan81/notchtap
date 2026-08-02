// Plan 180 (Step 3): the cross-language pin for plan 171's five-tab
// identity set. That set is hand-written in SIX places, three of them in
// one rust file and three spread across two languages:
//
//   1. `src-tauri/src/tabs.rs` — `Tab::ORDER` (the strip's left-to-right
//      order, which the click hit-test zips against `hover.rs`'s rects)
//   2. `src-tauri/src/tabs.rs` — `Tab::from_prefix_digit` (`prefix+1..5`)
//   3. `src-tauri/src/tabs.rs` — `Tab::wire_label` (the `tab-selection-
//      changed` tokens; note "music", NOT "media")
//   4. `src/components/IconStrip.tsx` — the `Tab` union and `TAB_ORDER`
//   5. `src-tauri/src/lib.rs` — `PREFIX_FOLLOWUPS`' `Digit1..Digit5` rows
//   6. `src/lib/iconPresence.ts` — the `IconPresence` record's keys
//
// (A seventh site, `icon-strip.css`'s per-tab selectors, is deliberately
// NOT pinned here — plan 175's own geometry pin already covers it, and
// double-pinning would mean two tests failing for one edit.)
//
// **Why this needs a test at all: the drift is silent, and it fails in
// the safe-LOOKING direction.** `useTabSelection.ts` validates an
// incoming token against `TAB_ORDER` as a closed set, so a rust-side
// rename or reorder does not throw, does not log, and does not render an
// error — it coerces to `selected: null`, which is a real and expected
// value (spec §7's "none" page). The operator sees an app where clicking
// an icon simply does nothing, and every existing test stays green,
// because each layer in isolation is behaving exactly as designed. The
// only way to catch it is to compare the lists themselves.
//
// Same cheap register as `src/settings/hookEventParity.test.ts` — read
// the rust as TEXT, no parser, region-scoped so a marker that moves
// fails loudly rather than silently matching the wrong block. The TS side
// is imported for real (both `TAB_ORDER` and `iconPresenceFor` are
// exported), so only the rust half is string-matched.
import { readFileSync } from "node:fs";
import { fileURLToPath, URL as NodeURL } from "node:url";
import { describe, expect, it } from "vitest";
import { TAB_ORDER, type Tab } from "./components/IconStrip";
import { iconPresenceFor } from "./lib/iconPresence";

/// Named in every failure message below, because the fix for a failure
/// here is never "edit this test" — it is "one of these six lists moved
/// and the other five did not".
const SIX_SITES =
  "the five-tab identity set is hand-synced across SIX sites — tabs.rs's Tab::ORDER, " +
  "Tab::from_prefix_digit, and Tab::wire_label; IconStrip.tsx's Tab union + TAB_ORDER; " +
  "lib.rs's PREFIX_FOLLOWUPS digit rows; and iconPresence.ts's IconPresence keys. " +
  "Update ALL of them together (icon-strip.css too — pinned separately by plan 175)";

function readText(relativePath: string): string {
  const url = new NodeURL(relativePath, import.meta.url);
  return readFileSync(fileURLToPath(url), "utf-8");
}

const tabsRs = readText("../src-tauri/src/tabs.rs");
const libRs = readText("../src-tauri/src/lib.rs");

/** The text between a region's opening marker and its closing one. */
function region(source: string, start: string, end: string): string {
  const from = source.indexOf(start);
  if (from === -1) throw new Error(`region start not found: ${start}`);
  const to = source.indexOf(end, from + start.length);
  if (to === -1) throw new Error(`region end not found for: ${start}`);
  return source.slice(from, to);
}

/** Every match of a two-group pattern, in source order, as pairs. */
function pairs(text: string, pattern: RegExp): [string, string][] {
  return [...text.matchAll(pattern)].map((match) => [match[1], match[2]]);
}

// `"\n    }"` — the 4-space-indented closing brace — is the end of an
// `impl` method: every brace INSIDE these two bodies (the `match`'s own)
// closes at 8 spaces, so this marker cannot land early. `Tab::ORDER` and
// `PREFIX_FOLLOWUPS` are plain array consts and end at `];`, exactly as
// hookEventParity.test.ts's own rust regions do.
const METHOD_END = "\n    }";

/** `Tab::Agent => "agent",` pairs from `wire_label`, in source order. */
function wireLabelArms(): [string, string][] {
  return pairs(region(tabsRs, "pub fn wire_label", METHOD_END), /Tab::(\w+)\s*=>\s*"([^"]+)"/g);
}

/** `1 => Some(Tab::Agent),` pairs from `from_prefix_digit`. */
function prefixDigitArms(): [string, string][] {
  return pairs(
    region(tabsRs, "pub fn from_prefix_digit", METHOD_END),
    /(\d+)\s*=>\s*Some\(Tab::(\w+)\)/g,
  );
}

/** The variant names listed in `Tab::ORDER`, in order. */
function orderVariants(): string[] {
  return [...region(tabsRs, "pub const ORDER", "];").matchAll(/Tab::(\w+)/g)].map(
    (match) => match[1],
  );
}

/** `(Code::DigitN, prefix::PrefixKey::Digit(M))` pairs from lib.rs. */
function prefixFollowupDigits(): [string, string][] {
  return pairs(
    region(libRs, "const PREFIX_FOLLOWUPS", "];"),
    /Code::Digit(\d)\s*,\s*prefix::PrefixKey::Digit\((\d)\)/g,
  );
}

describe("tab wire parity (plan 180) — rust's five tabs match the frontend's", () => {
  it("wire_label emits exactly TAB_ORDER's five tokens, in TAB_ORDER's order", () => {
    const arms = wireLabelArms();
    // the tokens rust actually puts on `tab-selection-changed`...
    expect(
      arms.map(([, token]) => token),
      SIX_SITES,
    ).toEqual([...TAB_ORDER]);
    // ...and the variant each one came from, so a swapped pair of arms
    // (`Music => "weather"`) fails even though the token SET is intact.
    expect(
      arms.map(([variant]) => variant),
      SIX_SITES,
    ).toEqual(["Agent", "Football", "Music", "Weather", "News"]);
    // "music", not "media" — the strip's own vocabulary (tabs.rs's own
    // note on `wire_label`). The frontend calls the SOURCE `media`
    // everywhere else, so this one token is the likeliest to be
    // "corrected" into a bug.
    expect(arms.map(([, token]) => token)).toContain("music");
    expect(arms.map(([, token]) => token)).not.toContain("media");
  });

  it("Tab::ORDER lists the same five variants, in the same order", () => {
    expect(orderVariants(), SIX_SITES).toEqual(wireLabelArms().map(([variant]) => variant));
    expect(orderVariants().length).toBe(TAB_ORDER.length);
  });

  it("from_prefix_digit maps 1..5 onto Tab::ORDER positionally", () => {
    const arms = prefixDigitArms();
    expect(
      arms.map(([digit]) => digit),
      SIX_SITES,
    ).toEqual(["1", "2", "3", "4", "5"]);
    // `prefix+N` selects the Nth icon from the left — the spec §9 keymap
    // rule, expressed as "these arms ARE Tab::ORDER, in order".
    expect(
      arms.map(([, variant]) => variant),
      SIX_SITES,
    ).toEqual(orderVariants());
  });

  it("PREFIX_FOLLOWUPS grabs Digit1..Digit5 and hands each its own digit", () => {
    const rows = prefixFollowupDigits();
    expect(
      rows.map(([code]) => code),
      SIX_SITES,
    ).toEqual(["1", "2", "3", "4", "5"]);
    // an off-by-one here (`Digit3 -> Digit(2)`) would select the wrong
    // tab from the keyboard while the mouse path stayed perfect.
    for (const [code, key] of rows) {
      expect(
        code,
        `PREFIX_FOLLOWUPS: Code::Digit${code} must map to PrefixKey::Digit(${code})`,
      ).toBe(key);
    }
  });

  it("iconPresence's table is keyed by exactly those five tokens", () => {
    // the sixth site, checked by import rather than by text — a missing
    // key here is a tab whose icon can never light up.
    expect(Object.keys(iconPresenceFor(undefined)).sort(), SIX_SITES).toEqual(
      [...TAB_ORDER].sort(),
    );
  });

  it("the TS union and TAB_ORDER cannot drift apart (compile-time, pinned here in prose)", () => {
    // `TAB_ORDER` is typed `readonly Tab[]`, so a token the union does not
    // contain is a tsc error, not a runtime one — this assertion exists to
    // catch the OTHER direction: a union member nobody put in the array.
    const covered: Record<Tab, true> = {
      agent: true,
      football: true,
      music: true,
      weather: true,
      news: true,
    };
    expect(Object.keys(covered).sort(), SIX_SITES).toEqual([...TAB_ORDER].sort());
  });

  // The extraction helpers are the load-bearing part of a text-level pin:
  // if a marker stops matching, every assertion above would compare two
  // empty lists and pass. Same guard hookEventParity.test.ts's `region`
  // builds in — asserted here rather than assumed.
  describe("the extraction itself fails loudly", () => {
    it("throws when a region's opening marker is gone", () => {
      expect(() => region(tabsRs, "pub fn renamed_away", METHOD_END)).toThrow(
        /region start not found/,
      );
    });

    it("throws when a region's closing marker is gone", () => {
      expect(() => region(tabsRs, "pub fn wire_label", " not-in-any-file")).toThrow(
        /region end not found/,
      );
    });

    it("extracts a non-empty list from every region (no silent empty match)", () => {
      expect(wireLabelArms().length).toBe(5);
      expect(orderVariants().length).toBe(5);
      expect(prefixDigitArms().length).toBe(5);
      expect(prefixFollowupDigits().length).toBe(5);
    });
  });
});
