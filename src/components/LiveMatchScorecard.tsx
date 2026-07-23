import { convertFileSrc } from "@tauri-apps/api/core";
import { useState } from "react";
import type { EventKindPresentation, LivePillVariant } from "../lib/presentation";
import type { EspnMeta } from "../useSlotState";

// plan 084: the recurring live-match scorecard's crest — a filesystem path
// on the wire (083 workstream a), never a ready `asset://` URL, so every
// render must go through `convertFileSrc` itself. `onError` is defense in
// depth for a cache entry that's gone stale on disk between poll and
// render; the `broken` flag is deliberately sticky (not re-tried) so a
// permanently-404ing path doesn't flash between the two states forever.
function Crest({ abbrev, path }: { abbrev: string; path: string | null }) {
  const [broken, setBroken] = useState(false);
  const src = !broken && path !== null ? convertFileSrc(path) : null;
  return (
    <span className="crest">
      {src !== null ? <img src={src} alt="" onError={() => setBroken(true)} /> : abbrev}
    </span>
  );
}

// plan 084: the recurring live-match scorecard (POST-083 espn
// meta) — sticky medium-priority presence, no full-expand
// (operator lock). Deliberately ignores `expanded`: even if
// the slot's `expanded` flag arrives true, there is no
// manual-expand affordance for football, so this branch
// always renders this same compact scorecard rather than
// switching to a richer layout. No `Track` (a batch-position
// slider is meaningless for a single recurring presence —
// prototype lock) and no `TtlBar` either: the bar's
// countdown-to-rotation framing would visually contradict
// "sticky" (see plan 084's report for the reasoning). No
// generic `<Stamp>` — the live chip above already carries
// that role (Live/Break/Final) with more precision.
//
// plan 120: extracted verbatim from StatusRailCard.tsx's JSX
// (`:660-710` at 2a840c4) — every free variable the block read is now a
// prop, not re-derived here (the lower-risk "moved as-is" shape).
export function LiveMatchScorecard({
  liveEspn,
  pillVariant,
  pillLabel,
  eventPresentation,
  cardsClean,
  body,
}: {
  liveEspn: EspnMeta;
  pillVariant: LivePillVariant;
  pillLabel: string;
  eventPresentation: EventKindPresentation | null;
  cardsClean: boolean;
  body: string;
}) {
  return (
    <div className="notif-block">
      <div className="sc-head">
        <span className="chip chip-league">{liveEspn.league}</span>
        <span className={`chip chip-live${pillVariant === "live" ? "" : ` ${pillVariant}`}`}>
          {pillVariant !== "final" && <span className="live-dot" />}
          {pillLabel}
        </span>
        <span className="clock-pill">{liveEspn.clock}</span>
      </div>
      <div className="score-row">
        <div className="side">
          <Crest abbrev={liveEspn.homeAbbrev} path={liveEspn.homeCrest} />
        </div>
        <span className="score">
          {liveEspn.homeScore}
          <span className="dash">–</span>
          {liveEspn.awayScore}
        </span>
        <div className="side">
          <Crest abbrev={liveEspn.awayAbbrev} path={liveEspn.awayCrest} />
        </div>
      </div>
      <div
        className={`event-line${eventPresentation?.tintClass ? ` ${eventPresentation.tintClass}` : ""}`}
      >
        {eventPresentation && <span className={eventPresentation.iconClass} />}
        {body}
      </div>
      {!cardsClean && (
        <div className="cards-line">
          {liveEspn.homeAbbrev} {liveEspn.homeCards[0]}Y{liveEspn.homeCards[1]}R ·{" "}
          {liveEspn.awayAbbrev} {liveEspn.awayCards[0]}Y{liveEspn.awayCards[1]}R
        </div>
      )}
    </div>
  );
}
