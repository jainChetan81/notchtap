import type { StatusState } from "../useStatusState";
import { FootballGlyph, NewsGlyph } from "./IconStrip";

// Right flank status dots, fixed order Football/News. "Active" = source
// enabled (glow + pulse), "dim" = disabled (flat, 0.22) — same per-source
// `enabled` flags as useStatusState. An absent `status` prop (settings
// preview) renders every dot dim.
//
// The paused indicator: while `status.paused`, every dot forces the `dim`
// treatment — the per-source read is suppressed rather than layered with a
// fourth visual state, since "engine isn't delivering" reads clearer than
// three still-glowing dots. `.pause-glyph` (CSS-drawn, no asset) sits inside
// `.status-dots`' own flex row. Receive-only like the dots — indicator,
// never a button.
//
// A11y: each dot carries `role="img"` + a truthful `aria-label` and a
// non-color configured-state SHAPE. The label/shape read the RAW config
// flag (`status.<source>.enabled`), NEVER the pause-suppressed booleans
// below — those already fold in `!paused`, so while paused they'd announce
// "disabled" for a source that's actually on. One fact per element: dot =
// configuration, pause glyph = pause. An absent status is its own third
// state ("status unavailable"), since "off" and "unknown" differ. Enabled =
// filled circle, disabled = hollow square, unavailable = hollow circle,
// same 9x9 footprint; pause changes luminance only, never shape.
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
  const football = !paused && (footballConfigured ?? false);
  const news = !paused && (newsConfigured ?? false);
  return (
    <span className="status-dots">
      <span
        className={`status-dot football ${shapeClass(footballConfigured)}${football ? " active" : " dim"}`}
        role="img"
        aria-label={configuredLabel("Football", footballConfigured)}
      >
        <FootballGlyph />
      </span>
      <span
        className={`status-dot news ${shapeClass(newsConfigured)}${news ? " active" : " dim"}`}
        role="img"
        aria-label={configuredLabel("News", newsConfigured)}
      >
        <NewsGlyph charge={0} />
      </span>
      {paused && (
        <span className="pause-glyph" role="img" aria-label="Notifications paused">
          <span />
          <span />
        </span>
      )}
    </span>
  );
}
