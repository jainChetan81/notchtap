# Plan 114: Overlay window adopts shared-ui tokens (fonts + easing)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on. If a
> STOP condition occurs, stop and report — do not improvise. Do NOT push,
> merge, or open a PR — a reviewer merges. Do NOT edit `plans/README.md` — the
> reviewer maintains the index.
>
> **Drift check (run first)**: `git diff --stat 2c79e36..HEAD -- src/main.tsx src/styles.css src/overlay-card.css`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S–M
- **Risk**: MED
- **Depends on**: none (settings window already imports tokens.css; this extends the same contract to the overlay). Independent of plans 115/116/117.
- **Category**: tech-debt / migration
- **Planned at**: commit `2c79e36`, 2026-07-22

## Why this matters

The **overlay window never imports shared-ui's `tokens.css`** — only the
settings window does. As a result the overlay physically cannot reference
`--font-sans`, `--font-mono`, or `--ease-notchtap`, so all three were inlined as
raw literals: the body font stack (`styles.css:17`) has already **drifted** from
shared-ui (it is missing `"Helvetica Neue"`), the mono stack (`overlay-card.css:709`)
is a byte-exact duplicate, and the signature overlay easing
`cubic-bezier(0.22, 1, 0.36, 1)` is hand-copied **10×**. Wiring the token import
into the overlay closes all three at once and stops the overlay's typography and
motion feel from silently diverging from the design system. This is the keystone
of the token-hygiene sweep.

## Current state

- `src/main.tsx` is the overlay entry. Its CSS imports (lines 8–9) are:
  ```ts
  import "./overlay-card.css";
  import "./styles.css";
  ```
  There is **no** `tokens.css` import. (Contrast `src/settings/main.tsx`, which
  imports `./base.css` — and base.css imports `@chetanjain/shared-ui/design/tokens.css`.)
- `src/styles.css:17` — `body { font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", sans-serif; }`.
  shared-ui `--font-sans` (`vendor/shared-ui/design/tokens.css:60`) is
  `-apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue", sans-serif`
  — same stack **plus `"Helvetica Neue"`** (the drift).
- `src/overlay-card.css:709` — `.detail-value code { font-family: ui-monospace, "SFMono-Regular", Menlo, monospace; }`.
  shared-ui `--font-mono` (`tokens.css:61`) is byte-identical to this.
- `src/overlay-card.css` — `cubic-bezier(0.22, 1, 0.36, 1)` appears **10× as live
  declarations** (lines 510, 516, and 8 more — confirm with grep). shared-ui
  `--ease-notchtap` (`tokens.css:163`) is `cubic-bezier(.22, 1, .36, 1)` —
  numerically identical (`.22` == `0.22`).

**Key safety facts established during planning** (verify still true):
- `tokens.css` adds **only custom properties** to `:root` — it sets no direct
  `background`/`color`/`font-family`/`color-scheme` on `:root` or `body`, so
  importing it into the overlay does not restyle the overlay chrome directly.
  (Verify: `grep -nE '^\s*(background|color|font|color-scheme|margin|padding)\s*:' vendor/shared-ui/design/tokens.css` → no `:root`-level hits.)
- The **only** custom-property NAME that both the overlay CSS and `tokens.css`
  define/reference is `--accent`. The overlay sets `--accent` per-priority under
  `.card-assembly.low/.medium/.high` (`overlay-card.css:245/250/255`). tokens.css
  defines a `:root --accent` (a muted gray, `oklch(0.256 …)`). Inside
  `.card-root .card-assembly.{priority}` the scoped value wins as designed. The
  RISK is any overlay element that reads `var(--accent)` but is NOT under a
  priority scope: today that reads as *undefined*; after the import it resolves
  to shared-ui's gray. The 11 `var(--accent)`/`var(--accent-soft)` reader lines
  are: `overlay-card.css:431,576,577,708,715,716,775,779,780,807,873`. Step 3
  audits their scoping.

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Install | `npm ci` | exit 0 |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Tests | `npx vitest run` | 295 pass |
| Lint (CI gate) | `npx biome ci .` | exit 0 |
| Build | `npx vite build` | exit 0 |

