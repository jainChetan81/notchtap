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
//
// plan 110 (Step D): each dot now carries `role="img"` + a truthful
// `aria-label` and a non-color configured-state SHAPE, both independent of
// the pause-luminance (active/dim) treatment above. The label/shape read
// off the RAW config flag (`status.<source>.enabled`), never the
// pause-suppressed `football`/`news`/`weather` booleans below — those
// already fold in `!paused`, so while paused every dot would otherwise
// announce "disabled" even for a source that's actually configured on, a
// false statement about CONFIGURATION sitting right next to the pause
// glyph's own (true) statement about the pause. One fact per element: the
// dot announces configuration, the pause glyph announces the pause. A
// `status` prop that's entirely absent (settings preview / older callers,
// still loading) is NOT coerced to "disabled" — it gets its own third
// state, "status unavailable", since "off" and "unknown" are different
// facts. Visually: enabled = filled circle (the pre-091 default shape,
// unchanged), disabled = hollow square, unavailable = hollow circle — same
// 9x9 footprint throughout (styles.css); pause still only changes
// luminance (active/dim), never this shape.
function configuredLabel(name: string, configured: boolean | undefined): string {
  if (configured === undefined) {
    return `${name} — status unavailable`;
  }
  return configured ? `${name} — enabled` : `${name} — disabled`;
}

function shapeClass(configured: boolean | undefined): string {
  if (configured === undefined) {
    return "shape-unavailable";
  }
  return configured ? "shape-enabled" : "shape-disabled";
}

export function StatusDots({ status }: { status?: StatusState }) {
  const paused = status?.paused ?? false;
  const footballConfigured = status ? status.football.enabled : undefined;
  const newsConfigured = status ? status.news.enabled : undefined;
  const weatherConfigured = status ? status.weather.enabled : undefined;
  const football = !paused && (footballConfigured ?? false);
  const news = !paused && (newsConfigured ?? false);
  const weather = !paused && (weatherConfigured ?? false);
  return (
    <span className="status-dots">
      <span
        className={`status-dot football ${shapeClass(footballConfigured)}${football ? " active" : " dim"}`}
        role="img"
        aria-label={configuredLabel("Football", footballConfigured)}
      />
      <span
        className={`status-dot news ${shapeClass(newsConfigured)}${news ? " active" : " dim"}`}
        role="img"
        aria-label={configuredLabel("News", newsConfigured)}
      />
      <span
        className={`status-dot weather ${shapeClass(weatherConfigured)}${weather ? " active" : " dim"}`}
        role="img"
        aria-label={configuredLabel("Weather", weatherConfigured)}
      />
      {paused && (
        <span className="pause-glyph" role="img" aria-label="Notifications paused">
          <span />
          <span />
        </span>
      )}
    </span>
  );
}
