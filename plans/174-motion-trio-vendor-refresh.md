# 174 — shared-ui 0.4.0 vendor refresh: adopt the motion trio

- **Status**: DONE (2026-08-02) — upstream reconciled and merged as
  shared-ui 0.4.0 (`711c792`), vendored file refreshed byte-identical,
  `--ease-notchtap` retune adopted across all three lockstep twins,
  verify-snapshot re-pinned. `npx vitest run` 625/625 at this commit
  (626/626 at the branch tip — see "Doc truth-up" step 5 below),
  `node vendor/shared-ui/verify-snapshot.mjs` clean both axes.
- **Severity**: LOW (visual: subtle; process: closes a real
  two-machines-diverged incident)
- **Category**: Vendor sync + deliberate animation adoption
- **Numbering note**: 172/173 are reserved by the standing handoff for
  the EXPAND_MS feel-check and the card-chrome width→transform spike;
  this landed first chronologically but takes the next free number.

## What happened (the incident this closes)

`verify-snapshot.mjs`'s 2026-08-02 refresh (commit `01748dd`, plan 171
branch) flagged that a previously-documented "shared-ui 0.2.2 / tokens
commit `4722b88` motion trio" could not be found in `git log --all` on
the shared-ui checkout, and concluded it "either never got
pushed/merged, lived on a branch since deleted, or was aspirational."

None of those. It was real, reviewed work (shared-ui plans 012-017,
CHANGELOG 0.2.2, committed 2026-07-24) sitting UNPUSHED on the Mac
mini's local `../shared-ui` main. The session that wrote the note ran
remotely, fetched origin — which only had PR #1 (overlay trio, 0.3.0,
version-numbered past the invisible 0.2.2) — and correctly reported
what it could verify. Two sessions on two machines had extended the
same repo in parallel, one line never pushed.

## What this plan did

1. **Upstream (shared-ui)**: merged `origin/main` (f1d2bf7, PR #1) into
   local main (853b4ac, motion trio) — merge commit `711c792`, pushed.
   Conflicts were CHANGELOG.md (both entries kept, 0.3.0's sha
   backfilled `f1d2bf7`) and package.json version (0.2.2 vs 0.3.0 →
   **0.4.0**, minor per the changelog's own rules: relative to
   published 0.3.0 the merge ADDS `--ease-drawer`/`--ease-in-out-strong`).
   `design/tokens.css` auto-merged — the two changes touch disjoint
   regions. `npm run verify:tokens` passes (37 mirrored tokens).
2. **Vendored copy**: `vendor/shared-ui/design/tokens.css` replaced with
   the merged file — byte-identical to the sibling again. New content
   relative to the previous snapshot: `--ease-notchtap`
   `cubic-bezier(.22, 1, .36, 1)` → `cubic-bezier(0.23, 1, 0.32, 1)`
   (both strong ease-outs; the new one is the standard easeOutQuint
   shape), plus the two new tokens above — `--ease-drawer` and
   `--ease-in-out-strong` have no notchtap consumer yet (available, not
   adopted); `--ease-notchtap` itself is adopted below (step 3).
3. **Lockstep twins** (the trio `styles.css`'s plan-163 comment says
   must move together): `src/styles.css` `:root` redeclaration and
   `src/animationTiming.ts` `NOTCHTAP_EASE` both retuned to
   `(0.23, 1, 0.32, 1)`. `animationTiming.test.ts`'s token-parity test
   (parses the vendored token, compares to the array) guards the pair —
   it fails if any one of tokens.css/NOTCHTAP_EASE moves alone.
   Stale value-echoes in comments (IdleFace.tsx, StatusRailCard.tsx,
   animationTiming.ts's own header) updated or de-literalized so the
   four numbers stay written in as few places as possible.
4. **Re-pin**: `verify-snapshot.mjs` UPSTREAM_SHA → `711c792`, both
   SHA-256 pins → the merged file's hash, and the "4722b88 does not
   exist" note replaced with the corrected history (kept as narrative —
   the note was RIGHT about what was verifiable at the time; the lesson
   is "unpushed local work is invisible to remote sessions", worth
   keeping written down).
5. **Doc truth-up (drive-by)**: plan 171's Status line claimed
   `npx vitest run` 768/768 — actually 625/625; 768 was 625 (full
   suite) + 143 (the two evidence files re-run standalone) summed by
   mistake. Per-file counts (13, 130) re-verified exact. (A later
   commit on this same branch, 17d3933, adds one more guard test —
   `styles.css`'s `--ease-notchtap` twin-parity check in
   `animationTiming.test.ts` — bringing the branch tip from 625/625 to
   626/626; the counts recorded across plans 171/172/174 are each
   accurate as of their own commit, not stale.)

## What was deliberately NOT done

- No notchtap consumer for `--ease-drawer` / `--ease-in-out-strong` —
  they ship in the vendored file for availability only. Wiring them to
  the settings drawer/sidebar is its own future feel decision.
- `prototype/*.html` / `prototypes/*.html` still carry the old curve in
  their standalone `<style>` blocks — they drift by design (CLAUDE.md)
  and are not part of the build.
- No retune of `--ease-notchtap-pop` (the overshoot variant) — upstream
  never touched it.

## Feel note

The curve change is subtle (max vertical divergence between the two
beziers is a few percent, mid-curve). Plan 172's EXPAND_MS frame-sample
runs AFTER this adoption on purpose, so its measurements describe the
curve the app will actually ship.
