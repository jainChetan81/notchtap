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
// this snapshot was taken — ca4faf8 is the effective pin actually
// vendored below (adds --font-sans/--font-mono/--font-heading tokens
// upstream; no value changes to any token this app already consumed).
const UPSTREAM_SHA = "ca4faf8";
const PINNED_TOKENS_SHA256 =
  "c8416630c99a60737ff8dd9e1348b2ec771a5569501b0d5f8fbfd0ac584635c8";

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
