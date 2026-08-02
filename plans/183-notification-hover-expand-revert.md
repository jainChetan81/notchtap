# Plan 183: Revert notification manifest hover-expand back to keyboard-only

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat f810d58..HEAD -- src/useExitChoreography.ts src/components/StatusRailCard.test.tsx`
> If either file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: XS
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug / UX regression
- **Planned at**: commit `f810d58`, 2026-08-02
- **Spec**: `docs/superpowers/specs/2026-08-02-notification-hover-expand-revert-design.md`

## Why this matters

A 2026-08-02 change made a showing notification card's manifest open
merely by resting the cursor on it (`slot.expanded || hovered`), instead
of only via the `⌃⇧N` keyboard toggle (`slot.expanded` alone). The
operator who requested that change has since reversed the decision: it
reads as unwanted — the manifest pops open on any incidental hover,
overriding the deliberate keyboard toggle whenever the cursor happens to
sit on the card. This plan reverts exactly that one boolean expression.
Every OTHER hover-driven behavior in the app (TTL-bar hover-pause,
AgentBoard's own hero↔list hover-expand, the idle weather/scorecard/media
peek, the bare-notch rail reveal) is explicitly OUT of scope and must not
change.

## Current state

`src/useExitChoreography.ts:76-87` (the block to revert):

```ts
  // 2026-08-02 (operator request, replacing the deleted hover "breathe"):
  // a LIVE showing card also counts as expanded while hovered — hovering
  // a notification opens its manifest (and grows the shell to
  // `.expanded`'s width) instead of zooming the whole shell. Precedent:
  // App.tsx already passes `expanded={hovered}` to AgentBoard (plan 142)
  // off this same `hover-changed`-sourced boolean; this gives the
  // notification cards the same behavior. Deliberately only on the LIVE
  // (`showing`) branch: during the exit-choreography window the
  // `renderedShowing` fallback below must keep serving the frozen
  // outgoing value exactly as before, or a hover held through an exit
  // would re-expand a card that's already collapsing.
  const expanded = showing ? slot.expanded || hovered : renderedShowing && renderedSlot.expanded;
```

`src/components/StatusRailCard.test.tsx:1940-1966` (the tests pinning
the behavior being reverted):

```tsx
  // 2026-08-02 (operator request): hover-expand REPLACES the deleted
  // hover "breathe" scale — hovering a live showing card now opens its
  // manifest and grows the shell to `.expanded`'s width instead of
  // zooming the whole shell. The OR itself lives in
  // useExitChoreography.ts's `expanded`; these pin it end-to-end through
  // both consumers of that one value (the shell class AND the manifest's
  // own open state). Same precedent as App.tsx's `expanded={hovered}` on
  // AgentBoard (plan 142).
  describe("hover-expand on a showing card (2026-08-02)", () => {
    const COLLAPSED: SlotState = { ...GOAL, expanded: false };

    it("renders .expanded and an open manifest while hovered, even with slot.expanded false", () => {
      const { container } = render(<StatusRailCard slot={COLLAPSED} hovered={true} />);
      expect(container.querySelector(".card-assembly.expanded")).not.toBeNull();
      const wrap = container.querySelector(".manifest-wrap");
      expect(wrap?.classList.contains("expanded")).toBe(true);
      expect(wrap?.getAttribute("aria-hidden")).toBe("false");
    });

    it("stays collapsed — no .expanded, manifest closed — while not hovered", () => {
      const { container } = render(<StatusRailCard slot={COLLAPSED} hovered={false} />);
      expect(container.querySelector(".card-assembly.expanded")).toBeNull();
      const wrap = container.querySelector(".manifest-wrap");
      expect(wrap?.classList.contains("expanded")).toBe(false);
      expect(wrap?.getAttribute("aria-hidden")).toBe("true");
    });
  });
