// Queue slider (plan 033): one segment per item in the current batch —
// consumed items dim (.done), the current one bright (.cur), advancing as
// the queue rotates. Replaces the 3-segment priority-load rail (segment
// count now means queue depth, not priority). Batches render at most 10
// segments; beyond that the index maps proportionally — no "+N" labels.
const MAX_SEGMENTS = 10;

export function Track({ total, done }: { total: number; done: number }) {
  const n = Math.min(Math.max(total, 1), MAX_SEGMENTS);
  const current = total > MAX_SEGMENTS ? Math.floor((done * MAX_SEGMENTS) / total) : done;
  return (
    <div
      className="track"
      aria-hidden="true"
      // segment count is data, not theme — the grid template takes it via a
      // custom property so styles.css/preview-overlay.css stay static.
      style={{ "--queue-n": n } as React.CSSProperties}
    >
      {Array.from({ length: n }, (_, i) => (
        <span key={i} className={i < current ? "done" : i === current ? "cur" : undefined} />
      ))}
    </div>
  );
}
