import { useClock } from "../useClock";
import { type StatusState, statusRailActive } from "../useStatusState";

// plan 034: the idle card answers "what's happening / what's next" — a
// live match chip (pulsing green dot), the news gate, and the queue depth
// — on top of the plain clock. `status` is optional because StatusRailCard
// renders without it in the settings preview and older tests; absent or
// inactive means no rail and the narrow clock-only card (StatusRailCard
// keys the width class off the same statusRailActive predicate).
export function IdleView({ status }: { status?: StatusState }) {
  const { display, dayProgress } = useClock();
  const live = status?.football.live ?? null;
  const rail = status !== undefined && statusRailActive(status);
  return (
    <div className="idle-view">
      <span className="time">{display}</span>
      {rail && (
        <span className="src-rail">
          {live !== null && (
            <span className="src-chip live">
              <span className="live-dot" aria-hidden="true" />
              {live.label} · {live.minute}
            </span>
          )}
          <span className={`src-chip${status.news.enabled ? "" : " dim"}`}>
            {status.news.enabled ? "News" : "News paused"}
          </span>
          <span className="src-chip">
            {status.waiting > 0 ? `${status.waiting} queued` : "clear"}
          </span>
        </span>
      )}
      <span
        className="timeline"
        style={{ "--day-progress": `${dayProgress}%` } as React.CSSProperties}
        aria-hidden="true"
      />
    </div>
  );
}
