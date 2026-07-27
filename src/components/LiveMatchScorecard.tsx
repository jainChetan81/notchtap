import { convertFileSrc } from "@tauri-apps/api/core";
import { AnimatePresence, motion } from "motion/react";
import { useState } from "react";
import { NOTCHTAP_EASE } from "../animationTiming";
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

// plan 151 (item B): the score odometer's own timing. Deliberately LOCAL
// literals rather than new animationTiming.ts tokens — that file
// single-sources values with a CSS or cross-component counterpart that
// must stay in lockstep (see its header), and this roll has exactly one
// consumer and no CSS twin. The delay lets the goal celebration's own
// first beat land first: 120 + 360 = 480ms, well inside the 1240ms
// celebration window (choreography.css), so the digit finishes rolling
// while the burst is still playing rather than after it.
const SCORE_ROLL_S = 0.36;
const SCORE_ROLL_DELAY_S = 0.12;

// plan 151 (item B): one side's score digit, as a single-digit odometer.
//
// The payload used to be the one thing on this card that did NOT move —
// `goal-overshoot`/`goal-burst`/the ripple all celebrate AROUND a number
// that simply swapped between frames. The clip span is a fixed-height
// `overflow: hidden` box (live-scorecard.css `.score-digit`); the inner
// `motion.span` is KEYED ON THE VALUE, which is the whole restraint
// guard: React only remounts (and AnimatePresence only animates) when the
// number genuinely changes. Everything else that re-renders this card —
// most notably the once-a-minute clock pill, but also any same-slot
// rotation re-emit carrying an unchanged scoreline — reuses the same
// keyed child and rolls nothing. That is why this needs no `.rotation-swap`
// style off-switch (news-category.css): value-keying already expresses
// "only on a real change", and it can't be fooled by a re-emit.
//
// `initial={false}` on the AnimatePresence means a card mounting with a
// score already on it renders that score at rest — the roll is reserved
// for a change that happens while the card is on screen. `mode="popLayout"`
// takes the outgoing digit out of flow so the two digits overlap inside
// the clip instead of the box briefly widening to hold both. The
// percentage translate is compositor-only (no layout per frame), same
// discipline as `.ttl-fill`/`.media-bar-fill`.
function ScoreDigit({ value }: { value: number }) {
  return (
    <span className="score-digit">
      <AnimatePresence initial={false} mode="popLayout">
        <motion.span
          key={value}
          className="score-digit-roll"
          initial={{ y: "100%" }}
          animate={{ y: 0 }}
          exit={{ y: "-100%" }}
          transition={{ duration: SCORE_ROLL_S, ease: NOTCHTAP_EASE, delay: SCORE_ROLL_DELAY_S }}
        >
          {value}
        </motion.span>
      </AnimatePresence>
    </span>
  );
}

// plan 084: the recurring live-match scorecard (POST-083 espn
// meta) — sticky medium-priority presence, no full-expand
// (operator lock). Deliberately ignores `expanded`: even if
// the slot's `expanded` flag arrives true, there is no
// manual-expand affordance for football, so this branch
// always renders this same compact scorecard rather than
// switching to a richer layout. No `TtlBar` (a batch-position
// slider is meaningless for a single recurring presence — prototype
// lock — and since the stories merge (2026-07-24) the queue segmentation lives INSIDE TtlBar,
// omitting TtlBar omits both): the bar's countdown-to-rotation framing
// would visually contradict "sticky" (see plan 084's report for the
// reasoning). No generic `<Stamp>` — the live chip above already
// carries that role (Live/Break/Final) with more precision.
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
          {/* plan 151 (item A): the dot stays MOUNTED in every variant,
              including `final` — it leaves by fading to `opacity: 0`
              (live-scorecard.css `.chip-live.final .live-dot`) in step
              with the chip's own colour morph, instead of being
              unmounted and blinking out a frame before the colours
              change. Smallest structural change that lets it fade; the
              collapsed 5px+gap it leaves behind is animated in the same
              rule. */}
          <span className="live-dot" />
          {pillLabel}
        </span>
        <span className="chip clock-pill">{liveEspn.clock}</span>
      </div>
      <div className="score-row">
        <div className="side">
          <Crest abbrev={liveEspn.homeAbbrev} path={liveEspn.homeCrest} />
        </div>
        <span className="score">
          <ScoreDigit value={liveEspn.homeScore} />
          <span className="dash">–</span>
          <ScoreDigit value={liveEspn.awayScore} />
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
