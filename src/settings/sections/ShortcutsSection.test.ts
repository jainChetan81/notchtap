// Plan 180 (Step 4): the TS half of a two-language fixture table.
//
// `isValidPrefixShortcut` (ShortcutsSection.tsx) and
// `is_valid_prefix_shortcut` (src-tauri/src/settings.rs) are hand-written
// twins that both claim "exact sync". The UI one decides whether the
// Settings field looks valid and whether `patchConfig` fires at all; the
// rust one decides whether the config actually saves. When they disagree,
// the operator gets the worst possible failure shape: a field that reads
// as accepted and a save that quietly refuses it (or the reverse — a
// field that refuses a value rust would happily have taken).
//
// They DID disagree, on whitespace: rust's `char::is_whitespace` is
// Unicode `White_Space` (25 code points), while JavaScript's `\s` is a
// different set — it misses U+0085 (NEL) and adds U+FEFF (ZWNBSP).
//
// The fixture table below is the contract. The identical strings appear
// in `settings.rs`'s `prefix_shortcut_whitespace_table_matches_the_ts_mirror`
// — if you change either validator, run BOTH tables.
import { describe, expect, it } from "vitest";
import { isValidPrefixShortcut } from "./ShortcutsSection";

const PREFIX = "⌃⇧";

/** The shared table's accept side — mirrored verbatim in settings.rs. */
const ACCEPTED: [string, string][] = [
  [`${PREFIX}K`, "a single glyph, the shape the shipped seven shortcuts use"],
  [`${PREFIX}Space`, "a spelled-out key name, the spec's chosen default"],
  [`${PREFIX}K\u{FEFF}`, "U+FEFF is NOT Unicode White_Space — both sides accept it"],
  [`${PREFIX}${"K".repeat(24)}`, "24 chars of key name — the inclusive upper bound"],
];

/** The shared table's reject side — mirrored verbatim in settings.rs. */
const REJECTED: [string, string][] = [
  [`${PREFIX}K L`, "an ordinary space inside the key name"],
  [`${PREFIX}K\u{0085}`, "U+0085 (NEL) IS White_Space — the bug this table caught"],
  [PREFIX, "the prefix with no key name at all"],
  [`${PREFIX}${"K".repeat(25)}`, "25 chars — one past the upper bound"],
];

describe("isValidPrefixShortcut (plan 180's shared fixture table)", () => {
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

  // The two code points that motivated the change, called out by name so
  // a future "simplify this back to \s" reads as the regression it is.
  it("splits on exactly the two code points where \\s and White_Space disagree", () => {
    // `\s` does not match NEL, so the pre-plan-180 mirror accepted this
    // and rust then rejected it at save time.
    expect(/\s/.test("\u{0085}")).toBe(false);
    expect(isValidPrefixShortcut(`${PREFIX}K\u{0085}`)).toBe(false);

    // `\s` DOES match ZWNBSP, so the pre-plan-180 mirror rejected a value
    // rust considered perfectly fine.
    expect(/\s/.test("\u{FEFF}")).toBe(true);
    expect(isValidPrefixShortcut(`${PREFIX}K\u{FEFF}`)).toBe(true);
  });

  // The plan's own reviewer note asks for the 25-code-point class to be
  // verified against Unicode rather than eyeballed. This does it by
  // machine, through the public validator, over the whole BMP (Unicode
  // has no White_Space code point above U+3000, so the BMP is the whole
  // set). The rust side runs the identical sweep against
  // `char::is_whitespace` — see settings.rs.
  it("rejects a key name containing any BMP White_Space code point, and no others", () => {
    const isWhiteSpace = /\p{White_Space}/u;
    const offenders: string[] = [];
    for (let code = 0; code <= 0xffff; code++) {
      // lone surrogates are not scalar values; rust's own sweep skips
      // them too, so the two runs cover the same domain.
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
