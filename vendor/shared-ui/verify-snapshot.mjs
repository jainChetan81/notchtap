#!/usr/bin/env node
// Manual drift guard for the vendored shared-ui token snapshot. CI has no
// sibling checkout, so invoke this script manually:
//
//   node vendor/shared-ui/verify-snapshot.mjs
//
// It always checks the vendored copy against its pinned SHA-256. When the
// sibling checkout exists, it also checks that copy against its own pin.
// Either mismatch exits non-zero. A missing sibling remains valid.
import { createHash } from "node:crypto";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const vendoredPath = join(here, "design", "tokens.css");
// here = <repo-root>/vendor/shared-ui, so three levels up reaches the
// repo root's *parent* directory, where the sibling shared-ui checkout
// lives (<repo-root>/../shared-ui). NOTE: this resolves correctly for a
// normal checkout; a git worktree nested under a fixed subdirectory (as
// used for isolated agent sessions) sits one level deeper and this
// relative path will not reach the real sibling from inside one — that's
// expected, not a bug, and is why this script degrades to "sibling not
// found, pinned SHA-256 is authoritative" rather than failing.
const siblingPath = join(here, "..", "..", "..", "shared-ui", "design", "tokens.css");

// The upstream commit, vendored content, and sibling content are pinned
// independently so a deliberate refresh must update every affected value.
const UPSTREAM_SHA = "711c792";
const PINNED_TOKENS_SHA256 =
  "cdba5a467dfb81c51e425fb829d39724bbee7c91e19cbda482970f6376742e31";
const PINNED_SIBLING_SHA256 =
  "cdba5a467dfb81c51e425fb829d39724bbee7c91e19cbda482970f6376742e31";

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

const vendoredSha = sha256(vendoredPath);
if (vendoredSha !== PINNED_TOKENS_SHA256) {
  console.error(
    `FAIL: vendored vendor/shared-ui/design/tokens.css SHA-256 (${vendoredSha}) does not match the pinned value recorded in this script (${PINNED_TOKENS_SHA256}). The vendored file was edited outside of a deliberate refresh from upstream.`,
  );
  process.exit(1);
}

console.log(`upstream SHA: ${UPSTREAM_SHA}`);
console.log(`vendored design/tokens.css SHA-256: ${vendoredSha} (matches pinned)`);

if (!existsSync(siblingPath)) {
  console.log(
    `sibling checkout not found at ${siblingPath} — nothing to diff against on this machine. Pinned SHA-256 above is authoritative. Exiting 0.`,
  );
  process.exit(0);
}

const siblingSha = sha256(siblingPath);
if (siblingSha !== PINNED_SIBLING_SHA256) {
  console.error(
    `FAIL: sibling ../shared-ui/design/tokens.css SHA-256 (${siblingSha}) differs from the pinned sibling state recorded in this script (${PINNED_SIBLING_SHA256}). Read the upstream change, then port it deliberately or re-pin this constant.`,
  );
  process.exit(1);
}

console.log(
  "sibling ../shared-ui/design/tokens.css matches its pinned upstream state.",
);
process.exit(0);
