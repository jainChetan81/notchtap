const MAX_SEGMENTS = 10;

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
