import type { StatusState } from "../hooks/useStatusState";
import { FootballGlyph, NewsGlyph } from "./IconStrip";

function configuredLabel(name: string, configured: boolean | undefined): string {
  if (configured === undefined) {
    return `${name} — status unavailable`;
  }
  return configured ? `${name} — enabled` : `${name} — disabled`;
}

function configStateClass(configured: boolean | undefined): string {
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
        className={`status-dot football ${configStateClass(footballConfigured)}${football ? " active" : " dim"}`}
        role="img"
        aria-label={configuredLabel("Football", footballConfigured)}
      >
        <FootballGlyph />
      </span>
      <span
        className={`status-dot news ${configStateClass(newsConfigured)}${news ? " active" : " dim"}`}
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
