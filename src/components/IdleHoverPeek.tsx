import { AnimatePresence, motion } from "motion/react";
import { useClock } from "../hooks/useClock";
import type { LiveMatchSummary, StatusState } from "../hooks/useStatusState";
import { DISCLOSURE_SPRING } from "../lib/constants";

function ScorecardRevealContent({ live }: { live: LiveMatchSummary }) {
  return (
    <div className="idle-reveal-scorecard">
      <div className="sc-head">
        <span className="chip chip-live">
          <span className="live-dot" aria-hidden="true" />
          Live
        </span>
        <span className="chip clock-pill">{live.minute}</span>
      </div>
      <div className="idle-reveal-label">{live.label}</div>
    </div>
  );
}

function PeekTimeline() {
  const { dayProgress } = useClock();
  return (
    <span
      className="idle-peek-timeline"
      // SAFETY: CSS custom property valid for this component; React.CSSProperties lacks index signature.
      style={{ "--day-progress": `${dayProgress}%` } as React.CSSProperties}
      aria-hidden="true"
    />
  );
}

export type PeekPreference = "football" | null;

export function IdleHoverPeek({
  status,
  hovered,
  open = hovered,
  prefer = null,
}: {
  status?: StatusState;
  hovered: boolean;
  open?: boolean;
  prefer?: PeekPreference;
}) {
  const liveMatch = status?.football.live ?? null;

  const live = prefer === null || prefer === "football" ? liveMatch : null;

  return (
    <AnimatePresence>
      {open ? (
        <motion.div
          className="below-block idle-peek"
          initial={{ height: 0, opacity: 0, paddingTop: 0, paddingBottom: 0 }}
          animate={{ height: 100, opacity: 1, paddingTop: 12, paddingBottom: 13 }}
          exit={{ height: 0, opacity: 0, paddingTop: 0, paddingBottom: 0 }}
          transition={DISCLOSURE_SPRING}
          style={{ overflow: "hidden" }}
        >
          <div className="peek-content">
            {live !== null ? <ScorecardRevealContent live={live} /> : null}
            <PeekTimeline />
          </div>
        </motion.div>
      ) : null}
    </AnimatePresence>
  );
}
