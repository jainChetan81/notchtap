import type { Priority } from "../lib/presentation";

// 3-segment rail: 1/2/3 lit segments for low/medium/high — the review's
// requested second non-color cue (segment *count*, not just color).
const LIT_SEGMENTS: Record<Priority, number> = { low: 1, medium: 2, high: 3 };

export function Track({ priority }: { priority: Priority }) {
  const lit = LIT_SEGMENTS[priority];
  return (
    <div className="track" aria-hidden="true">
      {[0, 1, 2].map((i) => (
        <span key={i} className={i < lit ? "lit" : undefined} />
      ))}
    </div>
  );
}
