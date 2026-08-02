#!/usr/bin/env node
// Plan 112 Step 1: manual/local drift guard for the vendored shared-ui
// token snapshot. Distinct from shared-ui's own scripts/verify-tokens.mjs
// (that one serves kharcha hex-mirror parity and is unrelated to this
// check). NOT wired into `npm ci` or CI — CI has no ../shared-ui sibling
// checkout to compare against. Invoke manually by name:
//
//   node vendor/shared-ui/verify-snapshot.mjs
//
// Always checks the vendored copy against its own pinned SHA-256. If
// ../shared-ui/design/tokens.css also exists (this Mac, sibling checkout
// present), additionally checks the sibling against ITS own pinned
// SHA-256 — i.e. "has upstream moved since we last looked?", not "is the
// vendored copy byte-identical to upstream?" (it deliberately isn't
// anymore; see PINNED_SIBLING_SHA256 below). Either mismatch exits
// non-zero. If the sibling is absent (CI, other machines), prints the
// pinned SHA-256 + upstream commit and exits 0.
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
// 2026-07-23 refresh (round-4 two-axis review upstream): tokens.css is
// BYTE-IDENTICAL across ef85e85..03ac81e (the range is docs, gate wording,
// playground button cursor, plans only — version still 0.2.1), so
// PINNED_TOKENS_SHA256 is unchanged; only the reviewed-upstream pin moves.
// 2026-07-23 close-out refresh: sibling checkout advanced to 2321f37
// (main); tokens.css is BYTE-IDENTICAL to the 03ac81e snapshot (verified
// via this script's own sibling-diff check — "matches the vendored
// snapshot exactly. No drift."), so PINNED_TOKENS_SHA256 is unchanged;
// only the reviewed-upstream pin moves.
// 2026-08-02 re-pin: commit 892d661 (2026-07-25 audit remediation) deliberately
// ADDED --overlay-green/--overlay-amber/--overlay-fg to the vendored tokens.css
// without refreshing this pin, so the check had been failing ever since. The
// edit is intentional and stays; only the hash below moves to match it. The
// upstream pin is unchanged — these three tokens are this app's own bespoke
// overlay layer, not a refresh from the sibling shared-ui checkout, so
// UPSTREAM_SHA still names the last reviewed upstream commit. That also means
// the sibling-diff branch further below will now legitimately report drift on a
// machine that has the sibling checked out (the vendored copy is deliberately
// ahead of upstream by those three tokens) — treat that as expected here, not
// as accidental local editing.
//
// 2026-08-02 upstream-contribution refresh: the three bespoke
// --overlay-green/--overlay-amber/--overlay-fg tokens were CONTRIBUTED
// upstream this round (shared-ui PR #1, merged f1d2bf7, version 0.3.0) —
// they're no longer a local-only layer, so the vendored copy is now
// BYTE-IDENTICAL to the sibling checkout again, first time since 892d661.
// Both hashes below are the same value as a result.
//
// 2026-08-02 (later the same day) motion-trio adoption, closing the
// mystery the previous refresh flagged: "0.2.2 / tokens commit 4722b88"
// was NOT aspirational — it was real, reviewed work sitting UNPUSHED on
// the mac mini's own ../shared-ui checkout, invisible to the remote
// session that wrote that note (it fetched origin, which only had PR #1).
// The two lines were merged upstream as shared-ui 0.4.0 (merge commit
// 711c792: overlay trio from PR #1 + motion trio from 4722b88, both
// preserved with their original SHAs). This refresh vendors the merged
// file, so the vendored copy is again BYTE-IDENTICAL to the sibling and
// both hashes below share one value. Adopting the --ease-notchtap retune
// (cubic-bezier(.22, 1, .36, 1) -> (0.23, 1, 0.32, 1)) IS the deliberate
// animation decision the old note anticipated — its JS/CSS lockstep
// twins (animationTiming.ts NOTCHTAP_EASE, styles.css's :root
// redeclaration) move in the same commit, guarded by
// animationTiming.test.ts's token-parity tests — the styles.css twin
// had no guard of its own until this round's review added one.
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
    `FAIL: sibling ../shared-ui/design/tokens.css SHA-256 (${siblingSha}) differs from the reviewed sibling state recorded in this script (${PINNED_SIBLING_SHA256}). Upstream has moved since 2026-08-02 — read what changed and either port it into vendor/shared-ui deliberately or re-pin this constant, rather than editing token values here directly.`,
  );
  process.exit(1);
}

console.log(
  "sibling ../shared-ui/design/tokens.css matches its reviewed 2026-08-02 state. No new upstream drift. (It IS byte-identical to the vendored copy as of this refresh — the bespoke overlay-green/amber/fg layer was contributed upstream, see PINNED_TOKENS_SHA256's own comment.)",
);
process.exit(0);