## Scope

**In scope** (only files you may modify):
- `src/main.tsx` — add the `tokens.css` import (first, before the others).
- `src/styles.css` — swap the body font stack to `var(--font-sans)`.
- `src/overlay-card.css` — swap the mono stack to `var(--font-mono)`; swap the 10 easing literals to `var(--ease-notchtap)`.
- `src/entryImportOrder.test.ts` — extend the existing import-order assertions to pin `tokens.css` first in the overlay entry (this file already guards `main.tsx`'s CSS order; keep its invariants coherent).

**Out of scope** (do NOT touch):
- Any `overlay-card.css` **color literal** (the `#ff6b57`/`rgba(255,107,87,…)` celebration/red-alert art, the `.low/.medium/.high` `--accent` palette). That palette is the deliberate wire-driven art — audited clean, off-limits.
- The TtlBar rAF fill, celebration keyframes, day/night gradients — art.
- `src/settings/**` — the settings window already has tokens; this plan is overlay-only.
- `src-tauri/**`.

## Git workflow

- You are in an isolated worktree cut from a stale base. **FIRST**: `git reset --hard 2c79e36`, then verify Step 0 before editing.
- Commit per logical unit, conventional-commit style (e.g. `refactor(overlay): import shared-ui tokens.css, adopt --font-* and --ease-notchtap (plan 114)`).
- Do NOT push/merge/PR.

## Steps

### Step 0: Base-sync + verify + baseline
`git reset --hard 2c79e36`, then confirm: HEAD=`2c79e36`; `src/main.tsx` imports overlay-card.css then styles.css with no tokens.css; `styles.css:17` has the drifted sans stack; `overlay-card.css:709` mono; `grep -c 'cubic-bezier(0.22, 1, 0.36, 1)' src/overlay-card.css` == 10; the `tokens.css` no-`:root`-bleed grep is empty. Then baseline all gates green (`npm ci`, vitest 295, tsc 0, biome 0, vite build 0). Any mismatch → STOP.

### Step 1: Import tokens.css into the overlay entry
In `src/main.tsx`, add `import "@chetanjain/shared-ui/design/tokens.css";` as the FIRST css import (before `overlay-card.css` and `styles.css`), so the tokens' `:root` is established before the overlay's own stylesheets cascade. Match how `src/settings/base.css` imports the same package.

**Verify**: `npx vite build` → exit 0; the built overlay bundle now includes the token custom properties (spot-check the build output or a computed-style probe).

### Step 2: Adopt `--font-sans` and `--font-mono`
- `src/styles.css:17`: replace the hardcoded body font stack with `font-family: var(--font-sans);` (add a brief comment noting the token source, matching file style).
- `src/overlay-card.css:709`: replace the hardcoded mono stack with `font-family: var(--font-mono);`.

**Verify**:
- `grep -c '"SF Pro Text"' src/styles.css` → 0 (literal gone from styles.css).
- `grep -c 'ui-monospace, "SFMono-Regular"' src/overlay-card.css` → 0.
- Note: the sans swap is an INTENTIONAL visual change (it adds `"Helvetica Neue"` back into the fallback chain) — record it for the operator screenshot; it is not a regression.

### Step 3: Audit `--accent` reader scoping, THEN adopt `--ease-notchtap`
First, for each of the 11 `var(--accent)`/`var(--accent-soft)` reader lines
(`overlay-card.css:431,576,577,708,715,716,775,779,780,807,873`), determine
whether its selector is scoped under `.card-assembly.low/.medium/.high` (or a
descendant of one). Produce a short table: line → selector → priority-scoped?
(yes/no). Any reader that is NOT priority-scoped will change from *undefined* to
shared-ui gray after Step 1 — list those explicitly in your report for the
operator's screenshot check (do NOT try to "fix" them; undefined→gray is the
expected consequence of adopting the token system, and the operator judges the
visual).

