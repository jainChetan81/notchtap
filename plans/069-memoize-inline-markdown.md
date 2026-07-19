# Plan 069: Memoize `renderInlineMarkdown` so unrelated re-renders don't re-tokenize card bodies

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat f6c2f46..HEAD -- src/components/StatusRailCard.tsx src/components/Manifest.tsx src/lib/markdown.tsx`
> If any of these changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: perf
- **Planned at**: commit `f6c2f46`, 2026-07-20

## Why this matters

`renderInlineMarkdown` (`src/lib/markdown.tsx:61`) is called unmemoized
directly in two render paths: `StatusRailCard.tsx:141` (the card body)
and `Manifest.tsx:89` (the expanded-manifest message body). Both
components re-render on any prop change from their parent, including
ones unrelated to the card's own content — e.g. the ambient
football/weather idle-rail chip updating on its own poll cadence
(`status-state` events, independent of whichever card happens to be
visible right now) causes `App.tsx` state to change, which can cascade
into `StatusRailCard` re-rendering even when `slot.body` itself hasn't
changed. Every such re-render re-tokenizes the same markdown string from
scratch.

This app is meant to be cheap while idle (see the two prior idle-cost-cut
plans, 015/036's deadline-based heartbeat and 018's compositor-only news
shader) — an unmemoized parse on a hot re-render path works against that
same goal, even though it's a small, non-blocking cost per call rather
than a correctness issue.

## Current state

- `src/lib/markdown.tsx:61` — the function signature (read the full
  function body yourself; it's a pure `string -> ReactNode` tokenizer,
  no external state, so it's safe to memoize purely on its `text` input):

  ```tsx
  export function renderInlineMarkdown(text: string): ReactNode {
  ```

- `src/components/StatusRailCard.tsx:141` — call site:

  ```tsx
  <div className="body">{renderInlineMarkdown(slot.body)}</div>
  ```

- `src/components/Manifest.tsx:2,89` — import and call site:

  ```tsx
  import { renderInlineMarkdown } from "../lib/markdown";
  // ...
  <div className="detail-value message">{renderInlineMarkdown(body)}</div>
  ```

- Repo convention: this codebase already uses `React.useMemo` elsewhere
  for exactly this "expensive pure computation keyed on a prop" shape —
  find an existing example via
  `grep -rn "useMemo" src/` and match its import style
  (`import { useMemo } from "react"` vs `React.useMemo`) and formatting.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Frontend tests | `npx vitest run` | all pass (baseline at planning time: 112) |
| Lint/format | `npx biome ci .` | exit 0 |
| Build | `npx vite build` | exit 0 |

## Scope

**In scope**:
- `src/components/StatusRailCard.tsx`
- `src/components/Manifest.tsx`

**Out of scope**:
- `src/lib/markdown.tsx` itself — the tokenizer function doesn't need to
  change; the fix is memoizing its call sites, not the function.
- Any other component — only these two call `renderInlineMarkdown`
  (confirmed via `grep -rn "renderInlineMarkdown" src/`).

## Steps

### Step 1: Memoize the `StatusRailCard.tsx` call site

Wrap the call in `useMemo`, keyed on `slot.body` (the only input that
actually changes what gets rendered):

```tsx
const bodyContent = useMemo(() => renderInlineMarkdown(slot.body), [slot.body]);
// ...
<div className="body">{bodyContent}</div>
```

Add the `useMemo` import if not already present in this file (check
existing imports first).

**Verify**: `npx tsc --noEmit` → exit 0.

### Step 2: Memoize the `Manifest.tsx` call site

Same pattern, keyed on `body` (the local variable/prop already used in
the existing call):

```tsx
const messageContent = useMemo(() => renderInlineMarkdown(body), [body]);
// ...
<div className="detail-value message">{messageContent}</div>
```

Check whether `Manifest.tsx` calls `renderInlineMarkdown` in more than
one place (the audit found one call site at line 89, but re-confirm via
`grep -n "renderInlineMarkdown" src/components/Manifest.tsx` — if a
second call site exists elsewhere in the same component for a different
prop, apply the same memoization pattern there too, keyed on that call's
own input).

**Verify**: `npx tsc --noEmit` → exit 0.

### Step 3: Full frontend suite + lint + build

**Verify**:
- `npx vitest run` → all pass, same count as baseline (this is a
  render-optimization change, not a behavior change — no test count
  should shift)
- `npx biome ci .` → exit 0 (or the same pre-existing failures already
  tracked in this repo, if any — confirm you haven't added a NEW
  failure; compare against a clean `git stash` run if unsure)
- `npx vite build` → exit 0

## Test plan

- No new tests required — this is a pure performance optimization with
  no observable behavior change (the rendered output is identical,
  `useMemo` only skips redundant recomputation). The existing tests for
  `StatusRailCard.tsx` and `Manifest.tsx` (which assert rendered content)
  already cover correctness of what gets displayed; if either test file
  breaks after this change, that's a signal the memoization dependency
  array is missing an input, not that new tests are needed.
- If you want defense-in-depth: add one test asserting that
  `StatusRailCard` re-rendering with an unrelated prop change (e.g. a
  `status` update) doesn't produce a new `ReactNode` reference for the
  body — but this is optional given React's memoization is an
  implementation detail tests don't typically assert on directly; skip
  it unless the existing test file already has a precedent for this kind
  of referential-stability assertion.
- Verification: `npx vitest run` → all pass, same count as baseline.

## Done criteria

- [ ] `npx tsc --noEmit` exits 0
- [ ] `npx vitest run` exits 0, same test count as baseline (112 at planning time — re-confirm live count first)
- [ ] `npx biome ci .` exits 0 (or matches the pre-existing failure baseline exactly, no new failures)
- [ ] `npx vite build` exits 0
- [ ] `grep -n "useMemo" src/components/StatusRailCard.tsx src/components/Manifest.tsx` shows the new memoization in both files
- [ ] No files outside `src/components/StatusRailCard.tsx` and `src/components/Manifest.tsx` modified (`git status`)
- [ ] `plans/README.md` status row for 069 updated

## STOP conditions

- The call sites at `StatusRailCard.tsx:141` / `Manifest.tsx:89` don't
  match the excerpts above (drift since planning) — re-locate via grep
  and adjust.
- Adding `useMemo` causes any existing test to fail — this would mean
  the dependency array is missing something that actually varies
  independently of `slot.body`/`body`; investigate rather than widening
  the dependency array blindly (a wrong wide dependency defeats the
  point of the memoization).

## Maintenance notes

- If `renderInlineMarkdown` ever gains a second parameter (e.g. a
  formatting-options object), the `useMemo` dependency arrays in both
  call sites need to include it too, or the memoization will serve stale
  output when only the new parameter changes.
