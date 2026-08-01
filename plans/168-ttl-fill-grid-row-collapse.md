# 168 — Fix the TTL-bar fill collapsing to ~0px height

- **Status**: DONE (2026-08-01) — executed via `/improve execute`, reviewed, approved, and locally merged into `agent-card-ui-unification` (not pushed, not merged to master).
- **Commit**: 45af0df, merged via d0de6e4
- **Severity**: MEDIUM
- **Category**: Correctness (CSS Grid auto-placement) — found while building `prototype/proposal-unified-card.html`, confirmed against the real component, not the mock
- **Estimated scope**: 1 file (`src/overlay/ttl-bar.css`), one declaration added
- **Depends on**: none

## Problem

`TtlBar.tsx` places `.ttl-fill` on top of its current segment using only
an inline `gridColumn`:

```tsx
// src/components/TtlBar.tsx:207-238 (current)
<div className="ttl-bar" style={{ "--queue-n": segmentCount } as React.CSSProperties}>
  {Array.from({ length: segmentCount }, (_, i) => (
    <span key={i} className={i < current ? "ttl-seg done" : "ttl-seg"} />
  ))}
  <div
    className={hoverPaused ? "ttl-fill paused" : "ttl-fill"}
    ref={fillRef}
    style={{ gridColumn: current + 1 }}
  />
</div>
```

```css
/* src/overlay/ttl-bar.css:25-39, 84-98 (current) */
.card-root .ttl-bar {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  height: 4px;
  display: grid;
  grid-template-columns: repeat(var(--queue-n, 1), 1fr);
  gap: 3px;
}
.card-root .ttl-fill {
  height: 100%;
  width: 100%;
  background: var(--accent);
  transform-origin: left;
  will-change: transform;
  transition: opacity var(--hover-ms, 160ms) var(--ease-notchtap), background var(--hover-ms, 160ms) var(--ease-notchtap);
}
```

Neither the container nor `.ttl-fill` sets `grid-row`. CSS Grid's
auto-placement algorithm does not overlap items by default: when an item
is placed on only one axis (`grid-column`, here), the browser searches
for the first row where that column is *empty* to place the other axis —
and column `current + 1` is already occupied by that segment's own
`<span class="ttl-seg">` (auto-placed there in DOM order, since the spans
render before the fill). The browser resolves this by pushing `.ttl-fill`
into a new implicit row instead of the one visible row.

Confirmed empirically (not just reasoned about) while building the
prototype mock, isolating the exact markup/CSS above in a real
Chromium instance:

```
seg:0.5  fill:0.5   (broken — both collapse toward 0, matching the ttl-bar
                      barely being visible in practice)
```

Adding `grid-row: 1` to `.ttl-fill` and a definite `grid-template-rows: 4px`
to `.ttl-bar` (rather than leaving the single implicit row `auto`-sized,
which does not reliably stretch to the container's explicit height either)
fixes it:

```
seg:4  fill:4   (correct — both fill the bar's real 4px height)
```

The net visible effect in the shipped app: the queue-segmented TTL bar
(every card with `total > 1`, i.e. anything with more than one item
batched behind it) likely renders its progress fill far thinner than
intended, close to invisible against the 4px trough. Cards with
`total === 1` (the common case — a single item, no queue) are also
affected, since the same collapse applies regardless of segment count.

## Target

```css
/* src/overlay/ttl-bar.css (target) */
.card-root .ttl-bar {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  height: 4px;
  display: grid;
  grid-template-columns: repeat(var(--queue-n, 1), 1fr);
  grid-template-rows: 4px;
  gap: 3px;
}
```

```css
.card-root .ttl-fill {
  grid-row: 1;
  height: 100%;
  width: 100%;
  background: var(--accent);
  transform-origin: left;
  will-change: transform;
  transition: opacity var(--hover-ms, 160ms) var(--ease-notchtap), background var(--hover-ms, 160ms) var(--ease-notchtap);
}
```

## Repo conventions to follow

- Two separate declaration blocks in the same file (`.ttl-bar`'s own rule,
  `.ttl-fill`'s own rule, unchanged elsewhere) — add exactly one property
  to each, do not restructure either rule.
- Leave `TtlBar.tsx`'s inline `style={{ gridColumn: current + 1 }}` alone —
  the fix belongs entirely in CSS (a definite row template + an explicit
  `grid-row` on the one item that needs to share column-space with a
  sibling), not in the component.

## Steps

1. In `src/overlay/ttl-bar.css`, locate `.card-root .ttl-bar` (currently
   lines 25-39). Add `grid-template-rows: 4px;` immediately after
   `grid-template-columns: repeat(var(--queue-n, 1), 1fr);`.
2. Locate `.card-root .ttl-fill` (currently lines 84-98). Add
   `grid-row: 1;` as the first declaration in the block, before `height:
   100%;`.
3. Do not touch `.ttl-seg`/`.ttl-seg.done` — those correctly stretch to
   the row once the row itself has a definite size from step 1.

## Boundaries

- Do NOT touch `TtlBar.tsx` — this is a pure CSS placement bug, not a
  component logic bug.
- Do NOT touch `.ttl-fill.paused` or the reduced-motion override below it.
- If the current code at `ttl-bar.css:25-39`/`84-98` doesn't match the
  quoted excerpts (drift since this plan was written), STOP and report
  instead of improvising — the fix is placement-specific and needs to be
  re-verified against whatever the file actually says.

## Verification

- **Mechanical**: `npx vitest run` clean. `npx biome ci .` clean.
- **Visual, real app**: trigger a card with `total > 1` (queue behind it —
  e.g. push several manual notifications back to back via `./notchtap`)
  and confirm the TTL bar's fill segment is now a clearly visible solid
  block at the bar's full 4px height, not a hairline. Compare before/after
  with browser DevTools' computed-height inspector on `.ttl-fill` — should
  read `4px`, not a near-zero value.
- **Done when**: `.ttl-fill`'s computed height is confirmed 4px in a real
  build (not just the isolated test case in the Problem section above),
  and the visual fill reads as a solid bar across at least one segmented
  (`total > 1`) and one unsegmented (`total === 1`) card.
