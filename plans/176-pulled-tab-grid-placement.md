# Plan 176: Place the pulled-tab below-block in the grid row it was designed for

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 7ca82d5..HEAD -- src/components/StatusRailCard.tsx src/overlay/card-chrome.css src/components/TabBelowBlock.tsx`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: S
- **Risk**: LOW
- **Depends on**: 175 (soft — same `card-chrome.css:243` rule: 175 changes its value, this plan widens its selector; land 175 first and reconcile by reading)
- **Category**: bug
- **Planned at**: commit `7ca82d5`, 2026-08-02

## Why this matters

Plan 171's "pull" surface — click an icon (or `prefix+N`) and the selected
tab's card appears below the notch — mounts `TabBelowBlock` inside an
animated wrapper that has **no grid placement**. The `.card-assembly` shell
is a CSS grid (`1fr auto 1fr` columns: flank, cutout, flank), and the rule
that places content cards spans them across row 2:

`src/overlay/card-chrome.css:782-784`
```css
.card-root .below-block {
  grid-column: 1 / -1;
  grid-row: 2;
```

Grid placement only applies to **direct grid items**. The pulled-tab
wrapper `motion.div` is the direct child; the `.below-block` each tab
branch renders is a grandchild. So the wrapper is auto-placed into the
next free cell — the 85px-wide left-flank column — and the pulled card
renders squeezed into the flank instead of spanning the card. In the
notch resting state it is worse: the `.bare` shell only re-widens for the
ambient peek (`:has(.idle-peek)`), never for a tab block, so the column
the pulled card lands in can be effectively zero-width. The feature's
output surface is visually broken in its default mount.

## Current state

`src/components/StatusRailCard.tsx:1085-1113` — the pulled-tab mount. The
wrapper `motion.div` animates opacity/y and carries **no `style` and no
grid-placing class**:

```tsx
<AnimatePresence mode="wait" initial={false}>
  {pulledTab !== null && (
    <motion.div
      key={pulledTab}
      initial={{ opacity: 0, y: -4 }}
      animate={{ opacity: 1, y: 0, transition: { duration: ROTATION_ENTER_MS / 1000, ease: NOTCHTAP_EASE } }}
      exit={{ opacity: 0, y: -2, transition: { duration: ROTATION_EXIT_MS / 1000, ease: NOTCHTAP_EASE } }}
    >
      <TabBelowBlock
        selected={pulledTab}
        status={status}
        agentSessions={agentSessions}
        agentCapturedAtMs={agentCapturedAtMs}
      />
    </motion.div>
  )}
</AnimatePresence>
```

`TabBelowBlock`'s three branches each render their own `.below-block`
root (e.g. `src/components/AgentBelowBlock.tsx:78-81` renders
`className={"below-block agent-origin ..."}`), which is therefore a
grandchild of `.card-assembly`.

Contrast the showing-card path a few lines down, which solved this
deliberately — `src/components/StatusRailCard.tsx:1134-1141`: a static
wrapper with `style={{ display: "contents" }}` so the animating
`motion.div` that CARRIES `className={belowBlockClass}` (i.e. the
`.below-block` itself) stays a direct grid item. That exact split —
non-box wrapper outside, the grid item is the element with the class —
is the house pattern.

`src/overlay/card-chrome.css:242-245` — the only rule that re-widens the
`.bare` shell for hover-mounted content keys on `.idle-peek` specifically:

```css
.card-root .card-assembly.bare:has(.idle-peek) {
  --cw: ...;
  transition: width var(--reveal-ms, 260ms) var(--ease-notchtap);
}
```

`IdleHoverPeek` returns a root carrying `below-block idle-peek` classes,
so `:has(.below-block)` is a strict superset of `:has(.idle-peek)`.

Note for the fix: `display: contents` on the **animating** `motion.div`
would be wrong — an element with `display: contents` generates no box, so
its own opacity/transform animations would do nothing. The wrapper that
animates must be the grid item.

## Commands you will need

Run web commands from the repo root.

| Purpose | Command | Expected on success |
|---|---|---|
| Frontend tests | `npx vitest run` | all pass |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Lint gate | `npx biome ci .` | exit 0 |

(No rust changes → no cargo run required; run it anyway at the end if you
touched nothing else: it must stay green.)

## Scope

**In scope** (the only files you should modify):
- `src/components/StatusRailCard.tsx`
- `src/overlay/card-chrome.css`
- `src/components/StatusRailCard.test.tsx` (add assertions)
- `docs/TESTING_STRATEGY.md` §0 (counts, if changed)

**Out of scope** (do NOT touch, even though they look related):
- `src/components/TabBelowBlock.tsx` and the three per-tab blocks — their
  own `.below-block` roots are correct; the wrapper is the problem.
- The showing-card wrapper at `StatusRailCard.tsx:1134-1141` — already
  correct; it is the pattern, not a target.
- Animation values (`ROTATION_ENTER_MS`, eases) — motion was reviewed
  separately; keep them byte-identical.

## Git workflow

- Branch: `advisor/176-pulled-tab-grid-placement`
- Commit style: conventional, e.g. `fix(tabs): span the pulled below-block across the card grid`
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Give the pulled-tab wrapper its grid placement

In `src/overlay/card-chrome.css`, next to the `.below-block` placement rule
(`:782-784`), add a sibling rule:

```css
.card-root .tab-below-slot {
  grid-column: 1 / -1;
  grid-row: 2;
  position: relative;
  box-sizing: border-box;
}
```

(mirror whatever additional declarations of the `.below-block` rule are
placement-relevant — read the full rule before copying; do not duplicate
its padding/visual styling, only placement/box behaviour).

In `src/components/StatusRailCard.tsx`, add
`className="tab-below-slot"` to the pulled-tab wrapper `motion.div`
(quoted above). Do not add `display: contents` anywhere on this path —
see the note in "Current state".

Add a short comment on the wrapper explaining the constraint: the
animating wrapper must be the grid item, because the `.below-block` it
contains is a grandchild the grid cannot place.

**Verify**: `npx tsc --noEmit` → exit 0; `npx vitest run StatusRailCard`
→ existing tests pass.

### Step 2: Re-widen the bare shell for any hover-mounted block

In `src/overlay/card-chrome.css`, change the `:242` selector from

```css
.card-root .card-assembly.bare:has(.idle-peek) {
```

to

```css
.card-root .card-assembly.bare:has(.below-block) {
```

Keep the declaration block exactly as it is at that point in history (if
plan 175 landed first, that value now contains the `--present-icons`
formula — keep 175's value, change only the selector). Update the rule's
preceding comment to say it now covers both the ambient peek and the
pulled-tab blocks, and why the superset is safe (`.idle-peek` carries
`below-block` too).

**Verify**: `grep -n ":has(.below-block)" src/overlay/card-chrome.css` →
1 match at this rule; `grep -n ":has(.idle-peek)" src/overlay/card-chrome.css`
→ any remaining matches are in comments or other rules you did NOT change
(there are other `:has(.idle-peek)` usages elsewhere in `src/overlay/` —
leave them; only this width rule widens).

### Step 3: Pin the structure with tests

In `src/components/StatusRailCard.test.tsx`, find the existing pulled-tab
tests (search for `pulledTab` / `tab-selection` / the `agent-below-block`
testid — the file has a block around line ~2872-2972 exercising the
selection prop). Add assertions:

- when a tab with content is pulled, the element with
  `data-testid="agent-below-block"` (or the media equivalent) has a parent
  (or ancestor chain to `.card-assembly`) whose wrapper element carries
  class `tab-below-slot`;
- the wrapper is a **direct child** of the `.card-assembly` element.

Model the query style on the neighbouring tests in the same file.

**Verify**: `npx vitest run StatusRailCard` → all pass including new
assertions.

### Step 4: Full gates + counts

Run the three web commands; update `docs/TESTING_STRATEGY.md` §0 if the
vitest count changed (recount live, don't hand-adjust).

**Verify**: all green.

## Test plan

- Extended `src/components/StatusRailCard.test.tsx`: structural assertions
  from Step 3 (jsdom can assert DOM structure and class names; it cannot
  assert computed grid layout — the class + direct-child assertions are the
  machine-checkable proxy).
- Existing suites stay green.

## Done criteria

- [ ] `npx vitest run` exits 0, including the new structural assertions
- [ ] `npx tsc --noEmit` and `npx biome ci .` exit 0
- [ ] `grep -n "tab-below-slot" src/components/StatusRailCard.tsx src/overlay/card-chrome.css` → 1 match in each
- [ ] The `:242` width rule keys on `:has(.below-block)`
- [ ] No files outside the in-scope list are modified (`git status`)
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- The pulled-tab wrapper at `StatusRailCard.tsx:1085-1113` no longer
  matches the excerpt (another fix may have landed).
- Widening the `:has()` selector changes the width of the SHOWING card's
  state in any test (`.bare` should never co-occur with a showing card —
  if a test proves otherwise, the state machine drifted; report it).
- You find a second unplaced wrapper on another mount path — report it
  rather than fixing it silently.

## Maintenance notes

- Anyone adding a fourth tab branch to `TabBelowBlock` inherits correct
  placement for free — but a NEW mount path for below-blocks must either
  put `.below-block` on its own direct child or reuse `tab-below-slot`.
- Reviewer focus: confirm the animated wrapper still fades/slides (the
  placement class must not affect the motion values), and that plan 175's
  `--cw` value at the `:242` rule survived the selector edit byte-for-byte.
- On-hardware check owed (operator): pull each tab in notch mode and HUD
  mode; the card should span the full shell width in both.
