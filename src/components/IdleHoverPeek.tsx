import { AnimatePresence, motion } from "motion/react";
import { DISCLOSURE_SPRING } from "../animationTiming";
import { useClock } from "../useClock";
import type { LiveMatchSummary, StatusState } from "../useStatusState";

// The idle hover-expanded state — hovering the idle assembly (`hovered`,
// the hover primitive's prop, NEVER CSS `:hover` — the overlay window is
// click-through, only the rust tracking area knows the cursor) opens a
// `.below-block` beneath the flank row. Content precedence — one
// below-block at a time: a live match (`status.football.live`) fills the
// content slot; with none available, the block still opens with the
// day-progress timeline alone.
//
// Mount lifecycle: the CONTENT below opens/closes on the `open` prop via
// `AnimatePresence` + `motion.div`. `AnimatePresence` owns the exit
// window itself — `exit={...}` plays out before the node leaves the DOM.
// `IdleHoverPeek` itself stays always mounted by StatusRailCard; only
// `open` (defaulting to `hovered` for standalone callers) gates this
// AnimatePresence child, so a promotion arriving mid-peek (flipping
// `open` false without unmounting this component) lets the exit play
// instead of the whole subtree being torn out synchronously.
//
// This is also why the block is NOT always-mounted: `.below-block`'s
// mere DOM presence is what idle-peek.css's `:not(:has(.below-block))`
// rounding key off — the idle pill un-rounds only while open, including
// during `AnimatePresence`'s exit animation.

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

// The day-progress timeline; this peek is its sole home. `useClock`'s
// `dayProgress` is the 0–100 "how far through the local day" value.
function PeekTimeline() {
  const { dayProgress } = useClock();
  return (
    <span
      className="idle-peek-timeline"
      style={{ "--day-progress": `${dayProgress}%` } as React.CSSProperties}
      aria-hidden="true"
    />
  );
}

// Which source the caller wants this peek to show, overriding the
// ambient precedence chain below. `null` (the default) means "unchanged"
// — the shipped football > timeline chain. Only the football tab routes
// here; agent/news each have dedicated below-block components
// (`AgentBelowBlock`/`NewsBelowBlock`).
export type PeekPreference = "football" | null;

export function IdleHoverPeek({
  status,
  hovered,
  // `open` is the mount gate, read INSIDE this always-mounted component's
  // own AnimatePresence condition, so a promotion flipping `open` false
  // mid-peek lets the existing `exit={...}` collapse play while the
  // caller's card content enters above it, instead of vanishing
  // instantly. Defaults to `hovered` so standalone callers — this file's
  // tests, the settings preview — keep identical behavior without
  // passing a redundant `open` prop.
  open = hovered,
  // See `PeekPreference`'s doc above. Defaults to `null` so existing
  // callers keep the shipped precedence chain without passing anything.
  prefer = null,
}: {
  status?: StatusState;
  hovered: boolean;
  open?: boolean;
  prefer?: PeekPreference;
}) {
  const liveMatch = status?.football.live ?? null;

  // A preference NARROWS to one source, it never re-orders or falls
  // through; a preferred source with no data yields the timeline alone.
  // Kept as a prop so the routing seam (`peekPreferenceFor`,
  // StatusRailCard.tsx) stays explicit.
  const live = prefer === null || prefer === "football" ? liveMatch : null;

  return (
    <AnimatePresence>
      {open ? (
        // `height: 100` mirrors `hover.rs`'s `IDLE_PEEK_BELOW_BLOCK_H`
        // constant — a duplicated-constants pair; change both in the
        // same commit.
        <motion.div
          className="below-block idle-peek"
          /* `.idle-peek`'s CSS vertical padding (12px/13px) + border-box
             means a bare `height: 0` FLOORS at padding-top+bottom — the
             box could never reach zero. Animating the vertical paddings
             in lockstep with the height (matching the stylesheet at
             rest) lets the collapse genuinely reach 0. Horizontal
             padding stays CSS-owned (16px). */
          initial={{ height: 0, opacity: 0, paddingTop: 0, paddingBottom: 0 }}
          animate={{ height: 100, opacity: 1, paddingTop: 12, paddingBottom: 13 }}
          exit={{ height: 0, opacity: 0, paddingTop: 0, paddingBottom: 0 }}
          // Transition numbers live in animationTiming.ts as
          // DISCLOSURE_SPRING, shared with AgentBoard's disclosures. See
          // that constant's own doc for why a fixed opacity tween
          // desynced from the spring on interrupted hover flips.
          transition={DISCLOSURE_SPRING}
          style={{ overflow: "hidden" }}
        >
          <div className="peek-content">
            {/* precedence is football > timeline — one below-block at a
                time. */}
            {live !== null ? <ScorecardRevealContent live={live} /> : null}
            <PeekTimeline />
          </div>
        </motion.div>
      ) : null}
    </AnimatePresence>
  );
}