Then swap the easing: replace all 10 live `cubic-bezier(0.22, 1, 0.36, 1)`
declarations in `overlay-card.css` with `var(--ease-notchtap)`. Leave the one
occurrence inside a comment (line ~1432) as-is.

**Verify**:
- `grep -c 'cubic-bezier(0.22, 1, 0.36, 1)' src/overlay-card.css` → 1 (only the comment remains) — OR 0 if you also updated the comment; either is acceptable, state which.
- `grep -c 'var(--ease-notchtap)' src/overlay-card.css` → 10.

### Step 4: Extend the import-order test + final gates
In `src/entryImportOrder.test.ts`, add an assertion that the overlay `main.tsx`
imports `tokens.css` (or the shared-ui token entry) before `overlay-card.css`.
Follow the file's existing `importOrderIndex` pattern. Then run all gates.

**Verify**: `npx vitest run` (all pass incl. the new assertion), `npx tsc --noEmit` (0), `npx biome ci .` (0), `npx vite build` (0). `git diff --name-only 2c79e36..HEAD` shows only the in-scope files.

## Test plan

- Extend `src/entryImportOrder.test.ts` with the overlay tokens.css-first assertion (Step 4). Model after its existing settings-side order tests.
- No new product tests: the font/easing swaps are byte-or-near-equivalent and the mechanism (a CSS import) isn't unit-testable beyond import order.
- **Preview-equivalence is NOT a valid gate here** — this plan intentionally changes the overlay body font fallback and potentially some unscoped `--accent` reads. The real acceptance is the operator's WKWebView screenshot pass (below).

## Done criteria

- [ ] `src/main.tsx` imports `tokens.css` first; `entryImportOrder.test.ts` pins it
- [ ] `styles.css` uses `var(--font-sans)`; no `"SF Pro Text"` literal remains there
- [ ] `overlay-card.css` uses `var(--font-mono)` (no mono literal) and `var(--ease-notchtap)` ×10 (no live `cubic-bezier(0.22,1,0.36,1)`)
- [ ] Step 3 `--accent` scoping table produced; any non-priority-scoped reader listed for operator review
- [ ] vitest 295+ / tsc 0 / biome 0 / vite build 0
- [ ] `git diff --name-only` limited to the 4 in-scope files; `src-tauri/**`, `src/settings/**` empty diff
- [ ] React/Vite/TS/Vitest ranges unchanged

## STOP conditions

- Base-sync/Step-0 verification fails, or the easing count isn't 10, or tokens.css turns out to set `:root`-level `background`/`color`/`font` directly (bleed risk changes the plan).
- A gate fails twice after a reasonable fix.
- Step 3 reveals a `var(--accent)` reader whose undefined→gray change you believe is a genuine regression rather than a cosmetic shift — STOP and report for a design decision rather than guessing.
- Any out-of-scope file would need to change.

## Operator-owned acceptance (PENDING — not executor's job)

WKWebView screenshots of the **overlay** in its states (idle, compact, showing,
each priority low/medium/high, celebration, idle-peek) — confirm: the body text
renders correctly with the restored `"Helvetica Neue"` fallback; the easing feel
is unchanged; and any non-priority-scoped `--accent` element (from Step 3's
table) looks acceptable in shared-ui gray. Report these PENDING; never fake them.

## Maintenance notes

- After this lands, the overlay and settings windows share the same token source
  — a shared-ui token refresh (plan 113's flow) now reaches both.
- Reviewer should scrutinize Step 3's scoping table and the operator screenshot
  result for the `--accent` fallback, since that's the only behavioral risk.
- Future overlay CSS should reference tokens (`var(--font-*)`, `var(--ease-notchtap)`,
  radius tokens) rather than re-inlining literals.
