import { useEffect, useRef } from "react";

const MAX_SEGMENTS = 10;

export function TtlBar({
  slotId,
  ttlMs,
  remainingMs,
  hoverPaused = false,
  total = 1,
  done = 0,
}: {
  slotId: string;
  ttlMs: number;
  remainingMs: number;
  hoverPaused?: boolean;
  total?: number;
  done?: number;
}) {
  const fillRef = useRef<HTMLDivElement | null>(null);
  const hoverPausedRef = useRef(hoverPaused);
  const frameIdRef = useRef<number | null>(null);
  const resumeRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    const wasPaused = hoverPausedRef.current;
    hoverPausedRef.current = hoverPaused;
    if (wasPaused && !hoverPaused && frameIdRef.current === null) {
      resumeRef.current?.();
    }
  }, [hoverPaused]);

  // Re-anchor on slot/timing changes; hover pauses in place and must not restart the countdown.
  // biome-ignore lint/correctness/useExhaustiveDependencies: slotId isn't read in the body, but it's the deliberate re-anchor trigger documented above (same pattern as StatusRailCard's currentId) — a new promotion must restart the countdown even when ttlMs/remainingMs happen to coincide. hoverPaused is excluded on purpose too (see the comment above the effect).
  useEffect(() => {
    const fill = fillRef.current;
    if (!fill) {
      return;
    }

    if (ttlMs <= 0) {
      fill.style.transform = "scaleX(1)";
      resumeRef.current = null;
      return;
    }

    const deadline = performance.now() + remainingMs;
    let pausedAccumMs = 0;
    let pauseStartedAt: number | null = hoverPausedRef.current ? performance.now() : null;
    let cancelled = false;

    function tick() {
      if (cancelled) {
        return;
      }
      const fillEl = fillRef.current;
      if (!fillEl) {
        return;
      }
      const nowPaused = hoverPausedRef.current;
      if (nowPaused && pauseStartedAt === null) {
        pauseStartedAt = performance.now();
      } else if (!nowPaused && pauseStartedAt !== null) {
        pausedAccumMs += performance.now() - pauseStartedAt;
        pauseStartedAt = null;
      }
      const effectiveNow = pauseStartedAt ?? performance.now();
      const remaining = Math.max(0, deadline + pausedAccumMs - effectiveNow);
      const pct = Math.min(100, (remaining / ttlMs) * 100);
      fillEl.style.transform = `scaleX(${pct / 100})`;
      if (remaining <= 0) {
        frameIdRef.current = null;
        return;
      }
      if (nowPaused) {
        frameIdRef.current = null;
        return;
      }
      frameIdRef.current = requestAnimationFrame(tick);
    }

    resumeRef.current = () => {
      if (cancelled || frameIdRef.current !== null) {
        return;
      }
      frameIdRef.current = requestAnimationFrame(tick);
    };

    frameIdRef.current = requestAnimationFrame(tick);

    return () => {
      cancelled = true;
      resumeRef.current = null;
      if (frameIdRef.current !== null) {
        cancelAnimationFrame(frameIdRef.current);
        frameIdRef.current = null;
      }
    };
  }, [slotId, ttlMs, remainingMs]);

  const segmentCount = Math.min(Math.max(total, 1), MAX_SEGMENTS);
  const rawCurrent = total > MAX_SEGMENTS ? Math.floor((done * MAX_SEGMENTS) / total) : done;
  const current = Math.min(rawCurrent, segmentCount - 1);

  return (
    <div
      className="ttl-bar"
      // SAFETY: --queue-n is a valid CSS custom property for this component's grid; React.CSSProperties lacks index signature for custom props.
      style={{ "--queue-n": segmentCount } as React.CSSProperties}
    >
      {Array.from({ length: segmentCount }, (_, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: anonymous positional queue-segment slots (0..n) — index is the only identity there is, and the sequence is always rendered fresh, never reordered or spliced.
        <span key={i} className={i < current ? "ttl-seg done" : "ttl-seg"} />
      ))}
      <div
        className={hoverPaused ? "ttl-fill paused" : "ttl-fill"}
        ref={fillRef}
        style={{ gridColumn: current + 1 }}
      />
    </div>
  );
}
