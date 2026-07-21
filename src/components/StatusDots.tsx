import type { StatusState } from "../useStatusState";

// plan 091 (079 item 2): the right flank's three status dots — fixed order
// Football/News/Weather, per the locked reference
// (`prototype/notch-states.html:222-228`, `.status-dots`/`.status-dot`).
// Replaces the old text-pill `.src-rail` entirely (rewritten, not
// restyled): "active" now means simply "this source is enabled" (glow +
// pulse), "dim" means disabled (flat, 0.22 opacity) — the SAME per-source
// `enabled` flags the old rail read off `useStatusState`, no new semantics
// invented. `status` is optional for the same reason IdleView's was: the
// settings preview and older callers render without one, in which case
// every dot is dim (nothing to report).
export function StatusDots({ status }: { status?: StatusState }) {
  const football = status?.football.enabled ?? false;
  const news = status?.news.enabled ?? false;
  const weather = status?.weather.enabled ?? false;
  return (
    <span className="status-dots">
      <span className={`status-dot football${football ? " active" : " dim"}`} />
      <span className={`status-dot news${news ? " active" : " dim"}`} />
      <span className={`status-dot weather${weather ? " active" : " dim"}`} />
    </span>
  );
}