```

`GOAL` is an existing `SlotState` test fixture defined earlier in the
same file — locate it with `grep -n "^const GOAL" src/components/StatusRailCard.test.tsx`.

## Commands you will need

Run from repo root.

| Purpose | Command | Expected on success |
|---|---|---|
| Frontend tests | `npx vitest run` | all pass |
| Type check | `npx tsc --noEmit` | exit 0 |
| Lint | `npx biome ci .` | exit 0 |

## Scope

**In scope** (the only files you should modify):
- `src/useExitChoreography.ts` (the `expanded` expression + its comment,
  lines ~76-87)
- `src/components/StatusRailCard.test.tsx` (the `describe("hover-expand
  on a showing card (2026-08-02)", ...)` block, lines ~1940-1966)

**Out of scope** (do NOT touch, even though they look related):
- `App.tsx:305`'s `expanded={hovered}` on `AgentBoard` — different
  component, not part of this revert.
- `TtlBar.tsx`'s `hoverPaused` — unrelated mechanism.
- `IdleHoverPeek.tsx` — unrelated, stays hover-driven.
- `card-chrome.css`'s `.bare.hovered` rail-reveal rules — unrelated repaint.
- Anything in `src-tauri/` — this is a pure frontend revert.

## Git workflow

- Branch: `fix/notification-hover-expand-revert`
- Commit style: conventional, e.g. `fix(overlay): manifest expand is keyboard-only again`
- Open a PR when done.

## Steps

### Step 1: Revert the boolean expression and its comment

In `src/useExitChoreography.ts`, replace the excerpt in "Current state"
above with:

```ts
  // 2026-08-02: hover-expand (`slot.expanded || hovered`) was tried and
  // reverted the same day — it made the manifest pop open on any
  // incidental hover, overriding the deliberate `⌃⇧N` keyboard toggle
  // whenever the cursor happened to rest on the card. `expanded` is
  // keyboard-only again. This is now a DELIBERATE difference from
  // `App.tsx`'s `expanded={hovered}` on `AgentBoard` (plan 142), not an
  // oversight — do not "fix" this file to match that one.
  const expanded = showing ? slot.expanded : renderedShowing && renderedSlot.expanded;
```

**Verify**: `npx tsc --noEmit` → exit 0.

### Step 2: Update the tests to assert the reverted behavior

Replace the `describe("hover-expand on a showing card (2026-08-02)",
...)` block in `StatusRailCard.test.tsx` with:

```tsx
  // 2026-08-02: hover-expand was tried and reverted the same day (see
  // useExitChoreography.ts's own comment on `expanded`). These pin the
  // reverted behavior: hovering alone must never open the manifest or
  // add `.expanded` — only `slot.expanded` (the `⌃⇧N` keyboard toggle)
  // does.
  describe("manifest expand is keyboard-only, not hover-driven (2026-08-02 revert)", () => {
    const COLLAPSED: SlotState = { ...GOAL, expanded: false };
    const EXPANDED: SlotState = { ...GOAL, expanded: true };

    it("stays collapsed while hovered if slot.expanded is false", () => {
      const { container } = render(<StatusRailCard slot={COLLAPSED} hovered={true} />);
      expect(container.querySelector(".card-assembly.expanded")).toBeNull();
      const wrap = container.querySelector(".manifest-wrap");
      expect(wrap?.classList.contains("expanded")).toBe(false);
      expect(wrap?.getAttribute("aria-hidden")).toBe("true");
    });

    it("stays collapsed while not hovered if slot.expanded is false", () => {
      const { container } = render(<StatusRailCard slot={COLLAPSED} hovered={false} />);
      expect(container.querySelector(".card-assembly.expanded")).toBeNull();
    });

    it("stays expanded regardless of hover if slot.expanded is true", () => {
      const { container: hoveredCase } = render(<StatusRailCard slot={EXPANDED} hovered={true} />);
      expect(hoveredCase.querySelector(".card-assembly.expanded")).not.toBeNull();

      const { container: notHoveredCase } = render(<StatusRailCard slot={EXPANDED} hovered={false} />);
      expect(notHoveredCase.querySelector(".card-assembly.expanded")).not.toBeNull();
    });
  });
```

**Verify**: `npx vitest run StatusRailCard` → all pass (this includes
confirming the new "stays expanded regardless of hover" case, the
keyboard-driven path this revert restores as the only path).

### Step 3: Full suite + full verification

Run all three commands from "Commands you will need". All must pass —
this confirms no other test anywhere in the suite depended on the
`|| hovered` behavior (the spec's own escape hatch flagged this as
unconfirmed beyond `StatusRailCard.test.tsx`).

### Step 4: Commit and open PR

```bash
git add src/useExitChoreography.ts src/components/StatusRailCard.test.tsx
git commit -m "fix(overlay): manifest expand is keyboard-only again"
git push -u origin fix/notification-hover-expand-revert
gh pr create --title "fix(overlay): manifest expand is keyboard-only again" --body "Implements docs/superpowers/specs/2026-08-02-notification-hover-expand-revert-design.md. Reverts the 2026-08-02 slot.expanded || hovered change — the manifest now opens only via the ⌃⇧N keyboard toggle, matching the pre-2026-08-02 behavior. Every other hover-driven feature (TTL-bar hover-pause, AgentBoard hero/list, idle peek, rail reveal) is untouched."
```

## Test plan

- The three rewritten tests in Step 2 fully replace the two reverted
  ones and add explicit coverage for the `slot.expanded: true` +
  hover-doesn't-matter case, which the old test block never covered.

## Done criteria

- [ ] `npx vitest run` exits 0
- [ ] `npx tsc --noEmit` exits 0
- [ ] `npx biome ci .` exits 0
- [ ] `grep -n "|| hovered" src/useExitChoreography.ts` → no match
- [ ] No files outside the in-scope list modified (`git status`)
- [ ] `plans/README.md` status row updated

## STOP conditions

- `useExitChoreography.ts`'s `expanded` line no longer matches the
  excerpt above (a concurrent change landed) — re-diff before proceeding.
- Any OTHER test file in the suite (beyond `StatusRailCard.test.tsx`)
  fails after Step 1 — this means something else in the codebase
  depended on the `|| hovered` behavior; STOP and report which test,
  rather than changing its assertion without understanding why it was
  written that way.

## Maintenance notes

- If a future request re-adds hover-expand to notification cards,
  Step 1's replacement comment explains exactly why it was reverted —
  read it before re-adding, so the same "overrides the keyboard toggle
  on incidental hover" complaint doesn't resurface unaddressed.
- On-hardware check (operator-owed): hover a showing card without
  pressing `⌃⇧N` and confirm the manifest stays closed; press `⌃⇧N` and
  confirm it opens regardless of hover.
