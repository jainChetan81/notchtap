# Revert notification manifest hover-expand

**Date:** 2026-08-02
**Status:** approved, pending implementation plan

## Problem

On 2026-08-02, `useExitChoreography.ts:87` changed the notification
manifest's `expanded` computation from `slot.expanded` (driven solely by
the `⌃⇧N` keyboard toggle, `toggle_manual_expand` in
`src-tauri/src/lib.rs`) to `slot.expanded || hovered` — merely resting the
cursor on a showing card now force-opens its manifest and grows the shell,
independent of the keyboard toggle. This was an operator request at the
time (replacing a deleted hover "breathe" scale effect), but on reflection
it reads as unwanted: the manifest now pops open on any incidental hover,
which overrides the deliberate keyboard-driven expand/collapse toggle
whenever the cursor happens to be over the card.

## Non-goals — explicitly NOT in scope

Every other hover-driven behavior in the app stays exactly as it is
today. This spec touches one boolean expression in one file, nothing
else:

- **TTL-bar hover-pause** (`TtlBar.tsx`'s `hoverPaused`) — a fully
  separate mechanism (freezes the countdown fill on hover) that never
  read `expanded` and is untouched by this change.
- **AgentBoard hero↔full-list** (`App.tsx:305`, `expanded={hovered}`) —
  a different component, different prop wiring, not part of this spec.
- **Idle weather/scorecard/media peek** (`IdleHoverPeek.tsx`) — no
  keyboard equivalent exists and none is being added; stays hover-driven.
- **Bare-notch rail reveal** (`.bare.hovered` repaint) — a repaint, not
  a size change; untouched.

## Approach

In `src/useExitChoreography.ts:87`, revert:

```ts
const expanded = showing ? slot.expanded || hovered : renderedShowing && renderedSlot.expanded;
```

to:

```ts
const expanded = showing ? slot.expanded : renderedShowing && renderedSlot.expanded;
```

The `renderedShowing && renderedSlot.expanded` exit-window fallback branch
is untouched — it was never part of the `|| hovered` addition (see the
surrounding comment block, `useExitChoreography.ts:76-86`) and already
only reads `slot.expanded`, not `hovered`.

Remove the now-stale 2026-08-02 comment block (`useExitChoreography.ts:
76-86`) describing the `|| hovered` rationale, replacing it with a short
note that hover-expand was tried and reverted, so a future reader doesn't
wonder why `App.tsx`'s `expanded={hovered}` precedent (AgentBoard) isn't
mirrored here — that's now a deliberate difference, not an oversight.

## Testing

`src/components/StatusRailCard.test.tsx:1948-1966` has a
`describe("hover-expand on a showing card (2026-08-02)", ...)` block with
two tests that pin the exact behavior being reverted:

- `"renders .expanded and an open manifest while hovered, even with
  slot.expanded false"` — this assertion becomes false; the test must be
  rewritten to assert the opposite (hovering a `slot.expanded: false` card
  stays collapsed), or removed and replaced with a test pinning "hover
  alone never expands; `slot.expanded` alone controls it."
- `"stays collapsed — no .expanded, manifest closed — while not
  hovered"` — this one already asserts behavior compatible with the
  revert (not-hovered stays collapsed) but its name/framing should be
  updated since "while not hovered" no longer implies anything special
  once hover stops affecting `expanded` at all.

Add a test confirming `slot.expanded: true` still opens the manifest
regardless of `hovered` (true or false) — the keyboard-driven path this
revert is restoring as the *only* path.

## Escape hatch

If any other test elsewhere in the suite depends on the `|| hovered`
behavior (search wasn't exhaustive — only `StatusRailCard.test.tsx` was
checked), STOP and report back rather than changing that test's
assertion without understanding why it was written that way first.
