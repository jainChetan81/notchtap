// The pulled-view position indicator — a floor strip with no drain,
// shared between the agent tab's session bar and the news tab's batch
// position strip. One component, two callers: news's floor strip is
// implemented identically to agent's, never as a second invented shape.
//
// Reuses `card-chrome.css`'s `.ttl-bar`/`.ttl-seg`/`.ttl-seg .done`/
// `.ttl-fill` classes verbatim — no new CSS. Unlike `TtlBar.tsx` (which
// overlays a SEPARATE `.ttl-fill` div via an inline `grid-column` on top
// of an underlying `.ttl-seg` span, so a JS rAF loop can drain it), this
// bar has no drain and needs no overlay: the viewed segment's own
// `<span>` simply carries the `ttl-fill` class directly in place of
// `ttl-seg` — one span per item, never an N+1th overlay node, and never
// a `.ttl-fill` mounted as a draining overlay.
// `.ttl-fill`'s CSS (grid-row 1, 100%/100%, `background:
// var(--accent)`) paints correctly as a plain grid item with no inline
// `transform`/`grid-column` override needed — it simply sits in its own
// natural document-order column, motionless.
//
// Segment-count capping mirrors `TtlBar.tsx`'s own `MAX_SEGMENTS`/
// proportional-mapping math verbatim (ported, not reinvented) — a
// session count or news batch large enough to need it should collapse
// the same way an oversized notification queue already does, not grow a
// second, illegibly-thin grid.
const MAX_SEGMENTS = 10;

// Exported separately from the component so the proportional-mapping
// math is directly unit-testable without a DOM render, same reasoning
// TtlBar.tsx's own segment math earns dedicated tests for.
export type SegmentForResult = { segmentCount: number; segmentIndex: number };

export function segmentFor(
  current: number,
  total: number,
  maxSegments: number = MAX_SEGMENTS,
): SegmentForResult {
  const segmentCount = Math.min(Math.max(total, 1), maxSegments);
  const rawIndex = total > maxSegments ? Math.floor((current * maxSegments) / total) : current;
  const segmentIndex = Math.min(Math.max(rawIndex, 0), segmentCount - 1);
  return { segmentCount, segmentIndex };
}

function segmentClassName(i: number, segmentIndex: number): string {
  if (i === segmentIndex) {
    return "ttl-fill";
  }
  return i < segmentIndex ? "ttl-seg done" : "ttl-seg";
}

export function PositionBar({ total, current }: { total: number; current: number }) {
  // Same "nothing to show, render nothing" posture AgentBoard.tsx's own
  // `sessions.length === 0` guard uses — a below-block with zero items
  // to position among has nothing this bar can meaningfully draw.
  if (total <= 0) {
    return null;
  }
  const { segmentCount, segmentIndex } = segmentFor(current, total);

  return (
    // SAFETY: CSS custom property valid for this component; React.CSSProperties lacks index signature.
    <div className="ttl-bar" style={{ "--queue-n": segmentCount } as React.CSSProperties}>
      {Array.from({ length: segmentCount }, (_, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: anonymous positional segment slots (0..n), same reasoning TtlBar.tsx's own segment row documents — index is the only identity there is, and the sequence is always rendered fresh.
        <span key={i} className={segmentClassName(i, segmentIndex)} />
      ))}
    </div>
  );
}
