# Plan 113: Shared-ui final pass — refresh token pin, adopt `--font-mono`, verify reference parity

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in "STOP conditions" occurs, stop and report — do not
> improvise. Do NOT push, merge, or open a PR — a reviewer merges. Do NOT edit
> `plans/README.md` — the reviewer maintains the index.
>
> **Drift check (run first)**: `git diff --stat 82c36d4..HEAD -- src/settings/base.css vendor/shared-ui/`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: plans/112-settings-shadcn-migration.md (DONE — merged at `82c36d4`)
- **Category**: tech-debt / migration
- **Planned at**: commit `82c36d4`, 2026-07-22

## Why this matters

Plan 112 migrated the settings window onto shared-ui's OKLCH semantic tokens
and copied shadcn primitives locally. Two loose ends remain: (1) `base.css`
still **hardcodes** the monospace font stack in one place instead of consuming
shared-ui's `--font-mono` token — the exact drift the token contract exists to
prevent; and (2) the vendored token snapshot's recorded provenance (`UPSTREAM_SHA`,
package version) points at an older upstream commit than the sibling now sits at,
even though the token *values* are byte-identical. This pass closes both so the
app is a clean, fully-token-driven shared-ui consumer with an accurate pin —
nothing left that would silently drift from upstream. It is deliberately small
and zero-visual-delta: every change is either a byte-equivalent token
substitution or a comment/constant refresh.

## Current state

**shared-ui upstream** (sibling checkout at `/Users/chetanjain/Desktop/code/shared-ui`):
- HEAD is `2279978` (version 0.2.0). The vendored snapshot was pinned at `ca4faf8`.
- **`design/tokens.css` is BYTE-IDENTICAL between `ca4faf8` and `2279978`** —
  verified: `git -C /Users/chetanjain/Desktop/code/shared-ui diff --quiet ca4faf8..2279978 -- design/tokens.css` exits 0. So the vendored snapshot's content and its
  pinned SHA-256 do NOT change; only the recorded provenance advances.
- Upstream added a token-versioning ritual (CHANGELOG, `scripts/tokens-since.mjs`)
  and hardened `scripts/verify-tokens.mjs` (inverse bridge check + duplicate/
  fallback guards). That script is an **authoring** gate for shared-ui itself —
  notchtap is a token *consumer*, so it is out of scope here (see Scope).

**`src/settings/base.css`** — already consumes `var(--font-sans)` at the root
(line 218: `font-family: var(--font-sans);`), but the shared mono-font selector
list still hardcodes the stack (line ~321):

```css
  .field-caption,
  .unit,
  .status-chip,
  .relaunch-note,
  .shortcut-status,
  .error-title {
    font-family: ui-monospace, "SFMono-Regular", Menlo, monospace;
  }
```

(The full selector list also includes `.soon-badge, .sidebar-meta, .section-index`
earlier in the same block — confirm the actual selector set in the live file
before editing; the hardcoded `font-family` value is the target, wherever it sits.)

