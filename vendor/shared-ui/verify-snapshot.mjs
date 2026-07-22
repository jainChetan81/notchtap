#!/usr/bin/env node
// Plan 112 Step 1: manual/local drift guard for the vendored shared-ui
// token snapshot. Distinct from shared-ui's own scripts/verify-tokens.mjs
// (that one serves kharcha hex-mirror parity and is unrelated to this
// check). NOT wired into `npm ci` or CI — CI has no ../shared-ui sibling
// checkout to compare against. Invoke manually by name:
//
//   node vendor/shared-ui/verify-snapshot.mjs
//
// If ../shared-ui/design/tokens.css exists (this Mac, sibling checkout
// present), compares its SHA-256 against the vendored copy and exits
// non-zero on any difference. If the sibling is absent (CI, other
// machines), prints the pinned SHA-256 + upstream commit and exits 0.
//
// `npm ci` self-containment (this snapshot resolves without ../shared-ui
// present at all) is proven separately by the "npm ci with ../shared-ui
// absent" gate in the plan's Commands table -- not by this script.
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

// Reviewed upstream commit shared-ui is pinned to for this snapshot (see
// plans/112-settings-shadcn-migration.md "Portable token snapshot").
// NOTE: the plan text names 8e395a8 as the reviewed SHA, but the sibling
// checkout legitimately advanced past that (operator-authorized) before
// this snapshot was taken — ca4faf8 was the effective pin at that time
// (adds --font-sans/--font-mono/--font-heading tokens upstream; no value
// changes to any token this app already consumed).
//
// plan 113: refreshed the pin to 2279978 (sibling now versioned 0.2.0).
// `design/tokens.css` is BYTE-IDENTICAL across ca4faf8..2279978 — the
// range only touched upstream's own scripts/playground, not token
// values — so PINNED_TOKENS_SHA256 below is unchanged and does not need
// a re-hash.
// 0.2.1 refresh (2026-07-22, external-review round): upstream flipped
// --primary-foreground and --sidebar-primary-foreground from near-white to
// near-black #050607 (white on accent blue #0a84ff was 3.26:1, fails WCAG AA
// for normal text; dark-on-blue is 5.56:1, matching the destructive pattern).
// token content commit b0ba7bb; pinned at upstream HEAD ef85e85. re-hashed.
const UPSTREAM_SHA = "ef85e85";
const PINNED_TOKENS_SHA256 =
  "5b7bcefe6f89c9ed72a1487e9ac2bd15c644d56904582aa33735fcc2039e8313";

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
if (siblingSha !== vendoredSha) {
  console.error(
    `FAIL: sibling ../shared-ui/design/tokens.css SHA-256 (${siblingSha}) differs from the vendored snapshot (${vendoredSha}). Upstream has drifted since the ${UPSTREAM_SHA} snapshot was taken — refresh vendor/shared-ui deliberately (see this file's header) rather than editing token values here directly.`,
  );
  process.exit(1);
}

console.log("sibling ../shared-ui/design/tokens.css matches the vendored snapshot exactly. No drift.");
process.exit(0);
