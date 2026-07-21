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
//
// plan 092 (item 11): the paused indicator. While `status.paused` is true,
// every dot forces the `dim` treatment — the per-source enabled/disabled
// read is suppressed rather than layered with a fourth "paused" visual
// state, since "the engine isn't delivering anything right now" reads
// clearer than three independently-still-glowing dots. A small static
// two-bar glyph (`.pause-glyph`, CSS-drawn — no new asset) renders beside
// the dot row only while paused — nested INSIDE `.status-dots` (already a
// `display: flex; gap: 8px` row) rather than as an external sibling, so it
// falls into the same flex row and the existing gap spaces it from the
// last dot, with no extra wrapper needed. Receive-only, like the dots
// themselves: an indicator, never a button. Retires the old "News paused"
// idle text remnant that 091 already removed (IdleView) — this is its
// replacement.
export function StatusDots({ status }: { status?: StatusState }) {
  const paused = status?.paused ?? false;
  const football = !paused && (status?.football.enabled ?? false);
  const news = !paused && (status?.news.enabled ?? false);
  const weather = !paused && (status?.weather.enabled ?? false);
  return (
    <span className="status-dots">
      <span className={`status-dot football${football ? " active" : " dim"}`} />
      <span className={`status-dot news${news ? " active" : " dim"}`} />
      <span className={`status-dot weather${weather ? " active" : " dim"}`} />
      {paused && (
        <span className="pause-glyph" aria-hidden="true">
          <span />
          <span />
        </span>
      )}
    </span>
  );
}
