import { describe, expect, it } from "vitest";
import { isValidPrefixShortcut } from "./ShortcutsSection";

const PREFIX = "⌃⇧";

const ACCEPTED: [string, string][] = [
  [`${PREFIX}K`, "a single glyph, the shape the shipped seven shortcuts use"],
  [`${PREFIX}Space`, "a spelled-out key name, the shipped default"],
  [`${PREFIX}K\u{FEFF}`, "U+FEFF is NOT Unicode White_Space — both sides accept it"],
  [`${PREFIX}${"K".repeat(24)}`, "24 chars of key name — the inclusive upper bound"],
];

const REJECTED: [string, string][] = [
  [`${PREFIX}K L`, "an ordinary space inside the key name"],
  [`${PREFIX}K\u{0085}`, "U+0085 (NEL) IS White_Space — the bug this table caught"],
  [PREFIX, "the prefix with no key name at all"],
  [`${PREFIX}${"K".repeat(25)}`, "25 chars — one past the upper bound"],
];

describe("isValidPrefixShortcut", () => {
  for (const [value, why] of ACCEPTED) {
    it(`accepts ${JSON.stringify(value)} — ${why}`, () => {
      expect(isValidPrefixShortcut(value)).toBe(true);
    });
  }

  for (const [value, why] of REJECTED) {
    it(`rejects ${JSON.stringify(value)} — ${why}`, () => {
      expect(isValidPrefixShortcut(value)).toBe(false);
    });
  }

  it("splits on exactly the two code points where \\s and White_Space disagree", () => {
    expect(/\s/.test("\u{0085}")).toBe(false);
    expect(isValidPrefixShortcut(`${PREFIX}K\u{0085}`)).toBe(false);

    expect(/\s/.test("\u{FEFF}")).toBe(true);
    expect(isValidPrefixShortcut(`${PREFIX}K\u{FEFF}`)).toBe(true);
  });

  it("rejects a key name containing any BMP White_Space code point, and no others", () => {
    const isWhiteSpace = /\p{White_Space}/u;
    const offenders: string[] = [];
    for (let code = 0; code <= 0xffff; code++) {
      if (code >= 0xd800 && code <= 0xdfff) {
        continue;
      }
      const char = String.fromCharCode(code);
      const accepted = isValidPrefixShortcut(`${PREFIX}K${char}`);
      if (accepted === isWhiteSpace.test(char)) {
        offenders.push(`U+${code.toString(16).toUpperCase().padStart(4, "0")}`);
      }
    }
    expect(offenders, "each of these disagrees with Unicode White_Space").toEqual([]);
  });
});