shared-ui `tokens.css` owns the identical value:
```css
  --font-mono: ui-monospace, "SFMono-Regular", Menlo, monospace;
```
So replacing the literal with `var(--font-mono)` is **byte-equivalent** — zero
rendered change, pure token hygiene. `--font-sans` is already consumed; there is
no `--font-heading` consumer to add (base.css uses `--font-sans` at root, which is
correct — shared-ui's `--font-heading` defaults to `--font-sans` anyway).

**Vendored snapshot** (`vendor/shared-ui/`):
- `verify-snapshot.mjs` records `const UPSTREAM_SHA = "ca4faf8";` and
  `const PINNED_TOKENS_SHA256 = "c8416630c99a60737ff8dd9e1348b2ec771a5569501b0d5f8fbfd0ac584635c8";`.
  Because tokens.css is unchanged, **`PINNED_TOKENS_SHA256` must stay exactly as-is** —
  do NOT recompute or alter it. Only `UPSTREAM_SHA` (and the surrounding provenance
  comment) advances.
- `vendor/shared-ui/package.json` declares `"version": "0.1.0"`. Upstream is now `0.2.0`.

**Reference components** (sibling, already used as Step 4 styling references):
- `playground/src/components/ui/table.tsx` — Shortcuts table was styled against this in plan 112 Step 4.
- `playground/src/components/ui/label.tsx` — the shadcn Switch uses a `Label` for its accessible name (plan 112 Step 4).
These are reference conventions, NOT importable packages — shared-ui ships tokens only.

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Install (sibling-independent) | `npm ci` | exit 0 |
| Snapshot drift guard | `node vendor/shared-ui/verify-snapshot.mjs` | prints matching SHA-256; with sibling present, "matches exactly. No drift."; exit 0 |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Tests | `npx vitest run` | all pass (295 baseline) |
| Lint (CI gate) | `npx biome ci .` | exit 0 |
| Build | `npx vite build` | exit 0 |

## Scope

**In scope** (the only files you may modify):
- `src/settings/base.css` — swap the one hardcoded mono stack for `var(--font-mono)`.
- `vendor/shared-ui/verify-snapshot.mjs` — advance `UPSTREAM_SHA` + provenance comment ONLY.
- `vendor/shared-ui/package.json` — bump `version` to `0.2.0` to match upstream.
- `docs/TESTING_STRATEGY.md` — update the §0 test-count line ONCE, after final gates (only if the count changed).

**Out of scope** (do NOT touch):
- `vendor/shared-ui/design/tokens.css` — byte-identical to upstream; editing it would (correctly) trip the snapshot guard. Do not touch, do not recompute its SHA.
- `PINNED_TOKENS_SHA256` in `verify-snapshot.mjs` — unchanged (tokens.css unchanged).
- Any adoption of upstream's `scripts/verify-tokens.mjs` / `tokens-since.mjs` — those are shared-ui *authoring* gates; notchtap is a consumer. Explicitly deferred (see Maintenance notes).
- `src/settings/SettingsApp.tsx`, the shadcn Switch/table components, `src/components/ui/**` — Shortcuts table + Switch Label already match the reference from plan 112 Step 4; this pass only *verifies* that (Step 3), it does not re-style them.
- `src-tauri/**`, `src/overlay-card.css`, `src/styles.css`, `src/App.tsx`, overlay app/components/hooks/libs — empty diff required.

## Git workflow

- You are in an isolated worktree cut from a stale base. **FIRST**: `git reset --hard 82c36d4`, then verify (Step 0) before any edit.
- Commit per logical unit, conventional-commit style matching the repo (e.g. `refactor(settings): adopt shared-ui --font-mono token (plan 113)`).
- Do NOT push, merge, or open a PR.

## Steps

### Step 0: Base-sync and verify

Your worktree base is stale. Run `git reset --hard 82c36d4`, then HARD-VERIFY:
- `git rev-parse --short HEAD` → `82c36d4`
- `test ! -f src/settings/settings.css && echo GONE` → GONE (plan 112 deleted it)
- `wc -l src/settings/base.css` → 398
- `grep -c 'var(--font-sans)' src/settings/base.css` → ≥1 (root already tokenized)
- `grep -n 'ui-monospace, "SFMono-Regular", Menlo, monospace' src/settings/base.css` → exactly one hit (the target literal)
- `grep -n 'UPSTREAM_SHA = "ca4faf8"' vendor/shared-ui/verify-snapshot.mjs` → one hit

Then baseline all gates green: `npm ci`, `npx vitest run` (295), `npx tsc --noEmit` (0), `npx biome ci .` (0), `npx vite build` (0), and `node vendor/shared-ui/verify-snapshot.mjs` (exit 0).

If ANY verification fails, STOP and report.

**Verify**: all of the above match. → proceed.

### Step 1: Adopt `--font-mono` in base.css

In `src/settings/base.css`, replace the single hardcoded declaration
`font-family: ui-monospace, "SFMono-Regular", Menlo, monospace;` (the mono-font
selector-list block) with `font-family: var(--font-mono);`. Change ONLY that one
declaration's value — leave the selector list and every other line untouched.
Add a short inline comment noting the value now comes from shared-ui's token
(matching the file's existing comment style).

**Verify**:
- `grep -c 'ui-monospace, "SFMono-Regular", Menlo, monospace' src/settings/base.css` → `0` (the literal is gone from base.css; it now lives only in the vendored tokens.css, which is out of scope).
- `grep -c 'var(--font-mono)' src/settings/base.css` → `1`.
- `npx vite build` → exit 0; then re-run the Plan 111 preview-equivalence harness (see Test plan) → **zero delta** (the value is byte-identical, so nothing renders differently).

### Step 2: Refresh the vendored snapshot provenance

The token bytes did not change, so this is a provenance-only refresh:
- In `vendor/shared-ui/verify-snapshot.mjs`, change `const UPSTREAM_SHA = "ca4faf8";`
  to `const UPSTREAM_SHA = "2279978";`, and update the nearby comment that
  explains the pin (it currently narrates the `8e395a8 → ca4faf8` history) to note
  the snapshot is now verified current as of upstream `2279978` (v0.2.0), with
  `design/tokens.css` byte-identical across `ca4faf8..2279978` (so
  `PINNED_TOKENS_SHA256` is unchanged and still authoritative).
- Do NOT alter `PINNED_TOKENS_SHA256`.
- In `vendor/shared-ui/package.json`, bump `"version": "0.1.0"` → `"version": "0.2.0"`.

**Verify**:
- `grep -c 'c8416630c99a60737ff8dd9e1348b2ec771a5569501b0d5f8fbfd0ac584635c8' vendor/shared-ui/verify-snapshot.mjs` → still `1` (SHA-256 untouched).
- `node vendor/shared-ui/verify-snapshot.mjs` → exit 0; with the sibling present it prints "matches the vendored snapshot exactly. No drift." and reports `upstream SHA: 2279978`.
- `npm ci` with the sibling renamed aside still succeeds (snapshot resolves via `file:vendor/shared-ui`); restore the sibling immediately after. (Rename: `mv ../shared-ui ../shared-ui.aside` then `npm ci` then `mv ../shared-ui.aside ../shared-ui` — only if you have permission to; otherwise skip and note it.)

### Step 3: Verify Shortcuts table + Switch Label reference parity (no code change expected)

Confirm — do NOT re-style — that the plan 112 Step 4 work already matches the
sibling reference conventions:
- The Shortcuts table (`SettingsApp.tsx`) is a native `<table>` with `<thead>/<th>/<tbody>/<td>` semantics styled with utilities, consistent with `/Users/chetanjain/Desktop/code/shared-ui/playground/src/components/ui/table.tsx`'s class grammar.
- The shadcn Switch is named by an associated `Label` (per `label.tsx` reference), asserted by the existing `SettingsApp.test.tsx` Switch-contract tests.

If both already hold (expected), record "verified, no change" and move on. If a
genuine divergence is found that would require editing `SettingsApp.tsx` or the
components, STOP and report rather than expanding scope — that is a new plan, not
this one.

**Verify**: `npx vitest run` → the Switch-contract + table-semantics tests pass (already green from 112).

### Step 4: Final gates + test-count doc

Run the full gate suite: `npm ci`, `npx vitest run`, `npx tsc --noEmit`,
`npx biome ci .`, `npx vite build`, `node vendor/shared-ui/verify-snapshot.mjs`
— all clean. If the total vitest count changed from what `docs/TESTING_STRATEGY.md`
§0 records, update that ONE line to the observed count (per CLAUDE.md, test counts
live only in §0). If unchanged, leave the doc untouched.

**Verify**: all gates exit 0; scope diff is limited to the in-scope files
(`git diff --name-only 82c36d4..HEAD` shows only base.css, verify-snapshot.mjs,
package.json, and optionally TESTING_STRATEGY.md).

## Test plan

- No new product behavior → no new product tests required. The existing 295 must
  stay green (the font change is byte-equivalent; the pin change is not runtime).
- **Preview-equivalence harness** (Plan 111 technique, reused from the scratchpad
  at `/private/tmp/claude-501/-Users-chetanjain-Desktop-code-mac-notification-nudge/d23330e2-89b5-4fe9-8f29-80e31db32a04/scratchpad/harness/`
  and `step4work/final-head/`): re-run against the post-change build and confirm
  **zero delta** on all 8 Plan 111 preview samples and no new horizontal overflow
  across all 10 sections × 2 viewports × normal/reduced-motion. A non-zero delta
  here means the "byte-equivalent" assumption is wrong — STOP and report.
- Optionally add one focused test asserting base.css contains no raw
  `ui-monospace, "SFMono-Regular"` literal (guards against the hardcoded stack
  creeping back). Only if it fits the repo's existing CSS-assertion test style
  (see `overlayCardMirror.test.ts` for the string-scan pattern) — otherwise skip.

## Done criteria

- [ ] `src/settings/base.css` uses `var(--font-mono)`; no raw `ui-monospace, "SFMono-Regular"` literal remains in it
- [ ] `verify-snapshot.mjs` records `UPSTREAM_SHA = "2279978"`; `PINNED_TOKENS_SHA256` UNCHANGED; `node vendor/shared-ui/verify-snapshot.mjs` exits 0 (no drift vs sibling)
- [ ] `vendor/shared-ui/package.json` version is `0.2.0`
- [ ] `vendor/shared-ui/design/tokens.css` is byte-unchanged (not in the diff)
- [ ] Shortcuts table + Switch Label reference parity verified (Step 3), no code change
- [ ] `npx vitest run` / `npx tsc --noEmit` / `npx biome ci .` / `npx vite build` all clean
- [ ] Preview-equivalence harness: zero delta on all 8 samples, no new overflow
- [ ] `git diff --name-only 82c36d4..HEAD` limited to base.css, verify-snapshot.mjs, package.json (+ optionally TESTING_STRATEGY.md and one focused test); `src-tauri/**`, `src/overlay-card.css`, `src/styles.css`, `src/App.tsx` empty diff
- [ ] React/Vite/TypeScript/Vitest version ranges unchanged in package.json

## STOP conditions

Stop and report (do not improvise) if:
- Base-sync verification (Step 0) fails — wrong HEAD, settings.css still present, or the target literal is missing/duplicated.
- The `--font-mono` value in `vendor/shared-ui/design/tokens.css` is NOT byte-identical to the literal you're replacing (then it is NOT a zero-delta swap and needs review).
- The preview-equivalence harness shows ANY delta after the font change.
- `node vendor/shared-ui/verify-snapshot.mjs` reports drift after Step 2 (would mean tokens.css actually differs from the sibling — investigate, don't force the pin).
- Step 3 uncovers a real Shortcuts/Switch divergence requiring source edits.
- Any change would touch an out-of-scope file.

## Maintenance notes

- **Deferred out of this plan**: adopting shared-ui's hardened `scripts/verify-tokens.mjs`
  (inverse bridge check, duplicate/fallback guards) as a notchtap-side gate. It is
  an *authoring* invariant for the token source; notchtap only consumes tokens, and
  the SHA-256 snapshot guard already covers "did the tokens we vendored change?".
  Revisit only if notchtap starts authoring or overriding token *values* locally.
- The next shared-ui refresh: re-run `git -C ../shared-ui diff --quiet <pinned>..HEAD -- design/tokens.css`.
  If it exits 0, this is again a provenance-only bump (update `UPSTREAM_SHA`). If it
  exits 1, recompute `PINNED_TOKENS_SHA256`, re-vendor `design/tokens.css`, and
  re-review the token diff for value changes before pinning.
- Reviewer should scrutinize: that `PINNED_TOKENS_SHA256` and `tokens.css` are
  genuinely untouched (the whole point of the snapshot guard), and that the font
  swap produced zero preview delta.
